use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use hyper_util::client::legacy::Client;
use hyper_rustls::HttpsConnectorBuilder;
use tokio::net::TcpListener;

use crate::token_store::TokenStore;

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type BoxBodyType = BoxBody<Bytes, BoxError>;
type HttpsConnector = hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;

struct ProxyState {
    tokens: Vec<String>,
    current_key: AtomicUsize,
    client: Client<HttpsConnector, BoxBodyType>,
    upstream: String,
}

impl ProxyState {
    fn new(tokens: Vec<String>, upstream: String) -> Self {
        // Instalar CryptoProvider do rustls (necessário para TLS)
        let _ = rustls::crypto::ring::default_provider().install_default();
        
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("failed to load native roots")
            .https_only()
            .enable_http1()
            .build();
        
        let client = Client::builder(hyper_util::rt::TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .build(https);

        Self {
            tokens,
            current_key: AtomicUsize::new(0),
            client,
            upstream,
        }
    }

    fn next_token_index(&self) -> usize {
        self.current_key.load(Ordering::Relaxed)
    }

    fn set_current_key(&self, index: usize) {
        self.current_key.store(index, Ordering::Relaxed);
    }
}

fn box_body(body: Full<Bytes>) -> BoxBodyType {
    BoxBody::new(body.map_err(|e| -> BoxError { e.into() }))
}

pub async fn run_proxy(port: u16, upstream: String) -> Result<()> {
    eprintln!("[PROXY] iniciando TokenStore...");
    let store = TokenStore::new()?;
    eprintln!("[PROXY] carregando tokens...");
    let tokens = store.load()?;

    if tokens.is_empty() {
        anyhow::bail!("Nenhum token cadastrado. Use 'add' primeiro.");
    }

    eprintln!("[PROXY] {} tokens carregados", tokens.len());

    let state = Arc::new(ProxyState::new(tokens, upstream));
    let addr = format!("127.0.0.1:{}", port);

    eprintln!("[PROXY] bindando porta {}...", port);
    let listener = TcpListener::bind(&addr).await?;
    eprintln!("[PROXY] rodando em http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let state = state.clone();
                async move {
                    let result: Result<Response<BoxBodyType>, hyper::Error> = handle_request(req, state).await;
                    result
                }
            });

            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                eprintln!("Erro na conexão: {}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<Incoming>,
    state: Arc<ProxyState>,
) -> Result<Response<BoxBodyType>, hyper::Error> {
    let request_start = std::time::Instant::now();
    let (parts, body) = req.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            eprintln!("[PROXY] error=body_read_failed error_msg={}", e);
            let body = box_body(Full::new(Bytes::from(format!("Erro ao ler body: {}", e))));
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(body)
                .unwrap());
        }
    };

    let start_idx = state.next_token_index();
    let mut attempts = 0;

    loop {
        let idx = (start_idx + attempts) % state.tokens.len();
        let token = &state.tokens[idx];
        let attempt_start = std::time::Instant::now();

        match forward_request(&parts, &body_bytes, token, &state).await {
            Ok(resp) => {
                let status = resp.status();
                let elapsed = attempt_start.elapsed();

                eprintln!("[PROXY] token={} status={} latency_ms={}",
                    idx + 1,
                    status.as_u16(),
                    elapsed.as_millis()
                );

                if should_rotate(status) && attempts + 1 < state.tokens.len() {
                    eprintln!("[PROXY] rotating reason=status_failed token={} status={}",
                        idx + 1,
                        status.as_u16()
                    );
                    attempts += 1;
                    continue;
                }

                state.set_current_key(idx);
                eprintln!("[PROXY] request_complete total_latency_ms={}", request_start.elapsed().as_millis());
                return Ok(resp);
            }
            Err(e) => {
                let elapsed = attempt_start.elapsed();
                eprintln!("[PROXY] token={} error={} latency_ms={}",
                    idx + 1,
                    e,
                    elapsed.as_millis()
                );

                if attempts + 1 < state.tokens.len() {
                    eprintln!("[PROXY] rotating reason=error token={}", idx + 1);
                    attempts += 1;
                    continue;
                }

                eprintln!("[PROXY] all_tokens_failed total_attempts={}", attempts + 1);
                let body = box_body(Full::new(Bytes::from(format!("Todos tokens falharam: {}", e))));
                return Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(body)
                    .unwrap());
            }
        }
    }
}

async fn forward_request(
    parts: &hyper::http::request::Parts,
    body_bytes: &Bytes,
    token: &str,
    state: &ProxyState,
) -> Result<Response<BoxBodyType>> {
    let path = parts.uri.path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    
    let uri = format!("{}{}", state.upstream, path);

    let mut builder = Request::builder()
        .method(parts.method.clone())
        .uri(&uri);

    for (key, value) in &parts.headers {
        if should_skip_header(key.as_str()) {
            continue;
        }
        builder = builder.header(key, value.clone());
    }

    builder = builder.header("Authorization", format!("Bearer {}", token));

    let body = box_body(Full::new(body_bytes.clone()));
    
    let new_req = builder.body(body)?;
    let resp = tokio::time::timeout(Duration::from_secs(30), state.client.request(new_req))
        .await
        .map_err(|_| anyhow::anyhow!("request timed out after 30s"))??;

    let status = resp.status();
    let mut builder = Response::builder().status(status);

    for (key, value) in resp.headers() {
        if should_skip_response_header(key.as_str()) {
            continue;
        }
        builder = builder.header(key, value);
    }

    let body = BoxBody::new(resp.into_body().map_err(|e| -> BoxError { e.into() }));

    Ok(builder.body(body)?)
}

fn should_rotate(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        401 | 403 | 429 | 500..=599
    )
}

fn should_skip_header(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "host" | "content-length" | "authorization" | 
        "connection" | "transfer-encoding" | "proxy-connection"
    )
}

fn should_skip_response_header(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "content-length" | "transfer-encoding" | "connection" | "keep-alive" | "trailer"
    )
}

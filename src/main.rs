mod proxy;
mod token_store;
mod headroom;

use clap::{Parser, Subcommand};
use anyhow::Result;
use std::process;

#[derive(Parser)]
#[clap(name = "agnes-token-proxy", about = "Agnes Token Proxy com rotação automática")]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Inicia proxy + headroom
    Init {
        #[clap(long, default_value = "9020")]
        proxy_port: u16,
        #[clap(long, default_value = "9010")]
        headroom_port: u16,
        #[clap(long, default_value = "https://apihub.agnes-ai.com")]
        upstream: String,
    },
    /// Adiciona um token
    Add {
        token: String,
    },
    /// Lista tokens cadastrados
    List,
    /// Remove um token
    Remove,
}

#[tokio::main]
async fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("[PANIC] {}", panic_info);
    }));

    let cli = Cli::parse();
    let is_direct_run = cli.command.is_none();

    if is_direct_run {
        show_menu().await?;
    } else {
        let result = match cli.command.unwrap() {
            Commands::Init { proxy_port, headroom_port, upstream } => {
                run_init(proxy_port, headroom_port, upstream, false).await
            }
            Commands::Add { token } => {
                let store = token_store::TokenStore::new()?;
                store.add(&token)
            }
            Commands::List => {
                let store = token_store::TokenStore::new()?;
                store.list()
            }
            Commands::Remove => {
                remove_token_interactive()
            }
        };

        if let Err(e) = &result {
            eprintln!("Erro: {:?}", e);
        }
    }

    Ok(())
}

fn clear_screen() {
    let _ = std::process::Command::new("cmd")
        .args(&["/C", "cls"])
        .status();
}

fn wait_for_enter() {
    println!("\nPressione Enter para voltar ao menu...");
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
}

async fn show_menu() -> Result<()> {
    loop {
        clear_screen();
        println!("╔══════════════════════════════════════╗");
        println!("║   Agnes Token Proxy - Menu Principal ║");
        println!("╠══════════════════════════════════════╣");
        println!("║  1. Iniciar Proxy                    ║");
        println!("║  2. Adicionar Token                  ║");
        println!("║  3. Listar Tokens                    ║");
        println!("║  4. Remover Token                    ║");
        println!("║  5. Sair                             ║");
        println!("╚══════════════════════════════════════╝");
        print!("\nEscolha uma opção: ");
        use std::io::Write;
        let _ = std::io::stdout().flush();

        let mut choice = String::new();
        std::io::stdin().read_line(&mut choice)?;
        let choice = choice.trim();

        match choice {
            "1" => {
                clear_screen();
                run_init(9020, 9010, "https://apihub.agnes-ai.com".to_string(), true).await?;
                break;
            }
            "2" => {
                clear_screen();
                println!("=== Adicionar Token ===\n");
                print!("Digite o token: ");
                let _ = std::io::stdout().flush();
                let mut token = String::new();
                std::io::stdin().read_line(&mut token)?;
                let store = token_store::TokenStore::new()?;
                store.add(token.trim())?;
                wait_for_enter();
            }
            "3" => {
                clear_screen();
                println!("=== Tokens Cadastrados ===\n");
                let store = token_store::TokenStore::new()?;
                store.list()?;
                wait_for_enter();
            }
            "4" => {
                clear_screen();
                println!("=== Remover Token ===\n");
                remove_token_interactive()?;
                wait_for_enter();
            }
            "5" => {
                println!("Até logo!");
                break;
            }
            _ => {
                println!("Opção inválida!");
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }

    Ok(())
}

fn remove_token_interactive() -> Result<()> {
    let store = token_store::TokenStore::new()?;
    let tokens = store.load()?;

    if tokens.is_empty() {
        println!("Nenhum token cadastrado.");
        return Ok(());
    }

    println!("\nTokens cadastrados:");
    for (i, token) in tokens.iter().enumerate() {
        let masked = if token.len() > 12 {
            format!("{}{}{}",
                &token[..8],
                "*".repeat(token.len() - 12),
                &token[token.len()-4..])
        } else {
            "*".repeat(token.len())
        };
        println!("[{}] {}", i + 1, masked);
    }

    println!("\nDigite o número do token para remover (0 para cancelar): ");
    let mut choice = String::new();
    std::io::stdin().read_line(&mut choice)?;
    let choice: usize = match choice.trim().parse() {
        Ok(n) => n,
        Err(_) => {
            println!("Número inválido!");
            return Ok(());
        }
    };

    if choice == 0 {
        println!("Cancelado.");
        return Ok(());
    }

    if choice > tokens.len() {
        println!("Número inválido!");
        return Ok(());
    }

    let mut new_tokens = tokens.clone();
    new_tokens.remove(choice - 1);
    store.save(&new_tokens)?;
    println!("Token removido. Total: {}", new_tokens.len());

    Ok(())
}

async fn run_init(proxy_port: u16, headroom_port: u16, upstream: String, is_direct_run: bool) -> Result<()> {
    println!("Verificando headroom...");
    if let Err(e) = headroom::check_headroom() {
        eprintln!("Erro: {}", e);
        if is_direct_run { pause(); }
        process::exit(1);
    }

    eprintln!("[INIT] starting proxy on port {}", proxy_port);
    eprintln!("[INIT] starting headroom on port {}", headroom_port);

    // Iniciar headroom via cmd.exe (evita problemas de spawn em background)
    let mut headroom_child = headroom::spawn_headroom_via_cmd(proxy_port, headroom_port)?;

    // Aguardar headroom iniciar
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    eprintln!("[INIT] headroom spawned, continuando...");
    println!();
    println!("OpenCode:");
    println!("  Endpoint: http://127.0.0.1:{}/v1", headroom_port);
    println!("  API Key:  key123");
    println!();

    tokio::select! {
        result = proxy::run_proxy(proxy_port, upstream) => {
            if let Err(e) = result {
                eprintln!("[PROXY] fatal error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            eprintln!("[PROXY] shutting down gracefully...");
            let _ = headroom_child.kill();
            eprintln!("[PROXY] headroom terminated");
        }
    }

    Ok(())
}

fn pause() {
    println!();
    println!("Pressione Enter para fechar...");
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
}

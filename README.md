# Agnes Token Proxy

Proxy de rotação automática de tokens para AgnesAI. Gerencia múltiplos tokens com round-robin e integra com Headroom AI para caching e otimização de requisições.

## Requisitos

| Requisito | Versão |
|-----------|--------|
| Headroom AI | ≥ 0.39.1 |
| Rust toolchain | stable |
| Tokens AgnesAI | válidos |

## Arquitetura

```
┌─────────────┐      ┌─────────────────────┐      ┌───────────────────┐      ┌─────────────┐
│   Cliente   │ ───▶ │  Headroom (9010)    │ ───▶ │  Agnes Proxy      │ ───▶ │  AgnesAI    │
│  (OpenCode) │      │                     │      │     (9020)        │      │     API     │
└─────────────┘      └─────────────────────┘      └───────────────────┘      └─────────────┘
                            │                            │
                            ▼                            ▼
                     ┌─────────────┐             ┌───────────────┐
                     │ Cache/Rate  │             │    Token      │
                     │  Limiting   │             │   Rotation    │
                     └─────────────┘             │ (Round-robin) │
                                                 └───────────────┘
```

### Como funciona

1. **Headroom AI** (porta 9010): recebe requisições do cliente, aplica cache e rate limiting
2. **Agnes Proxy** (porta 9020): recebe requisições do Headroom, seleciona token via round-robin e forward para AgnesAI
3. **Token Rotation**: distribui requisições uniformemente entre todos os tokens cadastrados
4. **HTTPS**: usa rustls com certificados nativos do sistema para conexões seguras

## Modelo agnes-2.5-flash

| Especificação | Valor |
|---------------|-------|
| Input tokens | ilimitado (context window grande) |
| Output tokens | até 8192 |
| Context window | 32k+ tokens |
| Suporte | chat completions, reasoning |
| Velocidade | alta (flash) |
| Custo | baixo |

## Instalação

```bash
# Compilar
cargo build --release

# Adicionar tokens
./target/release/agnes-token-proxy add <token1>
./target/release/agnes-token-proxy add <token2>

# Iniciar proxy + headroom
./target/release/agnes-token-proxy init
```

## Uso

### Com OpenCode

```bash
OPENAI_BASE_URL=http://127.0.0.1:9010/v1 open-code
```

### Endpoint direto

```bash
curl http://127.0.0.1:9020/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "agnes-2.5-flash",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

## Comandos

| Comando | Descrição |
|---------|-----------|
| `init` | Inicia proxy (9020) + Headroom (9010) |
| `add <token>` | Adiciona token ao pool |
| `list` | Lista todos os tokens cadastrados |
| `remove <index>` | Remove token por índice |

## Implementação

- **Linguagem**: Rust
- **HTTP/HTTPS**: hyper + hyper-rustls + rustls
- **Async runtime**: tokio
- **CLI**: clap
- **Token storage**: arquivo JSON em `~/.config/agnes-token-proxy/tokens.json`
- **Rotação**: round-robin com atomic counter
- **Integração**: Headroom AI para caching e otimização

## Licença

Apache 2.0

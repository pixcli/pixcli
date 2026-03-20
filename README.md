# 🏦 pixcli

**Make Pix payments from your terminal. Built for developers and AI agents.**

[![CI](https://github.com/pixcli/pixcli/actions/workflows/ci.yml/badge.svg)](https://github.com/pixcli/pixcli/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/pixcli.svg)](https://crates.io/crates/pixcli)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## Features

- 💸 **Send & receive Pix payments** via CLI
- 🏦 **Multi-PSP support** — Efí (Gerencianet) built-in, extensible to others
- 📱 **QR code generation** — render in terminal or export as PNG
- 🔍 **QR code decoding** — parse any Pix EMV/BR Code payload
- 🤖 **MCP server** for AI agent integration (Claude Code, OpenClaw, etc.)
- 🔔 **Webhook receiver** for real-time payment notifications
- 🔒 **File-level permission protection** — config stored with `0600` (owner-only) permissions
- 📊 **Multiple output formats** — human-readable, JSON, and table

## Quick Start

```bash
# Install from crates.io
cargo install pixcli

# Run the setup wizard
pixcli config init

# Check your balance
pixcli balance
```

## Installation

### From crates.io (recommended)

```bash
cargo install pixcli
```

### From Homebrew (macOS/Linux)

```bash
brew tap pixcli/tap
brew install pixcli
```

### From GitHub Releases

Download the latest binary for your platform from
[GitHub Releases](https://github.com/pixcli/pixcli/releases/latest).

```bash
# Example for Linux x86_64
curl -LO https://github.com/pixcli/pixcli/releases/latest/download/pixcli-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
tar xzf pixcli-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
sudo mv pixcli /usr/local/bin/
```

### Build from source

```bash
git clone https://github.com/pixcli/pixcli.git
cd pixcli
cargo build --release
# Binaries: target/release/pixcli, target/release/pix-mcp, target/release/pix-webhook-server
```

## Usage

### Check balance

```bash
pixcli balance
# 💰 Balance: R$ 1.234,56
```

### Create a charge

```bash
pixcli charge create --amount 25.50 --description "Coffee ☕"
# ✅ Charge created: txid=abc123
# 📋 Copy & paste: 00020126...

# Use --output json for scripting
pixcli charge create --amount 25.50 --description "Invoice #42" --output json
```

### List charges

```bash
pixcli charge list
pixcli charge list --output json
```

### Get charge details

```bash
pixcli charge get --txid abc123def456
```

### List received Pix transactions

```bash
pixcli pix list --days 7
pixcli pix get --e2eid E12345678901234567890123456789012
```

### Generate a QR code (offline)

```bash
# Terminal QR code
pixcli qr generate --key "+5511999999999" --amount 10.00 --name "FULANO DE TAL" --city "SAO PAULO"

# Save as PNG
pixcli qr generate --key "email@example.com" --amount 50.00 --name "LOJA" --city "RIO" --png qr.png
```

### Decode a QR code payload

```bash
pixcli qr decode --payload "00020126580014br.gov.bcb.pix..."
```

### Webhook management

```bash
# Register a webhook URL
pixcli webhook register --key "+5511999999999" --url "https://example.com/pix"

# Check registered webhook
pixcli webhook get --key "+5511999999999"

# Start a local webhook listener
pixcli webhook listen --port 8080
```

### Configuration

```bash
# Interactive setup
pixcli config init

# Show current config (secrets redacted)
pixcli config show
```

### Global options

```bash
# Use a specific profile
pixcli --profile my-company balance

# JSON output
pixcli --output json charge list

# Sandbox mode
pixcli --sandbox balance

# Verbose logging
pixcli --verbose charge create --amount 10.00 --description "test"
```

## For AI Agents (MCP)

Pixcli ships with an MCP (Model Context Protocol) server that exposes Pix operations
as tools for AI agents.

### Start the MCP server

```bash
pix-mcp
```

### Configure in Claude Code

Add to your Claude Code MCP config (`.claude/mcp.json`):

```json
{
  "mcpServers": {
    "pix": {
      "command": "pix-mcp",
      "args": []
    }
  }
}
```

### Configure in OpenClaw

Add to your OpenClaw config:

```yaml
plugins:
  entries:
    pix-mcp:
      enabled: true
      runtime: mcp
      config:
        command: pix-mcp
```

### Available MCP tools

| Tool | Description |
|------|-------------|
| `create_charge` | Create a Pix charge and get QR code / payment link |
| `get_charge` | Check the status of a charge by transaction ID |
| `get_balance` | Get the current account balance |
| `list_received_pix` | List recent received Pix transactions |
| `send_pix` | Send a Pix payment to a recipient |
| `generate_qr` | Generate a static Pix QR code payload (offline) |

## Configuration

Configuration is stored at `~/.pixcli/config.toml`.

```toml
[defaults]
profile = "default"
output = "human"

[profiles.default]
backend = "efi"
environment = "sandbox"         # or "production"
client_id = "your-client-id"
client_secret = "your-client-secret"
certificate = "/path/to/certificate.p12"
certificate_password = ""
default_pix_key = "+5511999999999"
```

### Environment variable overrides

You can override config values with environment variables:

| Variable | Overrides |
|----------|-----------|
| `PIXCLI_CONFIG` | Config file path |
| `PIXCLI_PROFILE` | Default profile |
| `PIXCLI_CLIENT_ID` | OAuth2 client ID |
| `PIXCLI_CLIENT_SECRET` | OAuth2 client secret |
| `PIXCLI_CERTIFICATE` | Certificate path |

## Supported PSPs

| Provider | Status | Notes |
|----------|--------|-------|
| Efí (Gerencianet) | ✅ Supported | OAuth2 + mTLS, full API coverage |
| Mercado Pago | 🚧 Planned | — |
| PagSeguro | 🚧 Planned | — |
| Banco do Brasil | 🚧 Planned | — |

Want to add a new PSP? See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security

- **Credential storage**: Config file is created with `600` permissions (owner read/write only). Credentials are stored in plaintext — no encryption at rest
- **mTLS authentication**: PKCS#12 certificates for secure API communication
- **No plaintext secrets in logs**: Sensitive values are always redacted in output
- **Sandbox mode**: Test safely without touching real money
- **Webhook server**: For production use, deploy behind a reverse proxy with mTLS termination

## Architecture

```
pixcli/
├── crates/
│   ├── pix-core/             # Core types: PixKey, CRC16, validation
│   ├── pix-brcode/           # EMV/BR Code encoder & decoder
│   ├── pix-provider/         # PixProvider trait + common types
│   ├── pix-efi/              # Efí backend (OAuth2 + mTLS)
│   ├── pix-mcp/              # MCP server for AI agents
│   └── pix-webhook-server/   # Standalone webhook receiver
└── src/                      # CLI binary (ties it all together)
```

### Library usage (Rust)

The individual crates can be used as libraries:

```rust
use pix_core::{PixKey, PixKeyType};
use pix_brcode::StaticQrPayload;

// Validate a Pix key
let key = PixKey::new(PixKeyType::Phone, "+5511999999999").unwrap();

// Generate a BR Code payload
let payload = StaticQrPayload::new(key.value(), "FULANO", "SAO PAULO")
    .with_amount("10.00")
    .encode();
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, how to add a new PSP backend, and contribution guidelines.

## License

Licensed under the [MIT License](LICENSE).

Copyright (c) 2026 Felipe Orlando

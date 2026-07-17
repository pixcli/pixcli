<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-pix-light.svg" />
    <source media="(prefers-color-scheme: light)" srcset="assets/logo-pix-dark.svg" />
    <img src="assets/logo-pix-dark.svg" alt="pixcli logo" width="120" />
  </picture>
</p>

<h1 align="center">pixcli</h1>

<p align="center"><strong>Make Pix payments from your terminal. Built for developers and AI agents.</strong></p>

<p align="center">
  <a href="https://github.com/pixcli/pixcli/actions/workflows/ci.yml"><img src="https://github.com/pixcli/pixcli/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <a href="https://docs-ecru-eta-65.vercel.app"><img src="https://img.shields.io/badge/docs-online-brightgreen" alt="Docs" /></a>
</p>

---

## Features

- 💸 **Send & receive Pix payments** via CLI
- 🏦 **Multi-PSP support** — Efí (Gerencianet) built-in, extensible to others
- 📱 **QR code generation** — render in terminal or export as PNG
- 🔍 **QR code decoding** — parse any Pix EMV/BR Code payload
- 🤖 **MCP server** for AI agent integration (Claude Code, OpenClaw, etc.)
- 🧠 **Agent skill** on [skills.sh](https://skills.sh) — `npx skills add pixcli/pixcli`
- 🔔 **Webhook receiver** for real-time payment notifications
- 🔒 **File-level permission protection** — config stored with `0600` (owner-only) permissions
- 📊 **Multiple output formats** — human-readable, JSON, and table

## Quick Start

```bash
# Install with cargo (from git)
cargo install --git https://github.com/pixcli/pixcli pixcli

# Run the setup wizard
pix config init

# Check your balance
pix balance
```

## Installation

### With cargo (recommended)

```bash
cargo install --git https://github.com/pixcli/pixcli pixcli
```

> **Note:** `cargo install pixcli` (from crates.io) is not yet available because
> the crate has not been published. Use the git command above or the
> [release binaries](#from-github-releases) instead.

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
curl -LO https://github.com/pixcli/pixcli/releases/latest/download/pixcli-v0.0.2-x86_64-unknown-linux-gnu.tar.gz
tar xzf pixcli-v0.0.2-x86_64-unknown-linux-gnu.tar.gz
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
pix balance
# 💰 Balance: R$ 1.234,56
```

### Create a charge

```bash
pix charge create --amount 25.50 --description "Coffee ☕"
# ✅ Charge created: txid=abc123
# 📋 Copy & paste: 00020126...

# Use --output json for scripting
pix charge create --amount 25.50 --description "Invoice #42" --output json
```

### List charges

```bash
pix charge list
pix charge list --output json
```

### Get charge details

```bash
pix charge get --txid abc123def456
```

### List received Pix transactions

```bash
pix pix list --days 7
pix pix get --e2eid E12345678901234567890123456789012
```

### Generate a QR code (offline)

```bash
# Terminal QR code
pix qr generate --key "+5511999999999" --amount 10.00 --name "FULANO DE TAL" --city "SAO PAULO"

# Save as PNG
pix qr generate --key "email@example.com" --amount 50.00 --name "LOJA" --city "RIO" --png qr.png
```

### Decode a QR code payload

```bash
pix qr decode --payload "00020126580014br.gov.bcb.pix..."
```

### Webhook management

```bash
# Register a webhook URL
pix webhook register --key "+5511999999999" --url "https://example.com/pix"

# Check registered webhook
pix webhook get --key "+5511999999999"

# Start a local webhook listener
pix webhook listen --port 8080
```

### Configuration

```bash
# Interactive setup
pix config init

# Show current config (secrets redacted)
pix config show
```

### Global options

```bash
# Use a specific profile
pix --profile my-company balance

# JSON output
pix --output json charge list

# Sandbox mode
pix --sandbox balance

# Verbose logging
pix --verbose charge create --amount 10.00 --description "test"
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

### Agent Skill (skills.sh)

Pixcli also ships an **agent skill** — a self-contained `SKILL.md` that teaches an AI agent when and how to use pixcli safely (trigger phrases, command reference, and safety rules for irreversible Pix payments). Install it into Claude Code, Cursor, or any [skills.sh](https://skills.sh)-compatible agent with a single command:

```bash
npx skills add pixcli/pixcli
```

The skill lives at [`skills/pixcli/SKILL.md`](skills/pixcli/SKILL.md) and complements the MCP server: the skill provides the *playbook*, `pix-mcp` provides the *tools*. See the [Agent Skill docs](https://docs-ecru-eta-65.vercel.app/docs/mcp/skill) for details.

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

## Why Efí?

You might wonder why pixcli ships with Efí (formerly Gerencianet) as its first provider instead of Mercado Pago, Nubank, or another big name. The answer is simple: Efí is the only PSP in Brazil that gives developers a fully open Pix API with zero friction.

**Efí gets it right:**
- Sign up, get API keys, and start making real Pix calls in under 30 minutes
- Full BACEN-compliant Pix API (charges, payments, refunds, webhooks, DICT)
- Working sandbox environment for testing without real money
- mTLS certificate auth (which BACEN requires anyway)
- Excellent documentation at [dev.sejaefi.com.br](https://dev.sejaefi.com.br)
- Free account, no monthly fees, no approval process

**Why not the others:**
- **Nubank** — No public Pix API. No developer docs. No OAuth. Only works inside their app.
- **Mercado Pago** — Has a Pix API, but it's designed for their checkout ecosystem. Direct Pix API access requires commercial approval and is limited compared to the full BACEN spec.
- **PagSeguro** — Pix API exists but requires a lengthy approval process before you can make a single API call.
- **Inter, C6, BTG** — Some offer Open Finance APIs, but none expose a straightforward Pix API for developers to create charges and process payments.

pixcli is built on a **provider trait system** (`pix-provider` crate), so adding new PSPs is straightforward. Efí is the default because it lets you go from `cargo install` to real Pix transactions in minutes, not weeks.

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

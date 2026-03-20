# Contributing to Pixcli

Thank you for your interest in contributing! This guide will help you get started.

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) (stable, latest)
- [Git](https://git-scm.com/)

### Clone and build

```bash
git clone https://github.com/pixcli/pixcli.git
cd pixcli
cargo build --workspace
```

### Run tests

```bash
# All unit tests
cargo test --workspace

# With verbose output
cargo test --workspace -- --nocapture

# A specific crate
cargo test -p pix-core
```

### Run lints

```bash
# Format check
cargo fmt --all -- --check

# Clippy
cargo clippy --workspace --all-targets -- -D warnings

# Build docs
cargo doc --workspace --no-deps
```

## Project Structure

```
pixcli/
├── crates/
│   ├── pix-core/             # Core types, validation, CRC16
│   ├── pix-brcode/           # EMV/BR Code encoder & decoder
│   ├── pix-provider/         # PixProvider trait + shared types
│   ├── pix-efi/              # Efí (Gerencianet) backend
│   ├── pix-mcp/              # MCP server for AI agents
│   └── pix-webhook-server/   # Webhook receiver
├── src/                      # CLI binary
├── .github/workflows/        # CI/CD
└── Cargo.toml                # Workspace root
```

## How to Add a New PSP Backend

Adding a new payment provider is straightforward. Follow these steps:

### 1. Create the crate

```bash
cargo new crates/pix-newpsp --lib
```

### 2. Add workspace dependencies

In `crates/pix-newpsp/Cargo.toml`:

```toml
[package]
name = "pix-newpsp"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "NewPSP Pix provider"

[dependencies]
pix-core.workspace = true
pix-provider.workspace = true
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
reqwest.workspace = true
chrono.workspace = true
tracing.workspace = true
```

Add the crate to `[workspace.members]` in the root `Cargo.toml`.

### 3. Implement the `PixProvider` trait

Create `crates/pix-newpsp/src/lib.rs`:

```rust
use pix_provider::{
    PixProvider, ProviderError,
    Balance, ChargeRequest, ChargeResponse, PixCharge,
    PixTransaction, PixTransfer, TransactionFilter,
    DueDateChargeRequest,
};

pub struct NewPspProvider {
    // Your client state: HTTP client, credentials, tokens, etc.
}

impl PixProvider for NewPspProvider {
    async fn create_charge(&self, request: ChargeRequest) -> Result<ChargeResponse, ProviderError> {
        todo!("Implement charge creation")
    }

    async fn create_due_date_charge(&self, request: DueDateChargeRequest) -> Result<ChargeResponse, ProviderError> {
        todo!("Implement due-date charge")
    }

    async fn get_charge(&self, txid: &str) -> Result<PixCharge, ProviderError> {
        todo!("Implement get charge")
    }

    async fn list_charges(&self, filter: TransactionFilter) -> Result<Vec<PixCharge>, ProviderError> {
        todo!("Implement list charges")
    }

    async fn send_pix(&self, key: &str, amount: &str, description: Option<&str>) -> Result<PixTransfer, ProviderError> {
        todo!("Implement send Pix")
    }

    async fn get_pix(&self, e2eid: &str) -> Result<PixTransaction, ProviderError> {
        todo!("Implement get Pix")
    }

    async fn list_received_pix(&self, filter: TransactionFilter) -> Result<Vec<PixTransaction>, ProviderError> {
        todo!("Implement list received Pix")
    }

    async fn get_balance(&self) -> Result<Balance, ProviderError> {
        todo!("Implement balance")
    }

    fn provider_name(&self) -> &str {
        "newpsp"
    }
}
```

### 4. Register the backend in the CLI

In `src/client_factory.rs`, add your provider to the match statement:

```rust
"newpsp" => {
    let provider = pix_newpsp::NewPspProvider::new(/* config */);
    Box::new(provider)
}
```

### 5. Add tests

- Unit tests in `crates/pix-newpsp/src/lib.rs` (or `tests/` module)
- Use `wiremock` to mock HTTP responses (see `pix-efi` for examples)
- Optionally: integration tests gated behind `--features integration`

### 6. Update documentation

- Add the provider to the "Supported PSPs" table in `README.md`
- Add any provider-specific configuration to the config section

## Integration Testing with Efí Sandbox

To run integration tests against the Efí sandbox:

1. Create an Efí sandbox account at [https://dev.efipay.com.br](https://dev.efipay.com.br)
2. Download the sandbox certificate (`.p12` file)
3. Set up your config:

```bash
pixcli config init
# Select: backend = efi, environment = sandbox
# Provide your sandbox client_id, client_secret, certificate path
```

4. Run integration tests:

```bash
cargo test --workspace --features integration
```

## Code Style

- **Format**: Run `cargo fmt --all` before committing
- **Lint**: Run `cargo clippy --workspace --all-targets -- -D warnings`
- **Documentation**: All public items should have doc comments (`///`)
- **Error handling**: Use `thiserror` for library errors, `anyhow` for binary errors
- **Async**: Use `tokio` as the async runtime
- **Testing**: Aim for high test coverage; use property-based testing (`proptest`) where appropriate

## Pull Request Process

1. Fork the repository and create your branch from `main`
2. Make your changes with clear, descriptive commits
3. Ensure all checks pass: `cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --all -- --check`
4. Update documentation if you changed public APIs or behaviour
5. Open a PR with a clear description of what and why

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` — new feature
- `fix:` — bug fix
- `docs:` — documentation changes
- `chore:` — maintenance, CI, dependencies
- `refactor:` — code changes that neither fix bugs nor add features
- `test:` — adding or updating tests

## Questions?

Open an issue on [GitHub](https://github.com/pixcli/pixcli/issues) — we're happy to help!

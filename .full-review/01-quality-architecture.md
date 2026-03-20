# Code Quality & Architecture Review

## Summary
The pixcli project demonstrates solid Rust architecture with proper workspace organization, clear trait abstractions, and good error handling boundaries (thiserror in libraries, anyhow in binaries). The codebase follows Rust idioms well but has several areas for improvement.

## Architecture Strengths
- Clean workspace with 7 well-separated crates
- Provider trait pattern (PixProvider) enables future provider swaps
- EMV TLV encoding follows the spec faithfully
- Token caching with RwLock double-check locking
- Good error type hierarchy

## Findings

### CRITICAL

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| Q-C1 | Config module duplication | `src/config.rs` vs `crates/pix-efi/src/config.rs` | Two separate config modules with overlapping concerns. CLI's `PixConfig` wraps profiles while `EfiConfig` has the raw credentials. Should be unified or have clear layering. |
| Q-C2 | String-typed monetary amounts | Throughout (`pix-provider/src/types.rs`) | All amounts (Balance.available, ChargeRequest.amount, etc.) are `String` typed. Should use a decimal type (rust_decimal) to prevent floating-point errors in financial calculations. |

### HIGH

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| Q-H1 | Factory logic duplication | `src/client_factory.rs:46-80` | `build_provider()` and `build_efi_client()` have identical bodies. Should extract shared logic. |
| Q-H2 | DynPixProvider incomplete | `crates/pix-mcp/src/server.rs:36-67` | The DynPixProvider trait omits `create_due_date_charge`, `list_charges`, `get_pix` methods from PixProvider. MCP server cannot access these operations. |
| Q-H3 | Blocking I/O in async context | `crates/pix-efi/src/auth.rs:59` | `std::fs::read()` in `EfiAuth::new()` blocks the async runtime. Should use `tokio::fs::read()`. |
| Q-H4 | Blocking file I/O in webhook handler | `crates/pix-webhook-server/src/handlers.rs:69-80` | `std::fs::OpenOptions` used in async handler. Should use `tokio::fs` or spawn_blocking. |
| Q-H5 | Duplicate #[cfg(test)] | `crates/pix-mcp/src/server.rs:143-144` | Double `#[cfg(test)]` attribute on `from_dyn` method. |
| Q-H6 | No request body size limit | `crates/pix-webhook-server/src/main.rs` | No body size limit on webhook endpoint. An attacker could send massive payloads to exhaust memory. |

### MEDIUM

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| Q-M1 | Hardcoded retry parameters | `crates/pix-efi/src/client.rs` | Retry count (3), backoff base (500ms), and jitter are hardcoded. Should be configurable. |
| Q-M2 | No graceful shutdown | `crates/pix-webhook-server/src/main.rs` | Server has no signal handling for graceful shutdown. |
| Q-M3 | No rate limiting | `crates/pix-webhook-server/src/main.rs` | No rate limiting on webhook endpoint. |
| Q-M4 | Token URL duplication | `crates/pix-efi/src/auth.rs:164-168` vs `config.rs` | Token URL constructed in auth.rs and also available from config. |
| Q-M5 | No webhook payload validation | `crates/pix-webhook-server/src/handlers.rs` | Webhook accepts any valid JSON matching the struct but doesn't validate e2eid format, amount format, etc. |
| Q-M6 | Error type conversion loss | `crates/pix-efi/src/error.rs` | EfiError → ProviderError conversion loses error type granularity. |
| Q-M7 | No timeout on forwarding | `crates/pix-webhook-server/src/handlers.rs:84-100` | HTTP forwarding uses default client with no timeout configuration. |

### LOW

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| Q-L1 | Unused token_type field | `crates/pix-efi/src/auth.rs:21,31` | `TokenResponse.token_type` and `CachedToken.token_type` are stored but never used. |
| Q-L2 | Scope field unused | `crates/pix-efi/src/auth.rs:24` | `TokenResponse.scope` is deserialized but never used. |

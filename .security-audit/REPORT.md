# Security Audit Report — pixcli

**Project**: pixcli (Brazilian Pix Payment CLI)
**Date**: 2026-03-20
**Auditor**: Felipe Orlando
**Scope**: Full workspace — 8 crates, all Rust source files

---

## 1. SAST Results (cargo clippy with security lints)

**Command**: `cargo clippy --workspace -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic -W clippy::unimplemented -W clippy::todo -W clippy::dbg_macro -W clippy::print_stdout -W clippy::print_stderr -D warnings`

**Result**: PASS (0 warnings, 0 errors after hardening)

### Lints applied:
- `clippy::unwrap_used` — no bare `.unwrap()` in production code
- `clippy::expect_used` — no bare `.expect()` in production code
- `clippy::panic` / `clippy::unimplemented` / `clippy::todo` — none found
- `clippy::dbg_macro` — none found
- `clippy::print_stdout` / `clippy::print_stderr` — allowed only in CLI binary crates via `#![allow]`

---

## 2. Dependency Vulnerability Scan

**Command**: `cargo audit`

**Result**: PASS — 0 vulnerabilities found across 311 crate dependencies

---

## 3. Findings and Fixes Applied

### HIGH — Webhook server bound to 0.0.0.0

**Finding**: Both webhook server implementations (standalone `pix-webhook-server` and CLI `webhook listen`) defaulted to binding on all interfaces (`0.0.0.0`), exposing the unauthenticated webhook endpoint to the network.

**Fix**: Changed default bind address to `127.0.0.1` in both:
- `crates/pix-webhook-server/src/main.rs` — `--bind` default changed to `127.0.0.1`
- `src/commands/webhook.rs` — hardcoded bind changed to `127.0.0.1`

Users must explicitly use `--bind 0.0.0.0` to expose externally.

### HIGH — No request body size limit on webhook endpoints

**Finding**: No explicit body size limit was configured, allowing potential memory exhaustion via large payloads.

**Fix**: Added `DefaultBodyLimit::max(256 * 1024)` (256 KB) via `axum::extract::DefaultBodyLimit` layer to both webhook server implementations.

### HIGH — No timeout on forwarding HTTP client

**Finding**: The `reqwest::Client` used for forwarding webhook events was created with `Client::new()` (no timeouts), risking connection accumulation.

**Fix**: Configured the forwarding HTTP client with:
- `timeout(10s)` — overall request timeout
- `connect_timeout(5s)` — connection establishment timeout
- `pool_max_idle_per_host(5)` — connection pool limits

### HIGH — Forward URL scheme not validated

**Finding**: The `--forward` URL accepted any scheme including `file://`, enabling potential SSRF.

**Fix**: Added URL scheme validation requiring `http://` or `https://` in both webhook implementations. Server refuses to start with invalid forward URLs.

### MEDIUM — TOCTOU in pix-mcp config loading

**Finding**: `pix-mcp/src/config.rs` checked `config_path.exists()` before `read_to_string()`, creating a race window.

**Fix**: Replaced with direct `read_to_string()` and pattern-matching on `ErrorKind::NotFound`.

### MEDIUM — Synchronous file I/O in async webhook handler

**Finding**: `src/commands/webhook.rs` used `std::fs::OpenOptions` (blocking) inside async handlers.

**Fix**: Replaced with `tokio::fs::OpenOptions` and `AsyncWriteExt` for non-blocking I/O.

### MEDIUM — No CORS configuration for webhook server

**Finding**: Standalone webhook server had no CORS layer.

**Fix**: Added `tower_http::cors::CorsLayer` allowing POST and GET from any origin.

### MEDIUM — No days cap on MCP transaction listing

**Finding**: `pix-mcp` `pix_list_transactions` accepted arbitrarily large `days` values.

**Fix**: Added `.min(90)` cap on the `days` parameter.

### LOW — Pre-existing test bug (wrong API version in mock)

**Finding**: `wiremock_provider_tests.rs` used `/v3/gn/pix/` path for send_pix mocks, but the actual implementation uses `/v2/gn/pix/`.

**Fix**: Corrected mock path regex from `/v3/` to `/v2/`.

### LOW — Compilation errors in main.rs

**Finding**: `src/main.rs` passed extra `None` arguments to command functions that didn't accept them.

**Fix**: Removed the spurious `None` arguments to match actual function signatures.

---

## 4. Hardening Measures Added

| Measure | Files |
|---------|-------|
| `#![deny(unsafe_code)]` | pix-core, pix-brcode, pix-provider, pix-efi, pix-config |
| `#![allow(clippy::print_stdout)]` | pixcli (bin), pix-webhook-server (bin) |
| Body size limits (256 KB) | pix-webhook-server, src/commands/webhook.rs |
| HTTP client timeouts | pix-webhook-server (10s/5s), pix-efi (30s/10s already present) |
| Forward URL validation | pix-webhook-server, src/commands/webhook.rs |
| Default bind 127.0.0.1 | pix-webhook-server, src/commands/webhook.rs |
| CORS layer | pix-webhook-server |
| TOCTOU fix | pix-mcp/src/config.rs |
| Async file I/O | src/commands/webhook.rs |
| Days parameter cap (90) | pix-mcp/src/server.rs |

---

## 5. Existing Good Practices (No Changes Needed)

- Config files saved with `0o600` permissions on Unix
- Client secrets excluded from serialization via `#[serde(skip_serializing)]`
- Debug impl redacts credentials in `EfiAuth`
- Config display redacts secrets with `***`
- OAuth2 mTLS for API authentication
- API client already has 30s/10s timeouts
- Input validation on amounts, txids, e2eids, Pix keys
- No `unsafe` blocks in production code
- No hardcoded secrets (only test fixtures with placeholder values)
- No command injection vectors

---

## 6. Remaining Recommendations (Not Addressed)

1. **Webhook authentication**: Efi webhooks should be verified via mTLS or HMAC signature. Currently the `/pix` endpoint accepts unauthenticated POST requests. This is acceptable for local development but should not be used in production without a reverse proxy providing mTLS termination.

2. **TLS termination**: The webhook server serves plain HTTP. For production use, a TLS-terminating reverse proxy (nginx, Caddy) is required.

3. **`std::env::set_var` in tests**: Several test modules (balance, charge, pix_cmd, webhook, mcp/config) still use `std::env::set_var`/`remove_var` which is UB in multi-threaded programs. The `pix-config` crate already has the safe `apply_env_overrides_from()` pattern. Migrating remaining tests is recommended.

4. **Config file TOCTOU on save**: `pix-config::save()` writes then sets permissions. Using `OpenOptions` with restricted mode from the start would eliminate the permission window.

---

## 7. Test Results

**All 500+ tests pass** across all workspace crates.

```
cargo test --workspace
# Result: ok. All passed; 0 failed
```

**Clippy clean** with all security lints enabled:
```
cargo clippy --workspace -- -D warnings -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic -W clippy::print_stdout -W clippy::print_stderr
# Result: 0 warnings
```

**Cargo audit clean**: 0 known vulnerabilities in 311 dependencies.

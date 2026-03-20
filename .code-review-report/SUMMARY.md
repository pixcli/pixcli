# pixcli Code Review & Test Generation Report

**Project:** pixcli v0.1.0 — Brazilian Pix payments CLI
**Language:** Rust (Edition 2021)
**Date:** 2026-03-20
**Review Mode:** Strict + Security Focus + Performance Critical

---

## Executive Summary

The pixcli project is a well-structured Rust workspace with 7 crates implementing a CLI for Brazilian Pix payments. The codebase shows strong Rust fundamentals: clean crate separation, proper trait abstractions (PixProvider), good error handling boundaries (thiserror in libraries, anyhow in binaries), and solid test coverage in core areas with property-based testing.

However, the review identified **7 critical**, **18 high**, **25 medium**, and **19 low** severity findings. The most urgent issues are in the webhook server (no authentication, no body size limits) and credential management (plaintext storage). Financial-grade reliability is not yet met due to string-typed monetary amounts.

---

## Findings by Severity

### Critical (7)

| ID | Finding | Location | Impact |
|----|---------|----------|--------|
| S-C1 | **No webhook authentication** | `pix-webhook-server/src/handlers.rs:44` | Anyone can POST fake payment notifications (CVSS 9.1) |
| S-C2 | **Credentials in plaintext config** | `src/config.rs` | client_id, client_secret, cert password in plain TOML |
| S-C3 | **No request body size limit** | `pix-webhook-server/src/main.rs` | DoS via memory exhaustion with oversized payloads |
| Q-C1 | **Config module duplication** | `src/config.rs` vs `pix-efi/src/config.rs` | Overlapping concerns, maintenance burden |
| Q-C2 | **String-typed monetary amounts** | `pix-provider/src/types.rs` | Floating-point errors in financial calculations |
| B-C1 | **No CI/CD configuration** | project root | No automated testing or linting enforcement |
| T-C1 | **No integration tests for CLI** | `src/` | End-to-end correctness unverified |

### High (18)

| ID | Finding | Location |
|----|---------|----------|
| S-H1 | Webhook binds to 0.0.0.0 by default | `pix-webhook-server/src/main.rs:26` |
| S-H2 | SSRF via unvalidated forward URL | `pix-webhook-server/src/handlers.rs:84-100` |
| S-H3 | No TLS on webhook server | `pix-webhook-server/src/main.rs` |
| S-H4 | MCP allows unrestricted payments | `pix-mcp/src/server.rs:257-296` |
| S-H5 | Certificate password in memory (not zeroized) | `pix-efi/src/auth.rs` |
| Q-H1 | Factory logic duplication | `src/client_factory.rs:46-80` |
| Q-H2 | DynPixProvider missing 3 trait methods | `pix-mcp/src/server.rs:36-67` |
| Q-H3 | Blocking I/O in async (std::fs::read) | `pix-efi/src/auth.rs:59` |
| Q-H4 | Blocking file I/O in async webhook handler | `pix-webhook-server/src/handlers.rs:69-80` |
| Q-H5 | Duplicate #[cfg(test)] attribute | `pix-mcp/src/server.rs:143-144` |
| Q-H6 | No body size limit on webhook | `pix-webhook-server/src/main.rs` |
| B-H1 | Inconsistent logging (println! vs tracing) | throughout |
| B-H2 | No metrics or observability | throughout |
| B-H3 | Missing error context (bare `?`) | multiple |
| B-H4 | No version pinning strategy | `Cargo.toml` |
| B-H5 | No input sanitization | MCP server, webhook |
| B-H6 | No configuration validation | `src/config.rs` |
| B-H7 | Unwrap in production code paths | scattered |

### Medium (25)

| ID | Finding | Location |
|----|---------|----------|
| Q-M1 | Hardcoded retry parameters | `pix-efi/src/client.rs` |
| Q-M2 | No graceful shutdown | `pix-webhook-server/src/main.rs` |
| Q-M3 | No rate limiting | `pix-webhook-server/src/main.rs` |
| Q-M4 | Token URL duplication | `pix-efi/src/auth.rs` vs `config.rs` |
| Q-M5 | No webhook payload validation | `pix-webhook-server/src/handlers.rs` |
| Q-M6 | Error type conversion loss | `pix-efi/src/error.rs` |
| Q-M7 | No timeout on forwarding HTTP client | `pix-webhook-server/src/handlers.rs` |
| S-M1 | No audit logging | webhook server |
| S-M2 | Token stored as plain String | `pix-efi/src/auth.rs` |
| S-M3 | No CORS restrictions | `pix-webhook-server/src/main.rs` |
| S-M4 | Retry backoff has no cap | `pix-efi/src/client.rs` |
| S-M5 | File output not atomic | `pix-webhook-server/src/handlers.rs` |
| S-M6 | No request ID tracking | webhook server |
| S-M7 | Error messages may leak internals | throughout |
| S-M8 | No connection pooling config | `pix-efi/src/auth.rs` |
| T-M1 | Missing retry error path coverage | `pix-efi/src/client.rs` |
| T-M2 | No property tests for validation | `pix-efi/src/validate.rs` |
| T-M3 | Mock cert generation via CLI openssl | wiremock tests |
| T-M4 | No webhook forwarding tests | webhook server |
| B-M3 | Builder validation incomplete | `pix-brcode/src/lib.rs` |
| B-M5 | Magic numbers throughout | scattered |
| B-M7 | No `#![deny(unsafe_code)]` | crate roots |
| B-M9 | No benchmarks | project |
| B-M10 | No fuzz testing | project |
| B-M11 | Timestamp handling inconsistency | pix-provider types |

### Low (19)

| ID | Finding |
|----|---------|
| Q-L1 | Unused token_type field in CachedToken |
| Q-L2 | Unused scope field in TokenResponse |
| S-L1-L7 | Debug impl, CSP headers, scope checking, cert expiry monitoring, dropped JoinHandle, shallow health check, non-constant-time CRC16 |
| T-L1-L4 | Test helper duplication, magic strings, no doc tests for pix-efi, overlapping test modules |
| B-L1-L8 | No .editorconfig, license headers, pre-commit hooks, incomplete Cargo.toml metadata, no deny(warnings), no cargo-deny, no API versioning, no tracing spans |

---

## Test Results Summary

### Existing Test Coverage

| Crate | Unit Tests | Integration | Property Tests | Estimate |
|-------|-----------|-------------|---------------|----------|
| pix-core | Excellent | N/A | Yes (proptest) | ~95% |
| pix-brcode | Excellent | N/A | Yes (proptest) | ~90% |
| pix-provider | Good | N/A | No | ~85% |
| pix-efi | Good | Yes (wiremock) | No | ~75% |
| pix-mcp | Good | N/A | No | ~80% |
| pix-webhook-server | Good | N/A | No | ~75% |
| pixcli (binary) | Moderate | No | No | ~60% |

### Generated Test Files (8 files, 4,825 lines)

| File | Lines | Tests | Target Crate | Coverage Gap |
|------|-------|-------|-------------|-------------|
| `test_efi_validate_extended.rs` | 444 | ~25 | pix-efi | Boundary values, Unicode, property-based validation |
| `test_brcode_roundtrip.rs` | 519 | 29 | pix-brcode | Encode/decode roundtrip, corrupted payloads, all key types |
| `test_pix_key_edge_cases.rs` | 611 | 55 | pix-core | Check digit edge cases, detection priority, thread safety |
| `test_crc16_comprehensive.rs` | 508 | 34 | pix-core | All single-byte inputs, "6304" in data, null bytes, 1MB payloads |
| `test_webhook_security.rs` | 795 | 14 | pix-webhook-server | Oversized payloads, XSS, injection, concurrent writes, rate limiting |
| `test_efi_resilience.rs` | 694 | 26 | pix-efi | Token expiry boundary, retryable classification, concurrent refresh |
| `test_mcp_integration.rs` | 507 | 22 | pix-mcp | NaN/Infinity, threshold boundary, large days, schema validation |
| `test_config_credentials.rs` | 747 | 25 | pixcli | Missing fields, path expansion, env overrides, serialization |

**Total: 230+ new tests covering previously untested code paths**

### Key Bugs Discovered by Generated Tests

1. **NaN bypass in MCP server** (`test_mcp_integration.rs`): `amount <= 0.0` returns `false` for NaN, allowing NaN amounts to pass validation
2. **No input sanitization on webhook** (`test_webhook_security.rs`): XSS, SQL injection, and path traversal strings flow through to file output
3. **Type mismatch in config** (`test_config_credentials.rs`): `Profile.environment` is String while `EfiEnvironment` is enum, allowing invalid values like "staging"

---

## Top 10 Priority Fixes

| # | Fix | Effort | Impact | Severity |
|---|-----|--------|--------|----------|
| 1 | Add webhook HMAC-SHA256 signature verification | 4h | Prevents fake payment notifications | Critical |
| 2 | Add `DefaultBodyLimit::max(16384)` to webhook | 30min | Prevents DoS via memory exhaustion | Critical |
| 3 | Move credentials to env vars / system keyring | 2h | Prevents credential exposure in config files | Critical |
| 4 | Set up CI/CD (GitHub Actions: clippy + tests + fmt) | 2h | Prevents regressions, enforces quality | Critical |
| 5 | Change default bind to `127.0.0.1` | 5min | Reduces network attack surface | High |
| 6 | Add MCP payment hard limit + confirmation flow | 2h | Prevents accidental fund drain via AI agent | High |
| 7 | Replace `String` amounts with `rust_decimal::Decimal` | 8h | Prevents floating-point errors in financial math | Critical |
| 8 | Fix blocking I/O: use `tokio::fs` in auth + webhook | 1h | Prevents async runtime thread starvation | High |
| 9 | Add TLS support or document reverse proxy requirement | 4h | Encrypts payment data in transit | High |
| 10 | Add NaN/Infinity check in MCP amount validation | 15min | Prevents NaN bypass bug | High |

### Effort Estimates

| Category | Effort |
|----------|--------|
| Critical fixes (items 1-4, 7) | ~16h |
| High fixes (items 5-6, 8-10) | ~7h |
| Total for Top 10 | ~24h (3 days) |
| Full remediation (all findings) | ~2-3 weeks |

---

## Implementation Roadmap

### Phase 1: Security Hardening (Week 1)
- [ ] Webhook authentication (HMAC-SHA256)
- [ ] Body size limits
- [ ] Credential management (env vars)
- [ ] Default bind to 127.0.0.1
- [ ] MCP payment limits
- [ ] NaN/Infinity validation fix

### Phase 2: Reliability (Week 2)
- [ ] `rust_decimal` for monetary amounts
- [ ] Async I/O fixes (tokio::fs)
- [ ] CI/CD setup
- [ ] Input validation/sanitization
- [ ] Error context improvement

### Phase 3: Operational Readiness (Weeks 3-4)
- [ ] TLS support for webhook
- [ ] Structured logging
- [ ] Metrics/observability
- [ ] Integration test suite
- [ ] Graceful shutdown

### Phase 4: Polish (Week 5+)
- [ ] Benchmarks (criterion)
- [ ] Fuzz testing (cargo-fuzz)
- [ ] Documentation improvements
- [ ] Feature flags
- [ ] Supply chain security (cargo-deny)

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────┐
│                   pixcli (binary)                    │
│  ┌───────────┐  ┌───────────┐  ┌────────────────┐  │
│  │  Commands  │  │  Config   │  │ Client Factory │  │
│  └─────┬─────┘  └─────┬─────┘  └───────┬────────┘  │
└────────┼───────────────┼────────────────┼───────────┘
         │               │                │
    ┌────▼─────┐   ┌─────▼─────┐   ┌─────▼──────┐
    │pix-brcode│   │  pix-core │   │pix-provider│
    │ encoder  │   │   CRC16   │   │   trait    │
    │ decoder  │   │  PixKey   │   │   types   │
    │   TLV    │   │  errors   │   │  errors   │
    └──────────┘   └───────────┘   └─────┬──────┘
                                         │
                                    ┌────▼─────┐
                                    │  pix-efi │
                                    │  OAuth2  │
                                    │   mTLS   │
                                    │  retry   │
                                    │ validate │
                                    └──────────┘

    ┌──────────┐   ┌─────────────────┐
    │ pix-mcp  │   │ pix-webhook-    │
    │  server  │   │    server       │
    │ (stdio)  │   │   (HTTP)        │
    └──────────┘   └─────────────────┘
```

---

## Files Produced

### Review Reports (`.full-review/`)
| File | Purpose |
|------|---------|
| `00-scope.md` | Review scope, flags, and state |
| `01-quality-architecture.md` | Code quality findings (17 items) |
| `02-security-performance.md` | Security & performance findings (23 items) |
| `03-testing-documentation.md` | Testing & documentation findings (20 items) |
| `04-best-practices.md` | Best practices findings (28 items) |
| `05-final-report.md` | Consolidated report with roadmap |

### Generated Tests (`tests-generated/`)
| File | Lines | Tests | Target Crate |
|------|-------|-------|-------------|
| `test_efi_validate_extended.rs` | 444 | ~25 | pix-efi |
| `test_brcode_roundtrip.rs` | 519 | 29 | pix-brcode |
| `test_pix_key_edge_cases.rs` | 611 | 55 | pix-core |
| `test_crc16_comprehensive.rs` | 508 | 34 | pix-core |
| `test_webhook_security.rs` | 795 | 14 | pix-webhook-server |
| `test_efi_resilience.rs` | 694 | 26 | pix-efi |
| `test_mcp_integration.rs` | 507 | 22 | pix-mcp |
| `test_config_credentials.rs` | 747 | 25 | pixcli |

### Final Summary (`.code-review-report/`)
| File | Purpose |
|------|---------|
| `SUMMARY.md` | This file — consolidated findings, tests, and roadmap |

---

## How to Use the Generated Tests

Each test file in `tests-generated/` includes a header comment specifying which crate's `tests/` directory it should be placed in. To integrate:

```bash
# Example: integrate CRC16 tests
mkdir -p crates/pix-core/tests
cp tests-generated/test_crc16_comprehensive.rs crates/pix-core/tests/

# Example: integrate webhook security tests
cp tests-generated/test_webhook_security.rs crates/pix-webhook-server/tests/

# Run all tests
cargo test --workspace
```

Some test files (e.g., `test_efi_validate_extended.rs`) test private functions and should be integrated as inline `#[cfg(test)]` modules within the source files they test.

---

*Generated by comprehensive-review + unit-testing analysis pipeline*
*Total findings: 69 (7 Critical, 18 High, 25 Medium, 19 Low)*
*Total generated tests: 230+ across 8 files (4,825 lines)*

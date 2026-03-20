# Consolidated Review Report

## Executive Summary

The pixcli project is a well-structured Rust workspace implementing a CLI tool for Brazilian Pix payments. The codebase demonstrates good Rust practices with clean crate separation, proper trait abstractions, and solid test coverage in core areas. However, it has critical security gaps in the webhook server, operational readiness issues, and areas where financial-grade reliability standards are not met.

## Findings by Severity

### Critical (7)
1. **S-C1** — No webhook authentication (CVSS 9.1)
2. **S-C2** — Credentials in plaintext config (CVSS 7.5)
3. **S-C3** — No request body size limit (CVSS 7.5)
4. **Q-C1** — Config module duplication
5. **Q-C2** — String-typed monetary amounts
6. **B-C1** — No CI/CD configuration
7. **T-C1** — No integration tests for CLI commands

### High (18)
1. **S-H1** — Webhook binds to 0.0.0.0
2. **S-H2** — SSRF via forward URL
3. **S-H3** — No TLS on webhook server
4. **S-H4** — MCP unrestricted payments
5. **S-H5** — Certificate password in memory
6. **Q-H1** — Factory logic duplication
7. **Q-H2** — DynPixProvider incomplete
8. **Q-H3** — Blocking I/O in async context
9. **Q-H4** — Blocking file I/O in webhook
10. **Q-H5** — Duplicate #[cfg(test)]
11. **Q-H6** — No request body size limit
12. **B-H1** — No structured logging
13. **B-H2** — No metrics/observability
14. **B-H3** — Missing error context
15. **B-H4** — No version pinning strategy
16. **B-H5** — No input sanitization
17. **B-H6** — No configuration validation
18. **B-H7** — Unwrap in production paths

### Medium (25)
See individual reports 01-04 for full listing.

### Low (19)
See individual reports 01-04 for full listing.

## Top 10 Priority Fixes

| Priority | Fix | Effort | Impact |
|----------|-----|--------|--------|
| 1 | Add webhook signature verification (HMAC-SHA256) | 4h | Critical — prevents fake payment notifications |
| 2 | Add request body size limit (16KB default) | 30min | Critical — prevents DoS |
| 3 | Move credentials to env vars / keyring | 2h | Critical — prevents credential exposure |
| 4 | Add CI/CD with clippy + tests + fmt | 2h | High — prevents regressions |
| 5 | Change default bind to 127.0.0.1 | 5min | High — reduces attack surface |
| 6 | Add MCP payment hard limit + confirmation | 2h | High — prevents accidental fund drain |
| 7 | Replace String amounts with rust_decimal | 8h | Critical — prevents floating-point errors |
| 8 | Fix blocking I/O in async contexts | 1h | High — prevents runtime thread starvation |
| 9 | Add TLS support to webhook server | 4h | High — encrypts payment data in transit |
| 10 | Add integration tests for CLI commands | 6h | High — ensures end-to-end correctness |

## Test Results Summary
- **Total test files reviewed**: 15+
- **Existing test quality**: Good to Excellent in core crates
- **Property-based testing**: Present in pix-core and pix-brcode
- **Integration testing**: Present with wiremock for pix-efi
- **Gaps**: CLI integration, webhook security, concurrent token refresh, MCP edge cases

## Architecture Diagram

```
┌──────────────────────────────────────────────────┐
│                  pixcli (binary)                  │
│  ┌──────────┐  ┌──────────┐  ┌─────────────┐    │
│  │ Commands │  │  Config  │  │   Factory    │    │
│  └────┬─────┘  └────┬─────┘  └──────┬──────┘    │
└───────┼──────────────┼───────────────┼───────────┘
        │              │               │
   ┌────▼────┐    ┌────▼────┐    ┌────▼──────┐
   │pix-brcode│    │pix-core │    │pix-provider│
   │ encoder  │    │  CRC16  │    │   trait    │
   │ decoder  │    │ PixKey  │    │   types    │
   └──────────┘    └─────────┘    └─────┬─────┘
                                        │
                                   ┌────▼────┐
                                   │ pix-efi │
                                   │  OAuth  │
                                   │  mTLS   │
                                   │  retry  │
                                   └─────────┘
   ┌──────────┐    ┌────────────────┐
   │ pix-mcp  │    │pix-webhook-    │
   │  server  │    │    server      │
   └──────────┘    └────────────────┘
```

## Implementation Roadmap

### Phase 1: Security Hardening (Week 1)
- Webhook authentication
- Body size limits
- Credential management
- Default bind address
- MCP payment limits

### Phase 2: Reliability (Week 2)
- Decimal amounts
- Async I/O fixes
- CI/CD setup
- Input validation
- Error context

### Phase 3: Operational Readiness (Week 3-4)
- TLS support
- Structured logging
- Metrics/observability
- Integration tests
- Graceful shutdown

### Phase 4: Polish (Week 5+)
- Benchmarks and fuzz testing
- Documentation improvements
- Feature flags
- Supply chain security (cargo-deny)

# Testing & Documentation Review

## Summary
The project has excellent test coverage in core areas (CRC16, PixKey validation, BRCode encoding/decoding) with property-based testing. Integration tests use wiremock effectively. Documentation is good at the module level but lacking at the project level.

## Test Coverage Assessment

| Crate | Unit Tests | Integration Tests | Property Tests | Coverage Estimate |
|-------|-----------|------------------|---------------|-------------------|
| pix-core | Excellent | N/A | Yes (proptest) | ~95% |
| pix-brcode | Excellent | N/A | Yes (proptest) | ~90% |
| pix-provider | Good | N/A | No | ~85% |
| pix-efi | Good | Yes (wiremock) | No | ~75% |
| pix-mcp | Good | N/A | No | ~80% |
| pix-webhook-server | Good | N/A | No | ~75% |
| pixcli (binary) | Moderate | No | No | ~60% |

## Findings

### CRITICAL

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| T-C1 | No integration tests for CLI commands | `src/` | The binary crate has no integration tests for actual CLI command execution (create charge, check balance, etc.). |

### HIGH

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| T-H1 | Env var race conditions | `crates/pix-efi/tests/` | Tests that set/read environment variables can race in parallel test execution. Need serial test execution or test isolation. |
| T-H2 | No negative security tests | webhook server tests | No tests for oversized payloads, malicious input, injection attempts. |
| T-H3 | No concurrent access tests | `crates/pix-efi/src/auth.rs` | Token refresh has RwLock double-check pattern but no tests verifying concurrent safety. |

### MEDIUM

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| T-M1 | Missing error path coverage | `crates/pix-efi/src/client.rs` | Retry logic has limited test coverage for timeout, network error, and partial retry scenarios. |
| T-M2 | No property tests for validation | `crates/pix-efi/src/validate.rs` | Validation functions lack property-based testing for fuzzing edge cases. |
| T-M3 | Mock certificate generation | wiremock tests | Tests generate OpenSSL certs via CLI - may fail if openssl not installed. Should use rcgen or similar. |
| T-M4 | No webhook forwarding tests | webhook server | Forward URL functionality untested with a mock HTTP server. |
| T-M5 | Documentation missing on PixProvider | `crates/pix-provider/src/lib.rs` | Trait documentation exists but no usage examples. |
| T-M6 | No README for individual crates | `crates/*/` | Individual crates lack README files for crates.io publishing. |

### LOW

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| T-L1 | Test helper duplication | multiple test files | `test_state()`, `test_app()` helpers duplicated across webhook test modules. |
| T-L2 | Magic strings in tests | throughout | Many tests use hardcoded strings that could be shared constants. |
| T-L3 | No doc tests for pix-efi | `crates/pix-efi/src/` | Public API lacks doc test examples. |
| T-L4 | Some tests overlap | webhook handler tests | Multiple test modules (tests, additional_webhook_tests) test similar scenarios. |

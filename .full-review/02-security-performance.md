# Security & Performance Review

## Summary
The project has several critical security concerns, particularly around the webhook server which lacks authentication, body size limits, and TLS. The EFI client handles mTLS correctly but credentials management needs improvement.

## Findings

### CRITICAL

| # | Finding | Location | Description | CVSS | Recommendation |
|---|---------|----------|-------------|------|----------------|
| S-C1 | No webhook authentication | `crates/pix-webhook-server/src/handlers.rs:44` | Webhook endpoint has no authentication or HMAC signature verification. Anyone can POST fake payment notifications. | 9.1 | Implement Efí webhook signature verification (HMAC-SHA256 with the client secret). |
| S-C2 | Credentials in plaintext config | `src/config.rs` | client_id, client_secret, certificate_password stored as plaintext in TOML config file. | 7.5 | Support environment variables, system keyring, or encrypted config. |
| S-C3 | No request body size limit | `crates/pix-webhook-server/src/main.rs` | No body size limit allows DoS via memory exhaustion. | 7.5 | Add `axum::extract::DefaultBodyLimit` or tower middleware. |

### HIGH

| # | Finding | Location | Description | CVSS | Recommendation |
|---|---------|----------|-------------|------|----------------|
| S-H1 | Webhook binds to 0.0.0.0 | `crates/pix-webhook-server/src/main.rs:26` | Default bind address is 0.0.0.0, exposing webhook to all interfaces. | 6.5 | Default to 127.0.0.1 and require explicit opt-in for external binding. |
| S-H2 | SSRF via forward URL | `crates/pix-webhook-server/src/handlers.rs:84-100` | forward_url is not validated. Attacker with webhook access could trigger internal network requests if URL is configurable at runtime. | 6.1 | Validate forward URL scheme (https only) and block private IP ranges. |
| S-H3 | No TLS on webhook server | `crates/pix-webhook-server/src/main.rs` | Webhook server runs HTTP only. Payment notifications in transit are unencrypted. | 6.0 | Add TLS support or document requirement to run behind a reverse proxy with TLS. |
| S-H4 | MCP server allows unrestricted payments | `crates/pix-mcp/src/server.rs:257-296` | MCP send_payment tool has only a warning threshold, no hard limit or confirmation. An AI agent could drain the account. | 6.0 | Add configurable hard limit and require human-in-the-loop confirmation for high-value payments. |
| S-H5 | Certificate password in memory | `crates/pix-efi/src/auth.rs` | Certificate password remains in memory as String. Should be zeroized after use. | 5.5 | Use secrecy or zeroize crate for sensitive data. |

### MEDIUM

| # | Finding | Location | Description | Recommendation |
|---|---------|----------|-------------|----------------|
| S-M1 | No audit logging | webhook server | No structured audit log for payment events (who sent, from where, when). | Add structured logging with request metadata. |
| S-M2 | Token stored in plain memory | `crates/pix-efi/src/auth.rs:29-33` | OAuth2 access tokens stored as plain String in memory. | Use secrecy::SecretString. |
| S-M3 | No CORS restrictions | `crates/pix-webhook-server/src/main.rs` | tower-http CORS is imported but not configured as restrictive. | Configure strict CORS policy. |
| S-M4 | Retry without backoff cap | `crates/pix-efi/src/client.rs` | Exponential backoff has no maximum delay cap. | Add max_backoff_secs configuration. |
| S-M5 | File output not atomic | `crates/pix-webhook-server/src/handlers.rs:69-80` | JSONL file writes are not atomic - partial writes possible on crash. | Use write-then-rename pattern. |
| S-M6 | No request ID tracking | webhook server | No correlation ID for request tracing. | Add request ID middleware. |
| S-M7 | Error messages may leak internals | throughout | Error messages include internal details that could aid attackers. | Sanitize error messages in API responses. |
| S-M8 | No connection pooling config | `crates/pix-efi/src/auth.rs:71-76` | HTTP client created without explicit connection pool configuration. | Configure pool_max_idle_per_host. |

### LOW

| # | Finding | Location | Description |
|---|---------|----------|-------------|
| S-L1 | Debug impl may be incomplete | `crates/pix-efi/src/auth.rs:209-216` | Debug hides client_id but shows environment. Should also consider hiding base_url_override in prod. |
| S-L2 | No Content-Security-Policy | webhook server | HTTP responses lack security headers. |
| S-L3 | Unused scope in token response | `crates/pix-efi/src/auth.rs:24` | scope field parsed but never checked - could miss permission issues. |
| S-L4 | No certificate expiry monitoring | pix-efi | mTLS certificate expiration not monitored. |
| S-L5 | tokio::spawn error dropped | `crates/pix-webhook-server/src/handlers.rs:88-99` | JoinHandle from forwarding task is dropped, errors are silently lost. |
| S-L6 | No health check depth | webhook server | Health endpoint returns static "OK" without checking dependencies. |
| S-L7 | CRC16 not constant-time | `crates/pix-core/src/crc16.rs` | CRC16 computation is not constant-time, though this is acceptable for non-cryptographic use. |

### Performance Notes
- Token caching with 60s pre-expiry buffer is good
- Retry with exponential backoff is properly implemented
- BrCode encoding is allocation-efficient with String concatenation
- CRC16 is computed inline without tables (acceptable for small payloads)

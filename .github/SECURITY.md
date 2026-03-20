# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly.

**Do not open a public issue.** Instead, email **fobsouza@gmail.com** with:

- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response Times

- **Acknowledgement**: within 48 hours
- **Initial assessment**: within 1 week
- **Fix or mitigation**: depends on severity, typically within 90 days
- **Public disclosure**: coordinated disclosure after 90 days

We will credit reporters in the release notes (unless you prefer to remain anonymous).

## Security Features

Pixcli includes several security measures:

- **File permissions**: Config files are created with `0600` (owner read/write only)
- **mTLS authentication**: PKCS#12 certificates for Efi API communication
- **OAuth2 token caching**: Tokens are cached in memory with automatic refresh
- **Input validation**: Pix keys, BRCode fields, and URLs are validated before use
- **Body size limits**: Webhook server enforces a 256 KB body limit
- **No secrets in logs**: Sensitive values are excluded from tracing output
- **`#[non_exhaustive]` enums**: Forward-compatible API design
- **Credential serialization protection**: `client_secret` is skipped during serialization

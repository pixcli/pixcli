# Review Scope

## Project
- **Name:** pixcli
- **Version:** 0.1.0
- **Language:** Rust (Edition 2021)
- **License:** MIT

## Review Target
Full codebase at `/tmp/pixcli-dev`

## Flags
- `--strict-mode`: Enabled
- `--security-focus`: Enabled
- `--performance-critical`: Enabled
- `--framework`: rust

## Crates Reviewed
| Crate | Path | Purpose |
|-------|------|---------|
| pixcli (binary) | `src/` | CLI application |
| pix-core | `crates/pix-core/` | Core primitives, validation, CRC16 |
| pix-brcode | `crates/pix-brcode/` | EMV BR Code encoder/decoder |
| pix-provider | `crates/pix-provider/` | Provider trait abstraction |
| pix-efi | `crates/pix-efi/` | Efí (Gerencianet) provider |
| pix-mcp | `crates/pix-mcp/` | MCP server for AI agents |
| pix-webhook-server | `crates/pix-webhook-server/` | Webhook listener |

## Review Phases
1. Code Quality & Architecture (Phase 1)
2. Security & Performance (Phase 2)
3. Testing & Documentation (Phase 3)
4. Best Practices & Standards (Phase 4)
5. Consolidated Report (Phase 5)

## Review State
```json
{
  "started_at": "2026-03-20T00:00:00Z",
  "flags": {
    "strict_mode": true,
    "security_focus": true,
    "performance_critical": true,
    "framework": "rust"
  },
  "phases_completed": [],
  "current_phase": 1
}
```

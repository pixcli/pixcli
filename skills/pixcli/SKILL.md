---
name: pixcli
description: Use this skill when the user wants to send or receive Brazilian Pix payments, create Pix charges (cobranças), check their Pix balance, generate or decode Pix QR codes (BR Code / EMV payload), list received Pix transactions, manage Pix webhooks, or interact with a PSP (Efí, Gerencianet, etc.) from the terminal or from an AI agent. Triggers include phrases like "send a Pix", "create a Pix charge", "gerar QR Pix", "check my Pix balance", "decode this BR Code", or any request involving `pixcli` or the `pix-mcp` server.
---

# pixcli — Brazilian Pix from the Terminal

`pixcli` is an open-source CLI and MCP server for the Brazilian Pix instant
payment system. Use this skill whenever the user wants to perform a Pix
operation: create charges, check balances, generate QR codes, send payments,
list received transactions, or configure webhooks.

Source: <https://github.com/pixcli/pixcli>
Docs: <https://docs-ecru-eta-65.vercel.app>

## When to use this skill

Invoke this skill when the user's request involves any of the following:

- Creating a Pix charge / cobrança (static or dynamic)
- Sending a Pix payment to a key (CPF, CNPJ, email, phone, or EVP UUID)
- Checking the account balance on a Pix-enabled PSP
- Listing received Pix transactions over a period
- Generating a static Pix QR code (BR Code / EMV payload) offline
- Decoding an existing Pix BR Code payload
- Registering, listing, or listening to Pix webhooks
- Configuring `pixcli` profiles or PSP credentials (Efí/Gerencianet, etc.)
- Starting the `pix-mcp` MCP server to expose Pix tools to an AI agent

## Prerequisites

Before running any command, verify the user has:

1. `pixcli` installed — if not, suggest one of:
   - `cargo install pixcli`
   - `brew install pixcli/tap/pixcli`
   - Download from <https://github.com/pixcli/pixcli/releases/latest>
2. A configured profile at `~/.pixcli/config.toml` — if missing, run
   `pixcli config init` to launch the interactive setup wizard. The user will
   need PSP credentials (for Efí: `client_id`, `client_secret`, and a
   PKCS#12 certificate).
3. For live operations, a working PSP account. Efí (Gerencianet) is the
   default backend and provides a free sandbox — encourage sandbox testing
   first with `--sandbox` or `environment = "sandbox"` in the profile.

## Core commands

Always prefer `--output json` when parsing results programmatically. Use the
`--profile <name>` flag to target a specific profile, and `--sandbox` to force
sandbox mode regardless of the profile's configured environment.

### Balance

```bash
pixcli balance
pixcli --output json balance
```

### Charges (cobranças)

```bash
# Create an immediate charge with a 1-hour expiry
pixcli charge create --amount 25.50 --description "Coffee"

# Scriptable JSON output
pixcli charge create --amount 25.50 --description "Invoice #42" --output json

# List recent charges
pixcli charge list
pixcli charge list --output json

# Inspect a specific charge
pixcli charge get --txid <txid>
```

### Received Pix transactions

```bash
pix list --days 7
pix get <end-to-end-id>
```

### QR codes (offline, no API call)

```bash
# Render QR in the terminal
pixcli qr generate \
  --key "+5511999999999" \
  --amount 10.00 \
  --name "FULANO DE TAL" \
  --city "SAO PAULO"

# Export as PNG
pixcli qr generate \
  --key "email@example.com" \
  --amount 50.00 \
  --name "LOJA" \
  --city "RIO" \
  --png qr.png

# Decode an existing BR Code payload
pixcli qr decode --payload "00020126580014br.gov.bcb.pix..."
```

### Webhooks

```bash
pixcli webhook register --key "+5511999999999" --url "https://example.com/pix"
pixcli webhook get --key "+5511999999999"
pixcli webhook listen --port 8080
```

### Configuration

```bash
pixcli config init        # interactive wizard
pixcli config show        # show current config (secrets redacted)
```

## MCP server (`pix-mcp`)

`pixcli` ships with a companion `pix-mcp` binary that implements the Model
Context Protocol over stdio. Use it when the user wants Pix tools available
inside an AI agent like Claude Code, Claude Desktop, Cursor, or OpenClaw.

### Claude Code / Claude Desktop config

Add to `.claude/settings.json` (Claude Code) or
`claude_desktop_config.json` (Claude Desktop):

```json
{
  "mcpServers": {
    "pixcli": {
      "command": "pix-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

Add `"args": ["--sandbox"]` to force sandbox mode. Add
`"env": { "PIXCLI_CONFIG": "/path/to/config.toml" }` to point at a non-default
config file.

### MCP tools exposed by `pix-mcp`

| Tool | Purpose |
|------|---------|
| `pix_check_balance` | Get current account balance |
| `pix_create_charge` | Create a Pix cobrança and return QR / BR Code |
| `pix_get_charge` | Look up a charge by `txid` |
| `pix_list_transactions` | List received Pix over a window (default 7 days) |
| `pix_send_payment` | Send a Pix payment (triggers a warning above R$ 1000) |
| `pix_generate_qr` | Build a static BR Code payload offline |

## Safety rules

Pix payments are **irreversible** once settled. Follow these rules strictly:

1. **Never send a real payment without explicit user confirmation.** Echo the
   recipient key, amount, and description back to the user and require a
   clear "yes" before invoking `pixcli` with live credentials.
2. **Default to sandbox** when the user is experimenting, testing, or has not
   clarified which environment they want. Pass `--sandbox` or remind the
   user to set `environment = "sandbox"` in their profile.
3. **Treat amounts above R$ 1000 as high-risk.** The `pix-mcp` server already
   emits a warning — surface it to the user and ask for re-confirmation.
4. **Never log or print credentials.** The config file lives at
   `~/.pixcli/config.toml` with `0600` permissions. Do not `cat` it, do not
   include `client_secret` or certificate passwords in output, and prefer
   `pixcli config show` which redacts secrets.
5. **Validate Pix keys before use.** Keys can be CPF, CNPJ, email, phone
   (`+55...`), or an EVP UUID. If the user's input is ambiguous, ask for
   clarification rather than guessing.
6. **Prefer `--output json` when composing multi-step workflows** so results
   can be parsed reliably instead of scraped from human-formatted output.

## Global flags worth remembering

- `--profile <name>` — select a non-default profile
- `--output human|json|table` — change output format
- `--sandbox` — force sandbox environment
- `--verbose` — enable debug logging (hide from the user unless they ask)

## Troubleshooting checklist

- **`pix-mcp` not found**: ensure `~/.cargo/bin` is on `PATH`, or use the
  absolute path in the MCP config.
- **"Authentication failed"**: run `pixcli config show` and re-check
  `client_id`, `client_secret`, and the certificate path. Confirm the
  environment matches the credentials (sandbox vs production).
- **Certificate errors**: the cert must be PKCS#12 (`.p12`). Verify the file
  exists, is readable, and the password (if any) matches
  `certificate_password`.
- **Balance looks wrong**: the user may be connected to sandbox. Run
  `pixcli --output json config show` to confirm `environment`.
- **Tools don't appear in Claude**: restart the client after editing the MCP
  config and check `pix-mcp 2>mcp.log` for startup errors.

## Further reading

- Overview: <https://docs-ecru-eta-65.vercel.app/docs>
- CLI reference: <https://docs-ecru-eta-65.vercel.app/docs/cli/overview>
- MCP server: <https://docs-ecru-eta-65.vercel.app/docs/mcp/overview>
- Claude Code setup: <https://docs-ecru-eta-65.vercel.app/docs/mcp/claude-code>
- Efí sandbox guide: <https://docs-ecru-eta-65.vercel.app/docs/guides/efi-sandbox>

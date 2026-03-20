# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-03-20

### Added

- **Core library (`pix-core`)**: Pix key validation (CPF, CNPJ, phone, email, EVP), CRC16-CCITT checksum, and core payment types.
- **BR Code (`pix-brcode`)**: EMV/BR Code encoder and decoder for static and dynamic Pix QR codes.
- **Provider trait (`pix-provider`)**: Abstract `PixProvider` trait for multi-PSP support.
- **Efí backend (`pix-efi`)**: Full Efí (Gerencianet) integration with OAuth2 + mTLS authentication.
- **CLI (`pixcli`)**: Command-line interface with subcommands:
  - `balance` — check account balance
  - `charge create/get/list` — manage Pix charges
  - `pix list/get` — view received transactions
  - `qr generate/decode` — QR code generation and decoding
  - `webhook register/get/remove/listen` — webhook management
  - `config init/show` — configuration wizard and viewer
- **MCP server (`pix-mcp`)**: Model Context Protocol server exposing Pix tools for AI agents (Claude Code, OpenClaw, etc.).
- **Webhook server (`pix-webhook-server`)**: Standalone HTTP server for receiving Pix payment notifications.
- **Output formats**: Human-readable (with colours and emojis), JSON, and table output modes.
- **Sandbox mode**: `--sandbox` flag for safe testing without real money.
- **CI/CD**: GitHub Actions for testing, linting, and cross-platform release builds.
- **Documentation**: README, CONTRIBUTING guide, and this changelog.

[Unreleased]: https://github.com/pixcli/pixcli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/pixcli/pixcli/releases/tag/v0.1.0

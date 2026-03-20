//! MCP server binary for Pix payments.
//!
//! Starts an MCP server on stdio transport that exposes Pix payment
//! operations as tools for AI agents (Claude Code, OpenClaw, etc.).
//!
//! ## Usage
//!
//! ```bash
//! pix-mcp  # starts the MCP server on stdio
//! ```
//!
//! ## MCP Client Configuration
//!
//! For Claude Code, add to your MCP config:
//! ```json
//! {"mcpServers": {"pix": {"command": "pix-mcp", "args": []}}}
//! ```

use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

mod config;
mod server;
mod tools;

use config::PixConfig;
use pix_efi::config::{EfiConfig, EfiEnvironment};
use pix_efi::EfiClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stderr so it doesn't interfere with MCP stdio transport.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(
            std::env::var("PIX_MCP_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting pix-mcp server v{}", env!("CARGO_PKG_VERSION"));

    let config = config::load_mcp_config().map_err(|e| {
        tracing::error!("Failed to load config: {e}");
        e
    })?;

    let profile = config.get_profile(None).map_err(|e| {
        tracing::error!("Failed to get profile: {e}");
        e
    })?;

    let (efi_config, default_pix_key) = build_efi_config(profile)?;
    let client = EfiClient::new(efi_config).map_err(|e| {
        tracing::error!("Failed to create Efí client: {e}");
        anyhow::anyhow!("Efí client init error: {e}")
    })?;

    let mcp_server = server::PixMcpServer::new(client, default_pix_key);

    tracing::info!("MCP server ready, waiting for client on stdio");

    let transport = rmcp::transport::io::stdio();
    let service = mcp_server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}

/// Builds an `EfiConfig` from a config profile.
fn build_efi_config(profile: &config::Profile) -> anyhow::Result<(EfiConfig, Option<String>)> {
    match profile.backend.as_str() {
        "efi" => {
            let environment = if profile.environment == "production" {
                EfiEnvironment::Production
            } else {
                EfiEnvironment::Sandbox
            };

            let cert_path = PixConfig::expand_path(&profile.certificate);

            let efi_config = EfiConfig {
                client_id: profile.client_id.clone(),
                client_secret: profile.client_secret.clone(),
                certificate_path: cert_path,
                certificate_password: profile.certificate_password.clone(),
                environment,
            };

            Ok((efi_config, profile.default_pix_key.clone()))
        }
        other => anyhow::bail!("unknown backend: '{}'. Supported: efi", other),
    }
}

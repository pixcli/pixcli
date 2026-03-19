//! Webhook management CLI commands.
//!
//! Register, query, and remove Efí Pix webhooks, and start a local
//! webhook listener server.

use anyhow::Result;
use clap::Subcommand;

use crate::output::OutputFormat;

/// Webhook subcommands.
#[derive(Subcommand)]
pub enum WebhookCommand {
    /// Register a webhook URL for a Pix key
    Register {
        /// Pix key to associate with the webhook
        #[arg(long)]
        key: String,
        /// Webhook URL (Efí will append /pix to this)
        #[arg(long)]
        url: String,
    },
    /// Get the registered webhook for a Pix key
    Get {
        /// Pix key to look up
        #[arg(long)]
        key: String,
    },
    /// Remove the webhook for a Pix key
    Remove {
        /// Pix key whose webhook should be removed
        #[arg(long)]
        key: String,
    },
    /// Start a local webhook listener server
    Listen {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,
        /// Forward received events to this URL via POST
        #[arg(long)]
        forward: Option<String>,
        /// Append events to this file (JSONL format)
        #[arg(long)]
        output: Option<String>,
    },
}

/// Runs the webhook subcommand.
pub async fn run(
    cmd: WebhookCommand,
    profile: Option<&str>,
    sandbox: bool,
    format: OutputFormat,
) -> Result<()> {
    match cmd {
        WebhookCommand::Register { key, url } => {
            register(profile, sandbox, &key, &url, format).await
        }
        WebhookCommand::Get { key } => get(profile, sandbox, &key, format).await,
        WebhookCommand::Remove { key } => remove(profile, sandbox, &key, format).await,
        WebhookCommand::Listen {
            port,
            forward,
            output,
        } => listen(port, forward, output, format).await,
    }
}

/// Registers a webhook URL with Efí for the given Pix key.
async fn register(
    profile: Option<&str>,
    sandbox: bool,
    key: &str,
    url: &str,
    format: OutputFormat,
) -> Result<()> {
    let config = crate::config::PixConfig::load(None)?;
    let client = crate::client_factory::build_efi_client(&config, profile, sandbox)?;

    client.register_webhook(key, url).await?;

    match format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "status": "registered",
                "key": key,
                "webhook_url": url,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            println!("✅ Webhook registered for key: {key}");
            println!("   URL: {url}");
            println!("   ℹ️  Efí will POST events to: {url}/pix");
        }
    }

    Ok(())
}

/// Gets the webhook info registered for a Pix key.
async fn get(profile: Option<&str>, sandbox: bool, key: &str, format: OutputFormat) -> Result<()> {
    let config = crate::config::PixConfig::load(None)?;
    let client = crate::client_factory::build_efi_client(&config, profile, sandbox)?;

    let info = client.get_webhook(key).await?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        _ => {
            println!("🔔 Webhook for key: {key}");
            println!("   URL:     {}", info.webhook_url);
            if let Some(ref created) = info.created_at {
                println!("   Created: {created}");
            }
        }
    }

    Ok(())
}

/// Removes the webhook for the given Pix key.
async fn remove(
    profile: Option<&str>,
    sandbox: bool,
    key: &str,
    format: OutputFormat,
) -> Result<()> {
    let config = crate::config::PixConfig::load(None)?;
    let client = crate::client_factory::build_efi_client(&config, profile, sandbox)?;

    client.remove_webhook(key).await?;

    match format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "status": "removed",
                "key": key,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            println!("🗑️  Webhook removed for key: {key}");
        }
    }

    Ok(())
}

/// Starts a local webhook listener server.
async fn listen(
    port: u16,
    forward_url: Option<String>,
    output_file: Option<String>,
    _format: OutputFormat,
) -> Result<()> {
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;

    /// Shared state for the webhook listener.
    struct ListenState {
        forward_url: Option<String>,
        output_file: Option<String>,
        http_client: reqwest::Client,
    }

    /// Pix webhook event payload.
    #[derive(Debug, Deserialize, Serialize)]
    struct WebhookPayload {
        pix: Vec<PixEvent>,
    }

    /// A single Pix event from a webhook notification.
    #[derive(Debug, Deserialize, Serialize)]
    struct PixEvent {
        #[serde(rename = "endToEndId")]
        end_to_end_id: String,
        txid: Option<String>,
        valor: String,
        horario: String,
        #[serde(rename = "infoPagador")]
        info_pagador: Option<String>,
        chave: Option<String>,
        devolucoes: Option<Vec<serde_json::Value>>,
    }

    /// Handles incoming webhook POST requests.
    async fn handle_webhook(
        State(state): State<Arc<ListenState>>,
        Json(payload): Json<WebhookPayload>,
    ) -> StatusCode {
        tracing::info!("Received webhook with {} event(s)", payload.pix.len());

        for event in &payload.pix {
            let event_json = match serde_json::to_string(event) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!("Failed to serialize event: {e}");
                    continue;
                }
            };

            // Print to stdout
            if let Ok(pretty) = serde_json::to_string_pretty(event) {
                println!("{pretty}");
            }

            // Append to file
            if let Some(ref path) = state.output_file {
                use std::io::Write;
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    Ok(mut file) => {
                        if let Err(e) = writeln!(file, "{event_json}") {
                            tracing::error!("Failed to write to {path}: {e}");
                        }
                    }
                    Err(e) => tracing::error!("Failed to open {path}: {e}"),
                }
            }

            // Forward to URL
            if let Some(ref url) = state.forward_url {
                let client = state.http_client.clone();
                let url = url.clone();
                let body = event_json.clone();
                tokio::spawn(async move {
                    match client
                        .post(&url)
                        .body(body)
                        .header("Content-Type", "application/json")
                        .send()
                        .await
                    {
                        Ok(resp) => tracing::info!("Forwarded to {url}: {}", resp.status()),
                        Err(e) => tracing::warn!("Failed to forward to {url}: {e}"),
                    }
                });
            }
        }

        StatusCode::OK
    }

    let state = Arc::new(ListenState {
        forward_url,
        output_file,
        http_client: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/pix", post(handle_webhook))
        .route("/health", get(|| async { "OK" }))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    println!("🔔 Webhook listener starting on {addr}");
    println!("   Endpoint: POST http://{addr}/pix");
    println!("   Health:   GET  http://{addr}/health");
    println!("   Press Ctrl+C to stop");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_config(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, content).unwrap();
        std::env::set_var("PIXCLI_CONFIG", path.to_str().unwrap());
        dir
    }

    fn cleanup() {
        std::env::remove_var("PIXCLI_CONFIG");
    }

    const TEST_CONFIG: &str = r#"
[defaults]
profile = "test"

[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/nonexistent/cert.p12"
default_pix_key = "+5511999999999"
"#;

    #[tokio::test]
    async fn test_webhook_register_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let result = register(None, false, "+5511999999999", "https://example.com/webhook", OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_get_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let result = get(None, false, "+5511999999999", OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_remove_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let result = remove(None, false, "+5511999999999", OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_register_no_profiles() {
        let _dir = setup_config("");
        let result = register(None, false, "key", "https://example.com", OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_get_no_profiles() {
        let _dir = setup_config("");
        let result = get(None, false, "key", OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_remove_no_profiles() {
        let _dir = setup_config("");
        let result = remove(None, false, "key", OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_run_register() {
        let _dir = setup_config(TEST_CONFIG);
        let cmd = WebhookCommand::Register {
            key: "+5511999999999".to_string(),
            url: "https://example.com/webhook".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err()); // cert missing
    }

    #[tokio::test]
    async fn test_webhook_run_get() {
        let _dir = setup_config(TEST_CONFIG);
        let cmd = WebhookCommand::Get {
            key: "+5511999999999".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_run_remove() {
        let _dir = setup_config(TEST_CONFIG);
        let cmd = WebhookCommand::Remove {
            key: "+5511999999999".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_register_sandbox_flag() {
        let _dir = setup_config(TEST_CONFIG);
        let result = register(None, true, "key", "https://example.com", OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_get_with_profile() {
        let _dir = setup_config(TEST_CONFIG);
        let result = get(Some("test"), false, "key", OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_remove_with_profile() {
        let _dir = setup_config(TEST_CONFIG);
        let result = remove(Some("test"), false, "key", OutputFormat::Table).await;
        cleanup();
        assert!(result.is_err());
    }
}

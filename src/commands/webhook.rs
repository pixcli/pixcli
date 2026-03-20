//! Webhook management CLI commands.
//!
//! Register, query, and remove Efí Pix webhooks, and start a local
//! webhook listener server.

use std::path::Path;

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
    config_path: Option<&Path>,
) -> Result<()> {
    match cmd {
        WebhookCommand::Register { key, url } => {
            register(profile, sandbox, &key, &url, format, config_path).await
        }
        WebhookCommand::Get { key } => get(profile, sandbox, &key, format, config_path).await,
        WebhookCommand::Remove { key } => remove(profile, sandbox, &key, format, config_path).await,
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
    config_path: Option<&Path>,
) -> Result<()> {
    let config = crate::config::PixConfig::load(config_path)?;
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
async fn get(
    profile: Option<&str>,
    sandbox: bool,
    key: &str,
    format: OutputFormat,
    config_path: Option<&Path>,
) -> Result<()> {
    let config = crate::config::PixConfig::load(config_path)?;
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
    config_path: Option<&Path>,
) -> Result<()> {
    let config = crate::config::PixConfig::load(config_path)?;
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
///
/// Reuses the handler from `pix-webhook-server` to avoid logic duplication.
async fn listen(
    port: u16,
    forward_url: Option<String>,
    output_file: Option<String>,
    _format: OutputFormat,
) -> Result<()> {
    use axum::routing::{get, post};
    use axum::Router;
    use pix_webhook_server::AppState;
    use std::sync::Arc;

    let state = Arc::new(AppState {
        forward_url,
        output_file,
        quiet: false,
        http_client: reqwest::Client::new(),
        api_key: None,
        hmac_secret: None,
    });

    let app = Router::new()
        .route("/pix", post(pix_webhook_server::handlers::handle_webhook))
        .route("/health", get(|| async { "OK" }))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    println!("Webhook listener starting on {addr}");
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
    use std::path::PathBuf;

    fn setup_config(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, content).unwrap();
        (dir, path)
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
        let (_dir, path) = setup_config(TEST_CONFIG);
        let result = register(
            None,
            false,
            "+5511999999999",
            "https://example.com/webhook",
            OutputFormat::Json,
            Some(&path),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_get_fails_missing_cert() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let result = get(
            None,
            false,
            "+5511999999999",
            OutputFormat::Human,
            Some(&path),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_remove_fails_missing_cert() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let result = remove(
            None,
            false,
            "+5511999999999",
            OutputFormat::Json,
            Some(&path),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_register_no_profiles() {
        let (_dir, path) = setup_config("");
        let result = register(
            None,
            false,
            "key",
            "https://example.com",
            OutputFormat::Human,
            Some(&path),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_get_no_profiles() {
        let (_dir, path) = setup_config("");
        let result = get(None, false, "key", OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_remove_no_profiles() {
        let (_dir, path) = setup_config("");
        let result = remove(None, false, "key", OutputFormat::Human, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_run_register() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = WebhookCommand::Register {
            key: "+5511999999999".to_string(),
            url: "https://example.com/webhook".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err()); // cert missing
    }

    #[tokio::test]
    async fn test_webhook_run_get() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = WebhookCommand::Get {
            key: "+5511999999999".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Human, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_run_remove() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = WebhookCommand::Remove {
            key: "+5511999999999".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_register_sandbox_flag() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let result = register(
            None,
            true,
            "key",
            "https://example.com",
            OutputFormat::Human,
            Some(&path),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_get_with_profile() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let result = get(Some("test"), false, "key", OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_webhook_remove_with_profile() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let result = remove(Some("test"), false, "key", OutputFormat::Table, Some(&path)).await;
        assert!(result.is_err());
    }
}

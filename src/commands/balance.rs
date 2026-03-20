//! `pixcli balance` — show account balance.

use std::path::Path;

use anyhow::Result;
use pix_provider::PixProvider;

use crate::config::PixConfig;
use crate::output::{self, OutputFormat};

/// Runs the balance command.
pub async fn run(
    profile: Option<&str>,
    sandbox: bool,
    format: OutputFormat,
    config_path: Option<&Path>,
) -> Result<()> {
    let config = PixConfig::load(config_path)?;
    let client = crate::client_factory::build_provider(&config, profile, sandbox)?;
    let balance = client.get_balance().await?;
    output::print_balance(&balance, format)?;
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

    #[tokio::test]
    async fn test_balance_fails_no_config() {
        let (_dir, path) = setup_config("");
        let result = run(None, false, OutputFormat::Human, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_balance_fails_missing_cert() {
        let (_dir, path) = setup_config(
            r#"
[defaults]
profile = "test"

[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/nonexistent/cert.p12"
"#,
        );
        let result = run(None, false, OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_balance_fails_unknown_backend() {
        let (_dir, path) = setup_config(
            r#"
[defaults]
profile = "test"

[profiles.test]
backend = "unknown"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/cert.p12"
"#,
        );
        let result = run(None, false, OutputFormat::Human, Some(&path)).await;
        assert!(result.is_err());
    }
}

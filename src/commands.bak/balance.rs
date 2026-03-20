//! `pixcli balance` — show account balance.

use anyhow::Result;
use pix_provider::PixProvider;

use crate::config::PixConfig;
use crate::output::{self, OutputFormat};

/// Runs the balance command.
pub async fn run(profile: Option<&str>, sandbox: bool, format: OutputFormat) -> Result<()> {
    let config = PixConfig::load(None)?;
    let client = crate::client_factory::build_provider(&config, profile, sandbox)?;
    let balance = client.get_balance().await?;
    output::print_balance(&balance, format)?;
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

    #[tokio::test]
    async fn test_balance_fails_no_config() {
        let _dir = setup_config("");
        // Empty config → no profiles → build_provider fails
        let result = run(None, false, OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_balance_fails_missing_cert() {
        let _dir = setup_config(
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
        // Config loads but cert doesn't exist → EfiClient::new fails
        let result = run(None, false, OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_balance_fails_unknown_backend() {
        let _dir = setup_config(
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
        let result = run(None, false, OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }
}

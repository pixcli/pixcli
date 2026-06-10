//! `pix list` / `pix get` — list and inspect received Pix transactions.

use anyhow::Result;
use chrono::{Duration, Utc};
use pix_provider::{PixProvider, TransactionFilter};

use crate::config::PixConfig;
use crate::output::{self, OutputFormat};

/// Builds a configured provider for the active profile.
fn provider(profile: Option<&str>, sandbox: bool) -> Result<impl PixProvider> {
    let config = PixConfig::load(None)?;
    crate::client_factory::build_provider(&config, profile, sandbox)
}

/// Lists received Pix transactions over a date window.
pub async fn list(
    days: u32,
    from: Option<String>,
    to: Option<String>,
    profile: Option<&str>,
    sandbox: bool,
    format: OutputFormat,
) -> Result<()> {
    let client = provider(profile, sandbox)?;

    let end = if let Some(ref to_str) = to {
        chrono::DateTime::parse_from_rfc3339(to_str)?.with_timezone(&Utc)
    } else {
        Utc::now()
    };
    let start = if let Some(ref from_str) = from {
        chrono::DateTime::parse_from_rfc3339(from_str)?.with_timezone(&Utc)
    } else {
        end - Duration::days(days as i64)
    };

    let filter = TransactionFilter {
        start: Some(start),
        end: Some(end),
        page: None,
        per_page: None,
    };

    let txs = client.list_received_pix(filter).await?;
    output::print_pix_transactions(&txs, format)?;

    Ok(())
}

/// Gets a single received Pix transaction by its end-to-end ID.
pub async fn get(
    e2eid: String,
    profile: Option<&str>,
    sandbox: bool,
    format: OutputFormat,
) -> Result<()> {
    let client = provider(profile, sandbox)?;

    let tx = client.get_pix(&e2eid).await?;
    output::print_pix_transaction(&tx, format)?;

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
"#;

    #[tokio::test]
    async fn test_pix_list_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let result = list(7, None, None, None, false, OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pix_list_with_dates_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let result = list(
            7,
            Some("2026-01-01T00:00:00Z".to_string()),
            Some("2026-01-31T23:59:59Z".to_string()),
            None,
            false,
            OutputFormat::Json,
        )
        .await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pix_get_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let result = get("E12345".to_string(), None, false, OutputFormat::Table).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pix_list_no_profiles() {
        let _dir = setup_config("");
        let result = list(7, None, None, None, false, OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }
}

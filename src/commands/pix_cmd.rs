//! `pixcli pix` — list and get received Pix transactions.

use anyhow::Result;
use chrono::{Duration, Utc};
use pix_provider::{PixProvider, TransactionFilter};

use crate::config::PixConfig;
use crate::output::{self, OutputFormat};

/// Sub-commands for Pix transactions.
#[derive(clap::Subcommand)]
pub enum PixCommand {
    /// List received Pix transactions.
    List {
        /// Number of days to look back (default: 7).
        #[arg(long, default_value = "7")]
        days: u32,
        /// Start date (ISO 8601, overrides --days).
        #[arg(long)]
        from: Option<String>,
        /// End date (ISO 8601).
        #[arg(long)]
        to: Option<String>,
    },
    /// Get a specific Pix transaction by its end-to-end ID.
    Get {
        /// End-to-end ID.
        e2eid: String,
    },
}

/// Runs a pix sub-command.
pub async fn run(
    cmd: PixCommand,
    profile: Option<&str>,
    sandbox: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = PixConfig::load(None)?;
    let client = crate::client_factory::build_provider(&config, profile, sandbox)?;

    match cmd {
        PixCommand::List { days, from, to } => {
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
        }
        PixCommand::Get { e2eid } => {
            let tx = client.get_pix(&e2eid).await?;
            output::print_pix_transaction(&tx, format)?;
        }
    }

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
        let cmd = PixCommand::List {
            days: 7,
            from: None,
            to: None,
        };
        let result = run(cmd, None, false, OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pix_list_with_dates_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let cmd = PixCommand::List {
            days: 7,
            from: Some("2026-01-01T00:00:00Z".to_string()),
            to: Some("2026-01-31T23:59:59Z".to_string()),
        };
        let result = run(cmd, None, false, OutputFormat::Json).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pix_get_fails_missing_cert() {
        let _dir = setup_config(TEST_CONFIG);
        let cmd = PixCommand::Get {
            e2eid: "E12345".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Table).await;
        cleanup();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pix_list_no_profiles() {
        let _dir = setup_config("");
        let cmd = PixCommand::List {
            days: 7,
            from: None,
            to: None,
        };
        let result = run(cmd, None, false, OutputFormat::Human).await;
        cleanup();
        assert!(result.is_err());
    }
}

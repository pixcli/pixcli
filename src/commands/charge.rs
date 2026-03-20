//! `pixcli charge` — create, get, and list Pix charges.

use std::path::Path;

use anyhow::Result;
use chrono::{Duration, Utc};
use pix_provider::{ChargeRequest, Debtor, PixProvider, TransactionFilter};

use crate::config::PixConfig;
use crate::output::{self, OutputFormat};

/// Sub-commands for charge management.
#[derive(clap::Subcommand)]
pub enum ChargeCommand {
    /// Create a new immediate Pix charge.
    Create {
        /// Amount in BRL (e.g. 10.50).
        #[arg(long)]
        amount: String,
        /// Pix key to receive payment (uses profile default if omitted).
        #[arg(long)]
        key: Option<String>,
        /// Description / payment request message.
        #[arg(long)]
        description: Option<String>,
        /// Expiry in seconds (default: 3600).
        #[arg(long, default_value = "3600")]
        expiry: u32,
        /// Debtor CPF or CNPJ.
        #[arg(long)]
        debtor_doc: Option<String>,
        /// Debtor name.
        #[arg(long)]
        debtor_name: Option<String>,
        /// Custom txid (auto-generated if omitted).
        #[arg(long)]
        txid: Option<String>,
    },
    /// Get details of a charge by its transaction ID.
    Get {
        /// Transaction ID.
        txid: String,
    },
    /// List recent charges.
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
}

/// Runs a charge sub-command.
pub async fn run(
    cmd: ChargeCommand,
    profile: Option<&str>,
    sandbox: bool,
    format: OutputFormat,
    config_path: Option<&Path>,
) -> Result<()> {
    let config = PixConfig::load(config_path)?;
    let client = crate::client_factory::build_provider(&config, profile, sandbox)?;

    match cmd {
        ChargeCommand::Create {
            amount,
            key,
            description,
            expiry,
            debtor_doc,
            debtor_name,
            txid,
        } => {
            let pix_key = key
                .or_else(|| {
                    config
                        .get_profile(profile)
                        .ok()
                        .and_then(|p| p.default_pix_key.clone())
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "no Pix key specified. Use --key or set default_pix_key in your profile."
                    )
                })?;

            let debtor = match (debtor_name, debtor_doc) {
                (Some(name), Some(doc)) => Some(Debtor {
                    name,
                    document: doc,
                }),
                _ => None,
            };

            let pix_key_display = pix_key.clone();
            let amount_display = amount.clone();

            let request = ChargeRequest {
                pix_key,
                description,
                amount,
                expiration_secs: expiry,
                debtor,
                txid,
            };

            let response = client.create_charge(request).await?;
            // Convert to PixCharge for display, carrying forward the original inputs.
            let charge = pix_provider::PixCharge {
                txid: response.txid,
                status: response.status,
                amount: amount_display,
                pix_key: pix_key_display,
                description: None,
                brcode: Some(response.brcode),
                debtor: None,
                created_at: response.created_at,
                expires_at: response.expires_at,
                e2eids: vec![],
            };
            output::print_charge(&charge, format)?;
        }
        ChargeCommand::Get { txid } => {
            let charge = client.get_charge(&txid).await?;
            output::print_charge(&charge, format)?;
        }
        ChargeCommand::List { days, from, to } => {
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

            let charges = client.list_charges(filter).await?;
            output::print_charges(&charges, format)?;
        }
    }

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
    async fn test_charge_create_fails_missing_cert() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = ChargeCommand::Create {
            amount: "10.50".to_string(),
            key: Some("user@example.com".to_string()),
            description: Some("Test".to_string()),
            expiry: 3600,
            debtor_doc: None,
            debtor_name: None,
            txid: None,
        };
        let result = run(cmd, None, false, OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_charge_create_with_debtor_fails_missing_cert() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = ChargeCommand::Create {
            amount: "50.00".to_string(),
            key: None,
            description: None,
            expiry: 1800,
            debtor_doc: Some("52998224725".to_string()),
            debtor_name: Some("João Silva".to_string()),
            txid: Some("custom_txid_12345678901234567".to_string()),
        };
        let result = run(cmd, None, false, OutputFormat::Human, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_charge_get_fails_missing_cert() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = ChargeCommand::Get {
            txid: "txid12345".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_charge_list_fails_missing_cert() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = ChargeCommand::List {
            days: 7,
            from: None,
            to: None,
        };
        let result = run(cmd, None, false, OutputFormat::Table, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_charge_list_with_dates_fails_missing_cert() {
        let (_dir, path) = setup_config(TEST_CONFIG);
        let cmd = ChargeCommand::List {
            days: 7,
            from: Some("2026-01-01T00:00:00Z".to_string()),
            to: Some("2026-01-31T23:59:59Z".to_string()),
        };
        let result = run(cmd, None, false, OutputFormat::Json, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_charge_no_profiles() {
        let (_dir, path) = setup_config("");
        let cmd = ChargeCommand::Get {
            txid: "test".to_string(),
        };
        let result = run(cmd, None, false, OutputFormat::Human, Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_charge_create_no_key_no_default() {
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
        let cmd = ChargeCommand::Create {
            amount: "10.00".to_string(),
            key: None,
            description: None,
            expiry: 3600,
            debtor_doc: None,
            debtor_name: None,
            txid: None,
        };
        let result = run(cmd, None, false, OutputFormat::Human, Some(&path)).await;
        assert!(result.is_err());
    }
}

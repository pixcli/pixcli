//! `pixcli charge` — create, get, and list Pix charges.

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
) -> Result<()> {
    let config = PixConfig::load(None)?;
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

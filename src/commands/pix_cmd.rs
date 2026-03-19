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

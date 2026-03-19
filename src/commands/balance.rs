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

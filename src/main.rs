//! Pixcli — a CLI tool for Brazilian Pix payments.
//!
//! Supports creating charges, listing transactions, sending payments,
//! and managing provider credentials.

use clap::Parser;
use tracing_subscriber::EnvFilter;

mod client_factory;
mod commands;
mod config;
mod output;

/// CLI tool for programmatic Pix payments.
#[derive(Parser)]
#[command(
    name = "pixcli",
    version,
    about = "CLI tool for Brazilian Pix payments"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// PSP profile to use.
    #[arg(short, long, global = true)]
    profile: Option<String>,

    /// Output format.
    #[arg(short, long, global = true, default_value = "human")]
    output: output::OutputFormat,

    /// Force sandbox environment.
    #[arg(long, global = true)]
    sandbox: bool,

    /// Enable verbose (debug) logging.
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Suppress non-essential output.
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Show account balance.
    Balance,
    /// Create and manage Pix charges.
    Charge {
        #[command(subcommand)]
        action: commands::charge::ChargeCommand,
    },
    /// List and get received Pix transactions.
    Pix {
        #[command(subcommand)]
        action: commands::pix_cmd::PixCommand,
    },
    /// Configuration management.
    Config {
        #[command(subcommand)]
        action: commands::config_cmd::ConfigCommand,
    },
    /// Generate and decode Pix QR codes.
    Qr {
        #[command(subcommand)]
        action: commands::qr::QrCommand,
    },
    /// Manage Pix webhooks.
    Webhook {
        #[command(subcommand)]
        action: commands::webhook::WebhookCommand,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Set up logging.
    let filter = if cli.verbose {
        "debug"
    } else if cli.quiet {
        "error"
    } else {
        "warn"
    };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .init();

    match cli.command {
        Commands::Balance => {
            commands::balance::run(cli.profile.as_deref(), cli.sandbox, cli.output).await
        }
        Commands::Charge { action } => {
            commands::charge::run(action, cli.profile.as_deref(), cli.sandbox, cli.output).await
        }
        Commands::Pix { action } => {
            commands::pix_cmd::run(action, cli.profile.as_deref(), cli.sandbox, cli.output).await
        }
        Commands::Config { action } => commands::config_cmd::run(action, cli.output),
        Commands::Qr { action } => commands::qr::run(action, cli.output),
        Commands::Webhook { action } => {
            commands::webhook::run(action, cli.profile.as_deref(), cli.sandbox, cli.output).await
        }
    }
}

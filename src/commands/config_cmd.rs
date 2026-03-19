//! `pixcli config` — configuration management commands.

use anyhow::Result;
use colored::Colorize;

use crate::config::PixConfig;
use crate::output::OutputFormat;

/// Sub-commands for config management.
#[derive(clap::Subcommand)]
pub enum ConfigCommand {
    /// Run the interactive setup wizard.
    Init,
    /// Show the current configuration (secrets are redacted).
    Show,
}

/// Runs a config sub-command.
pub fn run(cmd: ConfigCommand, format: OutputFormat) -> Result<()> {
    match cmd {
        ConfigCommand::Init => {
            crate::config::run_setup_wizard()?;
            Ok(())
        }
        ConfigCommand::Show => show(format),
    }
}

/// Displays the current configuration, redacting secrets.
fn show(format: OutputFormat) -> Result<()> {
    let config = PixConfig::load(None)?;

    match format {
        OutputFormat::Json => {
            // Build a redacted version for JSON output.
            let mut redacted = config.clone();
            for profile in redacted.profiles.values_mut() {
                profile.client_secret = "***".to_string();
                if !profile.certificate_password.is_empty() {
                    profile.certificate_password = "***".to_string();
                }
            }
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        }
        OutputFormat::Human | OutputFormat::Table => {
            println!("{}", "⚙️  Pixcli Configuration".bold());
            println!(
                "   Path:    {}",
                PixConfig::default_path().display().to_string().dimmed()
            );
            println!("   Default: {}", config.defaults.profile.cyan());
            println!("   Output:  {}", config.defaults.output);
            println!();

            if config.profiles.is_empty() {
                println!(
                    "   {}",
                    "No profiles configured. Run `pixcli config init`.".yellow()
                );
            } else {
                for (name, profile) in &config.profiles {
                    let is_default = name == &config.defaults.profile;
                    let marker = if is_default { " (default)" } else { "" };
                    println!("   📄 Profile: {}{}", name.bold(), marker.dimmed());
                    println!("      Backend:     {}", profile.backend);
                    println!("      Environment: {}", profile.environment);
                    println!("      Client ID:   {}", profile.client_id);
                    println!("      Secret:      ***");
                    println!("      Certificate: {}", profile.certificate);
                    if let Some(ref key) = profile.default_pix_key {
                        println!("      Pix Key:     {}", key);
                    }
                    println!();
                }
            }
        }
    }

    Ok(())
}

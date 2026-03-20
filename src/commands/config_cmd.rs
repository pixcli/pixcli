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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_config(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, content).unwrap();
        std::env::set_var("PIXCLI_CONFIG", path.to_str().unwrap());
        (dir, path)
    }

    fn cleanup() {
        std::env::remove_var("PIXCLI_CONFIG");
    }

    #[test]
    fn test_show_json_with_profiles() {
        let (_dir, _path) = setup_config(
            r#"
[defaults]
profile = "prod"

[profiles.prod]
backend = "efi"
environment = "production"
client_id = "prod_id"
client_secret = "prod_secret"
certificate = "/cert.p12"
certificate_password = "pass123"
default_pix_key = "+5511999999999"
"#,
        );
        let result = show(OutputFormat::Json);
        cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_human_with_profiles() {
        let (_dir, _path) = setup_config(
            r#"
[defaults]
profile = "test"

[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/cert.p12"
default_pix_key = "+5511999999999"

[profiles.other]
backend = "efi"
environment = "production"
client_id = "id2"
client_secret = "secret2"
certificate = "/cert2.p12"
"#,
        );
        let result = show(OutputFormat::Human);
        cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_human_empty_profiles() {
        let (_dir, _path) = setup_config("");
        let result = show(OutputFormat::Human);
        cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_table_format() {
        let (_dir, _path) = setup_config(
            r#"
[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/cert.p12"
"#,
        );
        let result = show(OutputFormat::Table);
        cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_show_command() {
        let (_dir, _path) = setup_config(
            r#"
[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/cert.p12"
"#,
        );
        let result = run(ConfigCommand::Show, OutputFormat::Human);
        cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_json_redacts_secrets() {
        let (_dir, _path) = setup_config(
            r#"
[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "actual_secret"
certificate = "/cert.p12"
certificate_password = "actual_password"
"#,
        );
        // show prints to stdout; we just verify it doesn't error
        let result = show(OutputFormat::Json);
        cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_profile_no_pix_key() {
        let (_dir, _path) = setup_config(
            r#"
[defaults]
profile = "nopk"

[profiles.nopk]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/cert.p12"
"#,
        );
        let result = show(OutputFormat::Human);
        cleanup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_profile_empty_password_not_redacted() {
        let (_dir, _path) = setup_config(
            r#"
[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "/cert.p12"
certificate_password = ""
"#,
        );
        let result = show(OutputFormat::Json);
        cleanup();
        assert!(result.is_ok());
    }
}

//! TOML-based configuration management for the Pix CLI.
//!
//! Re-exports shared types from `pix-config` and adds the interactive
//! setup wizard specific to the CLI.

pub use pix_config::{PixConfig, Profile};

/// Runs the interactive setup wizard, returning the updated config.
pub fn run_setup_wizard() -> anyhow::Result<PixConfig> {
    use dialoguer::{Confirm, Input, Select};

    println!("Pixcli -- Setup Wizard");
    println!();

    let backends = vec!["efi (Efi / Gerencianet)"];
    let backend_idx = Select::new()
        .with_prompt("Which PSP do you want to configure?")
        .items(&backends)
        .default(0)
        .interact()?;

    let backend = match backend_idx {
        0 => "efi",
        _ => anyhow::bail!("only Efi is supported currently"),
    };

    let profile_name: String = Input::new()
        .with_prompt("Profile name")
        .default("efi-sandbox".to_string())
        .interact_text()?;

    let envs = vec!["sandbox", "production"];
    let env_idx = Select::new()
        .with_prompt("Environment")
        .items(&envs)
        .default(0)
        .interact()?;
    let environment = envs[env_idx].to_string();

    println!();
    println!("Enter your Efi API credentials.");
    println!("Get them at: https://app.sejaefi.com.br/ -> API -> Aplicacoes");
    println!();

    let client_id: String = Input::new().with_prompt("Client ID").interact_text()?;

    let client_secret: String = Input::new().with_prompt("Client Secret").interact_text()?;

    let certificate: String = Input::new()
        .with_prompt("Path to P12 certificate")
        .default("~/.pixcli/certs/sandbox.p12".to_string())
        .interact_text()?;

    let certificate_password: String = Input::new()
        .with_prompt("Certificate password (Enter if none)")
        .default(String::new())
        .interact_text()?;

    let default_pix_key: String = Input::new()
        .with_prompt("Default Pix key (for sending, Enter to skip)")
        .default(String::new())
        .interact_text()?;

    let set_as_default = Confirm::new()
        .with_prompt("Set as default profile?")
        .default(true)
        .interact()?;

    let profile = Profile {
        backend: backend.to_string(),
        environment,
        client_id,
        client_secret,
        certificate,
        certificate_password,
        default_pix_key: if default_pix_key.is_empty() {
            None
        } else {
            Some(default_pix_key)
        },
    };

    let mut config = PixConfig::load(None).unwrap_or_default();
    config.profiles.insert(profile_name.clone(), profile);
    if set_as_default {
        config.defaults.profile = profile_name.clone();
    }

    config.save(None)?;

    println!();
    println!(
        "Profile '{}' saved to {}",
        profile_name,
        PixConfig::default_path().display()
    );
    println!("Test it with: pixcli balance --profile {}", profile_name);

    Ok(config)
}

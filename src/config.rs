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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_default_config() {
        let config = PixConfig::default();
        assert_eq!(config.defaults.profile, "default");
        assert_eq!(config.defaults.output, "human");
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let config = PixConfig::load(Some(Path::new("/tmp/nonexistent-pixcli-config.toml")));
        assert!(config.is_ok());
        let config = config.unwrap();
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = PixConfig::default();
        config.profiles.insert(
            "test".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: "my_id".to_string(),
                client_secret: "my_secret".to_string(),
                certificate: "/path/to/cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: Some("+5511999999999".to_string()),
            },
        );
        config.defaults.profile = "test".to_string();

        config.save(Some(&path)).unwrap();
        let loaded = PixConfig::load(Some(&path)).unwrap();

        assert_eq!(loaded.defaults.profile, "test");
        let profile = loaded.profiles.get("test").unwrap();
        assert_eq!(profile.client_id, "my_id");
        assert_eq!(profile.backend, "efi");
        assert_eq!(profile.default_pix_key, Some("+5511999999999".to_string()));
    }

    #[test]
    fn test_get_profile_missing() {
        let config = PixConfig::default();
        let result = config.get_profile(Some("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_profile_found() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "p1".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                certificate: "cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );

        let p = config.get_profile(Some("p1")).unwrap();
        assert_eq!(p.client_id, "id");
    }

    #[test]
    fn test_expand_path_tilde() {
        let expanded = PixConfig::expand_path("~/test/file.txt");
        assert!(!expanded.to_string_lossy().starts_with('~'));
        assert!(expanded.to_string_lossy().ends_with("test/file.txt"));
    }

    #[test]
    fn test_expand_path_absolute() {
        let expanded = PixConfig::expand_path("/absolute/path");
        assert_eq!(expanded, std::path::PathBuf::from("/absolute/path"));
    }

    #[cfg(unix)]
    #[test]
    fn test_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = PixConfig::default();
        config.save(Some(&path)).unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_corrupt_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "{{{{not valid toml}}}}").unwrap();
        let result = PixConfig::load(Some(&path));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("failed to parse config"));
    }

    #[test]
    fn test_empty_toml_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "").unwrap();
        let config = PixConfig::load(Some(&path)).unwrap();
        assert_eq!(config.defaults.profile, "default");
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_missing_fields_use_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[profiles.myprofile]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "cert.p12"
"#,
        )
        .unwrap();
        let config = PixConfig::load(Some(&path)).unwrap();
        assert_eq!(config.defaults.profile, "default");
        assert_eq!(config.defaults.output, "human");
        let p = config.profiles.get("myprofile").unwrap();
        assert_eq!(p.certificate_password, "");
        assert!(p.default_pix_key.is_none());
    }

    #[test]
    fn test_get_profile_default_fallback() {
        let mut config = PixConfig::default();
        config.defaults.profile = "myp".to_string();
        config.profiles.insert(
            "myp".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                certificate: "cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );
        let p = config.get_profile(None).unwrap();
        assert_eq!(p.client_id, "id");
    }

    #[test]
    fn test_multiple_profiles_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = PixConfig::default();
        for i in 0..3 {
            config.profiles.insert(
                format!("profile_{i}"),
                Profile {
                    backend: "efi".to_string(),
                    environment: if i == 0 { "production" } else { "sandbox" }.to_string(),
                    client_id: format!("id_{i}"),
                    client_secret: format!("secret_{i}"),
                    certificate: format!("cert_{i}.p12"),
                    certificate_password: String::new(),
                    default_pix_key: None,
                },
            );
        }

        config.save(Some(&path)).unwrap();
        let loaded = PixConfig::load(Some(&path)).unwrap();
        assert_eq!(loaded.profiles.len(), 3);
        assert_eq!(loaded.profiles["profile_0"].environment, "production");
        assert_eq!(loaded.profiles["profile_1"].client_id, "id_1");
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deeply").join("nested").join("config.toml");
        let config = PixConfig::default();
        config.save(Some(&path)).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_env_overrides_apply() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "default".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: "original_id".to_string(),
                client_secret: "original_secret".to_string(),
                certificate: "/original/cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );

        std::env::set_var("PIXCLI_CLIENT_ID", "env_id_override");
        config.apply_env_overrides();
        std::env::remove_var("PIXCLI_CLIENT_ID");

        let profile = config.profiles.get("default").unwrap();
        assert_eq!(profile.client_id, "env_id_override");
        assert_eq!(profile.client_secret, "original_secret");
    }

    #[test]
    fn test_env_overrides_empty_profile_removed() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "default".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: String::new(),
                client_secret: String::new(),
                certificate: String::new(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );

        std::env::remove_var("PIXCLI_CLIENT_ID");
        std::env::remove_var("PIXCLI_CLIENT_SECRET");
        std::env::remove_var("PIXCLI_CERTIFICATE");
        std::env::remove_var("PIXCLI_CERTIFICATE_PASSWORD");
        std::env::remove_var("PIXCLI_PIX_KEY");

        config.apply_env_overrides();
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_get_profile_missing_with_available_profiles() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "prod".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "production".to_string(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                certificate: "cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );
        let result = config.get_profile(Some("nonexistent"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("prod"));
    }

    #[test]
    fn test_get_profile_missing_no_profiles() {
        let config = PixConfig::default();
        let result = config.get_profile(Some("nonexistent"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("config init"));
    }

    #[test]
    fn test_expand_path_relative() {
        let expanded = PixConfig::expand_path("relative/path");
        assert_eq!(expanded, std::path::PathBuf::from("relative/path"));
    }

    #[test]
    fn test_defaults_custom_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[defaults]
profile = "custom"
output = "json"
"#,
        )
        .unwrap();
        let config = PixConfig::load(Some(&path)).unwrap();
        assert_eq!(config.defaults.profile, "custom");
        assert_eq!(config.defaults.output, "json");
    }

    #[test]
    fn test_profile_with_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[profiles.full]
backend = "efi"
environment = "production"
client_id = "full_id"
client_secret = "full_secret"
certificate = "/full/cert.p12"
certificate_password = "password123"
default_pix_key = "+5511999999999"
"#,
        )
        .unwrap();
        let config = PixConfig::load(Some(&path)).unwrap();
        let p = config.profiles.get("full").unwrap();
        assert_eq!(p.backend, "efi");
        assert_eq!(p.environment, "production");
        assert_eq!(p.certificate_password, "password123");
        assert_eq!(p.default_pix_key, Some("+5511999999999".to_string()));
    }

    #[test]
    fn test_default_path_with_env_var() {
        std::env::set_var("PIXCLI_CONFIG", "/custom/config.toml");
        let path = PixConfig::default_path();
        std::env::remove_var("PIXCLI_CONFIG");
        assert_eq!(path, std::path::PathBuf::from("/custom/config.toml"));
    }

    #[test]
    fn test_default_path_without_env_var() {
        std::env::remove_var("PIXCLI_CONFIG");
        let path = PixConfig::default_path();
        assert!(path.to_string_lossy().ends_with("config.toml"));
        assert!(path.to_string_lossy().contains(".pixcli"));
    }
}

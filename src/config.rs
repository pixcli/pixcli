//! TOML-based configuration management for the Pix CLI.
//!
//! Configuration is stored at `~/.pixcli/config.toml` (or overridden via
//! `PIXCLI_CONFIG` environment variable). Supports multiple named profiles,
//! environment variable overrides, and an interactive setup wizard.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top-level CLI configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PixConfig {
    /// Global defaults.
    #[serde(default)]
    pub defaults: Defaults,
    /// Named provider profiles.
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

/// Global default settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    /// Name of the default profile.
    #[serde(default = "default_profile_name")]
    pub profile: String,
    /// Default output format ("human", "json", "table").
    #[serde(default = "default_output")]
    pub output: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            profile: default_profile_name(),
            output: default_output(),
        }
    }
}

fn default_profile_name() -> String {
    "default".to_string()
}

fn default_output() -> String {
    "human".to_string()
}

/// A provider profile with credentials and settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Provider backend name (e.g., "efi").
    pub backend: String,
    /// Environment: "production" or "sandbox".
    pub environment: String,
    /// OAuth2 client ID.
    pub client_id: String,
    /// OAuth2 client secret.
    pub client_secret: String,
    /// Path to the PKCS#12 certificate.
    pub certificate: String,
    /// Certificate password (empty if none).
    #[serde(default)]
    pub certificate_password: String,
    /// Default Pix key used for sending.
    pub default_pix_key: Option<String>,
}

impl PixConfig {
    /// Loads the config from the default or specified path.
    ///
    /// If the file does not exist, returns a default (empty) config.
    /// Environment variable overrides are applied after loading.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);

        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read config: {}", config_path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("failed to parse config: {}", config_path.display()))?
        } else {
            Self::default()
        };

        config.apply_env_overrides();
        Ok(config)
    }

    /// Saves the config to disk, creating parent directories as needed.
    pub fn save(&self, path: Option<&Path>) -> Result<()> {
        let config_path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir: {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self).context("failed to serialize config")?;

        std::fs::write(&config_path, &content)
            .with_context(|| format!("failed to write config: {}", config_path.display()))?;

        // Set restrictive permissions on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&config_path, perms)?;
        }

        Ok(())
    }

    /// Returns the default config file path.
    ///
    /// Checks `PIXCLI_CONFIG` env var first, then falls back to
    /// `~/.pixcli/config.toml`.
    pub fn default_path() -> PathBuf {
        if let Ok(path) = std::env::var("PIXCLI_CONFIG") {
            return PathBuf::from(path);
        }
        let home = directories::BaseDirs::new()
            .map(|d| d.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".pixcli").join("config.toml")
    }

    /// Looks up a profile by name, falling back to the default profile.
    pub fn get_profile(&self, name: Option<&str>) -> Result<&Profile> {
        let profile_name = name.unwrap_or(&self.defaults.profile);
        self.profiles.get(profile_name).with_context(|| {
            let available: Vec<&String> = self.profiles.keys().collect();
            if available.is_empty() {
                format!(
                    "profile '{}' not found. Run `pixcli config init` to create one.",
                    profile_name
                )
            } else {
                format!(
                    "profile '{}' not found. Available: {}",
                    profile_name,
                    available
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        })
    }

    /// Expands `~` at the start of a path to the user's home directory.
    pub fn expand_path(path: &str) -> PathBuf {
        if let Some(rest) = path.strip_prefix("~/") {
            if let Some(base) = directories::BaseDirs::new() {
                return base.home_dir().join(rest);
            }
        }
        PathBuf::from(path)
    }

    /// Applies environment variable overrides to the default profile.
    ///
    /// Supported variables:
    /// - `PIXCLI_CLIENT_ID`
    /// - `PIXCLI_CLIENT_SECRET`
    /// - `PIXCLI_CERTIFICATE`
    /// - `PIXCLI_CERTIFICATE_PASSWORD`
    /// - `PIXCLI_PIX_KEY`
    fn apply_env_overrides(&mut self) {
        let profile_name = self.defaults.profile.clone();
        let profile = self
            .profiles
            .entry(profile_name)
            .or_insert_with(|| Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: String::new(),
                client_secret: String::new(),
                certificate: String::new(),
                certificate_password: String::new(),
                default_pix_key: None,
            });

        if let Ok(val) = std::env::var("PIXCLI_CLIENT_ID") {
            profile.client_id = val;
        }
        if let Ok(val) = std::env::var("PIXCLI_CLIENT_SECRET") {
            profile.client_secret = val;
        }
        if let Ok(val) = std::env::var("PIXCLI_CERTIFICATE") {
            profile.certificate = val;
        }
        if let Ok(val) = std::env::var("PIXCLI_CERTIFICATE_PASSWORD") {
            profile.certificate_password = val;
        }
        if let Ok(val) = std::env::var("PIXCLI_PIX_KEY") {
            profile.default_pix_key = Some(val);
        }

        // Remove the profile if it's entirely empty (no env vars set).
        let p = self.profiles.get(&self.defaults.profile);
        if let Some(p) = p {
            if p.client_id.is_empty() && p.client_secret.is_empty() && p.certificate.is_empty() {
                self.profiles.remove(&self.defaults.profile);
            }
        }
    }
}

/// Runs the interactive setup wizard, returning the updated config.
pub fn run_setup_wizard() -> Result<PixConfig> {
    use dialoguer::{Confirm, Input, Select};

    println!("🏦 Pixcli — Setup Wizard");
    println!();

    let backends = vec!["efi (Efí / Gerencianet)"];
    let backend_idx = Select::new()
        .with_prompt("Which PSP do you want to configure?")
        .items(&backends)
        .default(0)
        .interact()?;

    let backend = match backend_idx {
        0 => "efi",
        _ => anyhow::bail!("only Efí is supported currently"),
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
    println!("Enter your Efí API credentials.");
    println!("Get them at: https://app.sejaefi.com.br/ → API → Aplicações");
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
        "✅ Profile '{}' saved to {}",
        profile_name,
        PixConfig::default_path().display()
    );
    println!("Test it with: pixcli balance --profile {}", profile_name);

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
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
        // defaults section should have default values
        assert_eq!(config.defaults.profile, "default");
        assert_eq!(config.defaults.output, "human");
        // profile should exist with empty certificate_password default
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
        // None should fall back to default profile
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
}

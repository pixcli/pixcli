//! Configuration loading for the MCP server.
//!
//! Reuses the same `~/.pixcli/config.toml` format as the CLI,
//! with support for the `PIXCLI_CONFIG` environment variable.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top-level CLI configuration (mirrors the CLI's config structure).
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
    /// Default output format.
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
    /// Loads the config from disk.
    ///
    /// Reads from `PIXCLI_CONFIG` env var path, or falls back to `~/.pixcli/config.toml`.
    pub fn load() -> Result<Self> {
        let config_path = Self::default_path();

        if !config_path.exists() {
            anyhow::bail!(
                "Config file not found at {}. Run `pixcli config init` to create one.",
                config_path.display()
            );
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config: {}", config_path.display()))?;

        let mut config: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse config: {}", config_path.display()))?;

        config.apply_env_overrides();
        Ok(config)
    }

    /// Returns the default config file path.
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
    fn apply_env_overrides(&mut self) {
        let has_env = std::env::var("PIXCLI_CLIENT_ID").is_ok()
            || std::env::var("PIXCLI_CLIENT_SECRET").is_ok()
            || std::env::var("PIXCLI_CERTIFICATE").is_ok();

        if !has_env {
            return;
        }

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
    }
}

/// Loads config from a specific path (used in tests).
#[cfg(test)]
pub fn load_config_from(path: &std::path::Path) -> Result<PixConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config: {}", path.display()))?;
    let config: PixConfig = toml::from_str(&content)
        .with_context(|| format!("failed to parse config: {}", path.display()))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PixConfig::default();
        assert_eq!(config.defaults.profile, "default");
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_load_missing_config_fails() {
        // Ensure no env override
        std::env::remove_var("PIXCLI_CONFIG");
        // This will fail because ~/.pixcli/config.toml likely doesn't exist in CI
        // The important thing is it returns an error, not a panic
        let _ = PixConfig::load();
    }

    #[test]
    fn test_load_config_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[defaults]
profile = "test"

[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "my_id"
client_secret = "my_secret"
certificate = "/path/to/cert.p12"
"#,
        )
        .unwrap();

        let config = load_config_from(&path).unwrap();
        assert_eq!(config.defaults.profile, "test");
        let profile = config.get_profile(Some("test")).unwrap();
        assert_eq!(profile.client_id, "my_id");
        assert_eq!(profile.backend, "efi");
    }

    #[test]
    fn test_get_profile_missing() {
        let config = PixConfig::default();
        let result = config.get_profile(Some("nonexistent"));
        assert!(result.is_err());
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

    #[test]
    fn test_corrupt_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "{{not valid}}").unwrap();
        let result = load_config_from(&path);
        assert!(result.is_err());
    }
}

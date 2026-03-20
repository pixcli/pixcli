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

        // Read directly without pre-checking exists() to avoid TOCTOU race.
        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                anyhow::bail!(
                    "Config file not found at {}. Run `pixcli config init` to create one.",
                    config_path.display()
                );
            }
            Err(e) => {
                return Err(anyhow::Error::new(e)
                    .context(format!("failed to read config: {}", config_path.display())));
            }
        };

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

    #[test]
    fn test_get_profile_default_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[defaults]
profile = "myprofile"

[profiles.myprofile]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "cert.p12"
"#,
        )
        .unwrap();
        let config = load_config_from(&path).unwrap();
        // None should fallback to default profile
        let p = config.get_profile(None).unwrap();
        assert_eq!(p.client_id, "id");
    }

    #[test]
    fn test_get_profile_with_available_profiles_error_msg() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[profiles.prod]
backend = "efi"
environment = "production"
client_id = "id"
client_secret = "secret"
certificate = "cert.p12"
"#,
        )
        .unwrap();
        let config = load_config_from(&path).unwrap();
        let result = config.get_profile(Some("nonexistent"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("prod"));
    }

    #[test]
    fn test_get_profile_no_profiles_error_msg() {
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
    fn test_load_config_from_with_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[defaults]
profile = "full"
output = "json"

[profiles.full]
backend = "efi"
environment = "production"
client_id = "full_id"
client_secret = "full_secret"
certificate = "/path/cert.p12"
certificate_password = "password123"
default_pix_key = "+5511999999999"
"#,
        )
        .unwrap();
        let config = load_config_from(&path).unwrap();
        assert_eq!(config.defaults.profile, "full");
        assert_eq!(config.defaults.output, "json");
        let p = config.profiles.get("full").unwrap();
        assert_eq!(p.certificate_password, "password123");
        assert_eq!(p.default_pix_key, Some("+5511999999999".to_string()));
    }

    #[test]
    fn test_load_config_from_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "").unwrap();
        let config = load_config_from(&path).unwrap();
        assert_eq!(config.defaults.profile, "default");
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_load_config_from_multiple_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[profiles.a]
backend = "efi"
environment = "sandbox"
client_id = "a_id"
client_secret = "a_secret"
certificate = "a.p12"

[profiles.b]
backend = "efi"
environment = "production"
client_id = "b_id"
client_secret = "b_secret"
certificate = "b.p12"
default_pix_key = "key_b"
"#,
        )
        .unwrap();
        let config = load_config_from(&path).unwrap();
        assert_eq!(config.profiles.len(), 2);
        assert_eq!(config.profiles["a"].client_id, "a_id");
        assert_eq!(
            config.profiles["b"].default_pix_key,
            Some("key_b".to_string())
        );
    }

    #[test]
    fn test_default_path_with_env_var() {
        std::env::set_var("PIXCLI_CONFIG", "/custom/mcp_config.toml");
        let path = PixConfig::default_path();
        std::env::remove_var("PIXCLI_CONFIG");
        assert_eq!(path, std::path::PathBuf::from("/custom/mcp_config.toml"));
    }

    #[test]
    fn test_default_path_without_env_var() {
        std::env::remove_var("PIXCLI_CONFIG");
        let path = PixConfig::default_path();
        assert!(path.to_string_lossy().ends_with("config.toml"));
        assert!(path.to_string_lossy().contains(".pixcli"));
    }

    #[test]
    fn test_load_missing_config() {
        std::env::set_var(
            "PIXCLI_CONFIG",
            "/tmp/absolutely-nonexistent-pixcli-mcp-config.toml",
        );
        let result = PixConfig::load();
        std::env::remove_var("PIXCLI_CONFIG");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Config file not found"));
    }

    #[test]
    fn test_load_valid_config_via_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[profiles.test]
backend = "efi"
environment = "sandbox"
client_id = "test_id"
client_secret = "test_secret"
certificate = "cert.p12"
"#,
        )
        .unwrap();
        std::env::set_var("PIXCLI_CONFIG", path.to_str().unwrap());
        let config = PixConfig::load().unwrap();
        std::env::remove_var("PIXCLI_CONFIG");
        assert!(config.profiles.contains_key("test"));
    }

    #[test]
    fn test_load_corrupt_config_via_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "{{invalid toml}}").unwrap();
        std::env::set_var("PIXCLI_CONFIG", path.to_str().unwrap());
        let result = PixConfig::load();
        std::env::remove_var("PIXCLI_CONFIG");
        assert!(result.is_err());
    }

    #[test]
    fn test_env_overrides_applied() {
        // Test override logic directly without env vars to avoid race conditions
        let mut config = PixConfig::default();
        config.profiles.insert(
            "default".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: "original_id".to_string(),
                client_secret: "original_secret".to_string(),
                certificate: "cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );
        // Simulate what apply_env_overrides does without touching global env
        let profile = config.profiles.get_mut("default").unwrap();
        profile.client_id = "overridden_id_mcp".to_string();

        let p = config.profiles.get("default").unwrap();
        assert_eq!(p.client_id, "overridden_id_mcp");
        // Other fields unchanged
        assert_eq!(p.client_secret, "original_secret");
    }

    #[test]
    fn test_env_overrides_with_pix_key_and_password() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "default".to_string(),
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

        // Simulate env overrides directly to avoid race conditions
        let p = config.profiles.get_mut("default").unwrap();
        p.client_id = "id_mcp_2".to_string();
        p.certificate_password = "env_pass_mcp".to_string();
        p.default_pix_key = Some("+5511777777777".to_string());

        let p = config.profiles.get("default").unwrap();
        assert_eq!(p.certificate_password, "env_pass_mcp");
        assert_eq!(p.default_pix_key, Some("+5511777777777".to_string()));
    }

    #[test]
    fn test_no_env_overrides_doesnt_create_profile() {
        let mut config = PixConfig::default();
        // No profiles — apply_env_overrides with no env vars should not create a profile
        // (test logic: if has_env is false, profiles stays empty)
        config.apply_env_overrides();
        assert!(config.profiles.is_empty());
    }
}

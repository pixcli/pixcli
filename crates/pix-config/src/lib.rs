#![deny(unsafe_code)]
//! Shared TOML-based configuration types for the Pix CLI and MCP server.
//!
//! Configuration is stored at `~/.pixcli/config.toml` (or overridden via
//! `PIXCLI_CONFIG` environment variable). Supports multiple named profiles,
//! environment variable overrides, and path expansion.

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

        // Create the file with restrictive permissions from the start to avoid a
        // race window where secrets are world-readable.
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&config_path)
                .and_then(|mut f| f.write_all(content.as_bytes()))
                .with_context(|| format!("failed to write config: {}", config_path.display()))?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&config_path, &content)
                .with_context(|| format!("failed to write config: {}", config_path.display()))?;
        }

        Ok(())
    }

    /// Returns the default config file path.
    ///
    /// Checks `PIXCLI_CONFIG` env var first, then falls back to
    /// `~/.pixcli/config.toml`.
    pub fn default_path() -> PathBuf {
        Self::default_path_from_env(std::env::var("PIXCLI_CONFIG").ok())
    }

    /// Returns the default config file path from an explicit override value.
    ///
    /// Use this instead of [`PixConfig::default_path`] in tests to avoid mutating
    /// process-global environment variables (which is UB in multi-threaded
    /// programs).
    pub fn default_path_from_env(env_override: Option<String>) -> PathBuf {
        if let Some(path) = env_override {
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
    pub fn apply_env_overrides(&mut self) {
        let env: HashMap<String, String> = std::env::vars()
            .filter(|(k, _)| k.starts_with("PIXCLI_"))
            .collect();
        self.apply_env_overrides_from(&env);
    }

    /// Applies overrides from an explicit map of environment variables.
    ///
    /// Use this instead of [`PixConfig::apply_env_overrides`] in tests to avoid
    /// mutating process-global environment variables.
    pub fn apply_env_overrides_from(&mut self, env: &HashMap<String, String>) {
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

        if let Some(val) = env.get("PIXCLI_CLIENT_ID") {
            profile.client_id = val.clone();
        }
        if let Some(val) = env.get("PIXCLI_CLIENT_SECRET") {
            profile.client_secret = val.clone();
        }
        if let Some(val) = env.get("PIXCLI_CERTIFICATE") {
            profile.certificate = val.clone();
        }
        if let Some(val) = env.get("PIXCLI_CERTIFICATE_PASSWORD") {
            profile.certificate_password = val.clone();
        }
        if let Some(val) = env.get("PIXCLI_PIX_KEY") {
            profile.default_pix_key = Some(val.clone());
        }

        // Remove the profile if it's entirely empty (no env vars set).
        let p = self.profiles.get(&self.defaults.profile);
        if let Some(p) = p {
            if p.client_id.is_empty()
                && p.client_secret.is_empty()
                && p.certificate.is_empty()
                && p.certificate_password.is_empty()
                && p.default_pix_key.is_none()
            {
                self.profiles.remove(&self.defaults.profile);
            }
        }
    }
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

    #[test]
    fn test_expand_path_relative() {
        let expanded = PixConfig::expand_path("relative/path");
        assert_eq!(expanded, PathBuf::from("relative/path"));
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

        let env = HashMap::from([(
            "PIXCLI_CLIENT_ID".to_string(),
            "env_id_override".to_string(),
        )]);
        config.apply_env_overrides_from(&env);

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

        let env = HashMap::new();
        config.apply_env_overrides_from(&env);
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
    fn test_default_path_with_env_override() {
        let path = PixConfig::default_path_from_env(Some("/custom/config.toml".to_string()));
        assert_eq!(path, PathBuf::from("/custom/config.toml"));
    }

    #[test]
    fn test_default_path_without_env_override() {
        let path = PixConfig::default_path_from_env(None);
        assert!(path.to_string_lossy().ends_with("config.toml"));
        assert!(path.to_string_lossy().contains(".pixcli"));
    }
}

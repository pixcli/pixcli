//! Configuration loading for the MCP server.
//!
//! Re-exports the shared config types from `pix-config` and adds
//! MCP-specific loading behavior (requiring the config file to exist).

pub use pix_config::{PixConfig, Profile};

/// Loads the MCP server config from a specific path or the default location.
///
/// Unlike `PixConfig::load`, this requires the file to exist.
pub fn load_mcp_config(path: Option<&std::path::Path>) -> anyhow::Result<PixConfig> {
    let config_path = path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(PixConfig::default_path);
    if !config_path.exists() {
        anyhow::bail!(
            "Config file not found at {}. Run `pixcli config init` to create one.",
            config_path.display()
        );
    }
    PixConfig::load(Some(&config_path))
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
        let result = load_mcp_config(Some(std::path::Path::new(
            "/tmp/nonexistent-mcp-config.toml",
        )));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Config file not found"));
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

        let config = load_mcp_config(Some(&path)).unwrap();
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
        assert_eq!(expanded, std::path::PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_corrupt_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "{{not valid}}").unwrap();
        let result = load_mcp_config(Some(&path));
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
        let config = load_mcp_config(Some(&path)).unwrap();
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
        let config = load_mcp_config(Some(&path)).unwrap();
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
        let config = load_mcp_config(Some(&path)).unwrap();
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
        let config = PixConfig::load(Some(&path)).unwrap();
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
        let config = load_mcp_config(Some(&path)).unwrap();
        assert_eq!(config.profiles.len(), 2);
        assert_eq!(config.profiles["a"].client_id, "a_id");
        assert_eq!(
            config.profiles["b"].default_pix_key,
            Some("key_b".to_string())
        );
    }

    #[test]
    fn test_default_path_with_env_override() {
        let path = PixConfig::default_path_from_env(Some("/custom/mcp_config.toml".to_string()));
        assert_eq!(path, std::path::PathBuf::from("/custom/mcp_config.toml"));
    }

    #[test]
    fn test_default_path_without_env_override() {
        let path = PixConfig::default_path_from_env(None);
        assert!(path.to_string_lossy().ends_with("config.toml"));
        assert!(path.to_string_lossy().contains(".pixcli"));
    }

    #[test]
    fn test_load_valid_config() {
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
        let config = load_mcp_config(Some(&path)).unwrap();
        assert!(config.profiles.contains_key("test"));
    }

    #[test]
    fn test_load_corrupt_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "{{invalid toml}}").unwrap();
        let result = load_mcp_config(Some(&path));
        assert!(result.is_err());
    }
}

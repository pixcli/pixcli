//! Configuration loading for the MCP server.
//!
//! Re-exports the shared `pix-config` types and adds an MCP-specific
//! `load()` helper that errors when the config file is missing (unlike
//! the CLI which returns a default config for the setup wizard).

pub use pix_config::{PixConfig, Profile};

/// Loads the config from disk, returning an error if the file is missing.
///
/// The MCP server requires a valid config to connect to a Pix provider,
/// so unlike the CLI's `PixConfig::load()` (which returns defaults for
/// the setup wizard), this function treats a missing config as an error.
pub fn load() -> anyhow::Result<PixConfig> {
    let config_path = PixConfig::default_path();

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
    fn test_reexported_types_work() {
        let config = PixConfig::default();
        assert_eq!(config.defaults.profile, "default");
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_load_valid_config_from_path() {
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
        let config = PixConfig::load(Some(&path)).unwrap();
        assert!(config.profiles.contains_key("test"));
    }
}

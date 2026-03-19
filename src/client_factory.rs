//! Builds a `PixProvider` from CLI arguments and config.

use anyhow::Result;
use pix_efi::config::{EfiConfig, EfiEnvironment};
use pix_efi::EfiClient;
use pix_provider::PixProvider;

use crate::config::PixConfig;

/// Resolves an `EfiConfig` from the current CLI config and flags.
fn resolve_efi_config(
    config: &PixConfig,
    profile_name: Option<&str>,
    sandbox: bool,
) -> Result<(EfiConfig, Option<String>)> {
    let profile = config.get_profile(profile_name)?;

    match profile.backend.as_str() {
        "efi" => {
            let environment = if sandbox || profile.environment == "sandbox" {
                EfiEnvironment::Sandbox
            } else {
                EfiEnvironment::Production
            };

            let cert_path = PixConfig::expand_path(&profile.certificate);

            let efi_config = EfiConfig {
                client_id: profile.client_id.clone(),
                client_secret: profile.client_secret.clone(),
                certificate_path: cert_path,
                certificate_password: profile.certificate_password.clone(),
                environment,
            };

            Ok((efi_config, profile.default_pix_key.clone()))
        }
        other => anyhow::bail!("unknown backend: '{}'. Supported: efi", other),
    }
}

/// Builds an appropriate `PixProvider` from the current config and CLI flags.
///
/// - `profile` — optional profile name override.
/// - `sandbox` — if `true`, forces sandbox environment regardless of profile setting.
pub fn build_provider(
    config: &PixConfig,
    profile_name: Option<&str>,
    sandbox: bool,
) -> Result<impl PixProvider> {
    let (efi_config, pix_key) = resolve_efi_config(config, profile_name, sandbox)?;

    let client = if let Some(key) = pix_key {
        EfiClient::with_pix_key(efi_config, key)?
    } else {
        EfiClient::new(efi_config)?
    };

    Ok(client)
}

/// Builds an `EfiClient` directly (for webhook and other Efí-specific operations).
///
/// - `profile` — optional profile name override.
/// - `sandbox` — if `true`, forces sandbox environment regardless of profile setting.
pub fn build_efi_client(
    config: &PixConfig,
    profile_name: Option<&str>,
    sandbox: bool,
) -> Result<EfiClient> {
    let (efi_config, pix_key) = resolve_efi_config(config, profile_name, sandbox)?;

    let client = if let Some(key) = pix_key {
        EfiClient::with_pix_key(efi_config, key)?
    } else {
        EfiClient::new(efi_config)?
    };

    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config_with_profile(backend: &str, environment: &str) -> PixConfig {
        let mut config = PixConfig::default();
        config.defaults.profile = "test".to_string();
        config.profiles.insert(
            "test".to_string(),
            crate::config::Profile {
                backend: backend.to_string(),
                environment: environment.to_string(),
                client_id: "test_id".to_string(),
                client_secret: "test_secret".to_string(),
                certificate: "/nonexistent/cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );
        config
    }

    fn make_config_with_pix_key() -> PixConfig {
        let mut config = PixConfig::default();
        config.defaults.profile = "test".to_string();
        config.profiles.insert(
            "test".to_string(),
            crate::config::Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: "test_id".to_string(),
                client_secret: "test_secret".to_string(),
                certificate: "/nonexistent/cert.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: Some("+5511999999999".to_string()),
            },
        );
        config
    }

    #[test]
    fn test_resolve_efi_config_sandbox() {
        let config = make_config_with_profile("efi", "sandbox");
        let (efi_config, pix_key) = resolve_efi_config(&config, None, false).unwrap();
        assert_eq!(efi_config.environment, EfiEnvironment::Sandbox);
        assert_eq!(efi_config.client_id, "test_id");
        assert!(pix_key.is_none());
    }

    #[test]
    fn test_resolve_efi_config_production() {
        let config = make_config_with_profile("efi", "production");
        let (efi_config, _) = resolve_efi_config(&config, None, false).unwrap();
        assert_eq!(efi_config.environment, EfiEnvironment::Production);
    }

    #[test]
    fn test_resolve_efi_config_force_sandbox() {
        let config = make_config_with_profile("efi", "production");
        let (efi_config, _) = resolve_efi_config(&config, None, true).unwrap();
        assert_eq!(efi_config.environment, EfiEnvironment::Sandbox);
    }

    #[test]
    fn test_resolve_efi_config_with_pix_key() {
        let config = make_config_with_pix_key();
        let (_, pix_key) = resolve_efi_config(&config, None, false).unwrap();
        assert_eq!(pix_key, Some("+5511999999999".to_string()));
    }

    #[test]
    fn test_resolve_efi_config_unknown_backend() {
        let config = make_config_with_profile("mercadopago", "sandbox");
        let result = resolve_efi_config(&config, None, false);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unknown backend"));
    }

    #[test]
    fn test_resolve_efi_config_missing_profile() {
        let config = make_config_with_profile("efi", "sandbox");
        let result = resolve_efi_config(&config, Some("nonexistent"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_efi_config_named_profile() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "named".to_string(),
            crate::config::Profile {
                backend: "efi".to_string(),
                environment: "production".to_string(),
                client_id: "named_id".to_string(),
                client_secret: "named_secret".to_string(),
                certificate: "/cert.p12".to_string(),
                certificate_password: "pass".to_string(),
                default_pix_key: None,
            },
        );
        let (efi_config, _) = resolve_efi_config(&config, Some("named"), false).unwrap();
        assert_eq!(efi_config.client_id, "named_id");
        assert_eq!(efi_config.client_secret, "named_secret");
        assert_eq!(efi_config.certificate_password, "pass");
    }

    #[test]
    fn test_build_provider_cert_not_found() {
        let config = make_config_with_profile("efi", "sandbox");
        let result = build_provider(&config, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_efi_client_cert_not_found() {
        let config = make_config_with_profile("efi", "sandbox");
        let result = build_efi_client(&config, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_provider_with_pix_key_cert_not_found() {
        let config = make_config_with_pix_key();
        let result = build_provider(&config, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_efi_client_with_pix_key_cert_not_found() {
        let config = make_config_with_pix_key();
        let result = build_efi_client(&config, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_efi_config_cert_path_expansion() {
        let mut config = PixConfig::default();
        config.defaults.profile = "test".to_string();
        config.profiles.insert(
            "test".to_string(),
            crate::config::Profile {
                backend: "efi".to_string(),
                environment: "sandbox".to_string(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                certificate: "~/certs/test.p12".to_string(),
                certificate_password: String::new(),
                default_pix_key: None,
            },
        );
        let (efi_config, _) = resolve_efi_config(&config, None, false).unwrap();
        assert!(!efi_config.certificate_path.starts_with("~"));
    }
}

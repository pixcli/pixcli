//! Configuration types for the Efí provider.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The environment to connect to (production or sandbox).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EfiEnvironment {
    /// Production environment.
    Production,
    /// Sandbox/homologation environment for testing.
    Sandbox,
}

impl EfiEnvironment {
    /// Returns the base URL for the Pix API in this environment.
    pub fn base_url(&self) -> &str {
        match self {
            EfiEnvironment::Production => "https://pix.api.efipay.com.br",
            EfiEnvironment::Sandbox => "https://pix-h.api.efipay.com.br",
        }
    }

    /// Returns the OAuth2 token endpoint URL.
    pub fn token_url(&self) -> String {
        format!("{}/oauth/token", self.base_url())
    }
}

impl std::fmt::Display for EfiEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EfiEnvironment::Production => write!(f, "production"),
            EfiEnvironment::Sandbox => write!(f, "sandbox"),
        }
    }
}

/// Configuration for the Efí provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfiConfig {
    /// OAuth2 client ID.
    pub client_id: String,
    /// OAuth2 client secret.
    #[serde(skip_serializing)]
    pub client_secret: String,
    /// Path to the PKCS#12 (.p12) certificate file for mTLS.
    pub certificate_path: PathBuf,
    /// Password for the PKCS#12 certificate (empty string if none).
    #[serde(default)]
    pub certificate_password: String,
    /// The environment to connect to.
    pub environment: EfiEnvironment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_urls() {
        let env = EfiEnvironment::Production;
        assert_eq!(env.base_url(), "https://pix.api.efipay.com.br");
        assert_eq!(env.token_url(), "https://pix.api.efipay.com.br/oauth/token");
    }

    #[test]
    fn test_sandbox_urls() {
        let env = EfiEnvironment::Sandbox;
        assert_eq!(env.base_url(), "https://pix-h.api.efipay.com.br");
        assert_eq!(
            env.token_url(),
            "https://pix-h.api.efipay.com.br/oauth/token"
        );
    }

    #[test]
    fn test_environment_display() {
        assert_eq!(EfiEnvironment::Production.to_string(), "production");
        assert_eq!(EfiEnvironment::Sandbox.to_string(), "sandbox");
    }

    #[test]
    fn test_config_serialize() {
        let config = EfiConfig {
            client_id: "test_id".to_string(),
            client_secret: "test_secret".to_string(),
            certificate_path: PathBuf::from("/path/to/cert.p12"),
            certificate_password: String::new(),
            environment: EfiEnvironment::Sandbox,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test_id"));
        assert!(json.contains("sandbox"));
        // client_secret should not appear in serialized output
        assert!(!json.contains("test_secret"));
    }

    #[test]
    fn test_config_deserialize() {
        let json = r#"{
            "client_id": "my_id",
            "client_secret": "my_secret",
            "certificate_path": "/certs/efi.p12",
            "environment": "production"
        }"#;
        let config: EfiConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.client_id, "my_id");
        assert_eq!(config.environment, EfiEnvironment::Production);
        assert_eq!(config.certificate_password, ""); // Default
    }
}

#[cfg(test)]
mod additional_config_tests {
    use super::*;

    #[test]
    fn test_environment_equality() {
        assert_eq!(EfiEnvironment::Production, EfiEnvironment::Production);
        assert_ne!(EfiEnvironment::Production, EfiEnvironment::Sandbox);
    }

    #[test]
    fn test_environment_serde_roundtrip() {
        for env in [EfiEnvironment::Production, EfiEnvironment::Sandbox] {
            let json = serde_json::to_string(&env).unwrap();
            let back: EfiEnvironment = serde_json::from_str(&json).unwrap();
            assert_eq!(back, env);
        }
    }

    #[test]
    fn test_config_with_password() {
        let json = r#"{
            "client_id": "id",
            "client_secret": "secret",
            "certificate_path": "/cert.p12",
            "certificate_password": "mypassword",
            "environment": "sandbox"
        }"#;
        let config: EfiConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.certificate_password, "mypassword");
    }

    #[test]
    fn test_config_clone() {
        let config = EfiConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            certificate_path: "/cert.p12".into(),
            certificate_password: "".to_string(),
            environment: EfiEnvironment::Sandbox,
        };
        let cloned = config.clone();
        assert_eq!(cloned.client_id, "id");
        assert_eq!(cloned.environment, EfiEnvironment::Sandbox);
    }

    #[test]
    fn test_production_base_url() {
        assert!(EfiEnvironment::Production
            .base_url()
            .starts_with("https://"));
        assert!(EfiEnvironment::Production.base_url().contains("pix.api"));
    }

    #[test]
    fn test_sandbox_base_url() {
        assert!(EfiEnvironment::Sandbox.base_url().starts_with("https://"));
        assert!(EfiEnvironment::Sandbox.base_url().contains("pix-h.api"));
    }

    #[test]
    fn test_token_url_contains_oauth() {
        assert!(EfiEnvironment::Production
            .token_url()
            .contains("/oauth/token"));
        assert!(EfiEnvironment::Sandbox.token_url().contains("/oauth/token"));
    }
}

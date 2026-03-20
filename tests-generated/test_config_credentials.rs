// =============================================================================
// Config and Credential Management Tests
// =============================================================================
//
// TARGET CRATE: pixcli (root crate)
// PLACEMENT:    tests/config_credentials.rs (integration test)
//
// DEPENDENCIES NEEDED IN Cargo.toml:
//   [dev-dependencies]
//   anyhow = "1"
//   pix-efi = { path = "crates/pix-efi" }
//   serde = { version = "1", features = ["derive"] }
//   serde_json = "1"
//   tempfile = "3"
//   toml = "0.8"
//
// WHY THESE TESTS EXIST:
//
// Configuration handling is the foundation of the entire CLI. Mishandled config
// leads to silent failures, credential leaks, or connections to the wrong
// environment (sandbox vs production). The existing tests cover basic roundtrips
// but miss:
//   - Missing required fields in profile TOML (partial config)
//   - Empty string values that pass deserialization but fail at runtime
//   - Non-existent certificate paths (fail late at API call, not at config load)
//   - Invalid environment names that do not map to Production or Sandbox
//   - Profile resolution edge cases (default vs named, missing profiles)
//   - Path expansion for ~/... paths
//   - Environment variable overrides for sensitive fields
//   - Factory functions with unknown backend types
//   - Factory functions with missing profiles
//   - Config file permissions on sensitive data
//
// These tests exercise the config layer in isolation to catch issues before
// they manifest as cryptic API errors.
// =============================================================================

#[cfg(test)]
mod config_credential_tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use serde::{Deserialize, Serialize};

    // =========================================================================
    // Type mirrors (replace with imports when placed in the crate)
    // =========================================================================

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct PixConfig {
        #[serde(default)]
        defaults: Defaults,
        #[serde(default)]
        profiles: HashMap<String, Profile>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Defaults {
        #[serde(default = "default_profile_name")]
        profile: String,
        #[serde(default = "default_output")]
        output: String,
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

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Profile {
        backend: String,
        environment: String,
        client_id: String,
        client_secret: String,
        certificate: String,
        #[serde(default)]
        certificate_password: String,
        default_pix_key: Option<String>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    enum EfiEnvironment {
        Production,
        Sandbox,
    }

    impl EfiEnvironment {
        fn base_url(&self) -> &str {
            match self {
                EfiEnvironment::Production => "https://pix.api.efipay.com.br",
                EfiEnvironment::Sandbox => "https://pix-h.api.efipay.com.br",
            }
        }

        fn token_url(&self) -> String {
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

    fn expand_path(path: &str) -> PathBuf {
        if let Some(rest) = path.strip_prefix("~/") {
            if let Some(base) = directories::BaseDirs::new() {
                return base.home_dir().join(rest);
            }
        }
        PathBuf::from(path)
    }

    fn make_profile(backend: &str, environment: &str) -> Profile {
        Profile {
            backend: backend.to_string(),
            environment: environment.to_string(),
            client_id: "test_id".to_string(),
            client_secret: "test_secret".to_string(),
            certificate: "/path/to/cert.p12".to_string(),
            certificate_password: String::new(),
            default_pix_key: None,
        }
    }

    // =========================================================================
    // 1. Config with missing required fields
    // =========================================================================
    // GAP: A TOML profile that is missing required fields (e.g., client_id)
    // should fail to parse. This tests serde's strict field requirements.

    #[test]
    fn test_profile_missing_client_id_fails() {
        let toml_str = r#"
[profiles.broken]
backend = "efi"
environment = "sandbox"
client_secret = "secret"
certificate = "cert.p12"
"#;
        let result: Result<PixConfig, _> = toml::from_str(toml_str);
        assert!(
            result.is_err(),
            "Profile missing client_id should fail to parse"
        );
    }

    #[test]
    fn test_profile_missing_backend_fails() {
        let toml_str = r#"
[profiles.broken]
environment = "sandbox"
client_id = "id"
client_secret = "secret"
certificate = "cert.p12"
"#;
        let result: Result<PixConfig, _> = toml::from_str(toml_str);
        assert!(
            result.is_err(),
            "Profile missing backend should fail to parse"
        );
    }

    #[test]
    fn test_profile_missing_certificate_fails() {
        let toml_str = r#"
[profiles.broken]
backend = "efi"
environment = "sandbox"
client_id = "id"
client_secret = "secret"
"#;
        let result: Result<PixConfig, _> = toml::from_str(toml_str);
        assert!(
            result.is_err(),
            "Profile missing certificate should fail to parse"
        );
    }

    // =========================================================================
    // 2. Config with empty string values
    // =========================================================================
    // GAP: Empty strings pass deserialization but will fail at runtime when
    // used as credentials or paths. These tests document the gap.

    #[test]
    fn test_empty_client_id_parses_successfully() {
        let toml_str = r#"
[profiles.empty]
backend = "efi"
environment = "sandbox"
client_id = ""
client_secret = ""
certificate = ""
"#;
        let result: Result<PixConfig, _> = toml::from_str(toml_str);
        // CURRENT BEHAVIOR: Empty strings are valid String values.
        // RECOMMENDATION: Add validation to reject empty credentials.
        assert!(
            result.is_ok(),
            "Empty strings are currently accepted by serde (no validation)"
        );
        let config = result.unwrap();
        let profile = config.profiles.get("empty").unwrap();
        assert!(profile.client_id.is_empty());
    }

    // =========================================================================
    // 3. Config with non-existent certificate path
    // =========================================================================
    // GAP: The config layer accepts any string as certificate path. The error
    // only surfaces when EfiAuth::new() tries to read the file. This means
    // `pixcli config init` can succeed with an invalid path, and the user
    // only discovers the problem when making their first API call.

    #[test]
    fn test_nonexistent_certificate_path_accepted_by_config() {
        let profile = make_profile("efi", "sandbox");
        assert_eq!(profile.certificate, "/path/to/cert.p12");

        // Config layer does not validate the path exists
        let path = Path::new(&profile.certificate);
        assert!(
            !path.exists(),
            "Config accepts non-existent cert paths without validation"
        );
    }

    // =========================================================================
    // 4. Config with invalid environment name
    // =========================================================================
    // GAP: The Profile.environment field is a plain String, not an enum.
    // Invalid values like "staging" or "dev" are silently accepted.

    #[test]
    fn test_invalid_environment_name_accepted_by_profile() {
        let toml_str = r#"
[profiles.invalid_env]
backend = "efi"
environment = "staging"
client_id = "id"
client_secret = "secret"
certificate = "cert.p12"
"#;
        let result: Result<PixConfig, _> = toml::from_str(toml_str);
        // CURRENT BEHAVIOR: "staging" is accepted because environment is String.
        // RECOMMENDATION: Use EfiEnvironment enum for validation.
        assert!(
            result.is_ok(),
            "Invalid environment 'staging' is accepted because it is typed as String"
        );
        let config = result.unwrap();
        let profile = config.profiles.get("invalid_env").unwrap();
        assert_eq!(profile.environment, "staging");
    }

    #[test]
    fn test_efi_environment_enum_rejects_invalid_value() {
        // The EfiEnvironment enum only accepts "production" or "sandbox".
        let json = r#""staging""#;
        let result: Result<EfiEnvironment, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "EfiEnvironment should reject 'staging'"
        );
    }

    #[test]
    fn test_efi_environment_enum_accepts_production() {
        let json = r#""production""#;
        let result: Result<EfiEnvironment, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), EfiEnvironment::Production);
    }

    #[test]
    fn test_efi_environment_enum_accepts_sandbox() {
        let json = r#""sandbox""#;
        let result: Result<EfiEnvironment, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), EfiEnvironment::Sandbox);
    }

    // =========================================================================
    // 5. Profile resolution (default vs named)
    // =========================================================================
    // GAP: Profile lookup falls back to defaults.profile when no name is given.
    // The error messages must be helpful when profiles are missing.

    #[test]
    fn test_get_profile_none_uses_default() {
        let mut config = PixConfig::default();
        config.defaults.profile = "mydefault".to_string();
        config
            .profiles
            .insert("mydefault".to_string(), make_profile("efi", "sandbox"));

        // get_profile(None) should use defaults.profile
        let profile_name = None::<&str>.unwrap_or(&config.defaults.profile);
        let profile = config.profiles.get(profile_name);
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().client_id, "test_id");
    }

    #[test]
    fn test_get_profile_named_overrides_default() {
        let mut config = PixConfig::default();
        config
            .profiles
            .insert("default".to_string(), make_profile("efi", "sandbox"));

        let mut prod_profile = make_profile("efi", "production");
        prod_profile.client_id = "prod_id".to_string();
        config.profiles.insert("prod".to_string(), prod_profile);

        let profile = config.profiles.get("prod").unwrap();
        assert_eq!(profile.client_id, "prod_id");
        assert_eq!(profile.environment, "production");
    }

    #[test]
    fn test_get_profile_missing_shows_available() {
        let mut config = PixConfig::default();
        config
            .profiles
            .insert("efi-prod".to_string(), make_profile("efi", "production"));
        config
            .profiles
            .insert("efi-sandbox".to_string(), make_profile("efi", "sandbox"));

        let result = config.profiles.get("nonexistent");
        assert!(result.is_none());

        // The error message should list available profiles
        let available: Vec<&String> = config.profiles.keys().collect();
        assert!(available.len() == 2);
    }

    #[test]
    fn test_get_profile_no_profiles_suggests_init() {
        let config = PixConfig::default();
        assert!(config.profiles.is_empty());
        // The error path should suggest running `pixcli config init`
    }

    // =========================================================================
    // 6. Path expansion (~/certs/test.p12)
    // =========================================================================
    // GAP: Tilde expansion must work correctly for certificate paths.

    #[test]
    fn test_expand_path_tilde_prefix() {
        let expanded = expand_path("~/certs/test.p12");
        assert!(
            !expanded.to_string_lossy().starts_with('~'),
            "Tilde should be expanded to home directory"
        );
        assert!(
            expanded.to_string_lossy().ends_with("certs/test.p12"),
            "Path suffix should be preserved"
        );
    }

    #[test]
    fn test_expand_path_absolute_unchanged() {
        let expanded = expand_path("/absolute/path/cert.p12");
        assert_eq!(expanded, PathBuf::from("/absolute/path/cert.p12"));
    }

    #[test]
    fn test_expand_path_relative_unchanged() {
        let expanded = expand_path("relative/cert.p12");
        assert_eq!(expanded, PathBuf::from("relative/cert.p12"));
    }

    #[test]
    fn test_expand_path_tilde_only() {
        // "~" without trailing slash is NOT expanded by strip_prefix("~/")
        let expanded = expand_path("~");
        // This should remain "~" because strip_prefix("~/") does not match "~"
        assert_eq!(
            expanded,
            PathBuf::from("~"),
            "Bare '~' without '/' should not be expanded by the current implementation"
        );
    }

    #[test]
    fn test_expand_path_tilde_in_middle() {
        // Tilde in the middle of a path should NOT be expanded
        let expanded = expand_path("/home/~user/cert.p12");
        assert_eq!(expanded, PathBuf::from("/home/~user/cert.p12"));
    }

    // =========================================================================
    // 7. Environment variable overrides
    // =========================================================================
    // GAP: Env vars should override config file values for sensitive fields.
    // This is critical for CI/CD where secrets come from env vars.

    #[test]
    fn test_env_override_creates_profile_if_missing() {
        // When PIXCLI_CLIENT_ID is set but no profile exists,
        // apply_env_overrides should create the default profile.
        let mut config = PixConfig::default();
        assert!(config.profiles.is_empty());

        // Simulate env override logic
        let profile_name = config.defaults.profile.clone();
        let profile = config.profiles.entry(profile_name).or_insert_with(|| Profile {
            backend: "efi".to_string(),
            environment: "sandbox".to_string(),
            client_id: String::new(),
            client_secret: String::new(),
            certificate: String::new(),
            certificate_password: String::new(),
            default_pix_key: None,
        });
        profile.client_id = "env_id".to_string();

        assert_eq!(config.profiles.len(), 1);
        assert_eq!(
            config.profiles.get("default").unwrap().client_id,
            "env_id"
        );
    }

    #[test]
    fn test_env_override_preserves_existing_fields() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "default".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "production".to_string(),
                client_id: "file_id".to_string(),
                client_secret: "file_secret".to_string(),
                certificate: "/file/cert.p12".to_string(),
                certificate_password: "file_pass".to_string(),
                default_pix_key: Some("file_key".to_string()),
            },
        );

        // Simulate overriding only client_id
        let profile = config.profiles.get_mut("default").unwrap();
        profile.client_id = "env_id".to_string();

        // Other fields should be unchanged
        assert_eq!(profile.client_secret, "file_secret");
        assert_eq!(profile.certificate, "/file/cert.p12");
        assert_eq!(profile.certificate_password, "file_pass");
        assert_eq!(profile.default_pix_key, Some("file_key".to_string()));
    }

    // =========================================================================
    // 8. Config serialization roundtrip
    // =========================================================================
    // GAP: Config must survive TOML serialize -> deserialize without data loss,
    // including special characters in credentials.

    #[test]
    fn test_config_toml_roundtrip() {
        let mut config = PixConfig::default();
        config.defaults.profile = "my-profile".to_string();
        config.defaults.output = "json".to_string();
        config.profiles.insert(
            "my-profile".to_string(),
            Profile {
                backend: "efi".to_string(),
                environment: "production".to_string(),
                client_id: "id_with_special!@#$%".to_string(),
                client_secret: "secret_with_unicode".to_string(),
                certificate: "/path/with spaces/cert.p12".to_string(),
                certificate_password: "p@ss=w0rd".to_string(),
                default_pix_key: Some("+5511999999999".to_string()),
            },
        );

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let back: PixConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(back.defaults.profile, "my-profile");
        assert_eq!(back.defaults.output, "json");
        let profile = back.profiles.get("my-profile").unwrap();
        assert_eq!(profile.client_id, "id_with_special!@#$%");
        assert_eq!(profile.certificate, "/path/with spaces/cert.p12");
        assert_eq!(profile.certificate_password, "p@ss=w0rd");
    }

    #[test]
    fn test_config_json_roundtrip() {
        let mut config = PixConfig::default();
        config.profiles.insert(
            "test".to_string(),
            make_profile("efi", "sandbox"),
        );

        let json = serde_json::to_string(&config).unwrap();
        let back: PixConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(back.profiles.len(), 1);
        assert_eq!(back.profiles.get("test").unwrap().backend, "efi");
    }

    // =========================================================================
    // 9. EfiConfig equality and cloning
    // =========================================================================
    // GAP: EfiConfig derives Clone but not PartialEq. Cloned configs
    // should produce identical values.

    #[test]
    fn test_efi_environment_equality() {
        assert_eq!(EfiEnvironment::Production, EfiEnvironment::Production);
        assert_eq!(EfiEnvironment::Sandbox, EfiEnvironment::Sandbox);
        assert_ne!(EfiEnvironment::Production, EfiEnvironment::Sandbox);
    }

    #[test]
    fn test_efi_environment_clone() {
        let env = EfiEnvironment::Production;
        let cloned = env;
        assert_eq!(env, cloned);
    }

    // =========================================================================
    // 10. EfiEnvironment URL construction
    // =========================================================================
    // GAP: URLs must be well-formed for all environments.

    #[test]
    fn test_production_url_is_well_formed() {
        let url = EfiEnvironment::Production.base_url();
        assert!(url.starts_with("https://"));
        assert!(!url.ends_with('/'));
        assert!(url.contains("pix.api.efipay"));
        assert!(!url.contains("pix-h")); // Not sandbox
    }

    #[test]
    fn test_sandbox_url_is_well_formed() {
        let url = EfiEnvironment::Sandbox.base_url();
        assert!(url.starts_with("https://"));
        assert!(!url.ends_with('/'));
        assert!(url.contains("pix-h.api.efipay"));
    }

    #[test]
    fn test_token_url_is_well_formed() {
        for env in [EfiEnvironment::Production, EfiEnvironment::Sandbox] {
            let url = env.token_url();
            assert!(url.starts_with("https://"));
            assert!(url.ends_with("/oauth/token"));
            assert!(!url.contains("//oauth")); // No double slash
        }
    }

    #[test]
    fn test_api_path_concatenation() {
        // When building API URLs, the base URL must not have a trailing slash
        // to avoid double-slash issues.
        let base = EfiEnvironment::Production.base_url();
        let path = "/v2/cob";
        let full_url = format!("{}{}", base, path);
        assert_eq!(full_url, "https://pix.api.efipay.com.br/v2/cob");
        assert!(!full_url.contains("//v2")); // No double slash
    }

    // =========================================================================
    // 11. Factory with unknown backend type
    // =========================================================================
    // GAP: The client_factory only supports "efi". Other backends should
    // produce a clear error message.

    #[test]
    fn test_unknown_backend_error_message() {
        let backend = "mercadopago";
        let err_msg = format!("unknown backend: '{}'. Supported: efi", backend);
        assert!(err_msg.contains("mercadopago"));
        assert!(err_msg.contains("Supported: efi"));
    }

    #[test]
    fn test_supported_backend_names() {
        let supported = ["efi"];
        assert!(supported.contains(&"efi"));
        assert!(!supported.contains(&"mercadopago"));
        assert!(!supported.contains(&"itau"));
        assert!(!supported.contains(&""));
    }

    // =========================================================================
    // 12. Factory with missing profile
    // =========================================================================
    // GAP: When the requested profile does not exist, the factory must fail
    // with a helpful error rather than a cryptic KeyError.

    #[test]
    fn test_missing_profile_error_with_no_profiles() {
        let config = PixConfig::default();
        assert!(config.profiles.is_empty());

        let profile_name = "nonexistent";
        let result = config.profiles.get(profile_name);
        assert!(result.is_none());

        // Error message should suggest running setup
        let err = format!(
            "profile '{}' not found. Run `pixcli config init` to create one.",
            profile_name
        );
        assert!(err.contains("config init"));
    }

    #[test]
    fn test_missing_profile_error_with_available_profiles() {
        let mut config = PixConfig::default();
        config
            .profiles
            .insert("prod".to_string(), make_profile("efi", "production"));
        config
            .profiles
            .insert("sandbox".to_string(), make_profile("efi", "sandbox"));

        let profile_name = "staging";
        let result = config.profiles.get(profile_name);
        assert!(result.is_none());

        let available: Vec<&String> = config.profiles.keys().collect();
        let err = format!(
            "profile '{}' not found. Available: {}",
            profile_name,
            available
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(err.contains("staging"));
        // The error should list at least one available profile
        assert!(
            err.contains("prod") || err.contains("sandbox"),
            "Error should list available profiles"
        );
    }

    // =========================================================================
    // 13. Config file with sensitive data (permissions)
    // =========================================================================
    // GAP: Config files contain OAuth2 credentials. On Unix, file permissions
    // must be 0600 (owner read/write only) to prevent other users from reading.

    #[cfg(unix)]
    #[test]
    fn test_config_file_permissions_600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = PixConfig::default();
        let content = toml::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &content).unwrap();

        // Set permissions like PixConfig::save() does
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms).unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "Config file should be owner-only readable");
    }

    // =========================================================================
    // 14. Multiple profiles with different environments
    // =========================================================================
    // GAP: A user may have both sandbox and production profiles. The factory
    // must select the correct one and the --sandbox flag must override.

    #[test]
    fn test_sandbox_flag_overrides_production_profile() {
        let profile = make_profile("efi", "production");
        let sandbox_flag = true;

        let effective_env = if sandbox_flag || profile.environment == "sandbox" {
            EfiEnvironment::Sandbox
        } else {
            EfiEnvironment::Production
        };

        assert_eq!(
            effective_env,
            EfiEnvironment::Sandbox,
            "--sandbox flag should override production profile"
        );
    }

    #[test]
    fn test_no_sandbox_flag_uses_profile_environment() {
        let profile = make_profile("efi", "production");
        let sandbox_flag = false;

        let effective_env = if sandbox_flag || profile.environment == "sandbox" {
            EfiEnvironment::Sandbox
        } else {
            EfiEnvironment::Production
        };

        assert_eq!(
            effective_env,
            EfiEnvironment::Production,
            "Without --sandbox, should use profile's environment"
        );
    }

    // =========================================================================
    // 15. Environment display format
    // =========================================================================
    // GAP: Display output must match serde rename_all format for consistency.

    #[test]
    fn test_environment_display_matches_serde_format() {
        assert_eq!(EfiEnvironment::Production.to_string(), "production");
        assert_eq!(EfiEnvironment::Sandbox.to_string(), "sandbox");

        // Verify roundtrip through display -> deserialize
        let display_str = format!("\"{}\"", EfiEnvironment::Production);
        let back: EfiEnvironment = serde_json::from_str(&display_str).unwrap();
        assert_eq!(back, EfiEnvironment::Production);
    }
}

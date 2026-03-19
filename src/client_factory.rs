//! Builds a `PixProvider` from CLI arguments and config.

use anyhow::Result;
use pix_efi::config::{EfiConfig, EfiEnvironment};
use pix_efi::EfiClient;
use pix_provider::PixProvider;

use crate::config::PixConfig;

/// Builds an appropriate `PixProvider` from the current config and CLI flags.
///
/// - `profile` — optional profile name override.
/// - `sandbox` — if `true`, forces sandbox environment regardless of profile setting.
pub fn build_provider(
    config: &PixConfig,
    profile_name: Option<&str>,
    sandbox: bool,
) -> Result<impl PixProvider> {
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

            let client = if let Some(ref key) = profile.default_pix_key {
                EfiClient::with_pix_key(efi_config, key.clone())?
            } else {
                EfiClient::new(efi_config)?
            };

            Ok(client)
        }
        other => anyhow::bail!("unknown backend: '{}'. Supported: efi", other),
    }
}

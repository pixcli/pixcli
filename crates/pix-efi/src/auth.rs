//! OAuth2 + mTLS authentication for the Efí Pix API.
//!
//! Handles token acquisition, caching, and automatic refresh using
//! the client credentials flow with mutual TLS.

use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Identity};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::config::EfiConfig;
use crate::EfiError;

/// OAuth2 token response from Efí.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
    scope: Option<String>,
}

/// A cached access token with its expiration time.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CachedToken {
    access_token: String,
    token_type: String,
    expires_at: DateTime<Utc>,
}

impl CachedToken {
    /// Returns `true` if the token is expired or will expire within the next 60 seconds.
    fn is_expired(&self) -> bool {
        Utc::now() + Duration::seconds(60) >= self.expires_at
    }
}

/// Manages OAuth2 authentication with token caching and automatic refresh.
#[derive(Clone)]
pub struct EfiAuth {
    config: EfiConfig,
    http_client: Client,
    cached_token: Arc<RwLock<Option<CachedToken>>>,
    base_url_override: Option<String>,
}

impl EfiAuth {
    /// Creates a new `EfiAuth` instance, loading the mTLS certificate.
    ///
    /// # Errors
    ///
    /// Returns `EfiError::CertificateError` if the certificate file cannot be loaded
    /// or is in an invalid format.
    pub fn new(config: EfiConfig) -> Result<Self, EfiError> {
        let cert_bytes = std::fs::read(&config.certificate_path).map_err(|e| {
            EfiError::CertificateError(format!(
                "failed to read certificate at '{}': {e}",
                config.certificate_path.display()
            ))
        })?;

        let identity = Identity::from_pkcs12_der(&cert_bytes, &config.certificate_password)
            .map_err(|e| {
                EfiError::CertificateError(format!("failed to parse PKCS#12 certificate: {e}"))
            })?;

        let http_client = Client::builder()
            .identity(identity)
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| EfiError::CertificateError(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            config,
            http_client,
            cached_token: Arc::new(RwLock::new(None)),
            base_url_override: None,
        })
    }

    /// Creates an `EfiAuth` with a pre-built HTTP client (useful for testing).
    #[doc(hidden)]
    pub fn with_client(config: EfiConfig, http_client: Client) -> Self {
        Self {
            config,
            http_client,
            cached_token: Arc::new(RwLock::new(None)),
            base_url_override: None,
        }
    }

    /// Creates an `EfiAuth` with a pre-built HTTP client and a base URL override (for mock servers).
    #[doc(hidden)]
    pub fn with_client_and_url(config: EfiConfig, http_client: Client, base_url: String) -> Self {
        Self {
            config,
            http_client,
            cached_token: Arc::new(RwLock::new(None)),
            base_url_override: Some(base_url),
        }
    }

    /// Returns a valid access token, refreshing it if necessary.
    ///
    /// This method is safe to call concurrently — it uses a read-write lock
    /// to avoid redundant token requests.
    ///
    /// # Errors
    ///
    /// Returns `EfiError::TokenError` if the token request fails.
    pub async fn get_token(&self) -> Result<String, EfiError> {
        // Fast path: check if we have a valid cached token
        {
            let cached = self.cached_token.read().await;
            if let Some(ref token) = *cached {
                if !token.is_expired() {
                    return Ok(token.access_token.clone());
                }
            }
        }

        // Slow path: acquire write lock and refresh
        let mut cached = self.cached_token.write().await;

        // Double-check after acquiring write lock (another task may have refreshed)
        if let Some(ref token) = *cached {
            if !token.is_expired() {
                return Ok(token.access_token.clone());
            }
        }

        debug!("refreshing OAuth2 token for Efí API");
        let new_token = self.request_token().await?;
        let access_token = new_token.access_token.clone();
        *cached = Some(new_token);

        Ok(access_token)
    }

    /// Returns a reference to the underlying HTTP client (with mTLS identity).
    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    /// Returns the base URL for the configured environment.
    pub fn base_url(&self) -> &str {
        if let Some(ref url) = self.base_url_override {
            return url.as_str();
        }
        self.config.environment.base_url()
    }

    /// Requests a new OAuth2 access token from Efí.
    async fn request_token(&self) -> Result<CachedToken, EfiError> {
        let credentials = format!("{}:{}", self.config.client_id, self.config.client_secret);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        let auth_header = format!("Basic {encoded}");

        let token_url = self
            .base_url_override
            .as_ref()
            .map(|url| format!("{url}/oauth/token"))
            .unwrap_or_else(|| self.config.environment.token_url());

        let response = self
            .http_client
            .post(&token_url)
            .header(AUTHORIZATION, &auth_header)
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({
                "grant_type": "client_credentials"
            }))
            .send()
            .await
            .map_err(|e| EfiError::TokenError(format!("token request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            warn!("OAuth2 token request failed with status {status}: {body}");
            return Err(EfiError::TokenError(format!(
                "token request returned {status}: {body}"
            )));
        }

        let token_resp: TokenResponse = response
            .json()
            .await
            .map_err(|e| EfiError::TokenError(format!("failed to parse token response: {e}")))?;

        let expires_at = Utc::now() + Duration::seconds(token_resp.expires_in);

        Ok(CachedToken {
            access_token: token_resp.access_token,
            token_type: token_resp.token_type,
            expires_at,
        })
    }
}

impl std::fmt::Debug for EfiAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EfiAuth")
            .field("environment", &self.config.environment)
            .field("client_id", &"***")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> EfiConfig {
        EfiConfig {
            client_id: "test_client_id".to_string(),
            client_secret: "test_client_secret".to_string(),
            certificate_path: "/nonexistent/cert.p12".into(),
            certificate_password: String::new(),
            environment: crate::config::EfiEnvironment::Sandbox,
        }
    }

    #[test]
    fn test_cached_token_not_expired() {
        let token = CachedToken {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn test_cached_token_expired() {
        let token = CachedToken {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() - Duration::seconds(1),
        };
        assert!(token.is_expired());
    }

    #[test]
    fn test_cached_token_about_to_expire() {
        // Token expiring within 60 seconds should be considered expired
        let token = CachedToken {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() + Duration::seconds(30),
        };
        assert!(token.is_expired());
    }

    #[test]
    fn test_auth_debug_hides_credentials() {
        let config = test_config();
        let auth = EfiAuth::with_client(config, Client::new());
        let debug = format!("{:?}", auth);
        assert!(!debug.contains("test_client_id"));
        assert!(!debug.contains("test_client_secret"));
        assert!(debug.contains("***"));
    }

    #[test]
    fn test_auth_certificate_not_found() {
        let config = test_config();
        let result = EfiAuth::new(config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EfiError::CertificateError(_)));
    }

    #[tokio::test]
    async fn test_get_token_no_cache() {
        let config = test_config();
        let auth = EfiAuth::with_client(config, Client::new());

        // Without a real server, this will fail — but we can verify the cache is empty
        let cached = auth.cached_token.read().await;
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_get_token_returns_cached() {
        let config = test_config();
        let auth = EfiAuth::with_client(config, Client::new());

        // Pre-populate the cache
        {
            let mut cached = auth.cached_token.write().await;
            *cached = Some(CachedToken {
                access_token: "cached_token_value".to_string(),
                token_type: "Bearer".to_string(),
                expires_at: Utc::now() + Duration::hours(1),
            });
        }

        let token = auth.get_token().await.unwrap();
        assert_eq!(token, "cached_token_value");
    }
}

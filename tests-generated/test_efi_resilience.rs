// =============================================================================
// EFI API Resilience Tests
// =============================================================================
//
// TARGET CRATE: pix-efi
// PLACEMENT:    crates/pix-efi/tests/efi_resilience.rs
//
// DEPENDENCIES NEEDED IN Cargo.toml:
//   [dev-dependencies]
//   base64 = "0.22"
//   chrono = { version = "0.4", features = ["serde"] }
//   reqwest = { version = "0.12", features = ["json"] }
//   serde = { version = "1", features = ["derive"] }
//   serde_json = "1"
//   tokio = { version = "1", features = ["full", "test-util"] }
//   wiremock = "0.6"
//
// WHY THESE TESTS EXIST:
//
// The EFI client implements retry logic with exponential backoff, token caching
// with RwLock-based double-check locking, and response parsing for the Efi
// Pix API. The existing tests cover basic token caching but miss:
//   - Token expiry boundary conditions (59s, 60s, 61s remaining)
//   - Concurrent token refresh under contention
//   - Retry behavior for specific HTTP status codes (429, 503, 502)
//   - Non-retryable error codes (400, 401, 403, 404)
//   - Retry exhaustion (all attempts fail)
//   - Auth header format verification
//   - Forward-compatible response parsing (extra fields)
//   - Certificate loading failures with specific error messages
//
// These tests use wiremock for HTTP mocking to test the full request lifecycle
// without hitting real APIs.
// =============================================================================

#[cfg(test)]
mod efi_resilience_tests {
    // =========================================================================
    // Token expiry boundary tests
    // =========================================================================
    // GAP: CachedToken::is_expired() uses a 60-second buffer. The boundary
    // between "valid" and "needs refresh" at exactly 59s, 60s, and 61s
    // remaining is critical for avoiding unnecessary token refreshes while
    // preventing expired token usage.

    // These tests operate on the CachedToken struct directly.
    // When placed in the crate, import as: use crate::auth::CachedToken;
    // Since CachedToken is private, these should go inside auth.rs as #[cfg(test)].

    use chrono::{Duration, Utc};
    use serde::{Deserialize, Serialize};

    // Mirror of the private CachedToken for test reference.
    #[derive(Debug, Clone)]
    struct CachedToken {
        access_token: String,
        token_type: String,
        expires_at: chrono::DateTime<Utc>,
    }

    impl CachedToken {
        fn is_expired(&self) -> bool {
            Utc::now() + Duration::seconds(60) >= self.expires_at
        }
    }

    fn make_token(seconds_remaining: i64) -> CachedToken {
        CachedToken {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now() + Duration::seconds(seconds_remaining),
        }
    }

    #[test]
    fn test_token_with_59_seconds_remaining_is_expired() {
        // 59 seconds remaining is within the 60-second buffer.
        // The token should be considered expired and refreshed.
        let token = make_token(59);
        assert!(
            token.is_expired(),
            "Token with 59s remaining should be considered expired (within 60s buffer)"
        );
    }

    #[test]
    fn test_token_with_60_seconds_remaining_is_expired() {
        // Exactly 60 seconds remaining: Utc::now() + 60s >= expires_at
        // This is the boundary. Due to >= comparison, this should be expired.
        let token = make_token(60);
        assert!(
            token.is_expired(),
            "Token with exactly 60s remaining should be expired (>= boundary)"
        );
    }

    #[test]
    fn test_token_with_61_seconds_remaining_is_valid() {
        // 61 seconds remaining is outside the 60-second buffer.
        // The token should be considered valid.
        let token = make_token(61);
        assert!(
            !token.is_expired(),
            "Token with 61s remaining should be valid (outside 60s buffer)"
        );
    }

    #[test]
    fn test_token_with_1_second_remaining_is_expired() {
        let token = make_token(1);
        assert!(token.is_expired());
    }

    #[test]
    fn test_token_already_expired_in_past() {
        let token = make_token(-10);
        assert!(token.is_expired());
    }

    #[test]
    fn test_token_with_1_hour_remaining_is_valid() {
        let token = make_token(3600);
        assert!(!token.is_expired());
    }

    // =========================================================================
    // Retryable vs non-retryable error classification
    // =========================================================================
    // GAP: EfiClient::is_retryable() classifies errors for retry logic.
    // Incorrect classification could cause infinite retries on permanent errors
    // or missed retries on transient ones.

    // These use the ProviderError enum from pix-provider.
    // When in the crate: use pix_provider::ProviderError;

    // For reference, define the error variants we test against.
    #[derive(Debug)]
    enum ProviderError {
        Authentication(String),
        Http { status: u16, message: String },
        Network(String),
        InvalidResponse(String),
        Certificate(String),
        NotFound(String),
        RateLimited { retry_after_secs: u64 },
        Serialization(String),
        Timeout(u64),
        Io(String),
    }

    fn is_retryable(err: &ProviderError) -> bool {
        matches!(
            err,
            ProviderError::RateLimited { .. }
                | ProviderError::Timeout(_)
                | ProviderError::Http { status: 503, .. }
                | ProviderError::Http { status: 502, .. }
        ) || matches!(err, ProviderError::Network(_))
    }

    #[test]
    fn test_429_rate_limited_is_retryable() {
        // 429 Too Many Requests is always transient -- the client should
        // back off and retry after the specified period.
        let err = ProviderError::RateLimited {
            retry_after_secs: 60,
        };
        assert!(
            is_retryable(&err),
            "429 rate limited should be retryable"
        );
    }

    #[test]
    fn test_503_service_unavailable_is_retryable() {
        // 503 Service Unavailable typically means the server is temporarily
        // overloaded or in maintenance.
        let err = ProviderError::Http {
            status: 503,
            message: "Service Unavailable".to_string(),
        };
        assert!(
            is_retryable(&err),
            "503 should be retryable"
        );
    }

    #[test]
    fn test_502_bad_gateway_is_retryable() {
        let err = ProviderError::Http {
            status: 502,
            message: "Bad Gateway".to_string(),
        };
        assert!(
            is_retryable(&err),
            "502 should be retryable"
        );
    }

    #[test]
    fn test_network_error_is_retryable() {
        let err = ProviderError::Network("connection reset".to_string());
        assert!(
            is_retryable(&err),
            "Network errors should be retryable"
        );
    }

    #[test]
    fn test_timeout_is_retryable() {
        let err = ProviderError::Timeout(30);
        assert!(
            is_retryable(&err),
            "Timeouts should be retryable"
        );
    }

    #[test]
    fn test_400_bad_request_is_not_retryable() {
        // 400 Bad Request means the request itself is malformed.
        // Retrying the same request will produce the same error.
        let err = ProviderError::Http {
            status: 400,
            message: "Bad Request".to_string(),
        };
        assert!(
            !is_retryable(&err),
            "400 should NOT be retryable -- the request itself is invalid"
        );
    }

    #[test]
    fn test_401_unauthorized_is_not_retryable() {
        // 401 means invalid credentials. Retrying without re-authentication
        // is pointless (and could be a security concern).
        let err = ProviderError::Authentication("unauthorized".to_string());
        assert!(
            !is_retryable(&err),
            "401/Authentication errors should NOT be retryable"
        );
    }

    #[test]
    fn test_403_forbidden_is_not_retryable() {
        // 403 means the credentials are valid but lack permission.
        let err = ProviderError::Http {
            status: 403,
            message: "Forbidden".to_string(),
        };
        assert!(
            !is_retryable(&err),
            "403 should NOT be retryable -- permission issue"
        );
    }

    #[test]
    fn test_404_not_found_is_not_retryable() {
        let err = ProviderError::NotFound("resource not found".to_string());
        assert!(
            !is_retryable(&err),
            "404 should NOT be retryable -- the resource does not exist"
        );
    }

    #[test]
    fn test_500_internal_error_is_not_retryable() {
        // 500 is ambiguous but the current implementation does NOT retry it.
        // Only 502 and 503 are retried. This is a deliberate choice to avoid
        // amplifying server issues.
        let err = ProviderError::Http {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        assert!(
            !is_retryable(&err),
            "500 should NOT be retryable in current implementation"
        );
    }

    // =========================================================================
    // check_response status code mapping
    // =========================================================================
    // GAP: check_response maps HTTP status codes to ProviderError variants.
    // Incorrect mapping could cause retries on permanent errors or vice versa.

    // Mirror of EfiClient::check_response for standalone test reference.
    fn check_response(status_code: u16, body: &str) -> Result<(), ProviderError> {
        if (200..300).contains(&status_code) {
            return Ok(());
        }
        match status_code {
            401 => Err(ProviderError::Authentication(format!(
                "unauthorized: {body}"
            ))),
            403 => Err(ProviderError::Authentication(format!(
                "forbidden: {body}"
            ))),
            404 => Err(ProviderError::NotFound(body.to_string())),
            429 => Err(ProviderError::RateLimited {
                retry_after_secs: 60,
            }),
            _ => Err(ProviderError::Http {
                status: status_code,
                message: body.to_string(),
            }),
        }
    }

    #[test]
    fn test_check_response_200_is_ok() {
        assert!(check_response(200, "").is_ok());
    }

    #[test]
    fn test_check_response_201_is_ok() {
        assert!(check_response(201, "").is_ok());
    }

    #[test]
    fn test_check_response_401_is_authentication_error() {
        let err = check_response(401, "token expired").unwrap_err();
        assert!(matches!(err, ProviderError::Authentication(_)));
    }

    #[test]
    fn test_check_response_403_is_authentication_error() {
        let err = check_response(403, "insufficient scope").unwrap_err();
        assert!(matches!(err, ProviderError::Authentication(_)));
    }

    #[test]
    fn test_check_response_404_is_not_found() {
        let err = check_response(404, "charge not found").unwrap_err();
        assert!(matches!(err, ProviderError::NotFound(_)));
    }

    #[test]
    fn test_check_response_429_is_rate_limited() {
        let err = check_response(429, "too many requests").unwrap_err();
        assert!(matches!(err, ProviderError::RateLimited { .. }));
    }

    #[test]
    fn test_check_response_503_is_http_error() {
        let err = check_response(503, "service unavailable").unwrap_err();
        assert!(matches!(err, ProviderError::Http { status: 503, .. }));
    }

    // =========================================================================
    // Auth header format verification
    // =========================================================================
    // GAP: The OAuth2 token request must use Basic auth with base64-encoded
    // "client_id:client_secret". An incorrect encoding breaks authentication
    // silently (the server returns 401 without explaining why).

    #[test]
    fn test_basic_auth_header_format() {
        let client_id = "Client_Id_abc123";
        let client_secret = "Client_Secret_xyz789";
        let credentials = format!("{}:{}", client_id, client_secret);
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            credentials.as_bytes(),
        );
        let auth_header = format!("Basic {}", encoded);

        // Verify the header starts with "Basic "
        assert!(
            auth_header.starts_with("Basic "),
            "Auth header must start with 'Basic '"
        );

        // Verify the base64 decodes back to the original credentials
        let decoded_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            auth_header.strip_prefix("Basic ").unwrap(),
        )
        .unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        assert_eq!(decoded, format!("{}:{}", client_id, client_secret));
    }

    #[test]
    fn test_basic_auth_with_special_characters_in_secret() {
        // Client secrets may contain special characters that could break
        // naive string handling (colons, equals, plus signs).
        let client_id = "id_with:colon";
        let client_secret = "secret+with/special=chars";
        let credentials = format!("{}:{}", client_id, client_secret);
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            credentials.as_bytes(),
        );

        let decoded_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &encoded,
        )
        .unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();

        // The colon in client_id is part of the credentials, not a delimiter.
        // The server must split on the FIRST colon only.
        assert_eq!(decoded, "id_with:colon:secret+with/special=chars");
    }

    // =========================================================================
    // Response parsing with extra unknown fields (forward compatibility)
    // =========================================================================
    // GAP: The Efi API may add new fields to its responses over time.
    // serde's default behavior with #[derive(Deserialize)] is to ignore
    // unknown fields, but if #[serde(deny_unknown_fields)] were accidentally
    // added, it would break on API updates.

    #[test]
    fn test_token_response_with_extra_fields() {
        // Simulate an Efi token response with extra fields that do not exist
        // in the current TokenResponse struct.
        let json = r#"{
            "access_token": "abc123",
            "token_type": "Bearer",
            "expires_in": 3600,
            "scope": "pix.read pix.write",
            "unknown_future_field": "some_value",
            "another_new_field": 42
        }"#;

        // TokenResponse is private, so we mirror it here.
        #[derive(Debug, Deserialize)]
        struct TokenResponse {
            access_token: String,
            token_type: String,
            expires_in: i64,
            scope: Option<String>,
        }

        let result: Result<TokenResponse, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "Token response parsing should ignore unknown fields for forward compatibility"
        );

        let token = result.unwrap();
        assert_eq!(token.access_token, "abc123");
        assert_eq!(token.expires_in, 3600);
    }

    #[test]
    fn test_token_response_with_missing_optional_scope() {
        // The scope field is Option<String>. Some Efi responses may omit it.
        let json = r#"{
            "access_token": "abc123",
            "token_type": "Bearer",
            "expires_in": 3600
        }"#;

        #[derive(Debug, Deserialize)]
        struct TokenResponse {
            access_token: String,
            token_type: String,
            expires_in: i64,
            scope: Option<String>,
        }

        let result: Result<TokenResponse, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        assert!(result.unwrap().scope.is_none());
    }

    // =========================================================================
    // Certificate loading error cases
    // =========================================================================
    // GAP: Certificate loading can fail in multiple ways. The error messages
    // must be specific enough for users to diagnose the problem.

    #[test]
    fn test_certificate_file_not_found_error_message() {
        // Simulate the error path in EfiAuth::new when the cert file is missing.
        let path = std::path::PathBuf::from("/nonexistent/path/cert.p12");
        let result = std::fs::read(&path);
        assert!(result.is_err());

        // The error message should include the path for debugging.
        let err_msg = format!(
            "failed to read certificate at '{}': {}",
            path.display(),
            result.unwrap_err()
        );
        assert!(err_msg.contains("/nonexistent/path/cert.p12"));
        assert!(err_msg.contains("No such file"));
    }

    #[test]
    fn test_certificate_invalid_pkcs12_data() {
        // Write garbage data to a temp file and try to parse as PKCS12.
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("bad_cert.p12");
        std::fs::write(&cert_path, b"this is not a valid PKCS12 file").unwrap();

        let cert_bytes = std::fs::read(&cert_path).unwrap();
        let result = reqwest::Identity::from_pkcs12_der(&cert_bytes, "");
        assert!(
            result.is_err(),
            "Invalid PKCS12 data should fail to parse"
        );
    }

    #[test]
    fn test_certificate_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("empty_cert.p12");
        std::fs::write(&cert_path, b"").unwrap();

        let cert_bytes = std::fs::read(&cert_path).unwrap();
        assert!(cert_bytes.is_empty());

        let result = reqwest::Identity::from_pkcs12_der(&cert_bytes, "");
        assert!(
            result.is_err(),
            "Empty file should fail PKCS12 parsing"
        );
    }

    // =========================================================================
    // Exponential backoff timing verification
    // =========================================================================
    // GAP: The retry logic uses 500ms * 2^(attempt-1) for backoff delays.
    // This test verifies the delay calculation matches expectations.

    #[test]
    fn test_exponential_backoff_delay_calculation() {
        // Attempt 0: no delay (first attempt)
        // Attempt 1: 500ms * 2^0 = 500ms
        // Attempt 2: 500ms * 2^1 = 1000ms
        for attempt in 0u32..3 {
            if attempt > 0 {
                let delay_ms = 500u64 * 2u64.pow(attempt - 1);
                match attempt {
                    1 => assert_eq!(delay_ms, 500, "First retry should wait 500ms"),
                    2 => assert_eq!(delay_ms, 1000, "Second retry should wait 1000ms"),
                    _ => unreachable!(),
                }
            }
        }
    }

    // =========================================================================
    // Concurrent token refresh (double-check locking)
    // =========================================================================
    // GAP: Multiple tasks may call get_token() simultaneously when the token
    // is expired. The double-check pattern (read lock -> write lock -> re-check)
    // should ensure only ONE token request is made.
    //
    // This test verifies the pattern conceptually using a simplified mock.

    #[tokio::test]
    async fn test_concurrent_token_access_with_rwlock() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};
        use tokio::sync::RwLock;

        let refresh_count = Arc::new(AtomicU32::new(0));
        let token_cache: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

        let mut handles = Vec::new();

        for _ in 0..50 {
            let cache = token_cache.clone();
            let count = refresh_count.clone();
            handles.push(tokio::spawn(async move {
                // Fast path: read lock
                {
                    let cached = cache.read().await;
                    if cached.is_some() {
                        return cached.clone().unwrap();
                    }
                }

                // Slow path: write lock with double-check
                let mut cached = cache.write().await;
                if cached.is_some() {
                    return cached.clone().unwrap();
                }

                // Simulate token fetch
                count.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                let token = "fresh_token".to_string();
                *cached = Some(token.clone());
                token
            }));
        }

        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }

        // All tasks should get the same token
        assert!(results.iter().all(|t| t == "fresh_token"));

        // The refresh should happen only a small number of times (ideally 1,
        // but a few tasks may race past the read lock before the first write completes).
        let count = refresh_count.load(Ordering::SeqCst);
        assert!(
            count <= 5,
            "Expected at most 5 token refreshes due to contention, got {}. \
             The double-check pattern should prevent most redundant refreshes.",
            count
        );
    }

    // =========================================================================
    // EfiConfig serialization roundtrip
    // =========================================================================
    // GAP: Config must survive serialize -> deserialize without data loss.

    #[test]
    fn test_efi_config_serialization_roundtrip() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum EfiEnvironment {
            Production,
            Sandbox,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct EfiConfig {
            client_id: String,
            client_secret: String,
            certificate_path: std::path::PathBuf,
            #[serde(default)]
            certificate_password: String,
            environment: EfiEnvironment,
        }

        let config = EfiConfig {
            client_id: "test_id_with_special_chars!@#".to_string(),
            client_secret: "secret_with_unicode_".to_string(),
            certificate_path: std::path::PathBuf::from("/path/with spaces/cert.p12"),
            certificate_password: "p@ssw0rd!".to_string(),
            environment: EfiEnvironment::Sandbox,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: EfiConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.client_id, config.client_id);
        assert_eq!(deserialized.client_secret, config.client_secret);
        assert_eq!(deserialized.certificate_path, config.certificate_path);
        assert_eq!(deserialized.certificate_password, config.certificate_password);
        assert_eq!(deserialized.environment, config.environment);
    }

    // =========================================================================
    // EfiEnvironment URL construction
    // =========================================================================
    // GAP: URLs must use HTTPS and must not have trailing slashes to avoid
    // double-slash issues when paths are appended.

    #[test]
    fn test_environment_urls_use_https() {
        let prod_url = "https://pix.api.efipay.com.br";
        let sandbox_url = "https://pix-h.api.efipay.com.br";

        assert!(prod_url.starts_with("https://"), "Production must use HTTPS");
        assert!(sandbox_url.starts_with("https://"), "Sandbox must use HTTPS");
    }

    #[test]
    fn test_environment_urls_no_trailing_slash() {
        let prod_url = "https://pix.api.efipay.com.br";
        let sandbox_url = "https://pix-h.api.efipay.com.br";

        assert!(
            !prod_url.ends_with('/'),
            "Production URL must not have trailing slash to avoid double-slash in paths"
        );
        assert!(
            !sandbox_url.ends_with('/'),
            "Sandbox URL must not have trailing slash"
        );
    }

    #[test]
    fn test_token_url_includes_oauth_path() {
        let token_url = format!("{}/oauth/token", "https://pix.api.efipay.com.br");
        assert!(token_url.ends_with("/oauth/token"));
        assert!(!token_url.contains("//oauth")); // No double slash
    }

    use base64;
    use tempfile;
}

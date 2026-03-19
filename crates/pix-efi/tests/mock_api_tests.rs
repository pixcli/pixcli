//! Integration tests for the Efí API client.
//!
//! Since `EfiAuth` requires a real mTLS certificate, these tests focus on
//! validation logic, response checking, and retry classification — all of
//! which can be tested without a live server.

#[cfg(test)]
mod validation_tests {
    use pix_efi::validate;

    #[test]
    fn test_valid_txid_exact_26() {
        let txid = "a".repeat(26);
        assert!(validate::validate_txid(&txid).is_ok());
    }

    #[test]
    fn test_valid_txid_exact_35() {
        let txid = "a".repeat(35);
        assert!(validate::validate_txid(&txid).is_ok());
    }

    #[test]
    fn test_txid_25_chars_fails() {
        let txid = "a".repeat(25);
        assert!(validate::validate_txid(&txid).is_err());
    }

    #[test]
    fn test_txid_36_chars_fails() {
        let txid = "a".repeat(36);
        assert!(validate::validate_txid(&txid).is_err());
    }

    #[test]
    fn test_txid_with_hyphens_fails() {
        assert!(validate::validate_txid("pix550e8400-e29b-41d4-a716-4466").is_err());
    }

    #[test]
    fn test_txid_with_underscores_fails() {
        assert!(validate::validate_txid("pix550e8400_e29b_41d4_a716_4").is_err());
    }

    #[test]
    fn test_amount_valid_cents() {
        assert!(validate::validate_amount("0.01").is_ok());
    }

    #[test]
    fn test_amount_valid_large() {
        assert!(validate::validate_amount("999999.99").is_ok());
    }

    #[test]
    fn test_amount_no_decimal_point() {
        assert!(validate::validate_amount("100").is_err());
    }

    #[test]
    fn test_amount_one_decimal() {
        assert!(validate::validate_amount("10.5").is_err());
    }

    #[test]
    fn test_amount_three_decimals() {
        assert!(validate::validate_amount("10.500").is_err());
    }

    #[test]
    fn test_amount_zero_fails() {
        assert!(validate::validate_amount("0.00").is_err());
    }

    #[test]
    fn test_amount_letters_fail() {
        assert!(validate::validate_amount("abc.de").is_err());
    }

    #[test]
    fn test_e2eid_valid() {
        assert!(validate::validate_e2eid("E12345678901234567890123456789AB").is_ok());
    }

    #[test]
    fn test_e2eid_lowercase_prefix_fails() {
        assert!(validate::validate_e2eid("e12345678901234567890123456789AB").is_err());
    }

    #[test]
    fn test_e2eid_31_chars_fails() {
        assert!(validate::validate_e2eid("E1234567890123456789012345678AB").is_err());
    }

    #[test]
    fn test_e2eid_33_chars_fails() {
        assert!(validate::validate_e2eid("E123456789012345678901234567890AB").is_err());
    }
}

#[cfg(test)]
mod response_check_tests {
    use pix_efi::client::EfiClient;
    use pix_provider::ProviderError;
    use reqwest::StatusCode;

    #[test]
    fn test_200_ok() {
        assert!(EfiClient::check_response(StatusCode::OK, "").is_ok());
    }

    #[test]
    fn test_201_created() {
        assert!(EfiClient::check_response(StatusCode::CREATED, "").is_ok());
    }

    #[test]
    fn test_401_unauthorized() {
        let err = EfiClient::check_response(StatusCode::UNAUTHORIZED, "bad token").unwrap_err();
        assert!(matches!(err, ProviderError::Authentication(_)));
    }

    #[test]
    fn test_403_forbidden() {
        let err = EfiClient::check_response(StatusCode::FORBIDDEN, "no scope").unwrap_err();
        assert!(matches!(err, ProviderError::Authentication(_)));
    }

    #[test]
    fn test_404_not_found() {
        let err = EfiClient::check_response(StatusCode::NOT_FOUND, "gone").unwrap_err();
        assert!(matches!(err, ProviderError::NotFound(_)));
    }

    #[test]
    fn test_429_rate_limited() {
        let err =
            EfiClient::check_response(StatusCode::TOO_MANY_REQUESTS, "slow down").unwrap_err();
        match err {
            ProviderError::RateLimited { retry_after_secs } => {
                assert_eq!(retry_after_secs, 60);
            }
            _ => panic!("expected RateLimited, got {err:?}"),
        }
    }

    #[test]
    fn test_500_server_error() {
        let err = EfiClient::check_response(StatusCode::INTERNAL_SERVER_ERROR, "boom").unwrap_err();
        match err {
            ProviderError::Http { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "boom");
            }
            _ => panic!("expected Http, got {err:?}"),
        }
    }

    #[test]
    fn test_503_service_unavailable() {
        let err =
            EfiClient::check_response(StatusCode::SERVICE_UNAVAILABLE, "maintenance").unwrap_err();
        match err {
            ProviderError::Http { status, .. } => assert_eq!(status, 503),
            _ => panic!("expected Http, got {err:?}"),
        }
    }
}

#[cfg(test)]
mod retryable_tests {
    use pix_efi::client::EfiClient;
    use pix_provider::ProviderError;

    #[test]
    fn test_rate_limited_is_retryable() {
        let err = ProviderError::RateLimited {
            retry_after_secs: 60,
        };
        assert!(EfiClient::is_retryable(&err));
    }

    #[test]
    fn test_timeout_is_retryable() {
        assert!(EfiClient::is_retryable(&ProviderError::Timeout(30)));
    }

    #[test]
    fn test_503_is_retryable() {
        assert!(EfiClient::is_retryable(&ProviderError::Http {
            status: 503,
            message: "maintenance".to_string(),
        }));
    }

    #[test]
    fn test_502_is_retryable() {
        assert!(EfiClient::is_retryable(&ProviderError::Http {
            status: 502,
            message: "bad gateway".to_string(),
        }));
    }

    #[test]
    fn test_network_error_is_retryable() {
        assert!(EfiClient::is_retryable(&ProviderError::Network(
            "connection reset".to_string()
        )));
    }

    #[test]
    fn test_auth_error_not_retryable() {
        assert!(!EfiClient::is_retryable(&ProviderError::Authentication(
            "bad token".to_string()
        )));
    }

    #[test]
    fn test_not_found_not_retryable() {
        assert!(!EfiClient::is_retryable(&ProviderError::NotFound(
            "gone".to_string()
        )));
    }

    #[test]
    fn test_400_not_retryable() {
        assert!(!EfiClient::is_retryable(&ProviderError::Http {
            status: 400,
            message: "bad request".to_string(),
        }));
    }
}

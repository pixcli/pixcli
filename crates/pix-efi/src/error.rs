//! Error types specific to the Efí provider.

use pix_provider::ProviderError;
use thiserror::Error;

/// Errors specific to the Efí provider.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EfiError {
    /// OAuth2 token request failed.
    #[error("OAuth2 token error: {0}")]
    TokenError(String),

    /// Certificate loading failed.
    #[error("certificate error: {0}")]
    CertificateError(String),

    /// HTTP request error.
    #[error("request error: {0}")]
    RequestError(String),

    /// JSON parsing error.
    #[error("JSON error: {0}")]
    JsonError(String),

    /// I/O error (file read, etc.).
    #[error("I/O error: {0}")]
    IoError(String),
}

impl From<reqwest::Error> for EfiError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_connect() {
            EfiError::RequestError(format!("connection failed: {err}"))
        } else if err.is_timeout() {
            EfiError::RequestError(format!("request timed out: {err}"))
        } else {
            EfiError::RequestError(err.to_string())
        }
    }
}

impl From<std::io::Error> for EfiError {
    fn from(err: std::io::Error) -> Self {
        EfiError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for EfiError {
    fn from(err: serde_json::Error) -> Self {
        EfiError::JsonError(err.to_string())
    }
}

impl From<EfiError> for ProviderError {
    fn from(err: EfiError) -> Self {
        match err {
            EfiError::TokenError(msg) => ProviderError::Authentication(msg),
            EfiError::CertificateError(msg) => ProviderError::Certificate(msg),
            EfiError::RequestError(msg) => ProviderError::Network(msg),
            EfiError::JsonError(msg) => ProviderError::Serialization(msg),
            EfiError::IoError(msg) => ProviderError::Io(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_efi_error_display() {
        let err = EfiError::TokenError("invalid credentials".into());
        assert_eq!(err.to_string(), "OAuth2 token error: invalid credentials");
    }

    #[test]
    fn test_efi_error_to_provider_error() {
        let efi_err = EfiError::TokenError("expired".into());
        let provider_err: ProviderError = efi_err.into();
        assert!(matches!(provider_err, ProviderError::Authentication(_)));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let efi_err = EfiError::from(io_err);
        assert!(matches!(efi_err, EfiError::IoError(_)));
    }

    #[test]
    fn test_certificate_error_to_provider() {
        let efi_err = EfiError::CertificateError("bad cert".into());
        let provider_err: ProviderError = efi_err.into();
        assert!(matches!(provider_err, ProviderError::Certificate(_)));
    }
}

#[cfg(test)]
mod additional_error_tests {
    use super::*;

    #[test]
    fn test_token_error_display() {
        let err = EfiError::TokenError("expired".into());
        assert!(err.to_string().contains("expired"));
    }

    #[test]
    fn test_request_error_display() {
        let err = EfiError::RequestError("connection refused".into());
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn test_json_error_display() {
        let err = EfiError::JsonError("parse error".into());
        assert!(err.to_string().contains("parse error"));
    }

    #[test]
    fn test_json_error_conversion() {
        let json_err: serde_json::Error = serde_json::from_str::<String>("invalid").unwrap_err();
        let efi_err = EfiError::from(json_err);
        assert!(matches!(efi_err, EfiError::JsonError(_)));
    }

    #[test]
    fn test_request_error_to_provider() {
        let efi_err = EfiError::RequestError("timeout".into());
        let provider_err: ProviderError = efi_err.into();
        assert!(matches!(provider_err, ProviderError::Network(_)));
    }

    #[test]
    fn test_json_error_to_provider() {
        let efi_err = EfiError::JsonError("parse".into());
        let provider_err: ProviderError = efi_err.into();
        assert!(matches!(provider_err, ProviderError::Serialization(_)));
    }

    #[test]
    fn test_io_error_to_provider() {
        let efi_err = EfiError::IoError("not found".into());
        let provider_err: ProviderError = efi_err.into();
        assert!(matches!(provider_err, ProviderError::Io(_)));
    }

    #[test]
    fn test_certificate_error_display() {
        let err = EfiError::CertificateError("invalid format".into());
        assert!(err.to_string().contains("invalid format"));
        assert!(err.to_string().contains("certificate error"));
    }

    #[test]
    fn test_io_error_display() {
        let err = EfiError::IoError("permission denied".into());
        assert!(err.to_string().contains("permission denied"));
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn test_io_error_from_various_kinds() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
        let efi_err = EfiError::from(io_err);
        assert!(matches!(efi_err, EfiError::IoError(_)));
        assert!(efi_err.to_string().contains("no access"));
    }

    #[test]
    fn test_efi_error_is_debug() {
        let err = EfiError::TokenError("debug test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("TokenError"));
        assert!(debug.contains("debug test"));
    }

    #[test]
    fn test_certificate_error_to_provider_preserves_msg() {
        let efi_err = EfiError::CertificateError("cert invalid".into());
        let provider_err: ProviderError = efi_err.into();
        assert!(provider_err.to_string().contains("cert invalid"));
    }

    #[test]
    fn test_request_error_to_provider_preserves_msg() {
        let efi_err = EfiError::RequestError("server down".into());
        let provider_err: ProviderError = efi_err.into();
        assert!(provider_err.to_string().contains("server down"));
    }

    #[test]
    fn test_json_serde_error_conversion_preserves_msg() {
        let json_err: serde_json::Error = serde_json::from_str::<i32>("not a number").unwrap_err();
        let efi_err = EfiError::from(json_err);
        let provider_err: ProviderError = efi_err.into();
        assert!(matches!(provider_err, ProviderError::Serialization(_)));
    }
}

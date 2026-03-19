//! Error types specific to the Efí provider.

use pix_provider::ProviderError;
use thiserror::Error;

/// Errors specific to the Efí provider.
#[derive(Debug, Error)]
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

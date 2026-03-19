//! Common error types for the Pix ecosystem.

use thiserror::Error;

/// Top-level error type for all Pix operations.
#[derive(Debug, Error)]
pub enum PixError {
    /// Invalid Pix key format or check digit.
    #[error("invalid pix key: {0}")]
    InvalidPixKey(String),

    /// Invalid BRCode payload.
    #[error("invalid brcode: {0}")]
    InvalidBrCode(String),

    /// CRC checksum mismatch.
    #[error("CRC mismatch: expected {expected}, got {actual}")]
    CrcMismatch {
        /// Expected CRC value.
        expected: String,
        /// Actual CRC value.
        actual: String,
    },

    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Generic validation error.
    #[error("validation error: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_pix_key_display() {
        let err = PixError::InvalidPixKey("bad key".into());
        assert_eq!(err.to_string(), "invalid pix key: bad key");
    }

    #[test]
    fn test_invalid_brcode_display() {
        let err = PixError::InvalidBrCode("bad payload".into());
        assert_eq!(err.to_string(), "invalid brcode: bad payload");
    }

    #[test]
    fn test_crc_mismatch_display() {
        let err = PixError::CrcMismatch {
            expected: "ABCD".into(),
            actual: "1234".into(),
        };
        let s = err.to_string();
        assert!(s.contains("ABCD"));
        assert!(s.contains("1234"));
    }

    #[test]
    fn test_serialization_error_display() {
        let err = PixError::Serialization("parse failed".into());
        assert_eq!(err.to_string(), "serialization error: parse failed");
    }

    #[test]
    fn test_validation_error_display() {
        let err = PixError::Validation("field too long".into());
        assert_eq!(err.to_string(), "validation error: field too long");
    }

    #[test]
    fn test_error_is_debug() {
        let err = PixError::InvalidPixKey("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidPixKey"));
    }
}

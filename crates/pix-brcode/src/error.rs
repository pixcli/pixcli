//! Error types for BRCode encoding and decoding.

use thiserror::Error;

/// Errors that can occur during BRCode encoding or decoding.
#[derive(Debug, Error)]
pub enum BrCodeError {
    /// A field exceeds the maximum allowed length.
    #[error("field '{field}' is too long: max {max}, got {actual}")]
    FieldTooLong {
        /// Name of the field.
        field: String,
        /// Maximum allowed length.
        max: usize,
        /// Actual length.
        actual: usize,
    },

    /// The payload is incomplete or malformed.
    #[error("malformed TLV payload: {0}")]
    MalformedTlv(String),

    /// A required tag is missing from the payload.
    #[error("missing required tag: {0}")]
    MissingTag(String),

    /// CRC checksum mismatch during decoding.
    #[error("CRC mismatch: expected {expected}, got {actual}")]
    CrcMismatch {
        /// Expected CRC value.
        expected: String,
        /// Actual CRC value.
        actual: String,
    },

    /// Invalid field value.
    #[error("invalid field value for '{field}': {reason}")]
    InvalidField {
        /// Name of the field.
        field: String,
        /// Reason the value is invalid.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_too_long_display() {
        let err = BrCodeError::FieldTooLong {
            field: "merchant_name".into(),
            max: 25,
            actual: 30,
        };
        let s = err.to_string();
        assert!(s.contains("merchant_name"));
        assert!(s.contains("25"));
        assert!(s.contains("30"));
    }

    #[test]
    fn test_malformed_tlv_display() {
        let err = BrCodeError::MalformedTlv("unexpected end".into());
        assert!(err.to_string().contains("unexpected end"));
    }

    #[test]
    fn test_missing_tag_display() {
        let err = BrCodeError::MissingTag("00".into());
        assert!(err.to_string().contains("00"));
    }

    #[test]
    fn test_crc_mismatch_display() {
        let err = BrCodeError::CrcMismatch {
            expected: "ABCD".into(),
            actual: "1234".into(),
        };
        let s = err.to_string();
        assert!(s.contains("ABCD"));
        assert!(s.contains("1234"));
    }

    #[test]
    fn test_invalid_field_display() {
        let err = BrCodeError::InvalidField {
            field: "amount".into(),
            reason: "negative".into(),
        };
        let s = err.to_string();
        assert!(s.contains("amount"));
        assert!(s.contains("negative"));
    }

    #[test]
    fn test_error_is_debug() {
        let err = BrCodeError::MalformedTlv("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("MalformedTlv"));
    }
}

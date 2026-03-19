//! CRC16-CCITT checksum calculation for BRCode/Pix payloads.
//!
//! Uses the CRC16-CCITT-FALSE variant:
//! - Polynomial: 0x1021
//! - Initial value: 0xFFFF
//! - No input/output reflection
//! - No final XOR

/// Computes the CRC16-CCITT checksum for the given data.
///
/// This implementation follows the EMV specification for QR code payloads,
/// using polynomial 0x1021 with an initial value of 0xFFFF.
///
/// # Examples
///
/// ```
/// use pix_core::crc16::crc16_ccitt;
///
/// let crc = crc16_ccitt(b"123456789");
/// assert_eq!(crc, 0x29B1);
/// ```
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;

    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

/// Computes the CRC16-CCITT checksum and returns it as a 4-character uppercase hex string.
///
/// # Examples
///
/// ```
/// use pix_core::crc16::crc16_ccitt_hex;
///
/// let hex = crc16_ccitt_hex(b"123456789");
/// assert_eq!(hex, "29B1");
/// ```
pub fn crc16_ccitt_hex(data: &[u8]) -> String {
    format!("{:04X}", crc16_ccitt(data))
}

/// Validates the CRC16 checksum of a complete EMV/BRCode payload.
///
/// The payload must end with the CRC tag `6304` followed by 4 hex characters.
/// Returns `true` if the embedded CRC matches the computed value.
///
/// # Examples
///
/// ```
/// use pix_core::crc16::{crc16_ccitt_hex, validate_crc};
///
/// let mut payload = String::from("000201260014BR.GOV.BCB.PIX6304");
/// let crc = crc16_ccitt_hex(payload.as_bytes());
/// payload.push_str(&crc);
/// assert!(validate_crc(&payload));
/// ```
pub fn validate_crc(payload: &str) -> bool {
    if payload.len() < 8 {
        return false;
    }

    // Check that "6304" appears at the expected position
    let crc_tag_start = payload.len() - 8;
    if &payload[crc_tag_start..crc_tag_start + 4] != "6304" {
        return false;
    }

    let actual_crc = &payload[payload.len() - 4..];
    let data_for_crc = &payload[..payload.len() - 4];
    let expected_crc = crc16_ccitt_hex(data_for_crc.as_bytes());

    actual_crc.eq_ignore_ascii_case(&expected_crc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_known_value() {
        // Standard CRC16-CCITT test vector
        assert_eq!(crc16_ccitt(b"123456789"), 0x29B1);
    }

    #[test]
    fn test_crc16_empty() {
        assert_eq!(crc16_ccitt(b""), 0xFFFF);
    }

    #[test]
    fn test_crc16_single_byte() {
        let crc = crc16_ccitt(b"A");
        assert_ne!(crc, 0xFFFF); // Should differ from empty
    }

    #[test]
    fn test_crc16_hex_format() {
        assert_eq!(crc16_ccitt_hex(b"123456789"), "29B1");
    }

    #[test]
    fn test_crc16_hex_leading_zeros() {
        // Verify 4-char output with leading zeros if needed
        let hex = crc16_ccitt_hex(b"");
        assert_eq!(hex.len(), 4);
        assert_eq!(hex, "FFFF");
    }

    #[test]
    fn test_crc16_pix_payload_fragment() {
        // A typical BRCode payload fragment ending with "6304"
        let payload = b"00020126580014BR.GOV.BCB.PIX0136123e4567-e89b-12d3-a456-42661417400052040000530398654041.005802BR5913Fulano de Tal6008Brasilia6304";
        let crc = crc16_ccitt(payload);
        // Just verify it produces a non-trivial checksum
        assert_ne!(crc, 0);
    }

    #[test]
    fn test_crc16_deterministic() {
        let data = b"test data for crc";
        let crc1 = crc16_ccitt(data);
        let crc2 = crc16_ccitt(data);
        assert_eq!(crc1, crc2);
    }

    #[test]
    fn test_crc16_different_inputs_differ() {
        let crc1 = crc16_ccitt(b"hello");
        let crc2 = crc16_ccitt(b"world");
        assert_ne!(crc1, crc2);
    }

    #[test]
    fn test_validate_crc_valid_payload() {
        // Build a minimal payload with valid CRC
        let mut payload = String::from(
            "00020126330014BR.GOV.BCB.PIX011100000000000520400005303986540510.005802BR5905TESTE6009SAO PAULO6304",
        );
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc);
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_validate_crc_corrupted() {
        let mut payload = String::from(
            "00020126330014BR.GOV.BCB.PIX011100000000000520400005303986540510.005802BR5905TESTE6009SAO PAULO6304",
        );
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc);

        // Corrupt the last character
        let mut corrupted = payload[..payload.len() - 1].to_string();
        corrupted.push('0');
        assert!(!validate_crc(&corrupted));
    }

    #[test]
    fn test_validate_crc_too_short() {
        assert!(!validate_crc(""));
        assert!(!validate_crc("abc"));
        assert!(!validate_crc("6304AB"));
    }

    #[test]
    fn test_validate_crc_missing_tag() {
        // Valid length but no 6304 tag at expected position
        assert!(!validate_crc("00020101ABCD1234"));
    }

    #[test]
    fn test_validate_crc_case_insensitive() {
        let mut payload =
            String::from("000201010211520400005303986540510.005802BR5905TESTE6009SAO PAULO6304");
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc.to_lowercase());
        assert!(validate_crc(&payload));
    }
}

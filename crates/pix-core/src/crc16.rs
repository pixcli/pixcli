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
}

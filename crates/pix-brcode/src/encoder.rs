//! BRCode payload encoder.
//!
//! Encodes a `BrCode` struct into a Pix copy-and-paste string,
//! appending the CRC16-CCITT checksum.

use pix_core::crc16::crc16_ccitt_hex;

use crate::tlv::TlvEntry;
use crate::BrCode;

/// EMV tag IDs used in BRCode payloads.
mod tags {
    pub const PAYLOAD_FORMAT_INDICATOR: &str = "00";
    pub const POINT_OF_INITIATION: &str = "01";
    pub const MERCHANT_ACCOUNT_INFO: &str = "26";
    pub const MERCHANT_CATEGORY_CODE: &str = "52";
    pub const TRANSACTION_CURRENCY: &str = "53";
    pub const TRANSACTION_AMOUNT: &str = "54";
    pub const COUNTRY_CODE: &str = "58";
    pub const MERCHANT_NAME: &str = "59";
    pub const MERCHANT_CITY: &str = "60";
    pub const ADDITIONAL_DATA_FIELD: &str = "62";
    pub const CRC16: &str = "63";
}

/// Sub-tag IDs inside merchant account information (tag 26).
mod merchant_sub {
    pub const GUI: &str = "00";
    pub const PIX_KEY: &str = "01";
    pub const DESCRIPTION: &str = "02";
}

/// Sub-tag IDs inside additional data field (tag 62).
mod additional_sub {
    pub const TXID: &str = "05";
}

/// The globally unique identifier for Pix in Brazil.
const PIX_GUI: &str = "BR.GOV.BCB.PIX";

/// Encodes a `BrCode` into a BRCode payload string with CRC16.
///
/// The resulting string is suitable for use as a Pix "copy and paste" code
/// or for embedding in a QR code.
///
/// # Examples
///
/// ```
/// use pix_brcode::{BrCode, encode_brcode};
///
/// let brcode = BrCode::builder(
///     "user@example.com",
///     "Fulano de Tal",
///     "Brasilia",
/// )
/// .point_of_initiation("12")
/// .transaction_amount("10.00")
/// .txid("ABC123")
/// .build()
/// .unwrap();
///
/// let payload = encode_brcode(&brcode);
/// assert!(payload.starts_with("0002"));
/// assert!(payload.len() > 50);
/// ```
pub fn encode_brcode(brcode: &BrCode) -> String {
    let mut payload = String::new();

    // 00 - Payload Format Indicator (always "01")
    payload.push_str(&TlvEntry::new(tags::PAYLOAD_FORMAT_INDICATOR, "01").encode());

    // 01 - Point of Initiation Method (optional)
    if let Some(ref poi) = brcode.point_of_initiation {
        payload.push_str(&TlvEntry::new(tags::POINT_OF_INITIATION, poi).encode());
    }

    // 26 - Merchant Account Information
    let gui = TlvEntry::new(merchant_sub::GUI, PIX_GUI).encode();
    let key = TlvEntry::new(merchant_sub::PIX_KEY, &brcode.pix_key).encode();
    let mut merchant_info = format!("{gui}{key}");
    if let Some(ref desc) = brcode.description {
        if !desc.is_empty() {
            merchant_info.push_str(&TlvEntry::new(merchant_sub::DESCRIPTION, desc).encode());
        }
    }
    payload.push_str(&TlvEntry::new(tags::MERCHANT_ACCOUNT_INFO, &merchant_info).encode());

    // 52 - Merchant Category Code
    payload.push_str(
        &TlvEntry::new(tags::MERCHANT_CATEGORY_CODE, &brcode.merchant_category_code).encode(),
    );

    // 53 - Transaction Currency (986 = BRL)
    payload.push_str(
        &TlvEntry::new(tags::TRANSACTION_CURRENCY, &brcode.transaction_currency).encode(),
    );

    // 54 - Transaction Amount (optional)
    if let Some(ref amount) = brcode.transaction_amount {
        payload.push_str(&TlvEntry::new(tags::TRANSACTION_AMOUNT, amount).encode());
    }

    // 58 - Country Code
    payload.push_str(&TlvEntry::new(tags::COUNTRY_CODE, &brcode.country_code).encode());

    // 59 - Merchant Name
    payload.push_str(&TlvEntry::new(tags::MERCHANT_NAME, &brcode.merchant_name).encode());

    // 60 - Merchant City
    payload.push_str(&TlvEntry::new(tags::MERCHANT_CITY, &brcode.merchant_city).encode());

    // 62 - Additional Data Field (optional, contains txid)
    if let Some(ref txid) = brcode.txid {
        let txid_tlv = TlvEntry::new(additional_sub::TXID, txid).encode();
        payload.push_str(&TlvEntry::new(tags::ADDITIONAL_DATA_FIELD, &txid_tlv).encode());
    }

    // 63 - CRC16 (append the tag + length placeholder, then compute CRC)
    payload.push_str(&format!("{}04", tags::CRC16));
    let crc = crc16_ccitt_hex(payload.as_bytes());
    payload.push_str(&crc);

    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_basic() {
        let brcode = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);

        // Verify structure
        assert!(payload.starts_with("000201")); // Payload Format Indicator
        assert!(payload.contains("BR.GOV.BCB.PIX")); // GUI
        assert!(payload.contains("user@example.com")); // Pix key
        assert!(payload.contains("Fulano")); // Merchant name
        assert!(payload.contains("Brasilia")); // Merchant city
        assert!(payload.contains("5303986")); // Currency BRL
        assert!(payload.contains("5802BR")); // Country BR
    }

    #[test]
    fn test_encode_with_amount_and_txid() {
        let brcode = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .point_of_initiation("12")
            .transaction_amount("100.50")
            .txid("TXID123")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);

        assert!(payload.contains("010212")); // Dynamic QR
        assert!(payload.contains("100.50")); // Amount
        assert!(payload.contains("TXID123")); // Transaction ID
    }

    #[test]
    fn test_encode_ends_with_crc() {
        let brcode = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);

        // CRC tag is "63", length is "04", followed by 4 hex digits
        let crc_section = &payload[payload.len() - 8..];
        assert!(crc_section.starts_with("6304"));
        // The last 4 chars should be valid hex
        let hex_part = &payload[payload.len() - 4..];
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_encode_crc_is_valid() {
        let brcode = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);

        // Re-verify: CRC of everything before the last 4 chars should match those 4 chars
        let without_crc = &payload[..payload.len() - 4];
        let expected_crc = crc16_ccitt_hex(without_crc.as_bytes());
        let actual_crc = &payload[payload.len() - 4..];
        assert_eq!(expected_crc, actual_crc);
    }

    #[test]
    fn test_encode_static_qr() {
        let brcode = BrCode::builder("52998224725", "Maria", "Sao Paulo")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);

        assert!(payload.contains("010211")); // Static QR
                                             // No amount tag 54
        assert!(
            !payload.contains("5405") && !payload.contains("5404") && !payload.contains("5403")
        );
    }
}

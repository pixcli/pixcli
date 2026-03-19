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

    #[test]
    fn test_encode_with_description() {
        let brcode = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .description("Pagamento cafe")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("Pagamento cafe"));

        // Verify CRC is still valid
        let without_crc = &payload[..payload.len() - 4];
        let expected_crc = crc16_ccitt_hex(without_crc.as_bytes());
        let actual_crc = &payload[payload.len() - 4..];
        assert_eq!(expected_crc, actual_crc);
    }

    #[test]
    fn test_encode_empty_description_ignored() {
        let with_empty = BrCode::builder("key@test.com", "Name", "City")
            .description("")
            .build()
            .unwrap();
        let without = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();

        let payload_with = encode_brcode(&with_empty);
        let payload_without = encode_brcode(&without);

        // Empty description should produce same payload as no description
        assert_eq!(payload_with, payload_without);
    }

    #[test]
    fn test_encode_payload_not_too_long() {
        let brcode = BrCode::builder(
            "user@example.com",
            "A".repeat(25).as_str(),
            "B".repeat(15).as_str(),
        )
        .transaction_amount("99999999.99")
        .description("A long description for testing")
        .txid("TXID1234567890")
        .build()
        .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(
            payload.len() <= 512,
            "Payload too long: {} chars",
            payload.len()
        );
    }
}

#[cfg(test)]
mod additional_encoder_tests {
    use super::*;
    use crate::BrCode;
    use pix_core::crc16::validate_crc;

    #[test]
    fn test_encode_with_cpf_key() {
        let brcode = BrCode::builder("52998224725", "Maria", "Sao Paulo")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("52998224725"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_cnpj_key() {
        let brcode = BrCode::builder("11222333000181", "Empresa", "Rio")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("11222333000181"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_email_key() {
        let brcode = BrCode::builder("pix@empresa.com.br", "Loja", "Recife")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("pix@empresa.com.br"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_phone_key() {
        let brcode = BrCode::builder("+5511987654321", "Joao", "BH")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("+5511987654321"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_evp_key() {
        let brcode = BrCode::builder("123e4567-e89b-12d3-a456-426614174000", "Test", "SP")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("123e4567-e89b-12d3-a456-426614174000"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_minimum_amount() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("0.01")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("0.01"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_large_amount() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("999999.99")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("999999.99"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_without_amount() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        // Tag 54 should NOT appear
        assert!(!payload.contains("54"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_special_chars_description() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .description("Cafe and Pao 123")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_max_merchant_name_25() {
        let name = "A".repeat(25);
        let brcode = BrCode::builder("key@test.com", &name, "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains(&name));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_max_merchant_city_15() {
        let city = "B".repeat(15);
        let brcode = BrCode::builder("key@test.com", "Name", &city)
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains(&city));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_dynamic_qr() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("12")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("010212"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_static_qr() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("010211"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_no_point_of_initiation() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        // Tag "01" should not appear for POI
        assert!(!payload.starts_with("00020101"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_custom_mcc() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .merchant_category_code("5812")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("52045812"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_txid() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .txid("PIX123")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("PIX123"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_with_all_fields() {
        let brcode = BrCode::builder("test@example.com", "Fulano de Tal", "Sao Paulo")
            .point_of_initiation("12")
            .merchant_category_code("5812")
            .transaction_amount("42.50")
            .description("Pagamento teste")
            .txid("TX999")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("test@example.com"));
        assert!(payload.contains("42.50"));
        assert!(payload.contains("TX999"));
        assert!(payload.contains("Pagamento teste"));
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_brazilian_chars_in_description() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .description("Pagamento acao tres Joao")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(validate_crc(&payload));
    }

    #[test]
    fn test_encode_payload_starts_with_000201() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.starts_with("000201"));
    }

    #[test]
    fn test_encode_payload_contains_gui() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("BR.GOV.BCB.PIX"));
    }

    #[test]
    fn test_encode_payload_contains_brl_currency() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("5303986"));
    }

    #[test]
    fn test_encode_payload_contains_country_br() {
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("5802BR"));
    }
}

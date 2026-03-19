//! BRCode payload decoder.
//!
//! Parses a Pix copy-and-paste string into a `BrCode` struct,
//! verifying the CRC16-CCITT checksum.

use pix_core::crc16::crc16_ccitt_hex;

use crate::tlv::{find_tag, parse_tlv};
use crate::{BrCode, BrCodeError};

/// Decodes a BRCode payload string into a `BrCode` struct.
///
/// Parses the EMV TLV-encoded payload, extracts all Pix-specific fields,
/// and verifies the CRC16 checksum.
///
/// # Errors
///
/// Returns `BrCodeError` if:
/// - The payload is malformed or contains invalid TLV data
/// - Required tags are missing
/// - The CRC16 checksum doesn't match
///
/// # Examples
///
/// ```
/// use pix_brcode::{BrCode, encode_brcode, decode_brcode};
///
/// let brcode = BrCode::builder("user@example.com", "Fulano", "Brasilia")
///     .transaction_amount("10.00")
///     .build()
///     .unwrap();
/// let payload = encode_brcode(&brcode);
/// let decoded = decode_brcode(&payload).unwrap();
/// assert_eq!(decoded.pix_key, "user@example.com");
/// ```
pub fn decode_brcode(payload: &str) -> Result<BrCode, BrCodeError> {
    // Verify CRC16 before parsing
    if payload.len() < 8 {
        return Err(BrCodeError::MalformedTlv(
            "payload too short to contain CRC".into(),
        ));
    }

    let crc_tag_pos = payload.len() - 8;
    let crc_prefix = &payload[crc_tag_pos..crc_tag_pos + 4];

    if crc_prefix != "6304" {
        return Err(BrCodeError::MalformedTlv(
            "payload must end with CRC tag '6304'".into(),
        ));
    }

    let actual_crc = &payload[payload.len() - 4..];
    let data_for_crc = &payload[..payload.len() - 4];
    let expected_crc = crc16_ccitt_hex(data_for_crc.as_bytes());

    if actual_crc != expected_crc {
        return Err(BrCodeError::CrcMismatch {
            expected: expected_crc,
            actual: actual_crc.to_string(),
        });
    }

    // Parse root-level TLV entries
    let entries = parse_tlv(payload)?;

    // 00 - Payload Format Indicator
    let pfi = find_tag(&entries, "00")
        .ok_or_else(|| BrCodeError::MissingTag("00 (Payload Format Indicator)".into()))?;

    // 01 - Point of Initiation Method (optional)
    let poi = find_tag(&entries, "01").map(|e| e.value.clone());

    // 26 - Merchant Account Information
    let mai = find_tag(&entries, "26")
        .ok_or_else(|| BrCodeError::MissingTag("26 (Merchant Account Information)".into()))?;

    // Parse sub-tags inside tag 26
    let mai_entries = parse_tlv(&mai.value)?;
    let pix_key_entry = find_tag(&mai_entries, "01")
        .ok_or_else(|| BrCodeError::MissingTag("26.01 (Pix Key)".into()))?;

    // 26.02 - Description (optional)
    let description = find_tag(&mai_entries, "02").map(|e| e.value.clone());

    // 52 - Merchant Category Code
    let mcc = find_tag(&entries, "52")
        .ok_or_else(|| BrCodeError::MissingTag("52 (Merchant Category Code)".into()))?;

    // 53 - Transaction Currency
    let currency = find_tag(&entries, "53")
        .ok_or_else(|| BrCodeError::MissingTag("53 (Transaction Currency)".into()))?;

    // 54 - Transaction Amount (optional)
    let amount = find_tag(&entries, "54").map(|e| e.value.clone());

    // 58 - Country Code
    let country = find_tag(&entries, "58")
        .ok_or_else(|| BrCodeError::MissingTag("58 (Country Code)".into()))?;

    // 59 - Merchant Name
    let name = find_tag(&entries, "59")
        .ok_or_else(|| BrCodeError::MissingTag("59 (Merchant Name)".into()))?;

    // 60 - Merchant City
    let city = find_tag(&entries, "60")
        .ok_or_else(|| BrCodeError::MissingTag("60 (Merchant City)".into()))?;

    // 62 - Additional Data Field (optional)
    let txid = if let Some(adf) = find_tag(&entries, "62") {
        let adf_entries = parse_tlv(&adf.value)?;
        find_tag(&adf_entries, "05").map(|e| e.value.clone())
    } else {
        None
    };

    // 63 - CRC (already verified above)
    let crc =
        find_tag(&entries, "63").ok_or_else(|| BrCodeError::MissingTag("63 (CRC16)".into()))?;

    Ok(BrCode {
        payload_format_indicator: pfi.value.clone(),
        point_of_initiation: poi,
        pix_key: pix_key_entry.value.clone(),
        description,
        merchant_category_code: mcc.value.clone(),
        transaction_currency: currency.value.clone(),
        transaction_amount: amount,
        country_code: country.value.clone(),
        merchant_name: name.value.clone(),
        merchant_city: city.value.clone(),
        txid,
        crc: crc.value.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_brcode;

    #[test]
    fn test_roundtrip_basic() {
        let original = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.payload_format_indicator, "01");
        assert_eq!(decoded.pix_key, "user@example.com");
        assert_eq!(decoded.merchant_name, "Fulano");
        assert_eq!(decoded.merchant_city, "Brasilia");
        assert_eq!(decoded.transaction_currency, "986");
        assert_eq!(decoded.country_code, "BR");
        assert_eq!(decoded.merchant_category_code, "0000");
    }

    #[test]
    fn test_roundtrip_with_amount_and_txid() {
        let original = BrCode::builder("52998224725", "Maria Silva", "Sao Paulo")
            .point_of_initiation("12")
            .transaction_amount("99.99")
            .txid("PAG123")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.point_of_initiation, Some("12".to_string()));
        assert_eq!(decoded.transaction_amount, Some("99.99".to_string()));
        assert_eq!(decoded.txid, Some("PAG123".to_string()));
    }

    #[test]
    fn test_decode_invalid_crc() {
        let original = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .build()
            .unwrap();
        let mut payload = encode_brcode(&original);

        // Corrupt the CRC
        let len = payload.len();
        payload.replace_range(len - 4..len, "0000");

        assert!(matches!(
            decode_brcode(&payload),
            Err(BrCodeError::CrcMismatch { .. })
        ));
    }

    #[test]
    fn test_decode_too_short() {
        assert!(decode_brcode("short").is_err());
    }

    #[test]
    fn test_decode_missing_crc_tag() {
        assert!(decode_brcode("00020100000000").is_err());
    }

    #[test]
    fn test_roundtrip_static_qr_no_amount() {
        let original = BrCode::builder("pix@email.com", "Loja", "Rio")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.point_of_initiation, Some("11".to_string()));
        assert_eq!(decoded.transaction_amount, None);
        assert_eq!(decoded.txid, None);
    }
}

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
    fn test_roundtrip_with_description() {
        let original = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .description("Pagamento cafe")
            .transaction_amount("5.50")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.description, Some("Pagamento cafe".to_string()));
        assert_eq!(decoded.transaction_amount, Some("5.50".to_string()));
    }

    #[test]
    fn test_roundtrip_no_description() {
        let original = BrCode::builder("user@example.com", "Fulano", "Brasilia")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.description, None);
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

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::encode_brcode;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn encode_decode_roundtrip_pix_key(
            key in "[a-z]{3,20}@[a-z]{3,10}\\.[a-z]{2,4}",
            name in "[A-Z ]{1,25}",
            city in "[A-Z ]{1,15}",
        ) {
            let trimmed_name = &name[..name.len().min(25)];
            let trimmed_city = &city[..city.len().min(15)];
            if let Ok(brcode) = crate::BrCode::builder(&key, trimmed_name, trimmed_city).build() {
                let payload = encode_brcode(&brcode);
                let decoded = decode_brcode(&payload).unwrap();
                prop_assert_eq!(&decoded.pix_key, &key);
            }
        }

        #[test]
        fn encoded_payload_always_has_valid_crc(
            key in "[a-z]{5,15}@test\\.com",
        ) {
            let brcode = crate::BrCode::builder(&key, "TEST", "CITY").build().unwrap();
            let payload = encode_brcode(&brcode);
            prop_assert!(pix_core::crc16::validate_crc(&payload));
        }
    }
}

#[cfg(test)]
mod additional_decoder_tests {
    use super::*;
    use crate::encode_brcode;

    #[test]
    fn test_decode_empty_string() {
        assert!(decode_brcode("").is_err());
    }

    #[test]
    fn test_decode_short_string() {
        assert!(decode_brcode("abc").is_err());
    }

    #[test]
    fn test_decode_wrong_crc() {
        let original = BrCode::builder("user@example.com", "Name", "City")
            .build()
            .unwrap();
        let mut payload = encode_brcode(&original);
        let len = payload.len();
        payload.replace_range(len - 4..len, "FFFF");
        let result = decode_brcode(&payload);
        assert!(matches!(result, Err(BrCodeError::CrcMismatch { .. })));
    }

    #[test]
    fn test_decode_missing_pfi_tag() {
        // Payload without tag 00 but with valid CRC
        let data = "26100014BR.GOV.BCB.PIX6304";
        let crc = pix_core::crc16::crc16_ccitt_hex(data.as_bytes());
        let payload = format!("{data}{crc}");
        let result = decode_brcode(&payload);
        assert!(result.is_err());
    }

    #[test]
    fn test_roundtrip_cpf_key() {
        let original = BrCode::builder("52998224725", "Maria", "SP")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "52998224725");
    }

    #[test]
    fn test_roundtrip_cnpj_key() {
        let original = BrCode::builder("11222333000181", "Empresa", "RJ")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "11222333000181");
    }

    #[test]
    fn test_roundtrip_email_key() {
        let original = BrCode::builder("pix@empresa.com.br", "Loja ABC", "Recife")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "pix@empresa.com.br");
    }

    #[test]
    fn test_roundtrip_phone_key() {
        let original = BrCode::builder("+5511987654321", "Joao Silva", "BH")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "+5511987654321");
    }

    #[test]
    fn test_roundtrip_evp_key() {
        let original = BrCode::builder("123e4567-e89b-12d3-a456-426614174000", "Test", "SP")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "123e4567-e89b-12d3-a456-426614174000");
    }

    #[test]
    fn test_roundtrip_with_min_amount() {
        let original = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("0.01")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.transaction_amount, Some("0.01".to_string()));
    }

    #[test]
    fn test_roundtrip_with_max_amount() {
        let original = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("999999.99")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.transaction_amount, Some("999999.99".to_string()));
    }

    #[test]
    fn test_roundtrip_dynamic_qr() {
        let original = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("12")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.point_of_initiation, Some("12".to_string()));
    }

    #[test]
    fn test_roundtrip_with_description() {
        let original = BrCode::builder("key@test.com", "Name", "City")
            .description("Pagamento do cafe")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.description, Some("Pagamento do cafe".to_string()));
    }

    #[test]
    fn test_roundtrip_with_all_fields() {
        let original = BrCode::builder(
            "test@example.com",
            "Fulano de Tal Sobrenome",
            "Sao Paulo SP",
        )
        .point_of_initiation("12")
        .merchant_category_code("5812")
        .transaction_amount("42.50")
        .description("Pagamento teste")
        .txid("TX999")
        .build()
        .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.pix_key, "test@example.com");
        assert_eq!(decoded.merchant_name, "Fulano de Tal Sobrenome");
        assert_eq!(decoded.merchant_city, "Sao Paulo SP");
        assert_eq!(decoded.point_of_initiation, Some("12".to_string()));
        assert_eq!(decoded.merchant_category_code, "5812");
        assert_eq!(decoded.transaction_amount, Some("42.50".to_string()));
        assert_eq!(decoded.description, Some("Pagamento teste".to_string()));
        assert_eq!(decoded.txid, Some("TX999".to_string()));
        assert_eq!(decoded.transaction_currency, "986");
        assert_eq!(decoded.country_code, "BR");
        assert_eq!(decoded.payload_format_indicator, "01");
    }

    #[test]
    fn test_roundtrip_max_length_name_and_city() {
        let name = "A".repeat(25);
        let city = "B".repeat(15);
        let original = BrCode::builder("key@t.com", &name, &city).build().unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.merchant_name, name);
        assert_eq!(decoded.merchant_city, city);
    }

    #[test]
    fn test_decode_re_encode_matches() {
        let original = BrCode::builder("key@test.com", "Name", "City")
            .point_of_initiation("11")
            .transaction_amount("10.00")
            .txid("TX123")
            .build()
            .unwrap();
        let payload1 = encode_brcode(&original);
        let decoded = decode_brcode(&payload1).unwrap();

        // Re-encode
        let mut builder = BrCode::builder(
            &decoded.pix_key,
            &decoded.merchant_name,
            &decoded.merchant_city,
        );
        if let Some(ref poi) = decoded.point_of_initiation {
            builder = builder.point_of_initiation(poi);
        }
        builder = builder.merchant_category_code(&decoded.merchant_category_code);
        if let Some(ref amount) = decoded.transaction_amount {
            builder = builder.transaction_amount(amount);
        }
        if let Some(ref desc) = decoded.description {
            builder = builder.description(desc);
        }
        if let Some(ref txid) = decoded.txid {
            builder = builder.txid(txid);
        }
        let brcode2 = builder.build().unwrap();
        let payload2 = encode_brcode(&brcode2);
        assert_eq!(payload1, payload2);
    }

    #[test]
    fn test_decode_ascii_description_roundtrip() {
        let original = BrCode::builder("key@test.com", "Name", "City")
            .description("Acao tres Joao cafe nao")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(
            decoded.description,
            Some("Acao tres Joao cafe nao".to_string())
        );
    }

    #[test]
    fn test_decode_crc_stored_correctly() {
        let original = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.crc.len(), 4);
        assert!(decoded.crc.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

#[cfg(test)]
mod additional_proptests {
    use super::*;
    use crate::encode_brcode;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn roundtrip_with_amount(
            key in "[a-z]{3,10}@[a-z]{3,8}\\.[a-z]{2,3}",
            name in "[A-Z]{1,25}",
            city in "[A-Z]{1,15}",
            amount_int in 1u32..99999,
            amount_dec in 0u32..100,
        ) {
            let amount = format!("{}.{:02}", amount_int, amount_dec);
            let trimmed_name = &name[..name.len().min(25)];
            let trimmed_city = &city[..city.len().min(15)];
            if let Ok(brcode) = crate::BrCode::builder(&key, trimmed_name, trimmed_city)
                .transaction_amount(&amount)
                .build()
            {
                let payload = encode_brcode(&brcode);
                let decoded = decode_brcode(&payload).unwrap();
                prop_assert_eq!(decoded.transaction_amount, Some(amount));
                prop_assert_eq!(&decoded.pix_key, &key);
                prop_assert_eq!(&decoded.merchant_name, trimmed_name);
                prop_assert_eq!(&decoded.merchant_city, trimmed_city);
            }
        }

        #[test]
        fn roundtrip_with_description(
            key in "[a-z]{5,10}@test\\.com",
            desc in "[a-zA-Z0-9 ]{1,30}",
        ) {
            if let Ok(brcode) = crate::BrCode::builder(&key, "NAME", "CITY")
                .description(&desc)
                .build()
            {
                let payload = encode_brcode(&brcode);
                let decoded = decode_brcode(&payload).unwrap();
                prop_assert_eq!(decoded.description, Some(desc));
            }
        }
    }
}

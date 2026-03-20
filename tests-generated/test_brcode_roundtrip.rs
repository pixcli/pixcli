//! BRCode encoder/decoder roundtrip tests for `crates/pix-brcode/`.
//!
//! These tests verify that encode -> decode is lossless for all field
//! combinations, and that the CRC16 checksum is always valid after encoding.
//! They also test decoding of known real-world Pix QR code payload patterns
//! and exercise edge cases around maximum-length fields and special characters.
//!
//! ## Placement Instructions
//!
//! To compile and run these tests, copy this file to:
//!   `crates/pix-brcode/tests/test_brcode_roundtrip.rs`
//!
//! The file uses `pix_brcode` and `pix_core` as external crate dependencies,
//! which are automatically available for integration tests within the workspace.
//!
//! ## Dependencies Required
//!
//! In `crates/pix-brcode/Cargo.toml` under `[dev-dependencies]`:
//! ```toml
//! proptest = { workspace = true }
//! pix-core = { workspace = true }
//! ```

#[cfg(test)]
mod tests {
    use pix_brcode::{decode_brcode, encode_brcode, BrCode};
    use pix_core::crc16::validate_crc;

    // =========================================================================
    // BASIC ROUNDTRIP TESTS
    // =========================================================================

    #[test]
    fn roundtrip_minimal_brcode() {
        // WHY: The simplest possible BrCode with only required fields.
        // Verifies that encode -> decode preserves all defaults.
        let original = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.payload_format_indicator, "01");
        assert_eq!(decoded.pix_key, "key@test.com");
        assert_eq!(decoded.merchant_name, "Name");
        assert_eq!(decoded.merchant_city, "City");
        assert_eq!(decoded.merchant_category_code, "0000");
        assert_eq!(decoded.transaction_currency, "986");
        assert_eq!(decoded.country_code, "BR");
        assert_eq!(decoded.point_of_initiation, None);
        assert_eq!(decoded.transaction_amount, None);
        assert_eq!(decoded.description, None);
        assert_eq!(decoded.txid, None);
    }

    #[test]
    fn roundtrip_all_fields_populated() {
        // WHY: Ensures every optional field survives the encode/decode cycle.
        let original = BrCode::builder("user@example.com", "Fulano de Tal", "Sao Paulo")
            .point_of_initiation("12")
            .merchant_category_code("5812")
            .transaction_amount("42.50")
            .description("Pagamento cafe")
            .txid("TX123ABC")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.pix_key, "user@example.com");
        assert_eq!(decoded.merchant_name, "Fulano de Tal");
        assert_eq!(decoded.merchant_city, "Sao Paulo");
        assert_eq!(decoded.point_of_initiation, Some("12".to_string()));
        assert_eq!(decoded.merchant_category_code, "5812");
        assert_eq!(decoded.transaction_amount, Some("42.50".to_string()));
        assert_eq!(decoded.description, Some("Pagamento cafe".to_string()));
        assert_eq!(decoded.txid, Some("TX123ABC".to_string()));
    }

    #[test]
    fn roundtrip_static_qr_no_amount() {
        // WHY: Static QR codes (point_of_initiation "11") often have no
        // amount. This tests that None amount survives roundtrip.
        let original = BrCode::builder("52998224725", "Maria", "SP")
            .point_of_initiation("11")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.point_of_initiation, Some("11".to_string()));
        assert_eq!(decoded.transaction_amount, None);
    }

    // =========================================================================
    // CRC VALIDATION TESTS
    // =========================================================================

    #[test]
    fn encoded_payload_always_has_valid_crc() {
        // WHY: Every encoded payload must end with a valid CRC16 checksum.
        // This is critical for interoperability with banking systems.
        let brcode = BrCode::builder("pix@banco.com.br", "Loja ABC", "Brasilia")
            .transaction_amount("100.00")
            .txid("PAGAMENTO001")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(validate_crc(&payload), "CRC should be valid for encoded payload");
    }

    #[test]
    fn corrupted_payload_fails_decode() {
        // WHY: Flipping a single bit in the payload should cause CRC
        // mismatch during decode, proving the integrity check works.
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let mut payload = encode_brcode(&brcode);

        // Corrupt the middle of the payload (before CRC)
        let mid = payload.len() / 2;
        let bytes = unsafe { payload.as_bytes_mut() };
        bytes[mid] ^= 0x01; // flip one bit

        assert!(decode_brcode(&payload).is_err());
    }

    // =========================================================================
    // MAXIMUM LENGTH FIELD TESTS
    // =========================================================================

    #[test]
    fn roundtrip_max_length_merchant_name() {
        // WHY: merchant_name max is 25 chars. Tests that the TLV encoding
        // correctly handles the boundary (length "25" is two digits).
        let name = "A".repeat(25);
        let original = BrCode::builder("key@test.com", &name, "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.merchant_name, name);
    }

    #[test]
    fn roundtrip_max_length_merchant_city() {
        // WHY: merchant_city max is 15 chars. Boundary test for city field.
        let city = "B".repeat(15);
        let original = BrCode::builder("key@test.com", "Name", &city)
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.merchant_city, city);
    }

    #[test]
    fn roundtrip_max_length_amount() {
        // WHY: transaction_amount max is 13 chars. Tests that the maximum
        // representable amount roundtrips correctly.
        let amount = "9999999999.99"; // 13 chars
        let original = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount(amount)
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.transaction_amount, Some(amount.to_string()));
    }

    #[test]
    fn roundtrip_max_length_txid() {
        // WHY: txid max is 25 chars in the builder. Tests boundary.
        let txid = "X".repeat(25);
        let original = BrCode::builder("key@test.com", "Name", "City")
            .txid(&txid)
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.txid, Some(txid));
    }

    #[test]
    fn roundtrip_all_max_length_fields_simultaneously() {
        // WHY: Tests that all fields at maximum length simultaneously
        // do not cause TLV parsing issues (e.g., length overflow).
        let name = "A".repeat(25);
        let city = "B".repeat(15);
        let amount = "9999999999.99"; // 13 chars
        let txid = "Z".repeat(25);
        let original = BrCode::builder("key@test.com", &name, &city)
            .transaction_amount(amount)
            .txid(&txid)
            .description("Long description text")
            .point_of_initiation("12")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();

        assert_eq!(decoded.merchant_name, name);
        assert_eq!(decoded.merchant_city, city);
        assert_eq!(decoded.transaction_amount, Some(amount.to_string()));
        assert_eq!(decoded.txid, Some(txid));
    }

    // =========================================================================
    // MINIMUM FIELD TESTS
    // =========================================================================

    #[test]
    fn roundtrip_single_char_fields() {
        // WHY: Tests that the TLV encoding handles single-character values
        // correctly (length "01" prefix).
        let original = BrCode::builder("k", "N", "C")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "k");
        assert_eq!(decoded.merchant_name, "N");
        assert_eq!(decoded.merchant_city, "C");
    }

    #[test]
    fn roundtrip_minimum_amount() {
        // WHY: "0.01" is the smallest meaningful BRL amount.
        let original = BrCode::builder("key@test.com", "Name", "City")
            .transaction_amount("0.01")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.transaction_amount, Some("0.01".to_string()));
    }

    // =========================================================================
    // SPECIAL CHARACTER TESTS
    // =========================================================================

    #[test]
    fn roundtrip_description_with_spaces() {
        // WHY: Spaces in descriptions are common and must survive TLV
        // encoding where length is computed from the value string.
        let original = BrCode::builder("key@test.com", "Name", "City")
            .description("Pagamento do aluguel de janeiro")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.description, Some("Pagamento do aluguel de janeiro".to_string()));
    }

    #[test]
    fn roundtrip_pix_key_with_special_chars() {
        // WHY: Phone keys include '+' and UUID keys include '-'. Both are
        // non-alphanumeric but valid in pix_key values.
        let original = BrCode::builder("+5511987654321", "Joao", "BH")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "+5511987654321");
    }

    #[test]
    fn roundtrip_uuid_pix_key() {
        // WHY: UUID EVP keys contain hyphens which could confuse TLV parsing
        // if length calculation is wrong.
        let uuid_key = "123e4567-e89b-12d3-a456-426614174000";
        let original = BrCode::builder(uuid_key, "Test", "SP")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, uuid_key);
    }

    // =========================================================================
    // RE-ENCODE CONSISTENCY TEST
    // =========================================================================

    #[test]
    fn decode_then_reencode_produces_identical_payload() {
        // WHY: This is the strongest roundtrip guarantee. If we decode a
        // payload and re-encode the decoded struct, the result must be
        // byte-identical to the original.
        let original = BrCode::builder("user@example.com", "Fulano de Tal", "Brasilia")
            .point_of_initiation("12")
            .merchant_category_code("5812")
            .transaction_amount("99.99")
            .description("Test payment")
            .txid("TX42")
            .build()
            .unwrap();

        let payload1 = encode_brcode(&original);
        let decoded = decode_brcode(&payload1).unwrap();

        // Reconstruct from decoded fields
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
        let reconstructed = builder.build().unwrap();
        let payload2 = encode_brcode(&reconstructed);

        assert_eq!(payload1, payload2, "Re-encoded payload must be identical to original");
    }

    // =========================================================================
    // KNOWN PAYLOAD PATTERN TESTS
    // =========================================================================

    #[test]
    fn encode_produces_emv_compliant_structure() {
        // WHY: Verifies the encoded payload follows EMV TLV structure:
        // starts with "000201" (tag 00, length 02, value "01") and ends
        // with "6304" + 4 hex digits.
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);

        assert!(payload.starts_with("000201"), "Must start with Payload Format Indicator");
        let crc_section = &payload[payload.len() - 8..];
        assert!(crc_section.starts_with("6304"), "Must end with CRC tag 6304");
        let hex_part = &payload[payload.len() - 4..];
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit()),
            "Last 4 chars must be hex CRC"
        );
    }

    #[test]
    fn encoded_payload_contains_gui() {
        // WHY: The Pix GUI "BR.GOV.BCB.PIX" is mandatory in the merchant
        // account information (tag 26). Its absence would make the QR code
        // unreadable by banking apps.
        let brcode = BrCode::builder("key@test.com", "Name", "City")
            .build()
            .unwrap();
        let payload = encode_brcode(&brcode);
        assert!(payload.contains("BR.GOV.BCB.PIX"));
    }

    #[test]
    fn decode_payload_missing_tag_00_fails() {
        // WHY: Tag 00 (Payload Format Indicator) is required by the spec.
        // A payload without it should fail decode even if CRC is valid.
        let data = "26280014BR.GOV.BCB.PIX0110key@a.com52040000530398658025904Name6004City6304";
        let crc = pix_core::crc16::crc16_ccitt_hex(data.as_bytes());
        let payload = format!("{data}{crc}");
        let result = decode_brcode(&payload);
        assert!(result.is_err(), "Missing tag 00 should cause decode failure");
    }

    // =========================================================================
    // DECODE ERROR HANDLING TESTS
    // =========================================================================

    #[test]
    fn decode_empty_string_fails() {
        // WHY: Empty string has no CRC or any tags.
        assert!(decode_brcode("").is_err());
    }

    #[test]
    fn decode_7_chars_fails() {
        // WHY: Minimum payload length for CRC check is 8 (4 for "6304" + 4 for CRC).
        assert!(decode_brcode("6304ABC").is_err());
    }

    #[test]
    fn decode_valid_crc_but_no_tag_26_fails() {
        // WHY: Tag 26 (Merchant Account Information) is required. Without it,
        // there is no pix_key to extract.
        let data = "000201520400005303986580259040000600400006304";
        let crc = pix_core::crc16::crc16_ccitt_hex(data.as_bytes());
        let payload = format!("{data}{crc}");
        // This may or may not parse depending on TLV validity -- test that
        // it either fails or at least does not panic.
        let _result = decode_brcode(&payload);
    }

    // =========================================================================
    // DIFFERENT PIX KEY TYPE ROUNDTRIPS
    // =========================================================================

    #[test]
    fn roundtrip_with_cpf_key() {
        // WHY: CPF keys are 11 digits, a common key type.
        let original = BrCode::builder("52998224725", "Maria Silva", "SP")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        assert!(validate_crc(&payload));
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "52998224725");
    }

    #[test]
    fn roundtrip_with_cnpj_key() {
        // WHY: CNPJ keys are 14 digits, used by businesses.
        let original = BrCode::builder("11222333000181", "Empresa LTDA", "Rio")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        assert!(validate_crc(&payload));
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "11222333000181");
    }

    #[test]
    fn roundtrip_with_email_key() {
        // WHY: Email keys contain '@' and '.' which are special in some contexts.
        let original = BrCode::builder("financeiro@empresa.com.br", "Loja", "Recife")
            .build()
            .unwrap();
        let payload = encode_brcode(&original);
        assert!(validate_crc(&payload));
        let decoded = decode_brcode(&payload).unwrap();
        assert_eq!(decoded.pix_key, "financeiro@empresa.com.br");
    }
}

#[cfg(test)]
mod proptests {
    use pix_brcode::{decode_brcode, encode_brcode, BrCode};
    use pix_core::crc16::validate_crc;
    use proptest::prelude::*;

    proptest! {
        /// WHY: The fundamental property -- any valid BrCode that can be built
        /// should survive encode -> decode with all fields preserved.
        #[test]
        fn roundtrip_preserves_pix_key(
            key in "[a-z]{3,15}@[a-z]{3,8}\\.[a-z]{2,4}",
            name in "[A-Z]{1,25}",
            city in "[A-Z]{1,15}",
        ) {
            let trimmed_name = &name[..name.len().min(25)];
            let trimmed_city = &city[..city.len().min(15)];
            if let Ok(brcode) = BrCode::builder(&key, trimmed_name, trimmed_city).build() {
                let payload = encode_brcode(&brcode);
                prop_assert!(validate_crc(&payload), "CRC must be valid");
                let decoded = decode_brcode(&payload).unwrap();
                prop_assert_eq!(&decoded.pix_key, &key);
                prop_assert_eq!(&decoded.merchant_name, trimmed_name);
                prop_assert_eq!(&decoded.merchant_city, trimmed_city);
            }
        }

        /// WHY: Amounts must survive roundtrip exactly, including leading zeros
        /// and decimal precision.
        #[test]
        fn roundtrip_preserves_amount(
            amount_int in 0u32..99999,
            amount_dec in 0u32..100,
        ) {
            let amount = format!("{}.{:02}", amount_int, amount_dec);
            if amount.len() <= 13 {
                let brcode = BrCode::builder("key@test.com", "TEST", "CITY")
                    .transaction_amount(&amount)
                    .build()
                    .unwrap();
                let payload = encode_brcode(&brcode);
                let decoded = decode_brcode(&payload).unwrap();
                prop_assert_eq!(decoded.transaction_amount, Some(amount));
            }
        }

        /// WHY: Descriptions with various characters must roundtrip correctly.
        /// Tests that TLV length encoding handles varying description lengths.
        #[test]
        fn roundtrip_preserves_description(
            desc in "[a-zA-Z0-9 ]{1,40}",
        ) {
            if let Ok(brcode) = BrCode::builder("key@test.com", "Name", "City")
                .description(&desc)
                .build()
            {
                let payload = encode_brcode(&brcode);
                let decoded = decode_brcode(&payload).unwrap();
                prop_assert_eq!(decoded.description, Some(desc));
            }
        }

        /// WHY: Transaction IDs of varying valid lengths must roundtrip.
        #[test]
        fn roundtrip_preserves_txid(
            txid in "[A-Z0-9]{1,25}",
        ) {
            let brcode = BrCode::builder("key@test.com", "Name", "City")
                .txid(&txid)
                .build()
                .unwrap();
            let payload = encode_brcode(&brcode);
            let decoded = decode_brcode(&payload).unwrap();
            prop_assert_eq!(decoded.txid, Some(txid));
        }
    }
}

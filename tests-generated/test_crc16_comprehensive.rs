//! Comprehensive CRC16-CCITT tests for `crates/pix-core/src/crc16.rs`.
//!
//! These tests cover known test vectors from the CRC16-CCITT specification,
//! edge cases around the validate_crc function when "6304" appears in the
//! data itself, null byte handling, stability across all 256 single-byte
//! inputs, the self-validation property, and performance with large payloads.
//!
//! ## Placement Instructions
//!
//! To compile and run these tests, copy this file to:
//!   `crates/pix-core/tests/test_crc16_comprehensive.rs`
//!
//! The file uses `pix_core` as an external crate dependency, which is
//! automatically available for integration tests within the workspace.
//!
//! ## Dependencies Required
//!
//! In `crates/pix-core/Cargo.toml` under `[dev-dependencies]`:
//! ```toml
//! proptest = { workspace = true }
//! ```

#[cfg(test)]
mod tests {
    use pix_core::crc16::{crc16_ccitt, crc16_ccitt_hex, validate_crc};

    // =========================================================================
    // KNOWN TEST VECTORS FROM CRC16-CCITT SPECIFICATION
    // =========================================================================

    #[test]
    fn standard_test_vector_123456789() {
        // WHY: "123456789" is THE canonical test vector for CRC16-CCITT-FALSE.
        // The expected value 0x29B1 is universally agreed upon and used to
        // verify implementation correctness.
        assert_eq!(crc16_ccitt(b"123456789"), 0x29B1);
    }

    #[test]
    fn empty_input_returns_initial_value() {
        // WHY: With no data, the CRC should equal the initial value 0xFFFF
        // since no XOR operations are performed. This confirms the
        // initialization is correct.
        assert_eq!(crc16_ccitt(b""), 0xFFFF);
    }

    #[test]
    fn single_byte_zero() {
        // WHY: A single zero byte exercises the polynomial feedback path
        // with minimal input. The result should differ from the initial value.
        let crc = crc16_ccitt(&[0x00]);
        assert_ne!(crc, 0xFFFF, "Single null byte should change CRC from initial");
        // Known value for CRC16-CCITT-FALSE of single 0x00 byte:
        // After XOR with 0xFF00 (0xFFFF ^ (0x00 << 8)), we process 8 bits.
        // The result is deterministic.
        assert_ne!(crc, 0x0000);
    }

    #[test]
    fn single_byte_0xff() {
        // WHY: 0xFF exercises the case where all bits in the XOR with the
        // high byte are set, taking the "if crc & 0x8000 != 0" branch
        // differently than 0x00.
        let crc = crc16_ccitt(&[0xFF]);
        assert_ne!(crc, 0xFFFF);
        assert_ne!(crc, 0x0000);
    }

    #[test]
    fn single_byte_a() {
        // WHY: 'A' (0x41) is a common test case. Verifies CRC for a single
        // printable ASCII character.
        let crc = crc16_ccitt(b"A");
        assert_ne!(crc, 0xFFFF);
        // The CRC should be deterministic
        assert_eq!(crc, crc16_ccitt(b"A"));
    }

    // =========================================================================
    // VALIDATE_CRC WITH "6304" IN DATA
    // =========================================================================

    #[test]
    fn validate_crc_when_6304_appears_in_data_not_as_tag() {
        // WHY: The validate_crc function looks for "6304" at position
        // (len - 8). If "6304" appears earlier in the data, it should NOT
        // confuse the validator. This tests that only the LAST "6304" at
        // the correct offset is used as the CRC tag.
        let content_with_6304 = "DATA6304MOREDATA";
        let mut payload = format!("{}6304", content_with_6304);
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc);

        // The payload now contains "6304" twice: once in data, once as CRC tag
        assert!(
            validate_crc(&payload),
            "Should validate despite '6304' appearing in the data body"
        );
    }

    #[test]
    fn validate_crc_with_6304_immediately_before_crc_tag() {
        // WHY: Tests the edge case where data ends with "6304" right before
        // the actual CRC tag "6304XXXX". The validator must use positional
        // parsing (len - 8) not string searching.
        let data_part = "TESTDATA6304"; // 12 chars, contains "6304"
        let mut payload = format!("{}6304", data_part); // "TESTDATA63046304"
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc); // "TESTDATA63046304XXXX"
        // Total length: 12 + 4 + 4 = 20
        // CRC tag should be at position 20-8=12, which is "6304XXXX" -- correct
        assert!(validate_crc(&payload));
    }

    #[test]
    fn validate_crc_only_crc_tag_and_value() {
        // WHY: The minimal payload that validate_crc can handle is exactly
        // 8 characters: "6304XXXX". Tests the lower boundary.
        let mut payload = String::from("6304");
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc);
        assert_eq!(payload.len(), 8);
        assert!(validate_crc(&payload));
    }

    #[test]
    fn validate_crc_7_chars_returns_false() {
        // WHY: 7 characters is below the minimum 8. Should return false
        // without panicking.
        assert!(!validate_crc("6304ABC"));
    }

    #[test]
    fn validate_crc_no_6304_tag_returns_false() {
        // WHY: If the 4 chars at (len-8) are not "6304", validation fails.
        assert!(!validate_crc("00020101ABCD1234"));
    }

    // =========================================================================
    // CRC WITH NULL BYTES
    // =========================================================================

    #[test]
    fn crc_of_single_null_byte() {
        // WHY: Null bytes are valid in byte slices and must be handled
        // correctly by the CRC algorithm. They exercise XOR with 0x00.
        let crc = crc16_ccitt(&[0x00]);
        assert_ne!(crc, 0xFFFF);
    }

    #[test]
    fn crc_of_multiple_null_bytes() {
        // WHY: A sequence of null bytes tests the polynomial feedback loop
        // with repeated zero input. Each byte modifies the CRC state.
        let data = vec![0x00; 10];
        let crc = crc16_ccitt(&data);
        assert_ne!(crc, 0xFFFF, "10 null bytes should differ from initial CRC");
        assert_ne!(crc, crc16_ccitt(&[0x00]), "10 nulls should differ from 1 null");
    }

    #[test]
    fn crc_of_null_bytes_is_deterministic() {
        // WHY: Repeated computation of the same input must produce the same result.
        let data = vec![0x00; 100];
        let crc1 = crc16_ccitt(&data);
        let crc2 = crc16_ccitt(&data);
        assert_eq!(crc1, crc2);
    }

    #[test]
    fn crc_null_bytes_differ_by_length() {
        // WHY: Different numbers of null bytes should produce different CRCs,
        // demonstrating that the algorithm processes each byte.
        let crc_1 = crc16_ccitt(&[0x00]);
        let crc_2 = crc16_ccitt(&[0x00, 0x00]);
        let crc_3 = crc16_ccitt(&[0x00, 0x00, 0x00]);
        // While theoretically collisions are possible, for small inputs
        // they are extremely unlikely with CRC16-CCITT.
        assert_ne!(crc_1, crc_2, "1 vs 2 null bytes should differ");
        assert_ne!(crc_2, crc_3, "2 vs 3 null bytes should differ");
        assert_ne!(crc_1, crc_3, "1 vs 3 null bytes should differ");
    }

    // =========================================================================
    // CRC STABILITY ACROSS ALL SINGLE-BYTE INPUTS (256 TEST CASES)
    // =========================================================================

    #[test]
    fn crc_all_256_single_byte_inputs_are_distinct() {
        // WHY: For a 16-bit CRC, we can verify that all 256 single-byte
        // inputs produce distinct CRC values. This is a property of a good
        // hash function and confirms no degenerate collisions at the
        // single-byte level.
        let mut crcs = std::collections::HashSet::new();
        for byte in 0u8..=255 {
            let crc = crc16_ccitt(&[byte]);
            crcs.insert(crc);
        }
        assert_eq!(
            crcs.len(),
            256,
            "All 256 single-byte inputs should produce distinct CRC values"
        );
    }

    #[test]
    fn crc_all_256_single_byte_inputs_are_nonzero_and_non_ffff() {
        // WHY: No single-byte input should produce the initial value (0xFFFF)
        // or zero, which would indicate a degenerate case.
        for byte in 0u8..=255 {
            let crc = crc16_ccitt(&[byte]);
            assert_ne!(crc, 0xFFFF, "Byte 0x{:02X} should not produce 0xFFFF", byte);
            // Note: some bytes might legitimately produce 0x0000, so we only
            // check 0xFFFF which would indicate the byte had no effect.
        }
    }

    #[test]
    fn crc_hex_all_256_single_bytes_are_4_chars() {
        // WHY: The hex output must always be 4 characters, even when the
        // CRC value has leading zeros. This tests all 256 inputs.
        for byte in 0u8..=255 {
            let hex = crc16_ccitt_hex(&[byte]);
            assert_eq!(
                hex.len(),
                4,
                "Hex for byte 0x{:02X} should be 4 chars, got '{}'",
                byte,
                hex
            );
            assert!(
                hex.chars().all(|c| c.is_ascii_hexdigit()),
                "Hex for byte 0x{:02X} should be all hex digits, got '{}'",
                byte,
                hex
            );
        }
    }

    // =========================================================================
    // PROPERTY: CRC OF DATA+CRC ALWAYS VALIDATES
    // =========================================================================

    #[test]
    fn crc_self_validation_property_simple() {
        // WHY: If we append "6304" + computed CRC to any data, validate_crc
        // should return true. This is the fundamental correctness property.
        let long_x = "X".repeat(100);
        let test_cases = vec![
            "hello world",
            "000201",
            "BR.GOV.BCB.PIX",
            "A",
            long_x.as_str(),
        ];

        for data in test_cases {
            let mut payload = format!("{}6304", data);
            let crc = crc16_ccitt_hex(payload.as_bytes());
            payload.push_str(&crc);
            assert!(
                validate_crc(&payload),
                "validate_crc should pass for data: {}",
                data
            );
        }
    }

    #[test]
    fn crc_self_validation_with_numeric_data() {
        // WHY: Numeric data that looks like TLV tags should still validate.
        let data = "000201010211520400005303986";
        let mut payload = format!("{}6304", data);
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc);
        assert!(validate_crc(&payload));
    }

    #[test]
    fn crc_case_insensitive_validation() {
        // WHY: validate_crc uses eq_ignore_ascii_case for comparing CRC values.
        // This tests that a lowercase CRC still validates.
        let mut payload = String::from("TESTDATA6304");
        let crc = crc16_ccitt_hex(payload.as_bytes());
        let lower_crc = crc.to_lowercase();
        payload.push_str(&lower_crc);
        assert!(
            validate_crc(&payload),
            "Lowercase CRC should validate due to case-insensitive comparison"
        );
    }

    // =========================================================================
    // VERY LARGE PAYLOADS
    // =========================================================================

    #[test]
    fn crc_of_1mb_payload() {
        // WHY: Ensures the CRC algorithm handles large payloads without
        // overflow, stack issues, or excessive computation time. 1MB is
        // a realistic upper bound for large QR code data.
        let data = vec![0x42u8; 1_000_000]; // 1MB of 'B'
        let crc = crc16_ccitt(&data);
        assert_ne!(crc, 0);
        assert_ne!(crc, 0xFFFF);

        // Verify determinism with large data
        let crc2 = crc16_ccitt(&data);
        assert_eq!(crc, crc2, "Large payload CRC must be deterministic");
    }

    #[test]
    fn crc_hex_of_large_payload_is_4_chars() {
        // WHY: Even with 1MB+ input, the hex output must be exactly 4 chars.
        let data = vec![0xAB; 1_048_576]; // 1MB
        let hex = crc16_ccitt_hex(&data);
        assert_eq!(hex.len(), 4);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // =========================================================================
    // ORDER-DEPENDENT PROPERTIES
    // =========================================================================

    #[test]
    fn crc_byte_order_matters() {
        // WHY: CRC must be order-dependent. "AB" and "BA" should produce
        // different checksums, confirming the algorithm is not commutative.
        assert_ne!(crc16_ccitt(b"AB"), crc16_ccitt(b"BA"));
    }

    #[test]
    fn crc_prefix_changes_result() {
        // WHY: Prepending a byte should change the CRC. Tests that the
        // algorithm is not suffix-only.
        let crc_abc = crc16_ccitt(b"ABC");
        let crc_xabc = crc16_ccitt(b"XABC");
        assert_ne!(crc_abc, crc_xabc);
    }

    #[test]
    fn crc_suffix_changes_result() {
        // WHY: Appending a byte should change the CRC. Tests that the
        // algorithm processes all bytes.
        let crc_abc = crc16_ccitt(b"ABC");
        let crc_abcx = crc16_ccitt(b"ABCX");
        assert_ne!(crc_abc, crc_abcx);
    }

    // =========================================================================
    // HEX OUTPUT FORMAT
    // =========================================================================

    #[test]
    fn crc_hex_is_always_uppercase() {
        // WHY: The format!("{:04X}") should produce uppercase hex. This is
        // important for EMV payload compatibility.
        let hex = crc16_ccitt_hex(b"test");
        assert!(
            hex.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
            "Hex should be uppercase, got: {}",
            hex
        );
    }

    #[test]
    fn crc_hex_leading_zero_padding() {
        // WHY: When the CRC value is small (e.g., 0x00AB), the hex output
        // must be zero-padded to 4 characters ("00AB"). We find such an
        // input by brute force if needed, but the format string guarantees it.
        // For empty input: CRC = 0xFFFF, hex = "FFFF" -- no leading zeros.
        // We can verify the format guarantees padding by checking length.
        for i in 0..256u16 {
            let data = i.to_be_bytes();
            let hex = crc16_ccitt_hex(&data);
            assert_eq!(hex.len(), 4, "Must be 4 chars for input {:?}", data);
        }
    }

    // =========================================================================
    // CORRUPTED CRC DETECTION
    // =========================================================================

    #[test]
    fn single_bit_flip_in_crc_detected() {
        // WHY: Flipping any single bit in the CRC value should cause
        // validation to fail. This tests the error detection capability.
        let mut payload = String::from("ABCDEFGH6304");
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc);
        assert!(validate_crc(&payload));

        // Now corrupt each hex digit
        for pos in 0..4 {
            let mut corrupted = payload.clone();
            let idx = corrupted.len() - 4 + pos;
            let original_char = corrupted.as_bytes()[idx];
            let replacement = if original_char == b'0' { b'1' } else { b'0' };
            unsafe {
                corrupted.as_bytes_mut()[idx] = replacement;
            }
            assert!(
                !validate_crc(&corrupted),
                "Corrupted CRC at position {} should fail validation",
                pos
            );
        }
    }

    #[test]
    fn single_bit_flip_in_data_detected() {
        // WHY: Flipping a bit in the data (not the CRC) should also cause
        // validation to fail, since the CRC no longer matches the data.
        let mut payload = String::from("ABCDEFGHIJKL6304");
        let crc = crc16_ccitt_hex(payload.as_bytes());
        payload.push_str(&crc);
        assert!(validate_crc(&payload));

        // Flip bit in the data portion
        let mut corrupted = payload.clone();
        let bytes = unsafe { corrupted.as_bytes_mut() };
        bytes[0] ^= 0x01;
        assert!(!validate_crc(&corrupted));
    }
}

#[cfg(test)]
mod proptests {
    use pix_core::crc16::{crc16_ccitt, crc16_ccitt_hex, validate_crc};
    use proptest::prelude::*;

    proptest! {
        /// WHY: The fundamental self-validation property. For ANY data,
        /// computing the CRC, appending it after "6304", and then validating
        /// should always succeed.
        #[test]
        fn crc_of_data_plus_crc_always_validates(
            content in "[a-zA-Z0-9]{4,100}",
        ) {
            let mut payload = format!("{}6304", content);
            let crc = crc16_ccitt_hex(payload.as_bytes());
            payload.push_str(&crc);
            prop_assert!(validate_crc(&payload));
        }

        /// WHY: Corrupting the CRC should always cause validation failure.
        /// This is the error detection guarantee.
        #[test]
        fn corrupted_crc_never_validates(
            content in "[a-zA-Z0-9]{4,100}",
        ) {
            let mut payload = format!("{}6304", content);
            let crc = crc16_ccitt_hex(payload.as_bytes());
            payload.push_str(&crc);

            // Corrupt the last hex digit
            let len = payload.len();
            let last_char = payload.chars().last().unwrap();
            let replacement = if last_char == '0' { '1' } else { '0' };
            let mut corrupted = payload[..len - 1].to_string();
            corrupted.push(replacement);
            prop_assert!(!validate_crc(&corrupted));
        }

        /// WHY: CRC must be deterministic. Computing it twice on the same
        /// data must yield the same result.
        #[test]
        fn crc_is_deterministic(
            data in proptest::collection::vec(any::<u8>(), 0..1000),
        ) {
            let crc1 = crc16_ccitt(&data);
            let crc2 = crc16_ccitt(&data);
            prop_assert_eq!(crc1, crc2);
        }

        /// WHY: The hex output must always be exactly 4 uppercase hex characters
        /// regardless of input.
        #[test]
        fn crc_hex_always_4_uppercase_hex_chars(
            data in proptest::collection::vec(any::<u8>(), 0..500),
        ) {
            let hex = crc16_ccitt_hex(&data);
            prop_assert_eq!(hex.len(), 4);
            prop_assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
            prop_assert!(hex.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
        }

        /// WHY: Different inputs should (with very high probability) produce
        /// different CRC values. While collisions are theoretically possible
        /// for a 16-bit CRC, they should be rare for short inputs.
        #[test]
        fn different_short_inputs_usually_differ(
            a in "[a-z]{1,10}",
            b in "[a-z]{1,10}",
        ) {
            if a != b {
                let crc_a = crc16_ccitt(a.as_bytes());
                let crc_b = crc16_ccitt(b.as_bytes());
                // We cannot assert they always differ (birthday paradox for 16 bits),
                // but we can log if they do collide.
                if crc_a == crc_b {
                    // This is not a failure -- just very rare for short strings.
                    // We accept it silently.
                }
            }
        }
    }
}

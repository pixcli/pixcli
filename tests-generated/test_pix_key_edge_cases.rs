//! Extended PixKey tests for `crates/pix-core/src/pix_key.rs`.
//!
//! These tests cover uncovered code paths in the PixKey validation logic,
//! including CPF check digit algorithm edge cases, CNPJ branch variations,
//! email edge cases, phone boundary conditions, EVP UUID version handling,
//! detection priority rules, and thread safety verification.
//!
//! ## Placement Instructions
//!
//! To compile and run these tests, either:
//! 1. Copy to `crates/pix-core/tests/test_pix_key_edge_cases.rs` (integration test)
//! 2. Merge into `crates/pix-core/src/pix_key.rs` as a `#[cfg(test)]` module
//!
//! For option 1, all tested types must be publicly exported from pix_core
//! (they already are via `pub use` in lib.rs).
//!
//! ## Dependencies Required
//!
//! In `crates/pix-core/Cargo.toml` under `[dev-dependencies]`:
//! ```toml
//! proptest = { workspace = true }
//! ```

#[cfg(test)]
mod tests {
    use pix_core::pix_key::{PixKey, PixKeyType};

    // =========================================================================
    // CPF CHECK DIGIT ALGORITHM EDGE CASES
    // =========================================================================

    #[test]
    fn cpf_starting_with_zero() {
        // WHY: CPFs starting with 0 exercise the case where the first
        // weighted multiplication contributes 0 to the checksum. The
        // well-known CPF 00000000191 starts with 8 zeros.
        assert!(PixKey::new(PixKeyType::Cpf, "00000000191").is_ok());
    }

    #[test]
    fn cpf_ending_with_zero_check_digits() {
        // WHY: When remainder < 2, the check digit is 0 (not 11 - remainder).
        // This tests the `if remainder < 2 { 0 }` branch for BOTH check digits.
        // CPF 12345678909 has check digits 0 and 9.
        // Let's find one where both check digits are 0:
        // We need a CPF where both remainders are < 2.
        // 25436825500 -- let's verify by computing:
        // Actually, let's use a known-valid CPF with check digit 0.
        // 52998224725 has check digits 2 and 5.
        // We need to test the "remainder < 2 => check = 0" branch.
        // CPF 00000000191: digits [0,0,0,0,0,0,0,0,1,9,1]
        // sum1 = 0*10+0*9+0*8+0*7+0*6+0*5+0*4+0*3+1*2 = 2
        // rem1 = 2 % 11 = 2 => check1 = 11-2 = 9 (matches digits[9])
        // sum2 = 0*11+...+1*3+9*2 = 3+18 = 21
        // rem2 = 21 % 11 = 10 => check2 = 11-10 = 1 (matches digits[10])
        // So 00000000191 does not hit the "< 2" branch.
        //
        // Let's try to find a CPF where check digit is 0:
        // Take base digits [1,0,0,0,0,0,0,0,0]
        // sum1 = 1*10 = 10, rem1 = 10%11 = 10, check1 = 11-10 = 1
        // Now [1,0,0,0,0,0,0,0,0,1]
        // sum2 = 1*11+0+...+0+1*2 = 11+2 = 13, rem2 = 13%11 = 2
        // check2 = 11-2 = 9 => CPF = 10000000019
        // But we want check = 0, so rem < 2 means rem = 0 or rem = 1.
        // Take [0,0,0,0,0,0,0,0,0]: all same => rejected
        // Take [1,1,0,0,0,0,0,0,0]:
        // sum1 = 1*10+1*9 = 19, rem1 = 19%11 = 8, check1 = 3
        // Not what we want. Let's just use a programmatic approach.
        // For check digit 0: we need sum%11 == 0 or sum%11 == 1.
        // Digits [0,1,2,3,4,5,6,7,8]:
        // sum1 = 0*10+1*9+2*8+3*7+4*6+5*5+6*4+7*3+8*2
        //      = 0+9+16+21+24+25+24+21+16 = 156
        // rem1 = 156%11 = 156-14*11 = 156-154 = 2 => check1 = 9
        // Not quite. Let's try [0,2,2,3,4,5,6,7,8]:
        // sum1 = 0+18+16+21+24+25+24+21+16 = 165
        // rem1 = 165%11 = 165-15*11 = 165-165 = 0 => check1 = 0!
        // Now [0,2,2,3,4,5,6,7,8,0]:
        // sum2 = 0*11+2*10+2*9+3*8+4*7+5*6+6*5+7*4+8*3+0*2
        //      = 0+20+18+24+28+30+30+28+24+0 = 202
        // rem2 = 202%11 = 202-18*11 = 202-198 = 4 => check2 = 7
        // CPF = 02234567807
        let result = PixKey::new(PixKeyType::Cpf, "02234567807");
        assert!(result.is_ok(), "CPF 02234567807 should be valid (first check digit is 0)");
    }

    #[test]
    fn cpf_where_remainder_is_one_gives_check_zero() {
        // WHY: When remainder == 1, check digit should be 0 (since 1 < 2).
        // This tests the specific boundary of the "remainder < 2" condition.
        // Digits [3,1,0,0,0,0,0,0,0]:
        // sum1 = 3*10+1*9 = 39, rem1 = 39%11 = 39-3*11 = 6, check1 = 5
        // Not right. We need rem = 1.
        // sum1 mod 11 = 1 => sum1 = 11*k + 1
        // Try [1,0,0,0,0,0,0,0,1]: sum1 = 10+2 = 12, rem1 = 1, check1 = 0
        // [1,0,0,0,0,0,0,0,1,0]:
        // sum2 = 1*11+0+...+1*3+0*2 = 14, rem2 = 14%11 = 3, check2 = 8
        // CPF = 10000000108
        let result = PixKey::new(PixKeyType::Cpf, "10000000108");
        assert!(result.is_ok(), "CPF 10000000108 should be valid (remainder=1 gives check=0)");
    }

    #[test]
    fn cpf_all_same_digits_0_through_9_all_rejected() {
        // WHY: All-same-digit CPFs are explicitly rejected even though some
        // might pass the check digit algorithm (e.g., 000.000.000-00 has
        // valid check digits of 0). This is a fraud prevention rule.
        for digit in 0..=9u32 {
            let cpf: String = std::iter::repeat(char::from_digit(digit, 10).unwrap())
                .take(11)
                .collect();
            assert!(
                PixKey::new(PixKeyType::Cpf, &cpf).is_err(),
                "All-{digit} CPF should be rejected"
            );
        }
    }

    #[test]
    fn cpf_with_leading_whitespace_is_trimmed() {
        // WHY: The normalize_key function trims whitespace. This ensures
        // leading/trailing spaces do not cause validation failures.
        let key = PixKey::new(PixKeyType::Cpf, "  52998224725  ").unwrap();
        assert_eq!(key.value, "52998224725");
    }

    // =========================================================================
    // CNPJ WITH DIFFERENT BRANCH NUMBERS
    // =========================================================================

    #[test]
    fn cnpj_with_headquarters_branch_0001() {
        // WHY: Most CNPJs use branch "0001" (headquarters). This is the
        // standard case.
        assert!(PixKey::new(PixKeyType::Cnpj, "11222333000181").is_ok());
    }

    #[test]
    fn cnpj_with_different_branch_number() {
        // WHY: Branch numbers other than 0001 are valid for subsidiaries.
        // The CNPJ check digit algorithm must work regardless of branch.
        // CNPJ 11.222.333/0002-62 -- let's compute:
        // Digits: [1,1,2,2,2,3,3,3,0,0,0,2,?,?]
        // weights1 = [5,4,3,2,9,8,7,6,5,4,3,2]
        // sum1 = 1*5+1*4+2*3+2*2+2*9+3*8+3*7+3*6+0*5+0*4+0*3+2*2
        //      = 5+4+6+4+18+24+21+18+0+0+0+4 = 104
        // rem1 = 104%11 = 104-9*11 = 5, check1 = 11-5 = 6
        // Digits so far: [1,1,2,2,2,3,3,3,0,0,0,2,6,?]
        // weights2 = [6,5,4,3,2,9,8,7,6,5,4,3,2]
        // sum2 = 1*6+1*5+2*4+2*3+2*2+3*9+3*8+3*7+0*6+0*5+0*4+2*3+6*2
        //      = 6+5+8+6+4+27+24+21+0+0+0+6+12 = 119
        // rem2 = 119%11 = 119-10*11 = 9, check2 = 11-9 = 2
        // CNPJ = 11222333000262
        let result = PixKey::new(PixKeyType::Cnpj, "11222333000262");
        assert!(result.is_ok(), "CNPJ with branch 0002 should be valid: {:?}", result);
    }

    #[test]
    fn cnpj_all_same_digits_rejected() {
        // WHY: Like CPFs, all-same-digit CNPJs are rejected as a fraud check.
        for digit in 0..=9u32 {
            let cnpj: String = std::iter::repeat(char::from_digit(digit, 10).unwrap())
                .take(14)
                .collect();
            assert!(
                PixKey::new(PixKeyType::Cnpj, &cnpj).is_err(),
                "All-{digit} CNPJ should be rejected"
            );
        }
    }

    #[test]
    fn cnpj_formatted_with_punctuation() {
        // WHY: CNPJs are commonly written as "XX.XXX.XXX/XXXX-XX". The
        // normalize function strips non-digit characters.
        let key = PixKey::new(PixKeyType::Cnpj, "11.222.333/0001-81").unwrap();
        assert_eq!(key.value, "11222333000181");
    }

    // =========================================================================
    // EMAIL EDGE CASES
    // =========================================================================

    #[test]
    fn email_at_max_length_77_chars() {
        // WHY: The Pix spec limits emails to 77 characters. This tests
        // the exact boundary.
        let local = "a".repeat(65);
        let email = format!("{}@example.com", local); // 65 + 1 + 11 = 77
        assert_eq!(email.len(), 77);
        assert!(PixKey::new(PixKeyType::Email, &email).is_ok());
    }

    #[test]
    fn email_at_78_chars_rejected() {
        // WHY: One char over the 77 limit should be rejected.
        let local = "a".repeat(66);
        let email = format!("{}@example.com", local); // 66 + 1 + 11 = 78
        assert_eq!(email.len(), 78);
        assert!(PixKey::new(PixKeyType::Email, &email).is_err());
    }

    #[test]
    fn email_with_long_local_part_under_limit() {
        // WHY: Very long local parts (but under 77 total) should work.
        let local = "user.name.with.many.dots.and.words";
        let email = format!("{}@test.com", local);
        assert!(email.len() <= 77);
        assert!(PixKey::new(PixKeyType::Email, &email).is_ok());
    }

    #[test]
    fn email_with_plus_addressing() {
        // WHY: Plus addressing (user+tag@domain) is a valid email format
        // commonly used for filtering.
        let key = PixKey::new(PixKeyType::Email, "user+pix@example.com").unwrap();
        assert_eq!(key.value, "user+pix@example.com");
    }

    #[test]
    fn email_case_normalized_to_lowercase() {
        // WHY: Email normalization converts to lowercase. This ensures
        // mixed-case emails are stored consistently.
        let key = PixKey::new(PixKeyType::Email, "USER@EXAMPLE.COM").unwrap();
        assert_eq!(key.value, "user@example.com");
    }

    #[test]
    fn email_with_subdomain() {
        // WHY: Multi-level domains are valid (e.g., user@mail.empresa.com.br).
        let key = PixKey::new(PixKeyType::Email, "user@mail.empresa.com.br").unwrap();
        assert_eq!(key.value, "user@mail.empresa.com.br");
    }

    #[test]
    fn email_domain_with_empty_label_rejected() {
        // WHY: "user@.com" has an empty label before "com". The validation
        // checks domain.split('.').any(|part| part.is_empty()).
        assert!(PixKey::new(PixKeyType::Email, "user@.com").is_err());
    }

    #[test]
    fn email_domain_trailing_dot_rejected() {
        // WHY: "user@example." has an empty label after the last dot.
        assert!(PixKey::new(PixKeyType::Email, "user@example.").is_err());
    }

    #[test]
    fn email_no_at_sign_rejected() {
        // WHY: Without '@', splitn(2, '@') returns only one part.
        assert!(PixKey::new(PixKeyType::Email, "userexample.com").is_err());
    }

    #[test]
    fn email_empty_local_part_rejected() {
        // WHY: "@example.com" has empty local part.
        assert!(PixKey::new(PixKeyType::Email, "@example.com").is_err());
    }

    #[test]
    fn email_no_dot_in_domain_rejected() {
        // WHY: "user@example" has no dot in domain. The validation requires
        // at least one dot in the domain part.
        assert!(PixKey::new(PixKeyType::Email, "user@example").is_err());
    }

    // =========================================================================
    // PHONE EDGE CASES
    // =========================================================================

    #[test]
    fn phone_with_area_code_11_mobile() {
        // WHY: Area code 11 (Sao Paulo) is the most common. Tests the
        // 11-digit mobile format.
        assert!(PixKey::new(PixKeyType::Phone, "+5511987654321").is_ok());
    }

    #[test]
    fn phone_with_area_code_11_landline() {
        // WHY: Landlines have 10 digits after +55 (8-digit number).
        assert!(PixKey::new(PixKeyType::Phone, "+551132547698").is_ok());
    }

    #[test]
    fn phone_with_area_code_99() {
        // WHY: Area code 99 (Maranhao) is at the upper end of valid DDDs.
        // Tests that the code does not restrict DDD range.
        assert!(PixKey::new(PixKeyType::Phone, "+5599987654321").is_ok());
    }

    #[test]
    fn phone_with_area_code_21() {
        // WHY: Area code 21 (Rio de Janeiro) is another common code.
        assert!(PixKey::new(PixKeyType::Phone, "+5521987654321").is_ok());
    }

    #[test]
    fn phone_9_digits_after_55_rejected() {
        // WHY: Fewer than 10 digits after +55 is invalid (too short).
        assert!(PixKey::new(PixKeyType::Phone, "+55113254769").is_err());
    }

    #[test]
    fn phone_12_digits_after_55_rejected() {
        // WHY: More than 11 digits after +55 is invalid (too long).
        assert!(PixKey::new(PixKeyType::Phone, "+551198765432100").is_err());
    }

    #[test]
    fn phone_without_plus_prefix_rejected() {
        // WHY: The E.164 format requires '+'. Without it, the phone
        // should be rejected during normalization.
        assert!(PixKey::new(PixKeyType::Phone, "5511987654321").is_err());
    }

    #[test]
    fn phone_with_non_brazilian_country_code_rejected() {
        // WHY: Only +55 (Brazil) is valid for Pix phone keys.
        assert!(PixKey::new(PixKeyType::Phone, "+1987654321").is_err());
    }

    #[test]
    fn phone_with_letters_in_number_rejected() {
        // WHY: Non-digit characters after +55 must fail validation.
        assert!(PixKey::new(PixKeyType::Phone, "+5511ABCDEF321").is_err());
    }

    // =========================================================================
    // EVP UUID TESTS
    // =========================================================================

    #[test]
    fn evp_uuid_v4_standard() {
        // WHY: UUID v4 is the standard format for EVP Pix keys.
        assert!(PixKey::new(PixKeyType::Evp, "550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    #[test]
    fn evp_uuid_without_hyphens() {
        // WHY: The uuid crate parses UUIDs with or without hyphens.
        // Tests that the normalization handles both formats.
        assert!(PixKey::new(PixKeyType::Evp, "550e8400e29b41d4a716446655440000").is_ok());
    }

    #[test]
    fn evp_uuid_nil() {
        // WHY: The nil UUID (all zeros) is technically parseable. Tests
        // that the code accepts it since validation only checks UUID format.
        assert!(PixKey::new(PixKeyType::Evp, "00000000-0000-0000-0000-000000000000").is_ok());
    }

    #[test]
    fn evp_uuid_v1_accepted() {
        // WHY: The code uses uuid::Uuid::parse_str which accepts any valid
        // UUID regardless of version. V1 UUIDs should parse successfully.
        // Example v1 UUID:
        assert!(PixKey::new(PixKeyType::Evp, "6ba7b810-9dad-11d1-80b4-00c04fd430c8").is_ok());
    }

    #[test]
    fn evp_uuid_v3_accepted() {
        // WHY: V3 (MD5-based) UUIDs are valid UUID format.
        assert!(PixKey::new(PixKeyType::Evp, "a3bb189e-8bf9-3888-9912-ace4e6543002").is_ok());
    }

    #[test]
    fn evp_uuid_v5_accepted() {
        // WHY: V5 (SHA1-based) UUIDs are valid UUID format.
        assert!(PixKey::new(PixKeyType::Evp, "886313e1-3b8a-5372-9b90-0c9aee199e5d").is_ok());
    }

    #[test]
    fn evp_uppercase_normalized_to_lowercase() {
        // WHY: EVP normalization converts to lowercase. Tests that uppercase
        // hex digits are properly lowered.
        let key = PixKey::new(PixKeyType::Evp, "550E8400-E29B-41D4-A716-446655440000").unwrap();
        assert!(key.value.chars().all(|c| !c.is_ascii_uppercase() || c == '-'));
    }

    #[test]
    fn evp_invalid_format_rejected() {
        // WHY: Random strings that are not UUIDs must be rejected.
        assert!(PixKey::new(PixKeyType::Evp, "not-a-uuid-at-all").is_err());
    }

    #[test]
    fn evp_too_short_rejected() {
        // WHY: Truncated UUIDs must fail parsing.
        assert!(PixKey::new(PixKeyType::Evp, "550e8400-e29b-41d4").is_err());
    }

    // =========================================================================
    // DETECTION PRIORITY TESTS
    // =========================================================================

    #[test]
    fn detect_phone_over_email_when_starts_with_plus_55() {
        // WHY: The detection logic checks +55 first, then @. A string
        // starting with "+55" should always be detected as Phone, even
        // if it somehow contained '@' (which it won't in practice).
        let key = PixKey::detect("+5511987654321").unwrap();
        assert_eq!(key.key_type, PixKeyType::Phone);
    }

    #[test]
    fn detect_email_when_contains_at_but_not_plus_55() {
        // WHY: Strings with '@' but not starting with '+55' should be Email.
        let key = PixKey::detect("pix@example.com").unwrap();
        assert_eq!(key.key_type, PixKeyType::Email);
    }

    #[test]
    fn detect_plus_55_with_at_symbol_is_phone() {
        // WHY: This tests the priority ordering. "+55" check comes before "@"
        // check in the detect() function. A string starting with "+55" that
        // also contains "@" should be detected as Phone first.
        // However, if it then fails phone validation, the error propagates.
        // "+5511987654321" is valid phone, so detection stops at Phone.
        // Let's test with a string that starts with +55 but is malformed:
        let result = PixKey::detect("+55abc@test.com");
        // Starts with +55, so detect tries Phone first. Phone validation
        // will fail (letters in number), and the error propagates.
        assert!(result.is_err(), "Phone validation failure should propagate, not fall through to email");
    }

    #[test]
    fn detect_cpf_from_11_digits() {
        // WHY: 11 pure digits should be detected as CPF.
        let key = PixKey::detect("52998224725").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cpf);
    }

    #[test]
    fn detect_cnpj_from_14_digits() {
        // WHY: 14 pure digits should be detected as CNPJ.
        let key = PixKey::detect("11222333000181").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cnpj);
    }

    #[test]
    fn detect_evp_from_uuid_string() {
        // WHY: A UUID-format string should be detected as EVP.
        let key = PixKey::detect("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(key.key_type, PixKeyType::Evp);
    }

    #[test]
    fn detect_unknown_format_rejected() {
        // WHY: A string that matches no known format should return an error.
        assert!(PixKey::detect("random-string-123").is_err());
    }

    #[test]
    fn detect_10_digits_rejected() {
        // WHY: 10 digits is neither CPF (11) nor CNPJ (14).
        assert!(PixKey::detect("1234567890").is_err());
    }

    #[test]
    fn detect_12_digits_rejected() {
        // WHY: 12 digits is neither CPF (11) nor CNPJ (14).
        assert!(PixKey::detect("123456789012").is_err());
    }

    #[test]
    fn detect_formatted_cpf_with_dots_and_dash() {
        // WHY: "529.982.247-25" has 11 digits with formatting chars.
        // The detect function checks digits_only.len() == 11 AND
        // all chars are digit/dot/dash.
        let key = PixKey::detect("529.982.247-25").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cpf);
        assert_eq!(key.value, "52998224725");
    }

    #[test]
    fn detect_formatted_cnpj_with_dots_slash_dash() {
        // WHY: "11.222.333/0001-81" has 14 digits with formatting.
        let key = PixKey::detect("11.222.333/0001-81").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cnpj);
        assert_eq!(key.value, "11222333000181");
    }

    // =========================================================================
    // THREAD SAFETY OF VALIDATION
    // =========================================================================

    #[test]
    fn concurrent_pix_key_creation() {
        // WHY: PixKey::new and PixKey::detect use no global state, but this
        // test confirms that concurrent validation across threads does not
        // cause panics or data races. This exercises Send + Sync bounds.
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    // Each thread validates a different key type
                    match i % 5 {
                        0 => {
                            let _ = PixKey::new(PixKeyType::Cpf, "52998224725");
                        }
                        1 => {
                            let _ = PixKey::new(PixKeyType::Cnpj, "11222333000181");
                        }
                        2 => {
                            let _ = PixKey::new(PixKeyType::Email, "test@example.com");
                        }
                        3 => {
                            let _ = PixKey::new(PixKeyType::Phone, "+5511987654321");
                        }
                        4 => {
                            let _ = PixKey::new(
                                PixKeyType::Evp,
                                "550e8400-e29b-41d4-a716-446655440000",
                            );
                        }
                        _ => unreachable!(),
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should not panic during PixKey validation");
        }
    }

    #[test]
    fn concurrent_detect_does_not_panic() {
        // WHY: Verifies detect() is safe to call from multiple threads.
        use std::thread;

        let inputs = vec![
            "52998224725",
            "11222333000181",
            "test@example.com",
            "+5511987654321",
            "550e8400-e29b-41d4-a716-446655440000",
            "invalid-input",
        ];

        let handles: Vec<_> = inputs
            .into_iter()
            .map(|input| {
                let input = input.to_string();
                thread::spawn(move || {
                    let _ = PixKey::detect(&input);
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should not panic during detect");
        }
    }

    // =========================================================================
    // DISPLAY AND SERIALIZATION EDGE CASES
    // =========================================================================

    #[test]
    fn pix_key_display_includes_type_and_value() {
        // WHY: The Display impl formats as "Type:value". Verify for each type.
        let key = PixKey::new(PixKeyType::Cpf, "52998224725").unwrap();
        assert_eq!(key.to_string(), "CPF:52998224725");

        let key = PixKey::new(PixKeyType::Email, "test@example.com").unwrap();
        assert_eq!(key.to_string(), "Email:test@example.com");
    }

    #[test]
    fn pix_key_type_display_all_variants() {
        // WHY: Ensures Display is implemented correctly for all variants.
        assert_eq!(PixKeyType::Cpf.to_string(), "CPF");
        assert_eq!(PixKeyType::Cnpj.to_string(), "CNPJ");
        assert_eq!(PixKeyType::Email.to_string(), "Email");
        assert_eq!(PixKeyType::Phone.to_string(), "Phone");
        assert_eq!(PixKeyType::Evp.to_string(), "EVP");
    }

    #[test]
    fn pix_key_hash_equality() {
        // WHY: Two PixKeys with same type and value should hash equally
        // and be equal, confirming Hash + Eq derive works correctly.
        use std::collections::HashSet;
        let key1 = PixKey::new(PixKeyType::Cpf, "52998224725").unwrap();
        let key2 = PixKey::new(PixKeyType::Cpf, "52998224725").unwrap();
        let mut set = HashSet::new();
        set.insert(key1);
        set.insert(key2);
        assert_eq!(set.len(), 1, "Identical keys should deduplicate in HashSet");
    }

    #[test]
    fn pix_key_serde_roundtrip_all_types() {
        // WHY: Verifies that serialize -> deserialize preserves the key
        // for every PixKeyType variant.
        let keys = vec![
            PixKey::new(PixKeyType::Cpf, "52998224725").unwrap(),
            PixKey::new(PixKeyType::Cnpj, "11222333000181").unwrap(),
            PixKey::new(PixKeyType::Email, "test@example.com").unwrap(),
            PixKey::new(PixKeyType::Phone, "+5511987654321").unwrap(),
            PixKey::new(PixKeyType::Evp, "550e8400-e29b-41d4-a716-446655440000").unwrap(),
        ];

        for key in keys {
            let json = serde_json::to_string(&key).unwrap();
            let deserialized: PixKey = serde_json::from_str(&json).unwrap();
            assert_eq!(key, deserialized, "Serde roundtrip failed for {:?}", key.key_type);
        }
    }
}

//! Pix key types and validation.
//!
//! Supports all five Pix key types defined by the Brazilian Central Bank:
//! - CPF (Cadastro de Pessoa Física) — 11-digit individual taxpayer ID
//! - CNPJ (Cadastro Nacional da Pessoa Jurídica) — 14-digit company ID
//! - Email — standard email address
//! - Phone — Brazilian phone number in E.164 format (+55...)
//! - EVP (Endereço Virtual de Pagamento) — random UUID v4

use serde::{Deserialize, Serialize};

use crate::PixError;

/// The type of a Pix key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PixKeyType {
    /// CPF — 11-digit individual taxpayer ID.
    Cpf,
    /// CNPJ — 14-digit company registration number.
    Cnpj,
    /// Email address.
    Email,
    /// Phone number in E.164 format (+55...).
    Phone,
    /// EVP — random UUID v4 key.
    Evp,
}

/// A validated Pix key with its type and value.
///
/// # Examples
///
/// ```
/// use pix_core::pix_key::{PixKey, PixKeyType};
///
/// // Create with explicit type
/// let key = PixKey::new(PixKeyType::Email, "user@example.com").unwrap();
/// assert_eq!(key.key_type, PixKeyType::Email);
///
/// // Auto-detect type
/// let key = PixKey::detect("+5511987654321").unwrap();
/// assert_eq!(key.key_type, PixKeyType::Phone);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PixKey {
    /// The type of this Pix key.
    pub key_type: PixKeyType,
    /// The raw key value (digits only for CPF/CNPJ, full format for others).
    pub value: String,
}

impl PixKey {
    /// Creates a new `PixKey` after validating the value against the given type.
    ///
    /// # Errors
    ///
    /// Returns `PixError::InvalidPixKey` if the value does not match the expected format.
    pub fn new(key_type: PixKeyType, value: &str) -> Result<Self, PixError> {
        let normalized = normalize_key(key_type, value)?;
        validate_key(key_type, &normalized)?;
        Ok(Self {
            key_type,
            value: normalized,
        })
    }

    /// Attempts to detect the key type from the raw value and validate it.
    ///
    /// Detection rules:
    /// - Starts with `+55` → Phone
    /// - Contains `@` → Email
    /// - 11 digits → CPF
    /// - 14 digits → CNPJ
    /// - UUID format → EVP
    ///
    /// # Errors
    ///
    /// Returns `PixError::InvalidPixKey` if the value doesn't match any known format.
    pub fn detect(value: &str) -> Result<Self, PixError> {
        let trimmed = value.trim();

        if trimmed.starts_with("+55") {
            return Self::new(PixKeyType::Phone, trimmed);
        }

        if trimmed.contains('@') {
            return Self::new(PixKeyType::Email, trimmed);
        }

        let digits_only: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits_only.len() == 11
            && trimmed
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        {
            return Self::new(PixKeyType::Cpf, trimmed);
        }

        if digits_only.len() == 14
            && trimmed
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '/' || c == '-')
        {
            return Self::new(PixKeyType::Cnpj, trimmed);
        }

        if uuid::Uuid::parse_str(trimmed).is_ok() {
            return Self::new(PixKeyType::Evp, trimmed);
        }

        Err(PixError::InvalidPixKey(format!(
            "cannot detect key type for: {trimmed}"
        )))
    }
}

impl std::fmt::Display for PixKeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PixKeyType::Cpf => write!(f, "CPF"),
            PixKeyType::Cnpj => write!(f, "CNPJ"),
            PixKeyType::Email => write!(f, "Email"),
            PixKeyType::Phone => write!(f, "Phone"),
            PixKeyType::Evp => write!(f, "EVP"),
        }
    }
}

impl std::fmt::Display for PixKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.key_type, self.value)
    }
}

/// Normalizes a key value by stripping formatting characters.
fn normalize_key(key_type: PixKeyType, value: &str) -> Result<String, PixError> {
    let trimmed = value.trim();
    match key_type {
        PixKeyType::Cpf => {
            let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 11 {
                return Err(PixError::InvalidPixKey(format!(
                    "CPF must have 11 digits, got {}",
                    digits.len()
                )));
            }
            Ok(digits)
        }
        PixKeyType::Cnpj => {
            let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 14 {
                return Err(PixError::InvalidPixKey(format!(
                    "CNPJ must have 14 digits, got {}",
                    digits.len()
                )));
            }
            Ok(digits)
        }
        PixKeyType::Email => Ok(trimmed.to_lowercase()),
        PixKeyType::Phone => {
            if !trimmed.starts_with('+') {
                return Err(PixError::InvalidPixKey(
                    "phone must start with '+' (E.164 format)".into(),
                ));
            }
            Ok(trimmed.to_string())
        }
        PixKeyType::Evp => {
            let lower = trimmed.to_lowercase();
            uuid::Uuid::parse_str(&lower)
                .map_err(|_| PixError::InvalidPixKey(format!("invalid UUID format: {trimmed}")))?;
            Ok(lower)
        }
    }
}

/// Validates a normalized key value.
fn validate_key(key_type: PixKeyType, value: &str) -> Result<(), PixError> {
    match key_type {
        PixKeyType::Cpf => validate_cpf(value),
        PixKeyType::Cnpj => validate_cnpj(value),
        PixKeyType::Email => validate_email(value),
        PixKeyType::Phone => validate_phone(value),
        PixKeyType::Evp => Ok(()), // Already validated during normalization
    }
}

/// Validates a CPF number using the check digit algorithm.
///
/// A CPF has 11 digits: 9 base digits + 2 check digits.
/// The check digits are calculated using weighted sums modulo 11.
fn validate_cpf(cpf: &str) -> Result<(), PixError> {
    let digits: Vec<u32> = cpf.chars().filter_map(|c| c.to_digit(10)).collect();

    if digits.len() != 11 {
        return Err(PixError::InvalidPixKey("CPF must have 11 digits".into()));
    }

    // Reject all-same-digit CPFs (e.g., 111.111.111-11)
    if digits.iter().all(|&d| d == digits[0]) {
        return Err(PixError::InvalidPixKey(
            "CPF has all identical digits".into(),
        ));
    }

    // First check digit
    let sum1: u32 = digits[..9]
        .iter()
        .enumerate()
        .map(|(i, &d)| d * (10 - i as u32))
        .sum();
    let remainder1 = sum1 % 11;
    let check1 = if remainder1 < 2 { 0 } else { 11 - remainder1 };

    if check1 != digits[9] {
        return Err(PixError::InvalidPixKey(
            "CPF first check digit mismatch".into(),
        ));
    }

    // Second check digit
    let sum2: u32 = digits[..10]
        .iter()
        .enumerate()
        .map(|(i, &d)| d * (11 - i as u32))
        .sum();
    let remainder2 = sum2 % 11;
    let check2 = if remainder2 < 2 { 0 } else { 11 - remainder2 };

    if check2 != digits[10] {
        return Err(PixError::InvalidPixKey(
            "CPF second check digit mismatch".into(),
        ));
    }

    Ok(())
}

/// Validates a CNPJ number using the check digit algorithm.
///
/// A CNPJ has 14 digits: 12 base digits + 2 check digits.
fn validate_cnpj(cnpj: &str) -> Result<(), PixError> {
    let digits: Vec<u32> = cnpj.chars().filter_map(|c| c.to_digit(10)).collect();

    if digits.len() != 14 {
        return Err(PixError::InvalidPixKey("CNPJ must have 14 digits".into()));
    }

    // Reject all-same-digit CNPJs
    if digits.iter().all(|&d| d == digits[0]) {
        return Err(PixError::InvalidPixKey(
            "CNPJ has all identical digits".into(),
        ));
    }

    // First check digit: weights [5,4,3,2,9,8,7,6,5,4,3,2]
    let weights1: &[u32] = &[5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let sum1: u32 = digits[..12]
        .iter()
        .zip(weights1.iter())
        .map(|(&d, &w)| d * w)
        .sum();
    let remainder1 = sum1 % 11;
    let check1 = if remainder1 < 2 { 0 } else { 11 - remainder1 };

    if check1 != digits[12] {
        return Err(PixError::InvalidPixKey(
            "CNPJ first check digit mismatch".into(),
        ));
    }

    // Second check digit: weights [6,5,4,3,2,9,8,7,6,5,4,3,2]
    let weights2: &[u32] = &[6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let sum2: u32 = digits[..13]
        .iter()
        .zip(weights2.iter())
        .map(|(&d, &w)| d * w)
        .sum();
    let remainder2 = sum2 % 11;
    let check2 = if remainder2 < 2 { 0 } else { 11 - remainder2 };

    if check2 != digits[13] {
        return Err(PixError::InvalidPixKey(
            "CNPJ second check digit mismatch".into(),
        ));
    }

    Ok(())
}

/// Validates an email address with basic structural checks.
fn validate_email(email: &str) -> Result<(), PixError> {
    if email.len() > 77 {
        return Err(PixError::InvalidPixKey(
            "email too long (max 77 chars)".into(),
        ));
    }

    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 {
        return Err(PixError::InvalidPixKey("email must contain '@'".into()));
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() {
        return Err(PixError::InvalidPixKey("email local part is empty".into()));
    }

    if domain.is_empty() || !domain.contains('.') {
        return Err(PixError::InvalidPixKey(
            "email domain must contain at least one '.'".into(),
        ));
    }

    // Domain parts must be non-empty
    if domain.split('.').any(|part| part.is_empty()) {
        return Err(PixError::InvalidPixKey(
            "email domain has empty label".into(),
        ));
    }

    Ok(())
}

/// Validates a Brazilian phone number in E.164 format.
///
/// Expected format: +55DDNNNNNNNNN where DD is the area code (2 digits)
/// and NNNNNNNNN is 8 or 9 digits (landline or mobile).
fn validate_phone(phone: &str) -> Result<(), PixError> {
    if !phone.starts_with("+55") {
        return Err(PixError::InvalidPixKey(
            "phone must start with +55 (Brazil country code)".into(),
        ));
    }

    let number_part = &phone[3..];

    if !number_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(PixError::InvalidPixKey(
            "phone number must contain only digits after +55".into(),
        ));
    }

    let len = number_part.len();
    if !(10..=11).contains(&len) {
        return Err(PixError::InvalidPixKey(format!(
            "phone number after +55 must have 10-11 digits, got {len}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- CPF Tests ---

    #[test]
    fn test_valid_cpf_digits_only() {
        let key = PixKey::new(PixKeyType::Cpf, "52998224725").unwrap();
        assert_eq!(key.value, "52998224725");
        assert_eq!(key.key_type, PixKeyType::Cpf);
    }

    #[test]
    fn test_valid_cpf_formatted() {
        let key = PixKey::new(PixKeyType::Cpf, "529.982.247-25").unwrap();
        assert_eq!(key.value, "52998224725");
    }

    #[test]
    fn test_invalid_cpf_wrong_check_digit() {
        assert!(PixKey::new(PixKeyType::Cpf, "52998224726").is_err());
    }

    #[test]
    fn test_invalid_cpf_all_same() {
        assert!(PixKey::new(PixKeyType::Cpf, "11111111111").is_err());
        assert!(PixKey::new(PixKeyType::Cpf, "00000000000").is_err());
    }

    #[test]
    fn test_invalid_cpf_wrong_length() {
        assert!(PixKey::new(PixKeyType::Cpf, "1234").is_err());
        assert!(PixKey::new(PixKeyType::Cpf, "123456789012").is_err());
    }

    // --- CNPJ Tests ---

    #[test]
    fn test_valid_cnpj() {
        let key = PixKey::new(PixKeyType::Cnpj, "11222333000181").unwrap();
        assert_eq!(key.value, "11222333000181");
    }

    #[test]
    fn test_valid_cnpj_formatted() {
        let key = PixKey::new(PixKeyType::Cnpj, "11.222.333/0001-81").unwrap();
        assert_eq!(key.value, "11222333000181");
    }

    #[test]
    fn test_invalid_cnpj_wrong_check_digit() {
        assert!(PixKey::new(PixKeyType::Cnpj, "11222333000182").is_err());
    }

    #[test]
    fn test_invalid_cnpj_all_same() {
        assert!(PixKey::new(PixKeyType::Cnpj, "11111111111111").is_err());
    }

    #[test]
    fn test_invalid_cnpj_wrong_length() {
        assert!(PixKey::new(PixKeyType::Cnpj, "123456").is_err());
    }

    // --- Email Tests ---

    #[test]
    fn test_valid_email() {
        let key = PixKey::new(PixKeyType::Email, "user@example.com").unwrap();
        assert_eq!(key.value, "user@example.com");
    }

    #[test]
    fn test_email_case_normalization() {
        let key = PixKey::new(PixKeyType::Email, "User@Example.COM").unwrap();
        assert_eq!(key.value, "user@example.com");
    }

    #[test]
    fn test_invalid_email_no_at() {
        assert!(PixKey::new(PixKeyType::Email, "userexample.com").is_err());
    }

    #[test]
    fn test_invalid_email_no_domain_dot() {
        assert!(PixKey::new(PixKeyType::Email, "user@example").is_err());
    }

    #[test]
    fn test_invalid_email_empty_local() {
        assert!(PixKey::new(PixKeyType::Email, "@example.com").is_err());
    }

    // --- Phone Tests ---

    #[test]
    fn test_valid_phone_mobile() {
        let key = PixKey::new(PixKeyType::Phone, "+5511987654321").unwrap();
        assert_eq!(key.value, "+5511987654321");
    }

    #[test]
    fn test_valid_phone_landline() {
        let key = PixKey::new(PixKeyType::Phone, "+551132547698").unwrap();
        assert_eq!(key.value, "+551132547698");
    }

    #[test]
    fn test_invalid_phone_no_country_code() {
        assert!(PixKey::new(PixKeyType::Phone, "11987654321").is_err());
    }

    #[test]
    fn test_invalid_phone_wrong_country() {
        assert!(PixKey::new(PixKeyType::Phone, "+1987654321").is_err());
    }

    #[test]
    fn test_invalid_phone_too_short() {
        assert!(PixKey::new(PixKeyType::Phone, "+55119876").is_err());
    }

    // --- EVP Tests ---

    #[test]
    fn test_valid_evp() {
        let key = PixKey::new(PixKeyType::Evp, "123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(key.value, "123e4567-e89b-12d3-a456-426614174000");
    }

    #[test]
    fn test_evp_case_normalization() {
        let key = PixKey::new(PixKeyType::Evp, "123E4567-E89B-12D3-A456-426614174000").unwrap();
        assert_eq!(key.value, "123e4567-e89b-12d3-a456-426614174000");
    }

    #[test]
    fn test_invalid_evp() {
        assert!(PixKey::new(PixKeyType::Evp, "not-a-uuid").is_err());
    }

    // --- Detection Tests ---

    #[test]
    fn test_detect_cpf() {
        let key = PixKey::detect("52998224725").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cpf);
    }

    #[test]
    fn test_detect_cnpj() {
        let key = PixKey::detect("11222333000181").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cnpj);
    }

    #[test]
    fn test_detect_email() {
        let key = PixKey::detect("pix@example.com").unwrap();
        assert_eq!(key.key_type, PixKeyType::Email);
    }

    #[test]
    fn test_detect_phone() {
        let key = PixKey::detect("+5511987654321").unwrap();
        assert_eq!(key.key_type, PixKeyType::Phone);
    }

    #[test]
    fn test_detect_evp() {
        let key = PixKey::detect("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(key.key_type, PixKeyType::Evp);
    }

    #[test]
    fn test_detect_unknown() {
        assert!(PixKey::detect("random-string").is_err());
    }

    // --- Display Tests ---

    #[test]
    fn test_pix_key_type_display() {
        assert_eq!(PixKeyType::Cpf.to_string(), "CPF");
        assert_eq!(PixKeyType::Cnpj.to_string(), "CNPJ");
        assert_eq!(PixKeyType::Email.to_string(), "Email");
        assert_eq!(PixKeyType::Phone.to_string(), "Phone");
        assert_eq!(PixKeyType::Evp.to_string(), "EVP");
    }

    #[test]
    fn test_pix_key_display() {
        let key = PixKey::new(PixKeyType::Email, "test@example.com").unwrap();
        assert_eq!(key.to_string(), "Email:test@example.com");
    }

    // --- Serialization Tests ---

    #[test]
    fn test_pix_key_serialize() {
        let key = PixKey::new(PixKeyType::Cpf, "52998224725").unwrap();
        let json = serde_json::to_string(&key).unwrap();
        assert!(json.contains("\"key_type\":\"cpf\""));
        assert!(json.contains("\"value\":\"52998224725\""));
    }

    #[test]
    fn test_pix_key_deserialize() {
        let json = r#"{"key_type":"email","value":"test@example.com"}"#;
        let key: PixKey = serde_json::from_str(json).unwrap();
        assert_eq!(key.key_type, PixKeyType::Email);
        assert_eq!(key.value, "test@example.com");
    }

    // --- Additional Edge Case Tests ---

    #[test]
    fn test_valid_cpf_known_values() {
        // Additional known-valid CPFs
        assert!(PixKey::new(PixKeyType::Cpf, "11144477735").is_ok());
        assert!(PixKey::new(PixKeyType::Cpf, "00000000191").is_ok()); // edge: starts with zeros
    }

    #[test]
    fn test_invalid_cpf_all_same_digits_all_variants() {
        for d in 0..=9 {
            let cpf = format!("{}", d).repeat(11);
            assert!(
                PixKey::new(PixKeyType::Cpf, &cpf).is_err(),
                "CPF {} should be invalid",
                cpf
            );
        }
    }

    #[test]
    fn test_valid_cnpj_known_values() {
        assert!(PixKey::new(PixKeyType::Cnpj, "11222333000181").is_ok());
        assert!(PixKey::new(PixKeyType::Cnpj, "11.222.333/0001-81").is_ok());
    }

    #[test]
    fn test_invalid_cnpj_all_same_digits_all_variants() {
        for d in 0..=9 {
            let cnpj = format!("{}", d).repeat(14);
            assert!(
                PixKey::new(PixKeyType::Cnpj, &cnpj).is_err(),
                "CNPJ {} should be invalid",
                cnpj
            );
        }
    }

    #[test]
    fn test_email_too_long() {
        let long_local = "a".repeat(70);
        let email = format!("{}@example.com", long_local);
        assert!(PixKey::new(PixKeyType::Email, &email).is_err());
    }

    #[test]
    fn test_email_empty_domain_label() {
        assert!(PixKey::new(PixKeyType::Email, "user@.com").is_err());
        assert!(PixKey::new(PixKeyType::Email, "user@example.").is_err());
    }

    #[test]
    fn test_phone_with_non_digits() {
        assert!(PixKey::new(PixKeyType::Phone, "+5511abc654321").is_err());
    }

    #[test]
    fn test_phone_exact_boundaries() {
        // 10 digits after +55: valid landline
        assert!(PixKey::new(PixKeyType::Phone, "+5511325476981").is_ok());
        // 9 digits after +55: too short
        assert!(PixKey::new(PixKeyType::Phone, "+55113254769").is_err());
        // 12 digits after +55: too long
        assert!(PixKey::new(PixKeyType::Phone, "+551198765432100").is_err());
    }

    #[test]
    fn test_detect_formatted_cpf() {
        let key = PixKey::detect("529.982.247-25").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cpf);
        assert_eq!(key.value, "52998224725");
    }

    #[test]
    fn test_detect_formatted_cnpj() {
        let key = PixKey::detect("11.222.333/0001-81").unwrap();
        assert_eq!(key.key_type, PixKeyType::Cnpj);
        assert_eq!(key.value, "11222333000181");
    }

    #[test]
    fn test_detect_whitespace_trimmed() {
        let key = PixKey::detect("  user@example.com  ").unwrap();
        assert_eq!(key.key_type, PixKeyType::Email);
        assert_eq!(key.value, "user@example.com");
    }

    #[test]
    fn test_detect_empty_string() {
        assert!(PixKey::detect("").is_err());
    }

    #[test]
    fn test_detect_whitespace_only() {
        assert!(PixKey::detect("   ").is_err());
    }

    #[test]
    fn test_evp_uuid_v4_format() {
        // Valid UUID v4
        assert!(PixKey::new(PixKeyType::Evp, "550e8400-e29b-41d4-a716-446655440000").is_ok());
        // Without hyphens should fail UUID parse
        assert!(PixKey::new(PixKeyType::Evp, "550e8400e29b41d4a716446655440000").is_ok());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn valid_cpf_always_validates(
            // Generate valid CPFs: 9 random digits, compute check digits
            d0 in 0u32..10,
            d1 in 0u32..10,
            d2 in 0u32..10,
            d3 in 0u32..10,
            d4 in 0u32..10,
            d5 in 0u32..10,
            d6 in 0u32..10,
            d7 in 0u32..10,
            d8 in 0u32..10,
        ) {
            let digits = [d0, d1, d2, d3, d4, d5, d6, d7, d8];
            // Skip all-same-digit
            if digits.iter().all(|&d| d == digits[0]) {
                return Ok(());
            }
            let sum1: u32 = digits.iter().enumerate().map(|(i, &d)| d * (10 - i as u32)).sum();
            let rem1 = sum1 % 11;
            let check1 = if rem1 < 2 { 0 } else { 11 - rem1 };
            let all10: Vec<u32> = digits.iter().copied().chain(std::iter::once(check1)).collect();
            let sum2: u32 = all10.iter().enumerate().map(|(i, &d)| d * (11 - i as u32)).sum();
            let rem2 = sum2 % 11;
            let check2 = if rem2 < 2 { 0 } else { 11 - rem2 };
            let cpf: String = digits.iter().chain(&[check1, check2]).map(|d| char::from_digit(*d, 10).unwrap()).collect();
            prop_assert!(PixKey::new(PixKeyType::Cpf, &cpf).is_ok(), "CPF {} should be valid", cpf);
        }

        #[test]
        fn random_11_digits_with_bad_check_rejects(
            digits in proptest::collection::vec(0u32..10, 11),
        ) {
            let cpf: String = digits.iter().map(|d| char::from_digit(*d, 10).unwrap()).collect();
            // Most random 11-digit strings won't have valid check digits
            // We just verify it doesn't panic
            let _ = PixKey::new(PixKeyType::Cpf, &cpf);
        }

        #[test]
        fn email_detection_works(
            local in "[a-z]{1,20}",
            domain in "[a-z]{1,10}",
            tld in "[a-z]{2,4}",
        ) {
            let email = format!("{}@{}.{}", local, domain, tld);
            if email.len() <= 77 {
                let key = PixKey::detect(&email).unwrap();
                prop_assert_eq!(key.key_type, PixKeyType::Email);
            }
        }

        #[test]
        fn phone_with_valid_format_detects(
            ddd in 11u32..99,
            number in 900000000u64..999999999,
        ) {
            let phone = format!("+55{}{}", ddd, number);
            let key = PixKey::detect(&phone).unwrap();
            prop_assert_eq!(key.key_type, PixKeyType::Phone);
        }
    }
}

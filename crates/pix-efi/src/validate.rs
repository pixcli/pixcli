//! Input validation helpers for the Efí API.
//!
//! Validates txid format, amount ranges, and Pix key format before
//! sending requests to avoid unnecessary API round-trips.

use pix_provider::ProviderError;

/// Validates a txid for the Efí API.
///
/// The Efí API requires txid to be 26–35 alphanumeric characters (`[a-zA-Z0-9]`).
pub fn validate_txid(txid: &str) -> Result<(), ProviderError> {
    let len = txid.len();
    if !(26..=35).contains(&len) {
        return Err(ProviderError::InvalidResponse(format!(
            "txid must be 26–35 characters, got {len}"
        )));
    }
    if !txid.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ProviderError::InvalidResponse(
            "txid must contain only alphanumeric characters [a-zA-Z0-9]".to_string(),
        ));
    }
    Ok(())
}

/// Validates a BRL amount string (e.g. "10.50").
///
/// Must be a positive decimal with exactly two decimal places.
pub fn validate_amount(amount: &str) -> Result<(), ProviderError> {
    let parts: Vec<&str> = amount.split('.').collect();
    if parts.len() != 2 {
        return Err(ProviderError::InvalidResponse(
            "amount must have exactly two decimal places (e.g. \"10.50\")".to_string(),
        ));
    }
    let integer_part = parts[0];
    let decimal_part = parts[1];

    if decimal_part.len() != 2 {
        return Err(ProviderError::InvalidResponse(
            "amount must have exactly two decimal places".to_string(),
        ));
    }

    if integer_part.is_empty()
        || !integer_part.chars().all(|c| c.is_ascii_digit())
        || !decimal_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(ProviderError::InvalidResponse(
            "amount must be a valid decimal number".to_string(),
        ));
    }

    // Must be > 0
    let value: f64 = amount
        .parse()
        .map_err(|_| ProviderError::InvalidResponse("amount is not a valid number".to_string()))?;
    if value <= 0.0 {
        return Err(ProviderError::InvalidResponse(
            "amount must be greater than zero".to_string(),
        ));
    }

    Ok(())
}

/// Validates an end-to-end ID (e2eid) format.
///
/// E2EIDs are 32 characters long and start with 'E'.
pub fn validate_e2eid(e2eid: &str) -> Result<(), ProviderError> {
    if e2eid.is_empty() {
        return Err(ProviderError::InvalidResponse(
            "e2eid must not be empty".to_string(),
        ));
    }
    if !e2eid.starts_with('E') {
        return Err(ProviderError::InvalidResponse(
            "e2eid must start with 'E'".to_string(),
        ));
    }
    if e2eid.len() != 32 {
        return Err(ProviderError::InvalidResponse(format!(
            "e2eid must be 32 characters, got {}",
            e2eid.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_txid() {
        // 35 chars: "pix" + 32 hex chars
        assert!(validate_txid("pix550e8400e29b41d4a716446655440000").is_ok());
    }

    #[test]
    fn test_txid_too_short() {
        assert!(validate_txid("abc").is_err());
    }

    #[test]
    fn test_txid_too_long() {
        let long = "a".repeat(36);
        assert!(validate_txid(&long).is_err());
    }

    #[test]
    fn test_txid_invalid_chars() {
        assert!(validate_txid("pix550e8400-e29b-41d4-a716-44665544").is_err());
    }

    #[test]
    fn test_valid_amount() {
        assert!(validate_amount("10.50").is_ok());
        assert!(validate_amount("0.01").is_ok());
        assert!(validate_amount("999999.99").is_ok());
    }

    #[test]
    fn test_amount_no_decimals() {
        assert!(validate_amount("10").is_err());
    }

    #[test]
    fn test_amount_too_many_decimals() {
        assert!(validate_amount("10.500").is_err());
    }

    #[test]
    fn test_amount_zero() {
        assert!(validate_amount("0.00").is_err());
    }

    #[test]
    fn test_amount_negative() {
        // No negative sign possible with our check, but would fail on parse
        assert!(validate_amount("-1.00").is_err());
    }

    #[test]
    fn test_valid_e2eid() {
        assert!(validate_e2eid("E12345678901234567890123456789AB").is_ok());
    }

    #[test]
    fn test_e2eid_empty() {
        assert!(validate_e2eid("").is_err());
    }

    #[test]
    fn test_e2eid_wrong_prefix() {
        assert!(validate_e2eid("X1234567890123456789012345678901").is_err());
    }

    #[test]
    fn test_e2eid_wrong_length() {
        assert!(validate_e2eid("E123").is_err());
    }
}

#[cfg(test)]
mod additional_validation_tests {
    use super::*;

    // --- txid edge cases ---

    #[test]
    fn test_txid_exactly_26_alphanumeric() {
        assert!(validate_txid("abcdefghijklmnopqrstuvwxyz").is_ok());
    }

    #[test]
    fn test_txid_exactly_35_alphanumeric() {
        assert!(validate_txid("pix550e8400e29b41d4a716446655440000").is_ok());
    }

    #[test]
    fn test_txid_mixed_case_valid() {
        assert!(validate_txid("ABCDEFghijklmnopqrstuvwxyz").is_ok());
    }

    #[test]
    fn test_txid_all_digits() {
        assert!(validate_txid("12345678901234567890123456").is_ok());
    }

    #[test]
    fn test_txid_with_spaces_fails() {
        assert!(validate_txid("pix 550e8400e29b41d4a71644665").is_err());
    }

    #[test]
    fn test_txid_with_special_chars_fails() {
        assert!(validate_txid("pix550e8400@e29b41d4a71644665").is_err());
    }

    #[test]
    fn test_txid_empty_fails() {
        assert!(validate_txid("").is_err());
    }

    // --- amount edge cases ---

    #[test]
    fn test_amount_minimum_valid() {
        assert!(validate_amount("0.01").is_ok());
    }

    #[test]
    fn test_amount_maximum_practical() {
        assert!(validate_amount("999999.99").is_ok());
    }

    #[test]
    fn test_amount_one_real() {
        assert!(validate_amount("1.00").is_ok());
    }

    #[test]
    fn test_amount_empty_string_fails() {
        assert!(validate_amount("").is_err());
    }

    #[test]
    fn test_amount_just_dot_fails() {
        assert!(validate_amount(".50").is_err());
    }

    #[test]
    fn test_amount_trailing_dot_fails() {
        assert!(validate_amount("10.").is_err());
    }

    #[test]
    fn test_amount_multiple_dots_fails() {
        assert!(validate_amount("10.00.00").is_err());
    }

    #[test]
    fn test_amount_negative_number_fails() {
        assert!(validate_amount("-10.00").is_err());
    }

    // --- e2eid edge cases ---

    #[test]
    fn test_e2eid_exactly_32_chars_starting_with_e() {
        let e2eid = format!("E{}", "A".repeat(31));
        assert!(validate_e2eid(&e2eid).is_ok());
    }

    #[test]
    fn test_e2eid_all_digits_after_e() {
        let e2eid = format!("E{}", "1".repeat(31));
        assert!(validate_e2eid(&e2eid).is_ok());
    }

    #[test]
    fn test_e2eid_no_e_prefix_fails() {
        let e2eid = "A".repeat(32);
        assert!(validate_e2eid(&e2eid).is_err());
    }

    #[test]
    fn test_e2eid_lowercase_e_fails() {
        let e2eid = format!("e{}", "1".repeat(31));
        assert!(validate_e2eid(&e2eid).is_err());
    }
}

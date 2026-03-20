//! EMV TLV (Tag-Length-Value) encoding and parsing.
//!
//! Tags and lengths are encoded as 2-digit ASCII decimal numbers.
//! Values are ASCII strings of the specified length.

use crate::BrCodeError;

/// A single TLV entry with a 2-digit tag ID and a string value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlvEntry {
    /// 2-digit tag identifier (00-99).
    pub tag: String,
    /// The value payload.
    pub value: String,
}

impl TlvEntry {
    /// Creates a new TLV entry.
    pub fn new(tag: &str, value: &str) -> Self {
        Self {
            tag: tag.to_string(),
            value: value.to_string(),
        }
    }

    /// Encodes this entry as a TLV string: `TTLLVVVV...`
    ///
    /// Where TT is the 2-digit tag, LL is the 2-digit length, and VVV... is the value.
    #[must_use]
    pub fn encode(&self) -> String {
        format!("{}{:02}{}", self.tag, self.value.len(), self.value)
    }
}

/// Parses a TLV-encoded string into a vector of `TlvEntry` items.
///
/// Each entry consists of:
/// - 2 characters: tag ID
/// - 2 characters: value length (decimal)
/// - N characters: value (where N is the parsed length)
///
/// # Errors
///
/// Returns `BrCodeError::MalformedTlv` if the input is not valid TLV data.
pub fn parse_tlv(input: &str) -> Result<Vec<TlvEntry>, BrCodeError> {
    let mut entries = Vec::new();
    let bytes = input.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        // Need at least 4 bytes for tag (2) + length (2)
        if pos + 4 > bytes.len() {
            return Err(BrCodeError::MalformedTlv(format!(
                "unexpected end of data at position {pos}"
            )));
        }

        // SAFETY: EMV TLV payloads are ASCII; slicing is safe at byte boundaries.
        let tag = &input[pos..pos + 2];
        let len_str = &input[pos + 2..pos + 4];
        let len: usize = len_str.parse().map_err(|_| {
            BrCodeError::MalformedTlv(format!("invalid length '{len_str}' at position {pos}"))
        })?;

        pos += 4;

        if pos + len > bytes.len() {
            return Err(BrCodeError::MalformedTlv(format!(
                "value extends past end of data: tag {tag}, expected {len} chars at position {pos}, but only {} remain",
                bytes.len() - pos
            )));
        }

        let value = &input[pos..pos + len];
        entries.push(TlvEntry {
            tag: tag.to_string(),
            value: value.to_string(),
        });
        pos += len;
    }

    Ok(entries)
}

/// Finds the first TLV entry with the given tag.
#[must_use]
pub fn find_tag<'a>(entries: &'a [TlvEntry], tag: &str) -> Option<&'a TlvEntry> {
    entries.iter().find(|e| e.tag == tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlv_encode() {
        let entry = TlvEntry::new("00", "01");
        assert_eq!(entry.encode(), "000201");
    }

    #[test]
    fn test_tlv_encode_longer_value() {
        let entry = TlvEntry::new("59", "Fulano de Tal");
        assert_eq!(entry.encode(), "5913Fulano de Tal");
    }

    #[test]
    fn test_tlv_parse_single() {
        let entries = parse_tlv("000201").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tag, "00");
        assert_eq!(entries[0].value, "01");
    }

    #[test]
    fn test_tlv_parse_multiple() {
        let entries = parse_tlv("000201010211").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].tag, "00");
        assert_eq!(entries[0].value, "01");
        assert_eq!(entries[1].tag, "01");
        assert_eq!(entries[1].value, "11");
    }

    #[test]
    fn test_tlv_roundtrip() {
        let original = TlvEntry::new("26", "00140BR.GOV.BCB.PIX011112345678900");
        let encoded = original.encode();
        let parsed = parse_tlv(&encoded).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], original);
    }

    #[test]
    fn test_tlv_parse_malformed_truncated() {
        assert!(parse_tlv("00").is_err());
    }

    #[test]
    fn test_tlv_parse_malformed_length() {
        assert!(parse_tlv("00XX").is_err());
    }

    #[test]
    fn test_tlv_parse_value_too_short() {
        assert!(parse_tlv("000501").is_err()); // Says length 5 but only 2 chars
    }

    #[test]
    fn test_find_tag() {
        let entries = parse_tlv("000201010211").unwrap();
        let found = find_tag(&entries, "01").unwrap();
        assert_eq!(found.value, "11");
        assert!(find_tag(&entries, "99").is_none());
    }
}

#[cfg(test)]
mod additional_tlv_tests {
    use super::*;

    #[test]
    fn test_tlv_encode_empty_value() {
        let entry = TlvEntry::new("99", "");
        assert_eq!(entry.encode(), "9900");
    }

    #[test]
    fn test_tlv_encode_long_value() {
        let value = "A".repeat(99);
        let entry = TlvEntry::new("26", &value);
        let encoded = entry.encode();
        assert!(encoded.starts_with("2699"));
        assert_eq!(encoded.len(), 4 + 99);
    }

    #[test]
    fn test_tlv_parse_empty_input() {
        let entries = parse_tlv("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_tlv_parse_three_entries() {
        let entries = parse_tlv("000201010211520400005303986").unwrap();
        assert!(entries.len() >= 3);
    }

    #[test]
    fn test_tlv_entry_equality() {
        let a = TlvEntry::new("00", "01");
        let b = TlvEntry::new("00", "01");
        assert_eq!(a, b);
    }

    #[test]
    fn test_tlv_entry_inequality() {
        let a = TlvEntry::new("00", "01");
        let b = TlvEntry::new("00", "02");
        assert_ne!(a, b);
    }

    #[test]
    fn test_tlv_parse_nested_content() {
        // Simulate merchant account info sub-tags
        let inner = format!(
            "{}{}",
            TlvEntry::new("00", "BR.GOV.BCB.PIX").encode(),
            TlvEntry::new("01", "key@test.com").encode()
        );
        let outer = TlvEntry::new("26", &inner);
        let encoded = outer.encode();

        let entries = parse_tlv(&encoded).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tag, "26");

        // Parse inner
        let inner_entries = parse_tlv(&entries[0].value).unwrap();
        assert_eq!(inner_entries.len(), 2);
        assert_eq!(inner_entries[0].tag, "00");
        assert_eq!(inner_entries[0].value, "BR.GOV.BCB.PIX");
        assert_eq!(inner_entries[1].tag, "01");
        assert_eq!(inner_entries[1].value, "key@test.com");
    }

    #[test]
    fn test_find_tag_returns_first_match() {
        let entries = vec![TlvEntry::new("01", "first"), TlvEntry::new("01", "second")];
        let found = find_tag(&entries, "01").unwrap();
        assert_eq!(found.value, "first");
    }

    #[test]
    fn test_find_tag_not_found() {
        let entries = vec![TlvEntry::new("00", "01")];
        assert!(find_tag(&entries, "99").is_none());
    }

    #[test]
    fn test_tlv_parse_malformed_short_length() {
        // Tag "00" with only 1 char left for length
        assert!(parse_tlv("001").is_err());
    }

    #[test]
    fn test_tlv_clone() {
        let entry = TlvEntry::new("00", "01");
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
    }

    #[test]
    fn test_tlv_debug() {
        let entry = TlvEntry::new("00", "01");
        let debug = format!("{:?}", entry);
        assert!(debug.contains("00"));
        assert!(debug.contains("01"));
    }
}

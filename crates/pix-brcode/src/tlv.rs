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
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        // Need at least 4 characters for tag (2) + length (2)
        if pos + 4 > chars.len() {
            return Err(BrCodeError::MalformedTlv(format!(
                "unexpected end of data at position {pos}"
            )));
        }

        let tag: String = chars[pos..pos + 2].iter().collect();
        let len_str: String = chars[pos + 2..pos + 4].iter().collect();
        let len: usize = len_str.parse().map_err(|_| {
            BrCodeError::MalformedTlv(format!("invalid length '{len_str}' at position {pos}"))
        })?;

        pos += 4;

        if pos + len > chars.len() {
            return Err(BrCodeError::MalformedTlv(format!(
                "value extends past end of data: tag {tag}, expected {len} chars at position {pos}, but only {} remain",
                chars.len() - pos
            )));
        }

        let value: String = chars[pos..pos + len].iter().collect();
        entries.push(TlvEntry::new(&tag, &value));
        pos += len;
    }

    Ok(entries)
}

/// Finds the first TLV entry with the given tag.
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

/// Parse a hex string (e.g. "68 04 07 00") into bytes.
pub fn parse_hex_string(s: &str) -> Result<Vec<u8>, ToolError> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !cleaned.len().is_multiple_of(2) {
        return Err(ToolError::InvalidHexLength);
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| ToolError::InvalidHexChar)
        })
        .collect()
}

/// Format bytes as hex string with spaces.
pub fn format_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format an IOA (Information Object Address) for display.
pub fn format_ioa(ioa: u32) -> String {
    format!("{}", ioa)
}

/// Format an IOA as hex.
pub fn format_ioa_hex(ioa: u32) -> String {
    format!("0x{:06X}", ioa)
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("hex string has odd length")]
    InvalidHexLength,
    #[error("invalid hex character")]
    InvalidHexChar,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_string() {
        let bytes = parse_hex_string("68 04 07 00").unwrap();
        assert_eq!(bytes, vec![0x68, 0x04, 0x07, 0x00]);
    }

    #[test]
    fn test_parse_hex_string_no_spaces() {
        let bytes = parse_hex_string("68040700").unwrap();
        assert_eq!(bytes, vec![0x68, 0x04, 0x07, 0x00]);
    }

    #[test]
    fn test_parse_hex_invalid() {
        assert!(parse_hex_string("6").is_err());
        assert!(parse_hex_string("GG").is_err());
    }

    #[test]
    fn test_format_hex() {
        assert_eq!(format_hex(&[0x68, 0x04]), "68 04");
    }

    #[test]
    fn test_format_ioa() {
        assert_eq!(format_ioa(100), "100");
        assert_eq!(format_ioa_hex(100), "0x000064");
    }
}

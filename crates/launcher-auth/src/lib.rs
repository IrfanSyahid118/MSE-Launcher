pub mod microsoft;

pub use microsoft::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_uuid_valid() {
        let raw = "5627dd983e213c21b8a8e92344183641";
        let formatted = format_uuid(raw).expect("format failed");
        assert_eq!(formatted, "5627dd98-3e21-3c21-b8a8-e92344183641");
    }

    #[test]
    fn test_format_uuid_invalid() {
        assert!(format_uuid("invalid").is_err());
        assert!(format_uuid("5627dd983e213c21b8a8e9234418364").is_err()); // 31 chars
    }
}

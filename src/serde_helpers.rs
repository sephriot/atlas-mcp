//! Serde helpers for handling MCP parameter type coercion.
//!
//! MCP clients sometimes send numeric parameters as strings (e.g., "5" instead of 5).
//! These helpers provide lenient deserialization that accepts both forms.

use serde::{de, Deserialize, Deserializer};

/// Deserialize an optional usize that may come as either a number or a string.
pub fn deserialize_optional_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        Number(usize),
        String(String),
    }

    let value: Option<StringOrNumber> = Option::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(StringOrNumber::Number(n)) => Ok(Some(n)),
        Some(StringOrNumber::String(s)) => s
            .parse::<usize>()
            .map(Some)
            .map_err(|_| de::Error::custom(format!("invalid number: {}", s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestStruct {
        #[serde(default, deserialize_with = "deserialize_optional_usize")]
        limit: Option<usize>,
    }

    #[test]
    fn test_deserialize_number() {
        let json = r#"{"limit": 5}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.limit, Some(5));
    }

    #[test]
    fn test_deserialize_string() {
        let json = r#"{"limit": "5"}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.limit, Some(5));
    }

    #[test]
    fn test_deserialize_null() {
        let json = r#"{"limit": null}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.limit, None);
    }

    #[test]
    fn test_deserialize_missing() {
        let json = r#"{}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.limit, None);
    }

    #[test]
    fn test_deserialize_invalid_string() {
        let json = r#"{"limit": "not a number"}"#;
        let result: Result<TestStruct, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}

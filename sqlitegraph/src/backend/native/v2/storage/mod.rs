//! JSON input validation for memory safety and DoS prevention.
//!
//! This module provides configurable limits for JSON parsing to prevent:
//! - Memory exhaustion from unbounded payload sizes
//! - Stack overflow from deeply nested structures
//! - CPU exhaustion from complex JSON documents
//!
//! # Usage
//!
//! ```rust
//! use sqlitegraph::backend::native::v2::storage::{JsonLimits, parse_and_validate_json};
//!
//! let limits = JsonLimits::default(); // 10MB size, 128 depth
//! let input = br#"{"key": "value"}"#;
//! let json = parse_and_validate_json(input, &limits)?;
//! ```
//!
//! # Security
//!
//! All JSON parsing should go through `parse_and_validate_json` to enforce
//! size and depth limits before serde_json processes the data.

use serde_json::Value;

/// Default maximum JSON payload size (10MB)
const DEFAULT_MAX_JSON_SIZE: usize = 10 * 1024 * 1024;

/// Default maximum JSON nesting depth (128 levels)
const DEFAULT_MAX_JSON_DEPTH: usize = 128;

/// Configurable limits for JSON input validation
///
/// These limits prevent DoS attacks through:
/// - Large payloads that exhaust memory
/// - Deeply nested structures that cause stack overflow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonLimits {
    /// Maximum JSON payload size in bytes
    pub max_size: usize,

    /// Maximum nesting depth of JSON structures
    pub max_depth: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_MAX_JSON_SIZE,
            max_depth: DEFAULT_MAX_JSON_DEPTH,
        }
    }
}

impl JsonLimits {
    /// Create custom JSON limits
    pub fn new(max_size: usize, max_depth: usize) -> Self {
        Self { max_size, max_depth }
    }

    /// Create limits for maximum size only (use default depth)
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            ..Default::default()
        }
    }

    /// Create limits for maximum depth only (use default size)
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
            ..Default::default()
        }
    }

    /// Create limits for testing (small values for faster test execution)
    #[cfg(test)]
    pub fn test_limits() -> Self {
        Self {
            max_size: 1000,
            max_depth: 10,
        }
    }
}

/// Errors that can occur during JSON validation
#[derive(Debug, thiserror::Error)]
pub enum JsonValidationError {
    /// JSON payload size exceeds maximum allowed
    #[error("JSON size {actual} bytes exceeds maximum {max} bytes")]
    SizeTooLarge { actual: usize, max: usize },

    /// JSON nesting depth exceeds maximum allowed
    #[error("JSON depth {actual} exceeds maximum {max}")]
    DepthTooLarge { actual: usize, max: usize },

    /// JSON parsing error from serde_json
    #[error("JSON parsing error: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Validate JSON input size before parsing
///
/// This check happens BEFORE serde_json processes the data,
/// preventing memory allocation for oversized payloads.
pub fn validate_json_size(input: &[u8], limits: &JsonLimits) -> Result<(), JsonValidationError> {
    if input.len() > limits.max_size {
        return Err(JsonValidationError::SizeTooLarge {
            actual: input.len(),
            max: limits.max_size,
        });
    }
    Ok(())
}

/// Calculate the maximum nesting depth of a JSON value
fn calculate_json_depth(value: &Value, current: usize) -> usize {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => current,
        Value::Array(arr) => {
            arr.iter()
                .map(|v| calculate_json_depth(v, current + 1))
                .max()
                .unwrap_or(current)
        }
        Value::Object(obj) => {
            obj.values()
                .map(|v| calculate_json_depth(v, current + 1))
                .max()
                .unwrap_or(current)
        }
    }
}

/// Validate JSON depth after parsing
pub fn validate_json_depth(value: &Value, limits: &JsonLimits) -> Result<(), JsonValidationError> {
    let depth = calculate_json_depth(value, 0);
    if depth > limits.max_depth {
        return Err(JsonValidationError::DepthTooLarge {
            actual: depth,
            max: limits.max_depth,
        });
    }
    Ok(())
}

/// Parse and validate JSON with enforced limits
///
/// This is the preferred way to parse JSON in SQLiteGraph. It enforces
/// size limits BEFORE parsing (preventing memory exhaustion) and
/// depth limits AFTER parsing (preventing stack overflow).
///
/// # Example
///
/// ```rust
/// use sqlitegraph::backend::native::v2::storage::{JsonLimits, parse_and_validate_json};
///
/// let limits = JsonLimits::default();
/// let input = br#"{"name": "test"}"#;
/// let json = parse_and_validate_json(input, &limits)?;
/// ```
pub fn parse_and_validate_json(
    input: &[u8],
    limits: &JsonLimits,
) -> Result<Value, JsonValidationError> {
    // Validate size FIRST (before any parsing)
    validate_json_size(input, limits)?;

    // Parse JSON
    let value: Value = serde_json::from_slice(input)?;

    // Validate depth after parsing
    validate_json_depth(&value, limits)?;

    Ok(value)
}

/// Parse JSON string with enforced limits
///
/// Convenience function for string input.
pub fn parse_and_validate_json_str(
    input: &str,
    limits: &JsonLimits,
) -> Result<Value, JsonValidationError> {
    parse_and_validate_json(input.as_bytes(), limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_size_within_limit() {
        let limits = JsonLimits {
            max_size: 1000,
            max_depth: 128,
        };
        let input = br#"{"test": "data"}"#;
        let result = parse_and_validate_json(input, &limits);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["test"], "data");
    }

    #[test]
    fn test_json_size_exceeds_limit() {
        let limits = JsonLimits {
            max_size: 10,
            max_depth: 128,
        };
        let input = br#"{"large": "this value exceeds limit"}"#;
        let result = parse_and_validate_json(input, &limits);
        assert!(matches!(result, Err(JsonValidationError::SizeTooLarge { .. })));
    }

    #[test]
    fn test_json_depth_within_limit() {
        let limits = JsonLimits {
            max_size: 10000,
            max_depth: 64,
        };
        let mut json_str = String::from("null");
        for _ in 0..10 {
            json_str = format!("[{}]", json_str);
        }
        let result = parse_and_validate_json(json_str.as_bytes(), &limits);
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_depth_exceeds_limit() {
        let limits = JsonLimits {
            max_size: 10000,
            max_depth: 10,
        };
        let mut json_str = String::from("null");
        for _ in 0..20 {
            json_str = format!("[{}]", json_str);
        }
        let result = parse_and_validate_json(json_str.as_bytes(), &limits);
        assert!(matches!(result, Err(JsonValidationError::DepthTooLarge { .. })));
    }

    #[test]
    fn test_json_depth_exactly_at_limit() {
        let limits = JsonLimits {
            max_size: 10000,
            max_depth: 10,
        };
        // Create JSON with exactly 10 levels of nesting
        let mut json_str = String::from("null");
        for _ in 0..10 {
            json_str = format!("[{}]", json_str);
        }
        let result = parse_and_validate_json(json_str.as_bytes(), &limits);
        // depth=10 should pass (max_depth=10, depth is 0-indexed from root)
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_json() {
        let limits = JsonLimits::default();
        let result = parse_and_validate_json(b"{}", &limits);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_array() {
        let limits = JsonLimits::default();
        let result = parse_and_validate_json(b"[]", &limits);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_json_syntax() {
        let limits = JsonLimits::default();
        let result = parse_and_validate_json(b"{invalid json}", &limits);
        assert!(matches!(result, Err(JsonValidationError::ParseError(_))));
    }

    #[test]
    fn test_nested_object_depth() {
        let limits = JsonLimits {
            max_size: 10000,
            max_depth: 5,
        };
        // Create nested objects: {"a": {"a": {"a": {"a": {"a": null}}}}}
        let json_str = r#"{"a":{"a":{"a":{"a":{"a":null}}}}}"#;
        let result = parse_and_validate_json(json_str.as_bytes(), &limits);
        // 5 levels of nesting should pass
        assert!(result.is_ok());
    }

    #[test]
    fn test_mixed_nesting_depth() {
        let limits = JsonLimits {
            max_size: 10000,
            max_depth: 4,
        };
        // Create mixed nesting: [{"a": [null]}]
        // Root array -> object -> array -> null = 4 levels
        let json_str = r#"[{"a": [null]}]"#;
        let result = parse_and_validate_json(json_str.as_bytes(), &limits);
        assert!(result.is_ok());
    }

    #[test]
    fn test_default_limits() {
        let limits = JsonLimits::default();
        assert_eq!(limits.max_size, 10 * 1024 * 1024);
        assert_eq!(limits.max_depth, 128);
    }

    #[test]
    fn test_custom_limits() {
        let limits = JsonLimits::new(100, 10);
        assert_eq!(limits.max_size, 100);
        assert_eq!(limits.max_depth, 10);
    }

    #[test]
    fn test_with_max_size() {
        let limits = JsonLimits::with_max_size(500);
        assert_eq!(limits.max_size, 500);
        assert_eq!(limits.max_depth, 128); // default depth
    }

    #[test]
    fn test_with_max_depth() {
        let limits = JsonLimits::with_max_depth(64);
        assert_eq!(limits.max_size, 10 * 1024 * 1024); // default size
        assert_eq!(limits.max_depth, 64);
    }

    #[test]
    fn test_string_input() {
        let limits = JsonLimits::default();
        let result = parse_and_validate_json_str(r#"{"key": "value"}"#, &limits);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["key"], "value");
    }

    #[test]
    fn test_zero_size_input() {
        let limits = JsonLimits::default();
        let result = parse_and_validate_json(b"", &limits);
        assert!(matches!(result, Err(JsonValidationError::ParseError(_))));
    }
}

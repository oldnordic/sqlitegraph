//! Malicious payload tests for JSON input validation.
//!
//! This test module verifies that JSON input validation properly rejects
//! malicious payloads that could cause DoS, memory exhaustion, or stack overflow.

use sqlitegraph::backend::native::v2::storage::{
    JsonLimits, JsonValidationError, parse_and_validate_json, parse_and_validate_json_str,
};

/// Test: Malicious payload that exceeds size limit
#[test]
fn test_malicious_json_payload_size() {
    let limits = JsonLimits::default(); // 10MB limit
    // Create 11MB payload
    // Actually create a real large payload
    let large_payload: String = "a".repeat(11 * 1024 * 1024);
    let json_str = format!(r#"{{"data": "{}"}}"#, large_payload);

    let result = parse_and_validate_json_str(&json_str, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::SizeTooLarge { .. })),
        "Expected SizeTooLarge error, got: {:?}",
        result
    );
}

/// Test: Malicious payload with excessive nesting depth
#[test]
fn test_malicious_json_payload_depth() {
    let limits = JsonLimits::default(); // 128 depth limit
    // Create 200 levels of nesting
    // Note: serde_json has its own recursion limit (default 128)
    // so deeply nested JSON may fail during parsing before our depth check
    let mut json_str = String::from("null");
    for _ in 0..200 {
        json_str = format!("[{}]", json_str);
    }

    let result = parse_and_validate_json_str(&json_str, &limits);
    // Either our depth validation catches it, or serde_json's recursion limit does
    // Both are valid protections against deeply nested payloads
    assert!(
        matches!(result, Err(JsonValidationError::DepthTooLarge { .. }) | Err(JsonValidationError::ParseError(_))),
        "Expected DepthTooLarge or ParseError (recursion limit), got: {:?}",
        result
    );
}

/// Test: Payload with both large size AND deep nesting
#[test]
fn test_malicious_combined_size_and_depth() {
    let limits = JsonLimits {
        max_size: 1000,
        max_depth: 10,
    };

    // Create deep nesting (will exceed depth limit)
    let mut json_str = String::from("null");
    for _ in 0..20 {
        json_str = format!("[{}]", json_str);
    }

    let result = parse_and_validate_json_str(&json_str, &limits);
    // Should fail on depth check (after size check passes)
    assert!(
        matches!(result, Err(JsonValidationError::DepthTooLarge { .. })),
        "Expected DepthTooLarge error, got: {:?}",
        result
    );
}

/// Test: Payload exactly at size limit boundary
#[test]
fn test_payload_at_size_boundary() {
    // Create exactly 88 bytes of JSON (actual length of the string below)
    let payload = r#"{"a":"12345678901234567890123456789012345678901234567890123456789012345678901234567890"}"#;
    let actual_len = payload.len();
    assert_eq!(actual_len, 88, "Payload length is {} not 88 as expected", actual_len);

    let limits = JsonLimits {
        max_size: 88,
        max_depth: 128,
    };

    let result = parse_and_validate_json_str(payload, &limits);
    // Should pass - exactly at limit
    assert!(result.is_ok(), "Expected success at size boundary, got: {:?}", result);
}

/// Test: Payload just over size limit boundary
#[test]
fn test_payload_just_over_size_boundary() {
    // Create 89 bytes of JSON (actual length + 1)
    let payload = r#"{"a":"123456789012345678901234567890123456789012345678901234567890123456789012345678901"}"#;
    let actual_len = payload.len();
    assert_eq!(actual_len, 89, "Payload length is {} not 89 as expected", actual_len);

    let limits = JsonLimits {
        max_size: 88,
        max_depth: 128,
    };

    let result = parse_and_validate_json_str(payload, &limits);
    // Should fail - just over limit
    assert!(
        matches!(result, Err(JsonValidationError::SizeTooLarge { actual: 89, max: 88 })),
        "Expected SizeTooLarge error, got: {:?}",
        result
    );
}

/// Test: Deeply nested object structure
#[test]
fn test_deeply_nested_objects() {
    let limits = JsonLimits {
        max_size: 10000,
        max_depth: 5,
    };

    // Create 6 levels of nested objects (exceeds limit of 5)
    let json_str = r#"{"a":{"b":{"c":{"d":{"e":{"f":null}}}}}}"#;

    let result = parse_and_validate_json_str(json_str, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::DepthTooLarge { .. })),
        "Expected DepthTooLarge error for 6 levels, got: {:?}",
        result
    );
}

/// Test: Wide array (many elements, shallow depth)
#[test]
fn test_wide_array_shallow_depth() {
    let limits = JsonLimits {
        max_size: 10000,
        max_depth: 128,
    };

    // Create array with 1000 elements (but shallow depth)
    let elements: Vec<&str> = (0..1000).map(|_| "null").collect();
    let json_str = format!("[{}]", elements.join(","));

    let result = parse_and_validate_json_str(&json_str, &limits);
    // Should pass - depth is only 2 (root array -> elements)
    assert!(result.is_ok(), "Wide array should pass, got: {:?}", result);
}

/// Test: Malformed JSON that passes size check
#[test]
fn test_malformed_json_with_valid_size() {
    let limits = JsonLimits::default();
    let invalid_json = r#"{"unclosed": ["array"#;

    let result = parse_and_validate_json_str(invalid_json, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::ParseError(_))),
        "Expected ParseError for malformed JSON, got: {:?}",
        result
    );
}

/// Test: Unicode payload that could cause issues
#[test]
fn test_unicode_payload() {
    let limits = JsonLimits {
        max_size: 1000,
        max_depth: 10,
    };

    // Valid JSON with unicode
    let unicode_json = r#"{"emoji": "😀🎉", "chinese": "你好", "arabic": "مرحبا"}"#;

    let result = parse_and_validate_json_str(unicode_json, &limits);
    assert!(result.is_ok(), "Unicode JSON should be valid, got: {:?}", result);
}

/// Test: Empty payload edge case
#[test]
fn test_empty_payload() {
    let limits = JsonLimits::default();
    let result = parse_and_validate_json(b"", &limits);
    assert!(
        matches!(result, Err(JsonValidationError::ParseError(_))),
        "Empty payload should fail to parse"
    );
}

/// Test: Single element edge cases
#[test]
fn test_single_element_edge_cases() {
    let limits = JsonLimits::default();

    // Test single values
    assert!(parse_and_validate_json(b"null", &limits).is_ok());
    assert!(parse_and_validate_json(b"true", &limits).is_ok());
    assert!(parse_and_validate_json(b"false", &limits).is_ok());
    assert!(parse_and_validate_json(b"42", &limits).is_ok());
    assert!(parse_and_validate_json(br#""hello""#, &limits).is_ok());
}

/// Test: Special characters that could be used for injection
#[test]
fn test_special_characters() {
    let limits = JsonLimits::default();

    // JSON with escaped special characters
    let escaped_json = r#"{"path": "C:\\Users\\test", "quote": "He said \"hello\"", "newline": "line1\nline2", "tab": "col1\tcol2"}"#;

    let result = parse_and_validate_json_str(escaped_json, &limits);
    assert!(result.is_ok(), "Escaped characters should be valid");
}

/// Test: Payload with many keys (potential hash DoS)
#[test]
fn test_many_keys_object() {
    let limits = JsonLimits {
        max_size: 50000,
        max_depth: 10,
    };

    // Create object with many keys
    let mut json_str = String::from("{");
    for i in 0..1000 {
        if i > 0 {
            json_str.push(',');
        }
        json_str.push_str(&format!(r#""key{}": "value{}""#, i, i));
    }
    json_str.push('}');

    let result = parse_and_validate_json_str(&json_str, &limits);
    // Should pass if size is within limits
    assert!(result.is_ok(), "Many keys object should pass size check");
}

/// Test: Alternating arrays and objects
#[test]
fn test_alternating_nesting() {
    let limits = JsonLimits {
        max_size: 10000,
        max_depth: 3, // Only allow 3 levels
    };

    // Create alternating nested structure: [{}, {}] - 4 levels deep
    let json_str = r#"[{"a": [{"b": null}]}, {"c": [{"d": null}]}]"#;

    let result = parse_and_validate_json_str(json_str, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::DepthTooLarge { .. })),
        "Expected DepthTooLarge for alternating 4-level structure"
    );
}

/// Test: Very long string value
#[test]
fn test_very_long_string_value() {
    let limits = JsonLimits {
        max_size: 500,
        max_depth: 10,
    };

    // Create a single long string that exceeds size limit
    // Need more than 500 - 12 = 488 characters
    let long_string = "x".repeat(490); // 490 chars plus JSON overhead = 502 bytes
    let json_str = format!(r#"{{"data": "{}"}}"#, long_string);

    let result = parse_and_validate_json_str(&json_str, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::SizeTooLarge { .. })),
        "Long string should exceed size limit"
    );
}

/// Test: Valid complex JSON within limits
#[test]
fn test_valid_complex_json_within_limits() {
    let limits = JsonLimits::default();

    // Complex but valid JSON
    let complex_json = r#"{
        "users": [
            {
                "id": 1,
                "name": "Alice",
                "roles": ["admin", "user"],
                "metadata": {"created": "2024-01-01", "active": true}
            },
            {
                "id": 2,
                "name": "Bob",
                "roles": ["user"],
                "metadata": {"created": "2024-01-02", "active": false}
            }
        ],
        "settings": {
            "timeout": 30,
            "retries": 3,
            "features": {"feature_a": true, "feature_b": false}
        }
    }"#;

    let result = parse_and_validate_json_str(complex_json, &limits);
    assert!(result.is_ok(), "Complex valid JSON should parse successfully");

    // Verify structure is preserved
    let parsed = result.unwrap();
    assert_eq!(parsed["users"][0]["name"], "Alice");
    assert_eq!(parsed["users"][1]["roles"][0], "user");
    assert_eq!(parsed["settings"]["timeout"], 30);
}

/// Test: Numeric edge cases
#[test]
fn test_numeric_edge_cases() {
    let limits = JsonLimits::default();

    // Very large number
    assert!(parse_and_validate_json_str(r#"{"big": 999999999999999999}"#, &limits).is_ok());

    // Negative number
    assert!(parse_and_validate_json_str(r#"{"neg": -42}"#, &limits).is_ok());

    // Float
    assert!(parse_and_validate_json_str(r#"{"float": 3.14159}"#, &limits).is_ok());

    // Scientific notation
    assert!(parse_and_validate_json_str(r#"{"sci": 1.5e10}"#, &limits).is_ok());
}

/// Test: Whitespace handling
#[test]
fn test_whitespace_handling() {
    let limits = JsonLimits::default();

    // JSON with lots of whitespace (still valid)
    let whitespace_json = r#"{
        "key1" : "value1",
        "key2" : "value2",
        "nested" : {
            "deep" : "value"
        }
    }"#;

    let result = parse_and_validate_json_str(whitespace_json, &limits);
    assert!(result.is_ok(), "Whitespace should be handled correctly");
}

/// Test: Zero max_size limit (rejects everything)
#[test]
fn test_zero_max_size_rejects_all() {
    let limits = JsonLimits {
        max_size: 0,
        max_depth: 128,
    };

    // Even empty object exceeds size 0
    let result = parse_and_validate_json_str(r#"{}"#, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::SizeTooLarge { .. })),
        "Zero max_size should reject even empty object"
    );
}

/// Test: Zero max_depth limit (rejects all nested structures)
#[test]
fn test_zero_max_depth_rejects_nested() {
    let limits = JsonLimits {
        max_size: 10000,
        max_depth: 0,
    };

    // Root level primitive values should pass (depth 0)
    assert!(parse_and_validate_json_str(r#"null"#, &limits).is_ok());
    assert!(parse_and_validate_json_str(r#"42"#, &limits).is_ok());
    assert!(parse_and_validate_json_str(r#""hello""#, &limits).is_ok());

    // Any nesting should fail (use non-empty to trigger depth calculation)
    let result = parse_and_validate_json_str(r#"[null]"#, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::DepthTooLarge { .. })),
        "Array with element should exceed max_depth=0"
    );

    let result = parse_and_validate_json_str(r#"{"a": null}"#, &limits);
    assert!(
        matches!(result, Err(JsonValidationError::DepthTooLarge { .. })),
        "Object with field should exceed max_depth=0"
    );
}

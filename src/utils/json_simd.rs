// High-performance JSON parsing with SIMD acceleration
// Falls back to standard serde_json when SIMD is not available

#![allow(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Error as JsonError, Value};

/// Parse JSON with SIMD acceleration when available
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
pub fn parse_json_fast<T: for<'de> Deserialize<'de>>(input: &[u8]) -> Result<T, JsonError> {
    // Runtime feature detection for AVX2
    if is_x86_feature_detected!("avx2") {
        // First validate UTF-8 using SIMD
        unsafe {
            if !crate::simd::json::validate_utf8_simd(input) {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid UTF-8",
                )));
            }
        }

        // Use optimized whitespace skipping
        let ws_positions = unsafe { crate::simd::json::find_whitespace_simd(input) };

        // If there's minimal whitespace, parse directly
        if ws_positions.len() < input.len() / 10 {
            serde_json::from_slice(input)
        } else {
            // Strip unnecessary whitespace first
            let mut cleaned = Vec::with_capacity(input.len() - ws_positions.len());
            let mut in_string = false;
            let mut escape_next = false;

            for (i, &byte) in input.iter().enumerate() {
                if escape_next {
                    escape_next = false;
                    cleaned.push(byte);
                    continue;
                }

                if byte == b'\\' && in_string {
                    escape_next = true;
                    cleaned.push(byte);
                    continue;
                }

                if byte == b'"' && !escape_next {
                    in_string = !in_string;
                    cleaned.push(byte);
                    continue;
                }

                if !in_string && ws_positions.binary_search(&i).is_ok() {
                    // Skip whitespace outside strings
                    continue;
                }

                cleaned.push(byte);
            }

            serde_json::from_slice(&cleaned)
        }
    } else {
        // Fallback to standard parsing
        serde_json::from_slice(input)
    }
}

/// Parse JSON - fallback for non-SIMD platforms
#[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
pub fn parse_json_fast<T: for<'de> Deserialize<'de>>(input: &[u8]) -> Result<T, JsonError> {
    serde_json::from_slice(input)
}

/// Serialize JSON with SIMD acceleration when available
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
pub fn serialize_json_fast<T: Serialize>(value: &T) -> Result<Vec<u8>, JsonError> {
    let json = serde_json::to_vec(value)?;

    // Runtime feature detection
    if is_x86_feature_detected!("avx2") {
        // Use SIMD to validate output
        unsafe {
            if !crate::simd::json::validate_utf8_simd(&json) {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Generated invalid UTF-8",
                )));
            }
        }
    }

    Ok(json)
}

/// Serialize JSON - fallback for non-SIMD platforms
#[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
pub fn serialize_json_fast<T: Serialize>(value: &T) -> Result<Vec<u8>, JsonError> {
    serde_json::to_vec(value)
}

/// Batch JSON parsing with optional parallelization
pub fn parse_json_batch<T: for<'de> Deserialize<'de>>(
    inputs: &[&[u8]],
) -> Vec<Result<T, JsonError>> {
    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;

        // Process in parallel when beneficial (more than 4 items)
        if inputs.len() > 4 {
            return inputs
                .par_iter()
                .map(|input| parse_json_fast(input))
                .collect();
        }
    }

    // Sequential processing for small batches or when rayon not available
    inputs.iter().map(|input| parse_json_fast(input)).collect()
}

/// Fast JSON pretty printing
pub fn pretty_print_fast(value: &Value) -> Result<String, JsonError> {
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            let compact = serde_json::to_vec(value)?;

            // Find all structure points using SIMD
            let escapes = unsafe { crate::simd::json::find_escapes_simd(&compact) };

            // Allocate with estimated size
            let mut result = String::with_capacity(compact.len() * 2);
            let mut indent = 0;
            let mut in_string = false;
            let mut escape_next = false;

            for (i, &byte) in compact.iter().enumerate() {
                if escape_next {
                    escape_next = false;
                    result.push(byte as char);
                    continue;
                }

                if escapes.binary_search(&i).is_ok() {
                    if byte == b'\\' && in_string {
                        escape_next = true;
                    } else if byte == b'"' {
                        in_string = !in_string;
                    }
                }

                if !in_string {
                    match byte {
                        b'{' | b'[' => {
                            result.push(byte as char);
                            indent += 2;
                            result.push('\n');
                            result.push_str(&" ".repeat(indent));
                        },
                        b'}' | b']' => {
                            indent = indent.saturating_sub(2);
                            result.push('\n');
                            result.push_str(&" ".repeat(indent));
                            result.push(byte as char);
                        },
                        b',' => {
                            result.push(',');
                            result.push('\n');
                            result.push_str(&" ".repeat(indent));
                        },
                        b':' => {
                            result.push_str(": ");
                        },
                        b' ' | b'\t' | b'\n' | b'\r' => {
                            // Skip whitespace
                        },
                        _ => {
                            result.push(byte as char);
                        },
                    }
                } else {
                    result.push(byte as char);
                }
            }

            return Ok(result);
        }
    }

    // Fallback to standard pretty printing
    serde_json::to_string_pretty(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_json_fast() {
        let input = r#"{"name": "test", "value": 42, "nested": {"array": [1, 2, 3]}}"#;
        let result: Value = parse_json_fast(input.as_bytes()).unwrap();

        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
        assert_eq!(result["nested"]["array"][0], 1);
    }

    #[test]
    fn test_serialize_json_fast() {
        let value = json!({
            "message": "Hello, SIMD!",
            "numbers": [1, 2, 3, 4, 5],
            "nested": {
                "key": "value"
            }
        });

        let serialized = serialize_json_fast(&value).unwrap();
        let parsed: Value = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(parsed, value);
    }

    #[test]
    fn test_batch_parsing() {
        let inputs = vec![
            r#"{"id": 1}"#.as_bytes(),
            r#"{"id": 2}"#.as_bytes(),
            r#"{"id": 3}"#.as_bytes(),
        ];

        let results: Vec<Result<Value, _>> = parse_json_batch(&inputs);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap()["id"], 1);
        assert_eq!(results[1].as_ref().unwrap()["id"], 2);
        assert_eq!(results[2].as_ref().unwrap()["id"], 3);
    }

    #[test]
    fn test_pretty_print() {
        let value = json!({"compact": true, "array": [1, 2, 3]});
        let pretty = pretty_print_fast(&value).unwrap();

        assert!(pretty.contains('\n'));
        assert!(pretty.contains("  "));
    }
}

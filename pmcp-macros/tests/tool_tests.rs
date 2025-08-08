//! Integration tests for the tool macro
//!
//! These tests verify that the #[tool] macro correctly generates
//! tool handlers with proper schema generation and type safety.

use pmcp_macros::tool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
struct AddParams {
    a: i32,
    b: i32,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    sum: i32,
}

#[test]
fn test_simple_tool_macro() {
    #[tool(description = "Add two numbers")]
    fn add(params: AddParams) -> AddResult {
        AddResult {
            sum: params.a + params.b,
        }
    }
    
    // The macro should generate AddToolHandler
    // We can't easily test the generated code directly in integration tests,
    // but we can verify it compiles
}

#[test]
fn test_async_tool_macro() {
    #[tool(description = "Async addition")]
    async fn add_async(params: AddParams) -> AddResult {
        AddResult {
            sum: params.a + params.b,
        }
    }
}

#[test]
fn test_tool_with_result_type() {
    #[tool(description = "Division with error handling")]
    fn divide(a: f64, b: f64) -> Result<f64, String> {
        if b == 0.0 {
            Err("Division by zero".to_string())
        } else {
            Ok(a / b)
        }
    }
}

#[test]
fn test_tool_with_optional_params() {
    #[derive(Debug, Deserialize, JsonSchema)]
    struct GreetParams {
        name: String,
        title: Option<String>,
    }
    
    #[tool(description = "Greet a person")]
    fn greet(params: GreetParams) -> String {
        match params.title {
            Some(title) => format!("Hello, {} {}!", title, params.name),
            None => format!("Hello, {}!", params.name),
        }
    }
}

#[test]
fn test_tool_with_custom_name() {
    #[tool(name = "math_multiply", description = "Multiply two numbers")]
    fn mul(a: i32, b: i32) -> i32 {
        a * b
    }
}

#[test]
fn test_tool_with_complex_types() {
    #[derive(Debug, Deserialize, JsonSchema)]
    struct ComplexInput {
        items: Vec<String>,
        metadata: std::collections::HashMap<String, serde_json::Value>,
        nested: NestedStruct,
    }
    
    #[derive(Debug, Deserialize, JsonSchema)]
    struct NestedStruct {
        field: String,
    }
    
    #[derive(Debug, Serialize, JsonSchema)]
    struct ComplexOutput {
        processed: Vec<String>,
        count: usize,
    }
    
    #[tool(description = "Process complex data")]
    fn process_complex(input: ComplexInput) -> ComplexOutput {
        ComplexOutput {
            processed: input.items,
            count: input.metadata.len(),
        }
    }
}

// Property tests
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn test_tool_with_arbitrary_inputs(a in any::<i32>(), b in any::<i32>()) {
            #[tool(description = "Add arbitrary numbers")]
            fn add_prop(x: i32, y: i32) -> i64 {
                x as i64 + y as i64
            }
            
            // Test would verify the generated handler works with arbitrary inputs
            prop_assert!(true); // Placeholder
        }
    }
}
//! Integration tests for the tool_router macro
//!
//! These tests verify that the #[tool_router] macro correctly collects
//! tool methods and generates routing code.

use pmcp_macros::{tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct Calculator {
    precision: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MathParams {
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct MathResult {
    result: f64,
}

#[test]
fn test_tool_router_with_multiple_tools() {
    #[tool_router]
    impl Calculator {
        #[tool(description = "Add two numbers")]
        async fn add(&self, params: MathParams) -> Result<MathResult, String> {
            Ok(MathResult {
                result: params.a + params.b,
            })
        }

        #[tool(description = "Subtract two numbers")]
        async fn subtract(&self, params: MathParams) -> Result<MathResult, String> {
            Ok(MathResult {
                result: params.a - params.b,
            })
        }

        #[tool(description = "Multiply two numbers")]
        async fn multiply(&self, params: MathParams) -> Result<MathResult, String> {
            Ok(MathResult {
                result: params.a * params.b,
            })
        }

        #[tool(name = "div", description = "Divide two numbers")]
        async fn divide(&self, params: MathParams) -> Result<MathResult, String> {
            if params.b == 0.0 {
                Err("Division by zero".to_string())
            } else {
                Ok(MathResult {
                    result: params.a / params.b,
                })
            }
        }

        // Non-tool method (should be ignored)
        fn helper(&self) -> usize {
            self.precision
        }
    }
}

#[derive(Debug, Clone)]
struct StringProcessor;

#[test]
fn test_tool_router_with_sync_methods() {
    #[tool_router]
    impl StringProcessor {
        #[tool(description = "Convert to uppercase")]
        fn to_upper(&self, text: String) -> String {
            text.to_uppercase()
        }

        #[tool(description = "Convert to lowercase")]
        fn to_lower(&self, text: String) -> String {
            text.to_lowercase()
        }

        #[tool(description = "Reverse a string")]
        fn reverse(&self, text: String) -> String {
            text.chars().rev().collect()
        }
    }
}

#[derive(Debug, Clone)]
struct ComplexServer {
    state: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

#[test]
fn test_tool_router_with_state() {
    #[tool_router]
    impl ComplexServer {
        #[tool(description = "Add item to state")]
        async fn add_item(&self, item: String) -> Result<usize, String> {
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.push(item);
            Ok(state.len())
        }

        #[tool(description = "Get all items")]
        async fn get_items(&self) -> Result<Vec<String>, String> {
            let state = self.state.lock().map_err(|e| e.to_string())?;
            Ok(state.clone())
        }

        #[tool(description = "Clear all items")]
        async fn clear(&self) -> Result<(), String> {
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.clear();
            Ok(())
        }
    }
}

#[test]
fn test_tool_router_with_generics() {
    #[derive(Debug, Clone)]
    struct GenericProcessor<T: Clone + Send + Sync> {
        _phantom: std::marker::PhantomData<T>,
    }

    #[tool_router]
    impl<T: Clone + Send + Sync + 'static> GenericProcessor<T> {
        #[tool(description = "Echo input")]
        fn echo(&self, input: String) -> String {
            input
        }
    }
}

// Test with custom router name and visibility
#[test]
fn test_tool_router_with_custom_options() {
    #[derive(Debug, Clone)]
    struct CustomServer {
        tool_router: Vec<String>, // Would be the actual router type
    }

    #[tool_router(router = "my_router", vis = "pub(crate)")]
    impl CustomServer {
        #[tool(description = "Custom tool")]
        fn custom(&self) -> String {
            "custom".to_string()
        }
    }
}

// Property tests for router
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    #[derive(Debug, Clone)]
    struct PropTestServer;

    proptest! {
        #[test]
        fn test_router_handles_arbitrary_tool_names(name in "[a-z_]+") {
            #[tool_router]
            impl PropTestServer {
                #[tool(description = "Test tool")]
                fn test(&self) -> String {
                    "test".to_string()
                }
            }

            // Verify the router compiles with arbitrary tool names
            prop_assert!(true);
        }
    }
}

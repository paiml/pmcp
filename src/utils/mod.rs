//! Utility modules for the MCP SDK.

pub mod validation;

// Re-export commonly used utilities
pub use validation::{validate_method_name, validate_protocol_version};

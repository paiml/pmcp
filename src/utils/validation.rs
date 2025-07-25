//! Validation utilities for MCP protocol.

use crate::error::{Error, Result};

/// Validate a protocol version string.
///
/// # Examples
///
/// ```rust
/// use mcp_sdk::utils::validate_protocol_version;
///
/// assert!(validate_protocol_version("2025-06-18").is_ok());
/// assert!(validate_protocol_version("invalid").is_err());
/// ```
pub fn validate_protocol_version(version: &str) -> Result<()> {
    if crate::SUPPORTED_PROTOCOL_VERSIONS.contains(&version) {
        Ok(())
    } else {
        Err(Error::validation(format!(
            "Unsupported protocol version: {}. Supported versions: {:?}",
            version,
            crate::SUPPORTED_PROTOCOL_VERSIONS
        )))
    }
}

/// Validate a method name format.
///
/// Method names should follow the pattern: `category/action` or just `action`.
///
/// # Examples
///
/// ```rust
/// use mcp_sdk::utils::validate_method_name;
///
/// assert!(validate_method_name("tools/list").is_ok());
/// assert!(validate_method_name("initialize").is_ok());
/// assert!(validate_method_name("invalid/method/name").is_err());
/// assert!(validate_method_name("").is_err());
/// ```
pub fn validate_method_name(method: &str) -> Result<()> {
    if method.is_empty() {
        return Err(Error::validation("Method name cannot be empty"));
    }

    let parts: Vec<&str> = method.split('/').collect();
    if parts.len() > 2 {
        return Err(Error::validation(format!(
            "Invalid method name format: {}. Expected 'category/action' or 'action'",
            method
        )));
    }

    for part in parts {
        if part.is_empty() {
            return Err(Error::validation(format!(
                "Invalid method name: {} contains empty parts",
                method
            )));
        }

        if !part
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(Error::validation(format!(
                "Invalid method name: {} contains invalid characters",
                method
            )));
        }
    }

    Ok(())
}

/// Validate a resource URI format.
pub fn validate_resource_uri(uri: &str) -> Result<()> {
    if uri.is_empty() {
        return Err(Error::validation("Resource URI cannot be empty"));
    }

    // Basic URI validation - could be expanded
    if !uri.contains(':') {
        return Err(Error::validation(format!(
            "Invalid resource URI: {} (missing scheme)",
            uri
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_protocol_version_valid() {
        assert!(validate_protocol_version("2025-06-18").is_ok());
        assert!(validate_protocol_version("2025-03-26").is_ok());
        assert!(validate_protocol_version("2024-11-05").is_ok());
    }

    #[test]
    fn validate_protocol_version_invalid() {
        assert!(validate_protocol_version("2023-01-01").is_err());
        assert!(validate_protocol_version("invalid").is_err());
        assert!(validate_protocol_version("").is_err());
    }

    #[test]
    fn validate_method_name_valid() {
        assert!(validate_method_name("initialize").is_ok());
        assert!(validate_method_name("tools/list").is_ok());
        assert!(validate_method_name("prompts/get").is_ok());
        assert!(validate_method_name("test-method").is_ok());
        assert!(validate_method_name("test_method").is_ok());
    }

    #[test]
    fn validate_method_name_invalid() {
        assert!(validate_method_name("").is_err());
        assert!(validate_method_name("a/b/c").is_err());
        assert!(validate_method_name("invalid/").is_err());
        assert!(validate_method_name("/invalid").is_err());
        assert!(validate_method_name("inv@lid").is_err());
    }
}

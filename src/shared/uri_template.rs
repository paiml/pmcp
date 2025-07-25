//! URI template expansion utilities.
//!
//! Implements a subset of RFC 6570 URI Template for resource URIs.

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

static TEMPLATE_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_template_regex() -> &'static Regex {
    TEMPLATE_REGEX.get_or_init(|| {
        Regex::new(r"\{([^}]+)\}").expect("hardcoded regex pattern should be valid")
    })
}

/// Expand a URI template with the given variables.
///
/// # Examples
///
/// ```rust
/// use mcp_sdk::shared::uri_template::expand;
/// use std::collections::HashMap;
///
/// let mut vars = HashMap::new();
/// vars.insert("user", "alice");
/// vars.insert("repo", "hello-world");
///
/// let result = expand("https://github.com/{user}/{repo}", &vars);
/// assert_eq!(result, "https://github.com/alice/hello-world");
/// ```
pub fn expand(template: &str, variables: &HashMap<&str, &str>) -> String {
    get_template_regex()
        .replace_all(template, |caps: &regex::Captures<'_>| {
            let var_name = &caps[1];
            variables.get(var_name).copied().unwrap_or("")
        })
        .to_string()
}

/// Extract variable names from a URI template.
///
/// # Examples
///
/// ```rust
/// use mcp_sdk::shared::uri_template::extract_variables;
///
/// let vars = extract_variables("https://api.example.com/{version}/users/{id}");
/// assert_eq!(vars, vec!["version", "id"]);
/// ```
pub fn extract_variables(template: &str) -> Vec<String> {
    get_template_regex()
        .captures_iter(template)
        .map(|caps| caps[1].to_string())
        .collect()
}

/// Check if a string is a URI template.
///
/// # Examples
///
/// ```rust
/// use mcp_sdk::shared::uri_template::is_template;
///
/// assert!(is_template("https://api.example.com/{version}/users"));
/// assert!(!is_template("https://api.example.com/v1/users"));
/// ```
pub fn is_template(uri: &str) -> bool {
    get_template_regex().is_match(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_basic() {
        let mut vars = HashMap::new();
        vars.insert("name", "world");

        assert_eq!(expand("hello {name}", &vars), "hello world");
    }

    #[test]
    fn expand_multiple() {
        let mut vars = HashMap::new();
        vars.insert("proto", "https");
        vars.insert("host", "example.com");
        vars.insert("path", "api/v1");

        assert_eq!(
            expand("{proto}://{host}/{path}", &vars),
            "https://example.com/api/v1"
        );
    }

    #[test]
    fn expand_missing_variable() {
        let vars = HashMap::new();
        assert_eq!(expand("hello {name}", &vars), "hello ");
    }

    #[test]
    fn extract_variables_basic() {
        let vars = extract_variables("user/{id}/posts/{post_id}");
        assert_eq!(vars, vec!["id", "post_id"]);
    }

    #[test]
    fn extract_variables_none() {
        let vars = extract_variables("no/templates/here");
        assert!(vars.is_empty());
    }

    #[test]
    fn is_template_check() {
        assert!(is_template("{var}"));
        assert!(is_template("prefix/{var}/suffix"));
        assert!(!is_template("no templates"));
        assert!(!is_template(""));
    }
}

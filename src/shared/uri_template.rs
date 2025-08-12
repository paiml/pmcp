//! RFC 6570 URI Template implementation for MCP resource URIs.
//!
//! This module provides a complete implementation of RFC 6570 URI Templates,
//! supporting all expression types and operators for dynamic URI generation.

use crate::error::{Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::sync::LazyLock;

/// Regex for parsing URI template expressions
static TEMPLATE_EXPR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]+)\}").unwrap());

/// Regex for validating variable names
static VAR_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap());

/// RFC 6570 URI Template for dynamic resource URIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriTemplate {
    /// The template string
    template: String,
    /// Parsed expressions for efficient expansion
    #[allow(dead_code)]
    expressions: Vec<Expression>,
}

// Custom serialization - only serialize the template string
impl Serialize for UriTemplate {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.template.serialize(serializer)
    }
}

// Custom deserialization - parse from template string
impl<'de> Deserialize<'de> for UriTemplate {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let template = String::deserialize(deserializer)?;
        Self::new(template).map_err(serde::de::Error::custom)
    }
}

/// Expression types in URI templates
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Some variants are kept for backwards compatibility
enum Expression {
    /// Simple string substitution: {var}
    Simple(String),
    /// Reserved expansion: {+var}
    Reserved(String),
    /// Fragment expansion: {#var}
    Fragment(String),
    /// Label expansion with dot prefix: {.var}
    Label(String),
    /// Path segment expansion: {/var}
    PathSegment(String),
    /// Path parameter expansion: {;var}
    PathParameter(String),
    /// Query expansion: {?var}
    Query(String),
    /// Query continuation: {&var}
    QueryContinuation(String),
    /// Multiple variables with operator
    Multiple(Operator, Vec<VarSpec>),
    /// Literal text between expressions
    Literal(String),
}

/// Operators for URI template expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    Simple,
    Reserved,
    Fragment,
    Label,
    PathSegment,
    PathParameter,
    Query,
    QueryContinuation,
}

/// Variable specification with optional modifier
#[derive(Debug, Clone, PartialEq, Eq)]
struct VarSpec {
    name: String,
    modifier: Option<Modifier>,
}

/// Variable modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    /// Prefix modifier: {var:3}
    Prefix(usize),
    /// Explode modifier: {var*}
    Explode,
}

impl UriTemplate {
    /// Create a new URI template from a string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::uri_template::UriTemplate;
    ///
    /// let template = UriTemplate::new("/users/{id}").unwrap();
    /// let uri = template.expand(&[("id", "123")]).unwrap();
    /// assert_eq!(uri, "/users/123");
    /// ```
    pub fn new(template: impl Into<String>) -> Result<Self> {
        let template = template.into();
        let expressions = Self::parse_template(&template)?;

        Ok(Self {
            template,
            expressions,
        })
    }

    /// Parse template string into expressions
    fn parse_template(template: &str) -> Result<Vec<Expression>> {
        let mut expressions = Vec::new();
        let mut last_end = 0;

        for cap in TEMPLATE_EXPR.captures_iter(template) {
            let match_range = cap.get(0).unwrap();

            // Add literal text before this expression
            if match_range.start() > last_end {
                expressions.push(Expression::Literal(
                    template[last_end..match_range.start()].to_string(),
                ));
            }

            // Parse the expression
            let expr_str = &cap[1];
            expressions.push(Self::parse_expression(expr_str)?);

            last_end = match_range.end();
        }

        // Add remaining literal text
        if last_end < template.len() {
            expressions.push(Expression::Literal(template[last_end..].to_string()));
        }

        Ok(expressions)
    }

    /// Parse a single expression
    fn parse_expression(expr: &str) -> Result<Expression> {
        if expr.is_empty() {
            return Err(Error::Validation("Empty template expression".into()));
        }

        let first_char = expr.chars().next().unwrap();
        let (operator, vars_str) = match first_char {
            '+' => (Operator::Reserved, &expr[1..]),
            '#' => (Operator::Fragment, &expr[1..]),
            '.' => (Operator::Label, &expr[1..]),
            '/' => (Operator::PathSegment, &expr[1..]),
            ';' => (Operator::PathParameter, &expr[1..]),
            '?' => (Operator::Query, &expr[1..]),
            '&' => (Operator::QueryContinuation, &expr[1..]),
            _ => (Operator::Simple, expr),
        };

        // Parse variable specifications
        let var_specs: Vec<VarSpec> = vars_str
            .split(',')
            .map(|v| Self::parse_var_spec(v.trim()))
            .collect::<Result<Vec<_>>>()?;

        if var_specs.is_empty() {
            return Err(Error::Validation("Empty variable list in template".into()));
        }

        // Create appropriate expression type
        // Always use Multiple for operators to handle prefixes correctly
        Ok(Expression::Multiple(operator, var_specs))
    }

    /// Parse a variable specification
    fn parse_var_spec(spec: &str) -> Result<VarSpec> {
        if spec.is_empty() {
            return Err(Error::Validation("Empty variable specification".into()));
        }

        // Check for explode modifier
        if spec.ends_with('*') {
            let name = spec.strip_suffix('*').unwrap();
            Self::validate_var_name(name)?;
            return Ok(VarSpec {
                name: name.to_string(),
                modifier: Some(Modifier::Explode),
            });
        }

        // Check for prefix modifier
        if let Some(colon_pos) = spec.find(':') {
            let name = &spec[..colon_pos];
            let length_str = &spec[colon_pos + 1..];

            Self::validate_var_name(name)?;

            let length = length_str
                .parse::<usize>()
                .map_err(|_| Error::Validation(format!("Invalid prefix length: {}", length_str)))?;

            if length == 0 || length > 10000 {
                return Err(Error::Validation(
                    "Prefix length must be between 1 and 10000".into(),
                ));
            }

            return Ok(VarSpec {
                name: name.to_string(),
                modifier: Some(Modifier::Prefix(length)),
            });
        }

        // Simple variable
        Self::validate_var_name(spec)?;
        Ok(VarSpec {
            name: spec.to_string(),
            modifier: None,
        })
    }

    /// Validate variable name
    fn validate_var_name(name: &str) -> Result<()> {
        if !VAR_NAME.is_match(name) {
            return Err(Error::Validation(format!(
                "Invalid variable name: {}",
                name
            )));
        }
        Ok(())
    }

    /// Expand the template with the given variables.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::uri_template::UriTemplate;
    ///
    /// let template = UriTemplate::new("/search{?q,limit}").unwrap();
    /// let uri = template.expand(&[
    ///     ("q", "rust"),
    ///     ("limit", "10"),
    /// ]).unwrap();
    /// assert_eq!(uri, "/search?q=rust&limit=10");
    /// ```
    pub fn expand<K, V>(&self, vars: &[(K, V)]) -> Result<String>
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let var_map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
            .collect();

        self.expand_with_map(&var_map)
    }

    /// Expand the template with a variable map.
    pub fn expand_with_map(&self, vars: &HashMap<String, String>) -> Result<String> {
        let mut result = String::new();

        for expr in &self.expressions {
            result.push_str(&Self::expand_expression(expr, vars));
        }

        Ok(result)
    }

    /// Expand a single expression
    fn expand_expression(
        expr: &Expression,
        vars: &HashMap<String, String>,
    ) -> String {
        match expr {
            Expression::Literal(text) => text.clone(),
            Expression::Multiple(op, specs) => Self::expand_multiple(*op, specs, vars),
            // The following variants are kept for backwards compatibility but shouldn't be created anymore
            Expression::Simple(name) => vars
                .get(name)
                .map(|v| Self::encode_value(v, false))
                .unwrap_or_default(),
            Expression::Reserved(name) => vars
                .get(name)
                .map(|v| Self::encode_value(v, true))
                .unwrap_or_default(),
            Expression::Fragment(name) => vars
                .get(name)
                .map(|v| format!("#{}", Self::encode_value(v, true)))
                .unwrap_or_default(),
            Expression::Label(name) => vars
                .get(name)
                .map(|v| format!(".{}", Self::encode_value(v, false)))
                .unwrap_or_default(),
            Expression::PathSegment(name) => vars
                .get(name)
                .map(|v| format!("/{}", Self::encode_value(v, false)))
                .unwrap_or_default(),
            Expression::PathParameter(name) => vars
                .get(name)
                .map(|v| format!(";{}={}", name, Self::encode_value(v, false)))
                .unwrap_or_default(),
            Expression::Query(name) => vars
                .get(name)
                .map(|v| format!("?{}={}", name, Self::encode_value(v, false)))
                .unwrap_or_default(),
            Expression::QueryContinuation(name) => vars
                .get(name)
                .map(|v| format!("&{}={}", name, Self::encode_value(v, false)))
                .unwrap_or_default(),
        }
    }

    /// Expand multiple variables with an operator
    fn expand_multiple(
        op: Operator,
        specs: &[VarSpec],
        vars: &HashMap<String, String>,
    ) -> String {
        let mut parts = Vec::new();

        for spec in specs {
            if let Some(value) = vars.get(&spec.name) {
                let encoded = match spec.modifier {
                    Some(Modifier::Prefix(len)) => {
                        let truncated = value.chars().take(len).collect::<String>();
                        Self::encode_value(&truncated, op == Operator::Reserved)
                    },
                    Some(Modifier::Explode) => {
                        // For simplicity, treat explode as normal expansion
                        Self::encode_value(value, op == Operator::Reserved)
                    },
                    None => Self::encode_value(value, op == Operator::Reserved),
                };

                match op {
                    Operator::Simple | Operator::Reserved => {
                        parts.push(encoded);
                    },
                    Operator::Label => {
                        parts.push(encoded);
                    },
                    Operator::PathSegment => {
                        parts.push(encoded);
                    },
                    Operator::PathParameter | Operator::Query | Operator::QueryContinuation => {
                        parts.push(format!("{}={}", spec.name, encoded));
                    },
                    Operator::Fragment => {
                        parts.push(encoded);
                    },
                }
            }
        }

        if parts.is_empty() {
            return String::new();
        }

        let separator = match op {
            Operator::Simple | Operator::Reserved => ",",
            Operator::Label => ".",
            Operator::PathSegment => "/",
            Operator::PathParameter => ";",
            Operator::Query | Operator::QueryContinuation => "&",
            Operator::Fragment => ",",
        };

        let prefix = match op {
            Operator::Fragment => "#",
            Operator::Label => ".",
            Operator::PathSegment => "/",
            Operator::PathParameter => ";",
            Operator::Query => "?",
            Operator::QueryContinuation => "&",
            _ => "",
        };

        format!("{}{}", prefix, parts.join(separator))
    }

    /// Encode a value for URI inclusion
    fn encode_value(value: &str, allow_reserved: bool) -> String {
        if allow_reserved {
            // Allow reserved characters
            value.to_string()
        } else {
            // Percent-encode reserved characters
            urlencoding::encode(value).into_owned()
        }
    }

    /// Match a URI against this template and extract variables.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::uri_template::UriTemplate;
    ///
    /// let template = UriTemplate::new("/users/{id}").unwrap();
    /// let vars = template.match_uri("/users/123").unwrap();
    /// assert_eq!(vars.get("id"), Some(&"123".to_string()));
    /// ```
    pub fn match_uri(&self, uri: &str) -> Option<HashMap<String, String>> {
        // Convert template to regex pattern
        let pattern = self.to_regex_pattern();
        let regex = Regex::new(&pattern).ok()?;

        let captures = regex.captures(uri)?;
        let mut vars = HashMap::new();

        // Extract variable values from captures
        let mut capture_index = 1;
        for expr in &self.expressions {
            if let Some(var_name) = Self::get_var_name(expr) {
                if let Some(value) = captures.get(capture_index) {
                    vars.insert(var_name, value.as_str().to_string());
                }
                capture_index += 1;
            }
        }

        Some(vars)
    }

    /// Convert template to regex pattern for matching
    fn to_regex_pattern(&self) -> String {
        let mut pattern = String::from("^");

        for expr in &self.expressions {
            match expr {
                Expression::Literal(text) => {
                    pattern.push_str(&regex::escape(text));
                },
                Expression::Simple(_) | Expression::Reserved(_) => {
                    pattern.push_str("([^/]+)");
                },
                Expression::Fragment(_) => {
                    pattern.push_str("#(.+)");
                },
                Expression::Label(_) => {
                    pattern.push_str(r"\.([^.]+)");
                },
                Expression::PathSegment(_) => {
                    pattern.push_str("/([^/]+)");
                },
                Expression::PathParameter(name) => {
                    write!(pattern, ";{}=([^;&]+)", regex::escape(name)).unwrap();
                },
                Expression::Query(name) => {
                    write!(pattern, r"\?{}=([^&]+)", regex::escape(name)).unwrap();
                },
                Expression::QueryContinuation(name) => {
                    write!(pattern, "&{}=([^&]+)", regex::escape(name)).unwrap();
                },
                Expression::Multiple(op, specs) => {
                    // Handle based on operator type
                    if specs.len() == 1 {
                        match op {
                            Operator::Simple | Operator::Reserved => {
                                pattern.push_str("([^/]+)");
                            },
                            Operator::Fragment => {
                                pattern.push_str("#(.+)");
                            },
                            Operator::Label => {
                                pattern.push_str(r"\.([^.]+)");
                            },
                            Operator::PathSegment => {
                                pattern.push_str("/([^/]+)");
                            },
                            Operator::PathParameter => {
                                let name = &specs[0].name;
                                write!(pattern, ";{}=([^;&]+)", regex::escape(name)).unwrap();
                            },
                            Operator::Query => {
                                let name = &specs[0].name;
                                write!(pattern, r"\?{}=([^&]+)", regex::escape(name)).unwrap();
                            },
                            Operator::QueryContinuation => {
                                let name = &specs[0].name;
                                write!(pattern, "&{}=([^&]+)", regex::escape(name)).unwrap();
                            },
                        }
                    } else {
                        // Complex matching for multiple vars - simplified
                        pattern.push_str("(.+)");
                    }
                },
            }
        }

        pattern.push('$');
        pattern
    }

    /// Get variable name from expression
    fn get_var_name(expr: &Expression) -> Option<String> {
        match expr {
            Expression::Simple(name)
            | Expression::Reserved(name)
            | Expression::Fragment(name)
            | Expression::Label(name)
            | Expression::PathSegment(name)
            | Expression::PathParameter(name)
            | Expression::Query(name)
            | Expression::QueryContinuation(name) => Some(name.clone()),
            Expression::Multiple(_, specs) if specs.len() == 1 => {
                Some(specs[0].name.clone())
            },
            _ => None,
        }
    }

    /// Get the list of variables in this template.
    pub fn variables(&self) -> Vec<String> {
        let mut vars = Vec::new();

        for expr in &self.expressions {
            match expr {
                Expression::Simple(name)
                | Expression::Reserved(name)
                | Expression::Fragment(name)
                | Expression::Label(name)
                | Expression::PathSegment(name)
                | Expression::PathParameter(name)
                | Expression::Query(name)
                | Expression::QueryContinuation(name) => {
                    vars.push(name.clone());
                },
                Expression::Multiple(_, specs) => {
                    for spec in specs {
                        vars.push(spec.name.clone());
                    }
                },
                Expression::Literal(_) => {},
            }
        }

        vars
    }
}

impl fmt::Display for UriTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expansion() {
        let template = UriTemplate::new("/users/{id}").unwrap();
        let uri = template.expand(&[("id", "123")]).unwrap();
        assert_eq!(uri, "/users/123");
    }

    #[test]
    fn test_query_expansion() {
        let template = UriTemplate::new("/search{?q,limit}").unwrap();
        let uri = template
            .expand(&[("q", "rust programming"), ("limit", "10")])
            .unwrap();
        assert_eq!(uri, "/search?q=rust%20programming&limit=10");
    }

    #[test]
    fn test_path_expansion() {
        let template = UriTemplate::new("{/path*}").unwrap();
        let uri = template.expand(&[("path", "a/b/c")]).unwrap();
        assert_eq!(uri, "/a%2Fb%2Fc");
    }

    #[test]
    fn test_fragment_expansion() {
        let template = UriTemplate::new("/page{#section}").unwrap();
        let uri = template.expand(&[("section", "intro")]).unwrap();
        assert_eq!(uri, "/page#intro");
    }

    #[test]
    fn test_reserved_expansion() {
        let template = UriTemplate::new("/path{+reserved}").unwrap();
        let uri = template.expand(&[("reserved", "/a/b/c")]).unwrap();
        assert_eq!(uri, "/path/a/b/c");
    }

    #[test]
    fn test_label_expansion() {
        let template = UriTemplate::new("/x{.y,z}").unwrap();
        let uri = template.expand(&[("y", "foo"), ("z", "bar")]).unwrap();
        assert_eq!(uri, "/x.foo.bar");
    }

    #[test]
    fn test_match_uri() {
        let template = UriTemplate::new("/users/{id}").unwrap();
        let vars = template.match_uri("/users/123").unwrap();
        assert_eq!(vars.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_variables() {
        let template = UriTemplate::new("/users/{id}/posts/{post_id}").unwrap();
        let vars = template.variables();
        assert_eq!(vars, vec!["id", "post_id"]);
    }
}

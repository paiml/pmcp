//! Utility functions for macro implementations
//!
//! This module provides common utilities used across different macro implementations.

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{GenericParam, Generics, Type, TypePath};

/// Extract the inner type from Option<T>
pub fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

/// Extract the T and E types from Result<T, E>
pub fn extract_result_types(ty: &Type) -> Option<(&Type, &Type)> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if segment.ident == "Result" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    let mut iter = args.args.iter();
                    if let (
                        Some(syn::GenericArgument::Type(ok_type)),
                        Some(syn::GenericArgument::Type(err_type)),
                    ) = (iter.next(), iter.next())
                    {
                        return Some((ok_type, err_type));
                    }
                }
            }
        }
    }
    None
}

/// Generate a unique identifier with a prefix
pub fn generate_unique_ident(prefix: &str) -> Ident {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    Ident::new(
        &format!("{}_{}", prefix, count),
        proc_macro2::Span::call_site(),
    )
}

/// Convert snake_case to PascalCase
pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Convert PascalCase to snake_case
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_is_uppercase = false;

    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !prev_is_uppercase {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
            prev_is_uppercase = true;
        } else {
            result.push(ch);
            prev_is_uppercase = false;
        }
    }

    result
}

/// Generate a JSON schema for a Rust type using schemars
pub fn generate_schema_for_type(ty: &Type) -> TokenStream {
    quote! {
        {
            use schemars::JsonSchema;
            let settings = schemars::gen::SchemaSettings::default();
            let generator = schemars::gen::SchemaGenerator::new(settings);
            let schema = generator.into_root_schema_for::<#ty>();
            serde_json::to_value(schema).unwrap_or_else(|_| serde_json::json!({}))
        }
    }
}

/// Check if a type implements a specific trait
pub fn implements_trait(ty: &Type, trait_name: &str) -> TokenStream {
    quote! {
        {
            fn _implements_trait<T: #trait_name>() {}
            _implements_trait::<#ty>();
        }
    }
}

/// Strip lifetime parameters from generics
pub fn strip_lifetimes(generics: &Generics) -> Generics {
    let mut new_generics = generics.clone();
    new_generics.params = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Lifetime(_) => None,
            other => Some(other.clone()),
        })
        .collect();
    new_generics
}

/// Generate where clause for async trait bounds
pub fn add_async_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(syn::parse_quote!(Send));
            type_param.bounds.push(syn::parse_quote!(Sync));
            type_param.bounds.push(syn::parse_quote!('static));
        }
    }
    generics
}

/// Parse a doc comment from attributes
pub fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            // Simple extraction from doc comments
            let attr_str = quote!(#attr).to_string();
            if let Some(doc_start) = attr_str.find("\"") {
                let doc_start = doc_start + 1;
                if let Some(doc_end) = attr_str[doc_start..].find('"') {
                    let line = &attr_str[doc_start..doc_start + doc_end];
                    // Remove leading space if present
                    let line = if line.starts_with(' ') {
                        &line[1..]
                    } else {
                        line
                    };
                    doc_lines.push(line.to_string());
                }
            }
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

/// Generate error handling code for different error types
pub fn generate_error_conversion(error_type: &Type) -> TokenStream {
    // Check if it's already pmcp::Error
    if is_pmcp_error(error_type) {
        quote! { e }
    } else {
        quote! { pmcp::Error::ToolError(e.to_string()) }
    }
}

/// Check if a type is pmcp::Error
fn is_pmcp_error(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "Error"
                && path.segments.len() >= 2
                && path.segments.iter().any(|s| s.ident == "pmcp");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("add_numbers"), "AddNumbers");
        assert_eq!(to_pascal_case("simple"), "Simple");
        assert_eq!(to_pascal_case(""), "");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        assert_eq!(to_snake_case("AddNumbers"), "add_numbers");
        assert_eq!(to_snake_case("Simple"), "simple");
        assert_eq!(to_snake_case("XMLParser"), "xmlparser");
    }

    #[test]
    fn test_extract_option_inner() {
        let opt_type: Type = parse_quote!(Option<String>);
        assert!(extract_option_inner(&opt_type).is_some());

        let non_opt_type: Type = parse_quote!(String);
        assert!(extract_option_inner(&non_opt_type).is_none());

        let nested_opt: Type = parse_quote!(Option<Option<i32>>);
        assert!(extract_option_inner(&nested_opt).is_some());
    }

    #[test]
    fn test_extract_result_types() {
        let result_type: Type = parse_quote!(Result<String, std::io::Error>);
        let (ok_type, err_type) = extract_result_types(&result_type).unwrap();
        assert!(matches!(ok_type, Type::Path(_)));
        assert!(matches!(err_type, Type::Path(_)));

        let non_result: Type = parse_quote!(String);
        assert!(extract_result_types(&non_result).is_none());
    }

    #[test]
    fn test_generate_unique_ident() {
        let id1 = generate_unique_ident("test");
        let id2 = generate_unique_ident("test");
        assert_ne!(id1.to_string(), id2.to_string());
        assert!(id1.to_string().starts_with("test_"));
        assert!(id2.to_string().starts_with("test_"));
    }
}

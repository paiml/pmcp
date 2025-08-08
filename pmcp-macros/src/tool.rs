//! Tool macro implementation
//!
//! This module implements the `#[tool]` attribute macro for defining MCP tools
//! with automatic schema generation and handler implementation.

use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{
    parse_quote, FnArg, ItemFn, Pat, PatType, ReturnType, Type, TypePath,
};
use syn::parse::Parser;

/// Tool macro arguments
#[derive(Debug, FromMeta)]
struct ToolArgs {
    /// Tool name (defaults to function name)
    #[darling(default)]
    name: Option<String>,
    
    /// Tool description
    description: String,
    
    /// Additional annotations
    #[darling(default)]
    annotations: Option<ToolAnnotations>,
}

/// Tool annotations for metadata
#[derive(Debug, Default, FromMeta)]
struct ToolAnnotations {
    #[darling(default)]
    category: Option<String>,
    
    #[darling(default)]
    complexity: Option<String>,
    
    #[darling(default)]
    read_only: Option<bool>,
}

/// Expands the #[tool] attribute macro  
pub fn expand_tool(args: TokenStream, input: ItemFn) -> syn::Result<TokenStream> {
    // Parse macro arguments from TokenStream
    let nested_metas = if args.is_empty() {
        vec![]
    } else {
        // Parse as key-value pairs
        let parser = syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
        parser
            .parse2(args)
            .map(|p| p.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    
    let args = ToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;
    
    let fn_name = &input.sig.ident;
    let tool_name = args.name.unwrap_or_else(|| fn_name.to_string());
    let description = args.description;
    
    // Extract function parameters
    let params = extract_parameters(&input)?;
    
    // Extract return type
    let return_type = extract_return_type(&input)?;
    
    // Generate the wrapper struct and implementation
    let wrapper_name = Ident::new(
        &format!("{}ToolHandler", to_pascal_case(&fn_name.to_string())),
        fn_name.span(),
    );
    
    // Check if function is async
    let is_async = input.sig.asyncness.is_some();
    let await_token = if is_async { quote!(.await) } else { quote!() };
    
    // Generate parameter extraction code
    let param_extraction = generate_param_extraction(&params)?;
    let param_names: Vec<_> = params.iter().map(|p| &p.name).collect();
    
    // Generate result conversion
    let result_conversion = generate_result_conversion(&return_type)?;
    
    // Build the handler implementation
    let expanded = quote! {
        #input
        
        /// Auto-generated tool handler for #tool_name
        #[derive(Debug, Clone)]
        pub struct #wrapper_name;
        
        #[async_trait::async_trait]
        impl pmcp::ToolHandler for #wrapper_name {
            async fn handle(
                &self,
                args: serde_json::Value,
                _extra: pmcp::RequestHandlerExtra,
            ) -> pmcp::Result<serde_json::Value> {
                // Extract parameters from JSON
                #param_extraction
                
                // Call the original function
                let result = #fn_name(#(#param_names),*)#await_token;
                
                // Convert result to JSON
                #result_conversion
            }
        }
        
        impl #wrapper_name {
            /// Get tool definition
            pub fn definition() -> pmcp::types::Tool {
                pmcp::types::Tool {
                    name: #tool_name.to_string(),
                    description: Some(#description.to_string()),
                    input_schema: Some(Self::input_schema()),
                }
            }
            
            /// Generate input schema
            fn input_schema() -> serde_json::Value {
                // TODO: Use schemars to generate actual schema
                serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                })
            }
        }
    };
    
    Ok(expanded)
}

/// Parameter information
struct ParamInfo {
    name: Ident,
    ty: Type,
    optional: bool,
}

/// Extract parameters from function signature
fn extract_parameters(func: &ItemFn) -> syn::Result<Vec<ParamInfo>> {
    let mut params = Vec::new();
    
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Receiver(_) => {
                // Skip self parameter
                continue;
            }
            FnArg::Typed(PatType { pat, ty, .. }) => {
                if let Pat::Ident(pat_ident) = pat.as_ref() {
                    let name = pat_ident.ident.clone();
                    let ty = ty.as_ref().clone();
                    let optional = is_option_type(&ty);
                    
                    params.push(ParamInfo {
                        name,
                        ty,
                        optional,
                    });
                }
            }
        }
    }
    
    Ok(params)
}

/// Extract return type information
fn extract_return_type(func: &ItemFn) -> syn::Result<Type> {
    match &func.sig.output {
        ReturnType::Default => Ok(parse_quote!(())),
        ReturnType::Type(_, ty) => Ok(ty.as_ref().clone()),
    }
}

/// Generate parameter extraction code from JSON
fn generate_param_extraction(params: &[ParamInfo]) -> syn::Result<TokenStream> {
    let mut extractions = Vec::new();
    
    for param in params {
        let name = &param.name;
        let name_str = name.to_string();
        let ty = &param.ty;
        
        if param.optional {
            extractions.push(quote! {
                let #name: #ty = args.get(#name_str)
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
            });
        } else {
            extractions.push(quote! {
                let #name: #ty = args.get(#name_str)
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .ok_or_else(|| pmcp::Error::InvalidParams(
                        format!("Missing required parameter: {}", #name_str)
                    ))?;
            });
        }
    }
    
    Ok(quote! {
        #(#extractions)*
    })
}

/// Generate result conversion code
fn generate_result_conversion(return_type: &Type) -> syn::Result<TokenStream> {
    // Check if return type is Result<T, E>
    if is_result_type(return_type) {
        Ok(quote! {
            match result {
                Ok(value) => {
                    let json_value = serde_json::to_value(value)
                        .map_err(|e| pmcp::Error::InternalError(e.to_string()))?;
                    Ok(json_value)
                }
                Err(e) => Err(pmcp::Error::ToolError(e.to_string()))
            }
        })
    } else {
        Ok(quote! {
            let json_value = serde_json::to_value(result)
                .map_err(|e| pmcp::Error::InternalError(e.to_string()))?;
            Ok(json_value)
        })
    }
}

/// Check if a type is Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Check if a type is Result<T, E>
fn is_result_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "Result";
        }
    }
    false
}

/// Convert snake_case to PascalCase
fn to_pascal_case(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("add_numbers"), "AddNumbers");
        assert_eq!(to_pascal_case("simple"), "Simple");
    }
    
    #[test]
    fn test_is_option_type() {
        let opt_type: Type = parse_quote!(Option<String>);
        assert!(is_option_type(&opt_type));
        
        let non_opt_type: Type = parse_quote!(String);
        assert!(!is_option_type(&non_opt_type));
    }
    
    #[test]
    fn test_is_result_type() {
        let result_type: Type = parse_quote!(Result<String, Error>);
        assert!(is_result_type(&result_type));
        
        let non_result_type: Type = parse_quote!(String);
        assert!(!is_result_type(&non_result_type));
    }
}
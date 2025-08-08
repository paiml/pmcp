//! Tool router macro implementation
//!
//! This module implements the `#[tool_router]` attribute macro that collects
//! all tool methods from an impl block and generates routing code.

use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::Parser;
use syn::{parse_quote, Attribute, ImplItem, ImplItemFn, ItemImpl, Visibility};

/// Tool router macro arguments
#[derive(Debug, Default, FromMeta)]
struct ToolRouterArgs {
    /// Name of the generated router field (defaults to "tool_router")
    #[darling(default = "default_router_name")]
    router: String,

    /// Visibility of the generated methods (defaults to pub)
    #[darling(default)]
    vis: Option<String>,
}

fn default_router_name() -> String {
    "tool_router".to_string()
}

/// Information about a tool method
struct ToolMethod {
    name: Ident,
    tool_name: String,
    description: String,
    is_async: bool,
}

/// Expands the #[tool_router] attribute macro
pub fn expand_tool_router(args: TokenStream, mut input: ItemImpl) -> syn::Result<TokenStream> {
    // Parse macro arguments from TokenStream
    let nested_metas = if args.is_empty() {
        vec![]
    } else {
        // Parse as key-value pairs
        let parser = syn::punctuated::Punctuated::<NestedMeta, syn::Token![,]>::parse_terminated;
        parser
            .parse2(args)
            .map(|p| p.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
    };

    let args = ToolRouterArgs::from_list(&nested_metas).unwrap_or_default();

    // Find all methods marked with #[tool]
    let tool_methods = collect_tool_methods(&input)?;

    if tool_methods.is_empty() {
        return Err(syn::Error::new_spanned(
            &input,
            "No methods marked with #[tool] found in impl block",
        ));
    }

    // Generate router field name
    let router_field = Ident::new(&args.router, proc_macro2::Span::call_site());

    // Generate visibility
    let vis = parse_visibility(&args.vis)?;

    // Generate tool definitions method
    let tools_method = generate_tools_method(&tool_methods, &vis);

    // Generate handle_tool method
    let handle_tool_method = generate_handle_tool_method(&tool_methods, &vis);

    // Generate router initialization method
    let router_init = generate_router_init(&router_field, &vis);

    // Add generated methods to the impl block
    input.items.push(ImplItem::Fn(tools_method));
    input.items.push(ImplItem::Fn(handle_tool_method));
    input.items.push(ImplItem::Fn(router_init));

    // Add router field to the struct (this would need to be done separately)
    // For now, we'll document that the user needs to add it manually

    let expanded = quote! {
        #input

        impl ToolRouterInfo for Self {
            const ROUTER_FIELD: &'static str = stringify!(#router_field);
        }
    };

    Ok(expanded)
}

/// Collect all methods marked with #[tool] from the impl block
fn collect_tool_methods(impl_block: &ItemImpl) -> syn::Result<Vec<ToolMethod>> {
    let mut methods = Vec::new();

    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if let Some(tool_attr) = find_tool_attribute(&method.attrs) {
                let tool_info = parse_tool_attribute(tool_attr)?;
                let method_name = method.sig.ident.clone();
                let tool_name = tool_info.name.unwrap_or_else(|| method_name.to_string());

                methods.push(ToolMethod {
                    name: method_name,
                    tool_name,
                    description: tool_info.description,
                    is_async: method.sig.asyncness.is_some(),
                });
            }
        }
    }

    Ok(methods)
}

/// Tool attribute information
struct ToolInfo {
    name: Option<String>,
    description: String,
}

/// Find the #[tool] attribute in a list of attributes
fn find_tool_attribute(attrs: &[Attribute]) -> Option<&Attribute> {
    attrs.iter().find(|attr| attr.path().is_ident("tool"))
}

/// Parse the #[tool] attribute to extract its arguments
fn parse_tool_attribute(attr: &Attribute) -> syn::Result<ToolInfo> {
    // For simple parsing, we'll just look for key=value pairs in the meta
    let args_str = quote!(#attr).to_string();

    let mut name = None;
    let mut description = None;

    // Basic parsing - this is simplified but works for our use case
    if args_str.contains("description") {
        // Extract description value
        if let Some(desc_start) = args_str.find("description = \"") {
            let desc_start = desc_start + 15; // length of "description = \""
            if let Some(desc_end) = args_str[desc_start..].find('"') {
                description = Some(args_str[desc_start..desc_start + desc_end].to_string());
            }
        }
    }

    if args_str.contains("name") && args_str.contains("name = \"") {
        // Extract name value
        if let Some(name_start) = args_str.find("name = \"") {
            let name_start = name_start + 8; // length of "name = \""
            if let Some(name_end) = args_str[name_start..].find('"') {
                name = Some(args_str[name_start..name_start + name_end].to_string());
            }
        }
    }

    Ok(ToolInfo {
        name,
        description: description
            .ok_or_else(|| syn::Error::new_spanned(attr, "Tool must have a description"))?,
    })
}

/// Generate the tools() method that returns all tool definitions
fn generate_tools_method(methods: &[ToolMethod], vis: &Visibility) -> ImplItemFn {
    let tool_definitions: Vec<_> = methods
        .iter()
        .map(|method| {
            let name = &method.tool_name;
            let description = &method.description;
            quote! {
                pmcp::types::Tool {
                    name: #name.to_string(),
                    description: Some(#description.to_string()),
                    input_schema: Some(serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    })),
                }
            }
        })
        .collect();

    parse_quote! {
        #vis fn tools(&self) -> Vec<pmcp::types::Tool> {
            vec![
                #(#tool_definitions),*
            ]
        }
    }
}

/// Generate the handle_tool() method for routing tool calls
fn generate_handle_tool_method(methods: &[ToolMethod], vis: &Visibility) -> ImplItemFn {
    let match_arms: Vec<_> = methods
        .iter()
        .map(|method| {
            let tool_name = &method.tool_name;
            let method_name = &method.name;
            let await_token = if method.is_async {
                quote!(.await)
            } else {
                quote!()
            };

            quote! {
                #tool_name => {
                    let result = self.#method_name(args.clone())#await_token;
                    match result {
                        Ok(value) => Ok(serde_json::to_value(value)?),
                        Err(e) => Err(pmcp::Error::ToolError(e.to_string())),
                    }
                }
            }
        })
        .collect();

    parse_quote! {
        #vis async fn handle_tool(
            &self,
            name: &str,
            args: serde_json::Value,
            extra: pmcp::RequestHandlerExtra,
        ) -> pmcp::Result<serde_json::Value> {
            match name {
                #(#match_arms)*
                _ => Err(pmcp::Error::MethodNotFound(format!("Unknown tool: {}", name))),
            }
        }
    }
}

/// Generate router initialization method
fn generate_router_init(router_field: &Ident, vis: &Visibility) -> ImplItemFn {
    parse_quote! {
        #vis fn init_tool_router(&mut self) {
            // Initialize the tool router
            // This is a placeholder - actual implementation would depend on the router type
            self.#router_field = Default::default();
        }
    }
}

/// Parse visibility string into syn::Visibility
fn parse_visibility(vis_str: &Option<String>) -> syn::Result<Visibility> {
    match vis_str {
        None => Ok(parse_quote!(pub)),
        Some(s) if s == "pub" => Ok(parse_quote!(pub)),
        Some(s) if s == "pub(crate)" => Ok(parse_quote!(pub(crate))),
        Some(s) if s == "pub(super)" => Ok(parse_quote!(pub(super))),
        Some(s) if s.is_empty() => Ok(Visibility::Inherited),
        Some(s) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Invalid visibility: {}", s),
        )),
    }
}

/// Trait to mark types that have a tool router
trait ToolRouterInfo {
    const ROUTER_FIELD: &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_visibility() {
        assert!(parse_visibility(&None).is_ok());
        assert!(parse_visibility(&Some("pub".to_string())).is_ok());
        assert!(parse_visibility(&Some("pub(crate)".to_string())).is_ok());
        assert!(parse_visibility(&Some("invalid".to_string())).is_err());
    }
}

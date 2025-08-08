//! WASM client library for PMCP
//!
//! This library provides a WebAssembly-compatible MCP client that can be used
//! in web browsers to connect to MCP servers via WebSocket.

use pmcp::client::Client;
use pmcp::shared::wasm_websocket::WasmWebSocketTransport;
use pmcp::types::{ClientCapabilities, ListToolsResult, CallToolRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::prelude::*;
use web_sys::console;

/// WASM-compatible MCP client
#[wasm_bindgen]
pub struct WasmClient {
    url: String,
    client: Option<Client<WasmWebSocketTransport>>,
}

#[wasm_bindgen]
impl WasmClient {
    /// Create a new WASM client
    #[wasm_bindgen(constructor)]
    pub fn new(url: String) -> Self {
        // Set panic hook for better error messages
        console_error_panic_hook::set_once();
        
        Self {
            url,
            client: None,
        }
    }
    
    /// Connect to the MCP server
    #[wasm_bindgen]
    pub async fn connect(&mut self) -> Result<JsValue, JsValue> {
        console::log_1(&format!("Connecting to {}", self.url).into());
        
        // Create WebSocket transport
        let transport = WasmWebSocketTransport::connect(&self.url)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        // Create client
        let mut client = Client::new(transport);
        
        // Initialize connection
        let capabilities = ClientCapabilities::default();
        let init_result = client.initialize(capabilities)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        console::log_1(&format!("Connected to: {}", init_result.server_info.name).into());
        
        self.client = Some(client);
        
        // Return server info as JSON
        Ok(serde_wasm_bindgen::to_value(&init_result)?)
    }
    
    /// Disconnect from the server
    #[wasm_bindgen]
    pub async fn disconnect(&mut self) -> Result<(), JsValue> {
        if let Some(mut client) = self.client.take() {
            // Client will be dropped, closing the connection
            console::log_1(&"Disconnected from server".into());
        }
        Ok(())
    }
    
    /// List available tools
    #[wasm_bindgen]
    pub async fn list_tools(&mut self) -> Result<JsValue, JsValue> {
        let client = self.client.as_mut()
            .ok_or_else(|| JsValue::from_str("Not connected"))?;
        
        let tools = client.list_tools(None)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        // Convert to JS value
        Ok(serde_wasm_bindgen::to_value(&tools.tools)?)
    }
    
    /// Call a tool with arguments
    #[wasm_bindgen]
    pub async fn call_tool(&mut self, name: String, args: JsValue) -> Result<JsValue, JsValue> {
        let client = self.client.as_mut()
            .ok_or_else(|| JsValue::from_str("Not connected"))?;
        
        // Convert JS value to JSON
        let args_json: Value = serde_wasm_bindgen::from_value(args)?;
        
        console::log_1(&format!("Calling tool: {} with args: {:?}", name, args_json).into());
        
        let result = client.call_tool(name, args_json)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        // Convert result to JS value
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }
    
    /// List available resources
    #[wasm_bindgen]
    pub async fn list_resources(&mut self) -> Result<JsValue, JsValue> {
        let client = self.client.as_mut()
            .ok_or_else(|| JsValue::from_str("Not connected"))?;
        
        let resources = client.list_resources(None)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        Ok(serde_wasm_bindgen::to_value(&resources.resources)?)
    }
    
    /// Read a resource by URI
    #[wasm_bindgen]
    pub async fn read_resource(&mut self, uri: String) -> Result<JsValue, JsValue> {
        let client = self.client.as_mut()
            .ok_or_else(|| JsValue::from_str("Not connected"))?;
        
        let result = client.read_resource(uri)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }
    
    /// List available prompts
    #[wasm_bindgen]
    pub async fn list_prompts(&mut self) -> Result<JsValue, JsValue> {
        let client = self.client.as_mut()
            .ok_or_else(|| JsValue::from_str("Not connected"))?;
        
        let prompts = client.list_prompts(None)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        Ok(serde_wasm_bindgen::to_value(&prompts.prompts)?)
    }
    
    /// Get a prompt by name
    #[wasm_bindgen]
    pub async fn get_prompt(&mut self, name: String, arguments: JsValue) -> Result<JsValue, JsValue> {
        let client = self.client.as_mut()
            .ok_or_else(|| JsValue::from_str("Not connected"))?;
        
        // Convert arguments
        let args: std::collections::HashMap<String, String> = 
            serde_wasm_bindgen::from_value(arguments)?;
        
        let result = client.get_prompt(name, args)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }
}

/// Initialize logging for WASM
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    
    // Set up console logging
    tracing_wasm::set_as_global_default();
    
    console::log_1(&"PMCP WASM Client initialized".into());
}

/// Get the version of the PMCP library
#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Utility function to parse JSON from string
#[wasm_bindgen]
pub fn parse_json(json_str: &str) -> Result<JsValue, JsValue> {
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(serde_wasm_bindgen::to_value(&value)?)
}

/// Utility function to stringify JSON
#[wasm_bindgen]
pub fn stringify_json(value: JsValue) -> Result<String, JsValue> {
    let json_value: Value = serde_wasm_bindgen::from_value(value)?;
    serde_json::to_string_pretty(&json_value)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::{
    CallToolRequest, CallToolResult,
    ListResourcesResult,
    ReadResourceResult,
    GetPromptResult,
    ClientCapabilities, ServerCapabilities,
    ResourceInfo,
    PromptMessage,
    Role, Content,
};
use serde_json::{Value, from_slice, from_value};

fuzz_target!(|data: &[u8]| {
    // Try to parse as various protocol messages
    
    // 1. Try parsing as generic JSON
    if let Ok(json) = from_slice::<Value>(data) {
        // Try parsing as specific request/result types (using only exported types)
        let _ = from_value::<CallToolRequest>(json.clone());
        let _ = from_value::<CallToolResult>(json.clone());
        let _ = from_value::<ListResourcesResult>(json.clone());
        let _ = from_value::<ReadResourceResult>(json.clone());
        let _ = from_value::<GetPromptResult>(json.clone());
        
        // Try parsing as capability types
        let _ = from_value::<ClientCapabilities>(json.clone());
        let _ = from_value::<ServerCapabilities>(json.clone());
        
        // Try parsing as content types
        let _ = from_value::<ResourceInfo>(json.clone());
        let _ = from_value::<PromptMessage>(json.clone());
        let _ = from_value::<Role>(json.clone());
        let _ = from_value::<Content>(json.clone());
    }
    
    // 2. Try parsing as raw protocol message bytes
    if data.len() >= 4 {
        // Simulate message framing
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if len < 1_000_000 && data.len() >= 4 + len {
            let message_data = &data[4..4+len];
            let _ = from_slice::<Value>(message_data);
        }
    }
    
    // 3. Try parsing as newline-delimited JSON
    for line in data.split(|&b| b == b'\n') {
        if !line.is_empty() {
            let _ = from_slice::<Value>(line);
        }
    }
});
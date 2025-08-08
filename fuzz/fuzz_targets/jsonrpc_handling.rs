#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::{Value, from_slice, json};
use arbitrary::{Arbitrary, Unstructured};

// Custom types for fuzzing JSON-RPC internals
#[derive(Debug, Arbitrary)]
struct FuzzJsonRpcRequest {
    id: Option<FuzzId>,
    method: String,
    params: Option<FuzzParams>,
}

#[derive(Debug, Arbitrary)]
enum FuzzId {
    Number(i64),
    String(String),
    Null,
}

#[derive(Debug, Arbitrary)]
enum FuzzParams {
    Object(Vec<(String, FuzzValue)>),
    Array(Vec<FuzzValue>),
}

#[derive(Debug, Arbitrary)]
enum FuzzValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<FuzzValue>),
    Object(Vec<(String, FuzzValue)>),
}

impl FuzzValue {
    fn to_json(&self) -> Value {
        match self {
            FuzzValue::Null => Value::Null,
            FuzzValue::Bool(b) => Value::Bool(*b),
            FuzzValue::Number(n) => json!(n),
            FuzzValue::String(s) => Value::String(s.clone()),
            FuzzValue::Array(arr) => Value::Array(arr.iter().map(|v| v.to_json()).collect()),
            FuzzValue::Object(obj) => {
                let map = obj.iter().map(|(k, v)| (k.clone(), v.to_json())).collect();
                Value::Object(map)
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // 1. Parse arbitrary data as JSON-RPC messages
    if let Ok(json) = from_slice::<Value>(data) {
        // Check if it looks like a JSON-RPC request
        if let Some(obj) = json.as_object() {
            // Validate JSON-RPC structure
            if let Some(Value::String(version)) = obj.get("jsonrpc") {
                assert!(version == "2.0" || version == "1.0");
            }
            
            // Check for required fields
            if obj.contains_key("method") {
                // It's a request or notification
                assert!(obj.get("method").map(|v| v.is_string()).unwrap_or(false));
            } else if obj.contains_key("result") || obj.contains_key("error") {
                // It's a response
                assert!(!(obj.contains_key("result") && obj.contains_key("error")));
            }
        }
    }
    
    // 2. Generate structured JSON-RPC messages from arbitrary data
    let mut u = Unstructured::new(data);
    
    // Generate and test request
    if let Ok(fuzz_req) = FuzzJsonRpcRequest::arbitrary(&mut u) {
        let json_req = json!({
            "jsonrpc": "2.0",
            "id": match fuzz_req.id {
                Some(FuzzId::Number(n)) => json!(n),
                Some(FuzzId::String(s)) => json!(s),
                Some(FuzzId::Null) | None => Value::Null,
            },
            "method": fuzz_req.method,
            "params": fuzz_req.params.map(|p| match p {
                FuzzParams::Object(obj) => {
                    let map = obj.into_iter().map(|(k, v)| (k, v.to_json())).collect();
                    Value::Object(map)
                },
                FuzzParams::Array(arr) => {
                    Value::Array(arr.into_iter().map(|v| v.to_json()).collect())
                }
            }),
        });
        
        // Validate the generated JSON
        let _ = serde_json::to_string(&json_req);
    }
    
    // 3. Test batch request handling
    if data.len() > 0 {
        let batch_size = (data[0] % 10) as usize;
        let mut batch = Vec::new();
        
        for i in 0..batch_size {
            batch.push(json!({
                "jsonrpc": "2.0",
                "id": i,
                "method": format!("method_{}", i),
                "params": json!({"index": i})
            }));
        }
        
        if !batch.is_empty() {
            let batch_json = Value::Array(batch);
            let _ = serde_json::to_string(&batch_json);
        }
    }
    
    // 4. Test error handling edge cases
    let error_codes = vec![
        -32700, // Parse error
        -32600, // Invalid Request
        -32601, // Method not found
        -32602, // Invalid params
        -32603, // Internal error
        -32000, // Server error
    ];
    
    for code in error_codes {
        let error = json!({
            "code": code,
            "message": String::from_utf8_lossy(data).into_owned(),
            "data": json!({"raw": data.len()})
        });
        let _ = serde_json::to_string(&error);
    }
});
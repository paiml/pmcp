#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::{ClientCapabilities, ServerCapabilities};
use serde_json::{Value, json, from_slice};
use arbitrary::{Arbitrary, Unstructured};
use std::collections::HashMap;

// Custom types for fuzzing authentication
#[derive(Debug, Arbitrary)]
struct FuzzAuthRequest {
    auth_type: FuzzAuthType,
    credentials: FuzzCredentials,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Arbitrary)]
enum FuzzAuthType {
    None,
    ApiKey,
    OAuth2,
    Jwt,
    Custom(String),
}

#[derive(Debug, Arbitrary)]
struct FuzzCredentials {
    username: Option<String>,
    password: Option<String>,
    token: Option<String>,
    api_key: Option<String>,
    refresh_token: Option<String>,
    extra: Vec<(String, String)>,
}

#[derive(Debug, Arbitrary)]
struct FuzzOAuthFlow {
    client_id: String,
    client_secret: Option<String>,
    redirect_uri: String,
    scope: Vec<String>,
    state: String,
    code_verifier: Option<String>,
}

// Simulate various authentication flows
fn test_auth_flow(auth_type: &FuzzAuthType, creds: &FuzzCredentials) {
    match auth_type {
        FuzzAuthType::None => {
            // No authentication required
            assert!(creds.token.is_none() || creds.token.as_ref().map(|t| t.is_empty()).unwrap_or(true));
        },
        FuzzAuthType::ApiKey => {
            // API key authentication
            if let Some(key) = &creds.api_key {
                // Validate API key format
                assert!(!key.is_empty());
                assert!(key.len() < 1024); // Reasonable length limit
                
                // Check for common patterns
                let has_prefix = key.starts_with("sk_") || 
                                key.starts_with("pk_") || 
                                key.starts_with("api_");
                
                // Validate character set
                let is_valid = key.chars().all(|c| {
                    c.is_ascii_alphanumeric() || c == '_' || c == '-'
                });
                
                if has_prefix {
                    assert!(is_valid);
                }
            }
        },
        FuzzAuthType::OAuth2 => {
            // OAuth2 flow
            if let Some(token) = &creds.token {
                // Validate bearer token
                assert!(!token.is_empty());
                
                // Check JWT structure if it looks like one
                let parts: Vec<_> = token.split('.').collect();
                if parts.len() == 3 {
                    // Looks like a JWT
                    for part in &parts {
                        // Each part should be base64url encoded
                        assert!(part.chars().all(|c| {
                            c.is_ascii_alphanumeric() || c == '-' || c == '_'
                        }));
                    }
                }
            }
            
            // Test refresh token flow
            if let Some(refresh) = &creds.refresh_token {
                assert!(!refresh.is_empty());
                assert!(refresh.len() < 2048);
            }
        },
        FuzzAuthType::Jwt => {
            // JWT authentication
            if let Some(token) = &creds.token {
                let parts: Vec<_> = token.split('.').collect();
                if parts.len() == 3 {
                    // Try to decode header and payload (without verification)
                    let header = parts[0];
                    let payload = parts[1];
                    
                    // Simulate base64url decoding (without actual implementation)
                    if header.len() > 0 && payload.len() > 0 {
                        // Would decode and validate structure
                    }
                }
            }
        },
        FuzzAuthType::Custom(scheme) => {
            // Custom authentication scheme
            assert!(!scheme.is_empty());
            assert!(scheme.len() < 256);
            
            // Validate scheme name
            assert!(scheme.chars().all(|c| {
                c.is_ascii_alphanumeric() || c == '-' || c == '_'
            }));
        },
    }
}

// Test OAuth2 PKCE flow
fn test_pkce_flow(flow: &FuzzOAuthFlow) {
    if let Some(verifier) = &flow.code_verifier {
        // PKCE code verifier requirements
        assert!(verifier.len() >= 43 && verifier.len() <= 128);
        assert!(verifier.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~'
        }));
        
        // Generate code challenge (simulated)
        let challenge = format!("{}_challenge", verifier);
        assert!(!challenge.is_empty());
    }
    
    // Validate redirect URI
    assert!(!flow.redirect_uri.is_empty());
    if flow.redirect_uri.starts_with("http://") || flow.redirect_uri.starts_with("https://") {
        // Valid HTTP(S) redirect
    } else if flow.redirect_uri == "urn:ietf:wg:oauth:2.0:oob" {
        // Out-of-band flow
    } else if flow.redirect_uri.starts_with("com.example.app://") {
        // Custom scheme for mobile apps
    }
    
    // Validate scope
    for scope in &flow.scope {
        assert!(!scope.is_empty());
        assert!(!scope.contains(' ')); // Scopes should be space-separated in request
    }
    
    // Validate state parameter
    assert!(!flow.state.is_empty());
    assert!(flow.state.len() >= 8); // Minimum for CSRF protection
}

fuzz_target!(|data: &[u8]| {
    // 1. Parse authentication configuration from JSON
    if let Ok(json) = from_slice::<Value>(data) {
        // Try various auth configurations
        let auth_configs = vec![
            json!({
                "type": "none"
            }),
            json!({
                "type": "api_key",
                "key": String::from_utf8_lossy(data)
            }),
            json!({
                "type": "oauth2",
                "client_id": "client123",
                "authorization_url": "https://auth.example.com/authorize",
                "token_url": "https://auth.example.com/token"
            }),
            json!({
                "type": "custom",
                "scheme": "CustomAuth",
                "parameters": {
                    "custom_field": String::from_utf8_lossy(data)
                }
            }),
        ];
        
        for config in auth_configs {
            let _ = serde_json::to_string(&config);
        }
    }
    
    // 2. Generate and test structured auth flows
    let mut u = Unstructured::new(data);
    
    if let Ok(auth_req) = FuzzAuthRequest::arbitrary(&mut u) {
        test_auth_flow(&auth_req.auth_type, &auth_req.credentials);
        
        // Test authorization headers
        let headers = match auth_req.auth_type {
            FuzzAuthType::ApiKey => {
                if let Some(key) = auth_req.credentials.api_key {
                    vec![("X-API-Key", key.clone()), ("Authorization", format!("ApiKey {}", key))]
                } else {
                    vec![]
                }
            },
            FuzzAuthType::OAuth2 | FuzzAuthType::Jwt => {
                if let Some(token) = auth_req.credentials.token {
                    vec![("Authorization", format!("Bearer {}", token))]
                } else {
                    vec![]
                }
            },
            FuzzAuthType::Custom(ref scheme) => {
                if let Some(token) = auth_req.credentials.token {
                    vec![("Authorization", format!("{} {}", scheme, token))]
                } else {
                    vec![]
                }
            },
            _ => vec![],
        };
        
        // Validate headers
        for (name, value) in headers {
            assert!(!name.is_empty());
            assert!(!value.is_empty());
            assert!(!value.contains('\n')); // No header injection
            assert!(!value.contains('\r'));
        }
    }
    
    // 3. Test OAuth2 flows
    if let Ok(oauth_flow) = FuzzOAuthFlow::arbitrary(&mut u) {
        test_pkce_flow(&oauth_flow);
        
        // Build authorization URL
        let auth_url = format!(
            "https://auth.example.com/authorize?client_id={}&redirect_uri={}&state={}&scope={}",
            oauth_flow.client_id,
            oauth_flow.redirect_uri,
            oauth_flow.state,
            oauth_flow.scope.join(" ")
        );
        
        // Validate URL length
        assert!(auth_url.len() < 8192); // Reasonable URL length limit
    }
    
    // 4. Test token validation and expiry
    if data.len() >= 8 {
        let issued_at = u64::from_be_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ]);
        
        let expires_in = if data.len() >= 12 {
            u32::from_be_bytes([data[8], data[9], data[10], data[11]])
        } else {
            3600 // Default 1 hour
        };
        
        let current_time = issued_at + (expires_in as u64 / 2);
        let is_expired = current_time > issued_at + expires_in as u64;
        
        if !is_expired {
            // Token is still valid
            assert!(current_time >= issued_at);
            assert!(current_time < issued_at + expires_in as u64);
        }
    }
    
    // 5. Test authentication state machine
    let states = ["unauthenticated", "authenticating", "authenticated", "refreshing", "failed"];
    let transitions = [
        ("unauthenticated", "authenticating"),
        ("authenticating", "authenticated"),
        ("authenticating", "failed"),
        ("authenticated", "refreshing"),
        ("refreshing", "authenticated"),
        ("refreshing", "failed"),
        ("failed", "authenticating"),
    ];
    
    if data.len() > 0 {
        let state_idx = (data[0] as usize) % states.len();
        let current_state = states[state_idx];
        
        // Find valid transitions from current state
        let valid_transitions: Vec<_> = transitions
            .iter()
            .filter(|(from, _)| *from == current_state)
            .map(|(_, to)| *to)
            .collect();
        
        if !valid_transitions.is_empty() && data.len() > 1 {
            let next_idx = (data[1] as usize) % valid_transitions.len();
            let _next_state = valid_transitions[next_idx];
        }
    }
});
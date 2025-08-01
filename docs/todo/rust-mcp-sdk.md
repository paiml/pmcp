# Rust MCP SDK Design Document

## Overview

This document outlines the design for a Rust implementation of the Model Context Protocol (MCP) SDK, closely mirroring the TypeScript SDK's functionality while incorporating design principles and quality standards from the PAIML MCP Agent Toolkit (pmat) project.

## Goals

1. **API Parity**: Maintain exact API compatibility with the TypeScript SDK
2. **Quality Standards**: Apply pmat's zero-tolerance quality principles
3. **Performance**: Leverage Rust's performance characteristics for efficient protocol handling
4. **Safety**: Use Rust's type system to prevent protocol violations at compile time
5. **Ergonomics**: Provide idiomatic Rust APIs while maintaining MCP compatibility

## Architecture

### Module Structure

```
rust-mcp-sdk/
├── src/
│   ├── lib.rs                    # Main library entry point
│   ├── types/                    # Protocol types and schemas
│   │   ├── mod.rs
│   │   ├── protocol.rs           # Core protocol types
│   │   ├── jsonrpc.rs           # JSON-RPC types
│   │   ├── capabilities.rs       # Client/Server capabilities
│   │   └── auth.rs              # Authentication types
│   ├── client/                   # Client implementations
│   │   ├── mod.rs
│   │   ├── transport/           # Transport layer
│   │   │   ├── mod.rs
│   │   │   ├── stdio.rs         # Stdio transport
│   │   │   ├── websocket.rs    # WebSocket transport
│   │   │   ├── sse.rs          # SSE transport
│   │   │   └── http.rs         # HTTP streaming transport
│   │   ├── protocol.rs          # Client protocol implementation
│   │   └── auth.rs              # OAuth/auth handling
│   ├── server/                   # Server implementations
│   │   ├── mod.rs
│   │   ├── transport/           # Transport layer
│   │   │   ├── mod.rs
│   │   │   ├── stdio.rs
│   │   │   ├── websocket.rs
│   │   │   ├── sse.rs
│   │   │   └── http.rs
│   │   ├── protocol.rs          # Server protocol implementation
│   │   ├── handlers.rs          # Request handlers
│   │   └── auth/                # Auth provider system
│   │       ├── mod.rs
│   │       ├── provider.rs
│   │       └── middleware.rs
│   ├── shared/                   # Shared utilities
│   │   ├── mod.rs
│   │   ├── protocol.rs          # Protocol base implementation
│   │   ├── transport.rs         # Transport trait
│   │   └── uri_template.rs      # URI template utilities
│   ├── error.rs                 # Error types
│   └── utils/                    # Utility modules
│       ├── mod.rs
│       └── validation.rs         # Schema validation
├── examples/                     # Example implementations
├── tests/                        # Integration tests
└── benches/                      # Performance benchmarks
```

## Core Design Principles

### 1. Type Safety First

Use Rust's type system to enforce protocol correctness:

```rust
// Instead of loose typing, use strong enums
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum ClientRequest {
    #[serde(rename = "initialize")]
    Initialize(InitializeParams),
    #[serde(rename = "tools/list")]
    ToolsList(ToolsListParams),
    // ... other methods
}

// Compile-time protocol version checking
pub struct Protocol<const VERSION: &'static str> {
    // ...
}
```

### 2. Zero-Copy Where Possible

Minimize allocations using borrowed types:

```rust
pub struct Request<'a> {
    pub id: RequestId,
    pub method: &'a str,
    pub params: &'a RawValue,
}
```

### 3. Async-First Design

All I/O operations are async by default:

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&mut self, message: &[u8]) -> Result<()>;
    async fn receive(&mut self) -> Result<Vec<u8>>;
    async fn close(&mut self) -> Result<()>;
}
```

### 4. Builder Pattern for Complex Types

Provide ergonomic APIs for constructing requests:

```rust
let request = client
    .tools()
    .list()
    .with_cursor("next-page")
    .build()
    .await?;
```

## Quality Standards (from pmat)

### 1. Zero Technical Debt Tolerance

- No TODO/FIXME/HACK comments in production code
- All edge cases must be handled explicitly
- No unwrap() calls except in tests

### 2. Comprehensive Testing

```rust
// Unit tests for each module
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    // Property-based testing for protocol invariants
    proptest! {
        #[test]
        fn protocol_roundtrip(req: ClientRequest) {
            let encoded = serde_json::to_vec(&req).unwrap();
            let decoded: ClientRequest = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(req, decoded);
        }
    }
}
```

### 3. Documentation Standards

Every public API must have:
- Description of purpose
- Example usage
- Error conditions
- Performance characteristics

```rust
/// Sends a request to the server and waits for a response.
/// 
/// # Arguments
/// * `request` - The request to send
/// * `options` - Optional request configuration
/// 
/// # Returns
/// The server's response or an error if the request failed
/// 
/// # Example
/// ```rust
/// let response = client.request(
///     ClientRequest::ToolsList(Default::default()),
///     RequestOptions::default().with_timeout(Duration::from_secs(30))
/// ).await?;
/// ```
/// 
/// # Errors
/// - `Error::Timeout` if the request exceeds the timeout
/// - `Error::Protocol` if the server response is invalid
/// - `Error::Transport` for connection issues
pub async fn request(&mut self, request: ClientRequest, options: RequestOptions) -> Result<Response>
```

## API Design

### Client API

```rust
use mcp_sdk::{Client, StdioTransport, ClientCapabilities};

#[tokio::main]
async fn main() -> Result<()> {
    // Create client with stdio transport
    let transport = StdioTransport::new();
    let client = Client::new(transport);
    
    // Initialize connection
    let server_info = client.initialize(ClientCapabilities {
        tools: Some(Default::default()),
        prompts: Some(Default::default()),
        ..Default::default()
    }).await?;
    
    // List available tools
    let tools = client.tools().list().await?;
    
    // Call a tool
    let result = client.tools()
        .call("get-weather")
        .arg("location", "San Francisco")
        .await?;
    
    Ok(())
}
```

### Server API

```rust
use mcp_sdk::{Server, ServerCapabilities, ToolHandler, async_trait};

struct WeatherTool;

#[async_trait]
impl ToolHandler for WeatherTool {
    async fn handle(&self, args: Value) -> Result<Value> {
        // Implementation
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut server = Server::builder()
        .name("weather-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(Default::default()),
            ..Default::default()
        })
        .tool("get-weather", WeatherTool)
        .build();
    
    server.run_stdio().await?;
    Ok(())
}
```

## Implementation Phases

### Phase 1: Core Types and Protocol (Week 1-2)
- [ ] Define all protocol types with serde derives
- [ ] Implement JSON-RPC message handling
- [ ] Create error types and Result aliases
- [ ] Set up basic project structure with CI/CD

### Phase 2: Transport Layer (Week 2-3)
- [ ] Implement Transport trait
- [ ] Stdio transport (first priority)
- [ ] HTTP/SSE transport
- [ ] WebSocket transport
- [ ] Transport tests with mocked I/O

### Phase 3: Client Implementation (Week 3-4)
- [ ] Protocol state machine
- [ ] Request/response correlation
- [ ] Progress notification handling
- [ ] Cancellation support
- [ ] Client integration tests

### Phase 4: Server Implementation (Week 4-5)
- [ ] Request routing
- [ ] Handler traits
- [ ] Capability advertisement
- [ ] Error handling and recovery
- [ ] Server integration tests

### Phase 5: Advanced Features (Week 5-6)
- [ ] OAuth authentication support
- [ ] Resource subscriptions
- [ ] Sampling support
- [ ] Connection lifecycle management

### Phase 6: Quality and Polish (Week 6-7)
- [ ] Performance optimization
- [ ] Comprehensive documentation
- [ ] Example servers/clients
- [ ] Benchmark suite
- [ ] Fuzzing tests

## Testing Strategy

### Unit Tests
- Every public function must have tests
- Use property-based testing for protocol invariants
- Mock external dependencies

### Integration Tests
- Full client-server communication tests
- Cross-transport compatibility tests
- Error injection and recovery tests

### Performance Tests
- Benchmark message serialization/deserialization
- Measure transport latency
- Memory usage profiling

### Compatibility Tests
- Test against TypeScript SDK servers/clients
- Protocol version negotiation tests
- Edge case handling

## Performance Targets

Based on pmat's performance characteristics:
- Message parsing: < 1μs for typical requests
- Round-trip latency: < 100μs for stdio transport
- Memory usage: < 10MB for idle client/server
- Zero allocations in hot paths

## Security Considerations

1. **Input Validation**: All inputs must be validated before processing
2. **Resource Limits**: Implement limits on message sizes, concurrent requests
3. **Authentication**: Secure token handling for OAuth flows
4. **Transport Security**: TLS support for network transports

## Compatibility Matrix

| Feature | TypeScript SDK | Rust SDK | Notes |
|---------|---------------|----------|-------|
| Stdio Transport | ✓ | ✓ | Primary transport |
| HTTP/SSE | ✓ | ✓ | Streaming support |
| WebSocket | ✓ | ✓ | Full duplex |
| OAuth | ✓ | ✓ | Provider interface |
| Resources | ✓ | ✓ | Subscription support |
| Prompts | ✓ | ✓ | Template support |
| Tools | ✓ | ✓ | Full compatibility |
| Sampling | ✓ | ✓ | Model integration |

## Open Questions

1. Should we provide a `#[derive(MCPTool)]` macro for easier tool implementation?
2. How should we handle protocol version negotiation mismatches?
3. Should we support custom transports through a plugin system?
4. What level of backwards compatibility should we maintain?

## Success Metrics

1. **Compatibility**: 100% of TypeScript SDK test suite passes
2. **Performance**: 10x faster than TypeScript for message processing
3. **Quality**: Zero defects, 100% test coverage, no clippy warnings
4. **Adoption**: Used by at least 5 production MCP servers within 3 months

## References

- [MCP Specification](https://github.com/modelcontextprotocol/specification)
- [TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [PAIML MCP Agent Toolkit](https://github.com/paiml/paiml-mcp-agent-toolkit)

## Implementation Status (Updated: 2025-07-25)

### Phase 1: Core Types (Completed ✅)
- [x] JSON-RPC types with full serde support
- [x] Protocol message types (all request/response/notification types)
- [x] Error types with proper error codes
- [x] Capability types with builder patterns
- [x] Authentication types (OAuth, Bearer)

### Phase 2: Transport Layer (Partially Complete)
- [x] Transport trait abstraction
- [x] stdio transport with zero-copy framing
- [ ] HTTP/SSE transport
- [ ] WebSocket transport

### Phase 3: Protocol Implementation (In Progress)
- [x] Protocol state management
- [x] Request/response correlation
- [x] Progress notifications
- [x] Cancellation support
- [ ] Client state machine
- [ ] Server request routing

### Phase 4: Quality Assurance (Completed ✅)
- [x] Unit tests (34 tests)
- [x] Property tests (10 property test suites)
- [x] Doctests (14 doctests)
- [x] Quality gate (pmat standards)
- [x] Zero unwraps in production code
- [x] Zero TODOs/FIXMEs
- [x] Clippy pedantic mode
- [x] All tests passing

### Current Test Results
```
- Unit tests: 34 passed
- Property tests: 10 test suites, 100 cases each
- Doctests: 14 passed
- Quality gate: PASSED ✅
```

## Remaining TypeScript SDK Features to Port (Updated: 2025-08-01)

### High Priority Features

#### 1. OAuth 2.0 Server Implementation
- [ ] Full OAuth 2.0 server with authorization endpoints
- [ ] Authorization handler (`/oauth2/authorize`)
- [ ] Token handler (`/oauth2/token`)
- [ ] Client registration (`/oauth2/register`)
- [ ] Token revocation (`/oauth2/revoke`)
- [ ] OAuth metadata endpoints
- [ ] ProxyOAuthServerProvider for upstream delegation
- [ ] Bearer token middleware
- [ ] Client authentication middleware

#### 2. Completable Arguments System
- [ ] `completable()` function for argument auto-completion
- [ ] Integration with prompts system
- [ ] Integration with resources system
- [ ] Context-aware completion suggestions

#### 3. ResourceWatcher & File System Monitoring
- [ ] `ResourceWatcher` class implementation
- [ ] File system monitoring with configurable debouncing
- [ ] Automatic resource update notifications
- [ ] Watch pattern configuration
- [ ] Ignore rules support
- [ ] Integration with resource subscription system

#### 4. User Input Elicitation
- [ ] `elicitInput()` method for interactive workflows
- [ ] Support for different input types
- [ ] Validation and error handling
- [ ] Integration with tool execution

#### 5. Complete Streamable HTTP Transport
- [ ] Implement new streamable HTTP format
- [ ] Advanced SSE connection management
- [ ] Automatic reconnection and error recovery
- [ ] Session management for stateful servers
- [ ] Backwards compatibility with legacy SSE

### Medium Priority Features

#### 6. Parallel Batch Processing
- [ ] Implement parallel processing with order preservation
- [ ] Advanced batching strategies
- [ ] Performance optimization for batch operations

#### 7. WebSocket Enhancements
- [ ] Proper ping/pong frame handling
- [ ] Advanced connection lifecycle management
- [ ] Reconnection logic
- [ ] Connection health monitoring

#### 8. Dynamic Server Management
- [ ] Runtime addition/removal of tools
- [ ] Runtime addition/removal of prompts
- [ ] Runtime addition/removal of resources
- [ ] Automatic `listChanged` notifications
- [ ] State synchronization

#### 9. Notification Debouncing System
- [ ] Advanced debouncing for rapid state changes
- [ ] Configurable debounce intervals
- [ ] Batch notification delivery
- [ ] Network efficiency optimizations

### Low Priority Features

#### 10. Advanced Session Management
- [ ] Sophisticated HTTP session handling
- [ ] Session persistence across connections
- [ ] Session timeout management
- [ ] Multi-tenant session support

#### 11. Backwards Compatibility Features
- [ ] SSE fallback mechanisms
- [ ] Legacy protocol version support
- [ ] Graceful degradation strategies

### Implementation Tracking

Each feature will be implemented with:
- Comprehensive unit tests
- Integration tests
- Documentation updates
- Example code
- Performance benchmarks where applicable

## TypeScript SDK Feature Parity Status (Updated)

### Completed in v0.3.1
- ✅ **OAuth 2.0 server functionality**
  - Full authorization code flow with PKCE support
  - In-memory provider implementation
  - Bearer token and client credentials middleware
  - Scope-based authorization
  - Example: `examples/16_oauth_server.rs`
  
- ✅ **Completable arguments system**
  - Static and file-based completion providers
  - Integration with prompts and resources
  - Full TypeScript SDK compatibility
  - Example: `examples/17_completable_prompts.rs`
  
- ✅ **ResourceWatcher with file system monitoring**
  - Cross-platform file watching using notify crate
  - Pattern-based filtering with glob support
  - Configurable debouncing
  - Integration with resource update notifications
  - Example: `examples/18_resource_watcher.rs`
  
- ✅ **User input elicitation (elicitInput)**
  - Full protocol support for all input types
  - Timeout and cancellation handling
  - Elicitation manager with async response correlation
  - Example: `examples/19_elicit_input.rs`

## Implementation Tracking

### Update 1 (Completed - v0.4.0)
- [x] OAuth 2.0 server implementation
- [x] Completable arguments for prompts/tools
- [x] Resource watcher with file system monitoring
- [x] User input elicitation support

### Update 2 (Completed - v0.5.0)
- [x] Parallel batch processing with order preservation
- [x] WebSocket ping/pong handling fix
- [x] Dynamic server management
- [x] Notification debouncing system
- [ ] Streamable HTTP transport (skipped - requires async_stream dependency)

### Quality Status
- ✅ All clippy warnings resolved
- ✅ All library tests passing (156 tests)
- ✅ Code formatted with rustfmt
- ✅ Zero unwraps in production code
- ⚠️ Some example code needs updates for new features

### Features Completed in v0.5.0

#### Parallel Batch Processing
- Full parallel request processing with order preservation
- Configurable concurrency limits and timeouts
- Batch processor with metrics tracking
- Rate-limited batch processing support
- Located in: `src/utils/parallel_batch.rs`

#### WebSocket Ping/Pong Handling
- Fixed WebSocket ping/pong frame handling
- Proper async channel for pong responses
- Maintains connection health
- Located in: `src/server/transport/websocket.rs`

#### Dynamic Server Management
- Runtime addition/removal of tools and prompts
- Dynamic resource and sampling handler management
- Capability update notifications
- Configuration hot-reloading support
- Located in: `src/server/dynamic.rs`

#### Notification Debouncing System
- Advanced debouncing for rapid state changes
- Per-notification type configurable intervals
- Notification merging support
- Batch notification delivery
- Located in: `src/server/notification_debouncer.rs`

### Update 3 (Completed - v0.6.0)
- [x] Session management for HTTP/SSE transports
- [x] Advanced reconnection logic with exponential backoff
- [x] Enhanced error handling and recovery
- [x] Request context propagation
- [x] Advanced logging with correlation IDs

### Features Completed in v0.6.0

#### Session Management
- Complete session lifecycle management
- Cookie and header-based session ID extraction
- Session authentication tracking
- Automatic expired session cleanup
- Located in: `src/shared/session.rs`

#### Advanced Reconnection Logic
- Exponential backoff with jitter
- Circuit breaker pattern implementation
- Connection state management
- Comprehensive callbacks for monitoring
- Located in: `src/shared/reconnect.rs`

#### Enhanced Error Handling
- Multiple recovery strategies (retry, fallback, circuit breaker)
- Policy-based error handling configuration
- Recovery executors with handler registration
- Located in: `src/error/recovery.rs`

#### Request Context Propagation
- Distributed tracing with W3C Trace Context
- Task-local context storage
- HTTP header conversion for propagation
- Located in: `src/shared/context.rs`

#### Advanced Logging
- Structured logging with correlation IDs
- Multiple log formats (JSON, pretty, compact)
- Integration with tracing ecosystem
- Located in: `src/shared/logging.rs`

### Remaining TypeScript SDK Features
- Legacy protocol version support
- Additional transport implementations
- Advanced SSE connection management
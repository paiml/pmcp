# PMCP Examples

This directory contains comprehensive examples demonstrating all major features of the PMCP (Pragmatic Model Context Protocol) Rust SDK.

## Running Examples

All examples can be run using `cargo run --example <example_name>`. For example:

```bash
cargo run --example 01_client_initialize
```

## Examples Overview

### 01. Client Initialization
```bash
cargo run --example 01_client_initialize
```
Demonstrates:
- Creating a client with stdio transport
- Capability negotiation
- Server information retrieval
- Connection lifecycle

### 02. Basic Server
```bash
cargo run --example 02_server_basic
```
Demonstrates:
- Creating a server with tools
- Implementing tool handlers
- Calculator and string manipulation tools
- Server lifecycle management

### 03. Client Tools Usage
```bash
cargo run --example 03_client_tools
```
Demonstrates:
- Listing available tools from a server
- Calling tools with arguments
- Handling tool responses
- Error handling for tool calls

### 04. Server with Resources
```bash
cargo run --example 04_server_resources
```
Demonstrates:
- Creating a server that provides resources
- Implementing resource handlers
- Resource listing and reading
- URI template support

### 05. Client Resource Access
```bash
cargo run --example 05_client_resources
```
Demonstrates:
- Listing available resources
- Reading resource contents
- Handling different content types
- Resource pagination

### 06. Server with Prompts
```bash
cargo run --example 06_server_prompts
```
Demonstrates:
- Creating a server that provides prompts
- Implementing prompt handlers
- Dynamic prompt generation with arguments
- Prompt templates and formatting

### 07. Client Prompts Usage
```bash
cargo run --example 07_client_prompts
```
Demonstrates:
- Listing available prompts from a server
- Getting prompt details with arguments
- Executing prompts with parameters
- Handling prompt responses

### 08. Logging
```bash
cargo run --example 08_logging
```
Demonstrates:
- Server logging with different levels
- Client log message handling
- Structured logging with metadata
- Log filtering and processing

### 09. Authentication
```bash
cargo run --example 09_authentication
```
Demonstrates:
- OAuth 2.0 authentication flow
- Bearer token authentication
- Custom authentication handlers
- Token refresh and expiration

### 10. Progress Notifications
```bash
cargo run --example 10_progress_notifications
```
Demonstrates:
- Sending progress updates from tools
- Handling progress notifications in clients
- Progress tokens and tracking
- Cancellable operations with progress

### 11. Request Cancellation
```bash
cargo run --example 11_request_cancellation
```
Demonstrates:
- Cancelling in-flight requests
- Handling cancellation in tools
- Graceful shutdown on cancellation
- Cancellation tokens and propagation

### 12. Error Handling
```bash
cargo run --example 12_error_handling
```
Demonstrates:
- Different error types and codes
- Error recovery strategies
- Retry logic with backoff
- Custom error handling patterns

## Example Patterns

### Client-Server Communication

Most examples follow a client-server pattern. To test them properly:

1. Start the server in one terminal:
```bash
cargo run --example 02_server_basic
```

2. Run the client in another terminal:
```bash
cargo run --example 03_client_tools
```

### Standalone Examples

Some examples (like error handling and authentication) demonstrate concepts without requiring a separate server process.

### Common Features

All examples include:
- Proper error handling
- Logging setup with `tracing`
- Clear output formatting
- Inline documentation

## Building Your Own MCP Application

Use these examples as templates for your own MCP applications:

1. **Simple Tool Server**: Start with example 02
2. **Resource Provider**: Build on example 04
3. **AI Assistant**: Combine examples 06 (prompts) and 03 (tools)
4. **Robust Client**: Use examples 11 (cancellation) and 12 (error handling)

## Advanced Topics

For production applications, consider:

- **Authentication**: See example 09 for OAuth and token handling
- **Progress Tracking**: Example 10 for long-running operations
- **Error Recovery**: Example 12 for resilient error handling
- **Cancellation**: Example 11 for responsive applications

### 20. OIDC Discovery and OAuth 2.0
```bash
cargo run --example 20_oidc_discovery
```
Demonstrates:
- OpenID Connect discovery from well-known endpoints
- Automatic retry on CORS/network errors
- OAuth 2.0 token exchange (authorization code flow)
- Token refresh flow
- Transport isolation with unique IDs
- Concurrent transport operations

### 21. Macro-based Tool Generation
```bash
cargo run --example 21_macro_tools
```
Demonstrates:
- Using procedural macros for tool generation
- Automatic schema derivation from Rust types
- Simplified tool implementation

### 22. Stateful Streamable HTTP Server
```bash
cargo run --example 22_streamable_http_server_stateful --features streamable-http
```
Demonstrates:
- Running MCP server over HTTP with session management
- Session ID generation and tracking
- Multiple concurrent client support with isolated sessions
- Session validation and re-initialization prevention
- Best for long-running connections and complex workflows

### 23. Stateless Streamable HTTP Server
```bash
cargo run --example 23_streamable_http_server_stateless --features streamable-http
```
Demonstrates:
- Running MCP server over HTTP without session management
- Simplified stateless operation
- No session overhead - each request is independent
- Perfect for serverless deployments (AWS Lambda, Azure Functions)
- Horizontal scaling friendly

### 24. Streamable HTTP Client
```bash
# Connect to stateful server (port 8080)
cargo run --example 24_streamable_http_client --features streamable-http

# Connect to stateless server (port 8081)
cargo run --example 24_streamable_http_client --features streamable-http -- stateless
```
Demonstrates:
- Connecting to MCP servers over HTTP transport
- Working with both stateful and stateless servers
- Session handling for stateful connections
- Tool discovery and invocation over HTTP
- Error handling and retry logic

### Running the Streamable HTTP Demo
```bash
# Run comparison demo (both servers)
./examples/streamable_http_demo.sh

# Run specific server type
./examples/streamable_http_demo.sh stateful
./examples/streamable_http_demo.sh stateless

# Interactive mode (keeps servers running)
./examples/streamable_http_demo.sh interactive
```

The demo script showcases the differences between stateful and stateless operation modes,
helping you choose the right approach for your use case.

## Dependencies

All examples use the same dependencies as the main PMCP library. Some examples may demonstrate optional features like WebSocket or HTTP transports (when implemented).

## Contributing

When adding new examples:
1. Follow the numbered naming convention
2. Include comprehensive inline documentation
3. Demonstrate both success and error cases
4. Update this README with the example description
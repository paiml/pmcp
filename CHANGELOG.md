# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2025-08-01

### Added
- **Parallel batch processing** - Concurrent request processing with order preservation
  - Configurable concurrency limits and timeouts
  - Batch processor with metrics tracking
  - Rate-limited batch processing support
  - Integration with shared batch module for automatic parallel processing
- **WebSocket ping/pong handling** - Fixed proper ping/pong frame handling
  - Async channel for pong responses
  - Connection health monitoring
- **Dynamic server management** - Runtime configuration changes
  - Add/remove tools and prompts at runtime
  - Dynamic resource and sampling handler management
  - Capability update notifications
  - Configuration hot-reloading support
- **Notification debouncing system** - Advanced notification rate limiting
  - Per-notification type configurable intervals
  - Notification merging support
  - Batch notification delivery
  - Maximum wait time to prevent indefinite delays

### Improved
- **Batch processing** - Automatic parallel processing for batch requests
- **Type safety** - Added Debug implementations for all new types
- **Code organization** - New modules for better separation of concerns
  - `src/utils/parallel_batch.rs` for batch processing
  - `src/server/dynamic.rs` for dynamic management
  - `src/server/notification_debouncer.rs` for debouncing
- **Test coverage** - Added comprehensive tests for all new features (156 total tests)

### Fixed
- **WebSocket transport** - Fixed undefined `connection_tx` variable in ping handler
- **Type issues** - Fixed RequestId conversion from i32 to i64 in tests
- **Import issues** - Resolved various missing imports and visibility issues

## [0.4.0] - 2025-08-01

### Added
- **OAuth 2.0 server functionality** - Complete authorization code flow with PKCE support
  - In-memory OAuth provider implementation
  - Bearer token and client credentials middleware
  - Scope-based authorization middleware
  - Full example server (examples/16_oauth_server.rs)
- **Completable arguments system** - Auto-completion support for prompts and resources
  - Static and file-based completion providers
  - Full TypeScript SDK compatibility
  - Example implementation (examples/17_completable_prompts.rs)
- **ResourceWatcher** - File system monitoring for automatic resource updates
  - Cross-platform support using notify crate
  - Pattern-based filtering with glob support
  - Configurable debouncing
  - Example server (examples/18_resource_watcher.rs)
- **User input elicitation** - Interactive user input requests
  - Support for all input types (text, boolean, select, file path, etc.)
  - Timeout and cancellation handling
  - Async request/response correlation
  - Example implementation (examples/19_elicit_input.rs)

### Improved
- **Authentication support** - Extended RequestHandlerExtra to include auth_info
- **Protocol extensions** - Added ElicitInput to ServerRequest and ElicitInputResponse to ClientRequest
- **Error handling** - Comprehensive error types for all new features
- **Type safety** - All new types implement Debug trait
- **Code quality** - All clippy warnings resolved, improved documentation

### Dependencies
- Added `sha2` for OAuth PKCE support
- Added `base64` for token encoding
- Added `notify` for file system watching
- Added `glob-match` for pattern matching
- Added `reqwest` for future proxy OAuth support

## [0.3.1] - 2025-01-26

### Fixed
- **Publication issue** - Republished with the correct implementation containing all v0.3 features
- **Documentation** - Updated version references to 0.3.1

## [0.3.0] - 2025-01-26

### Added
- **Server-side subscription management** - Complete resource subscription and notification system
- **WebSocket server transport** - Full WebSocket server implementation for real-time communication
- **Roots support** - Directory/URI registration and management for both client and server
- **Request cancellation** - Comprehensive cancellation token system with async support
- **Batch request processing** - JSON-RPC 2.0 compliant batch request handling
- **Advanced server capabilities**:
  - Resource subscription and notification
  - WebSocket transport layer
  - Root directory registration
  - Request cancellation with CancellationToken
  - Batch request support

### Improved
- **Enhanced type safety** - Boxed large enum variants to reduce memory usage
- **Better error handling** - Comprehensive error propagation and handling
- **Performance optimizations** - Reduced memory allocations and improved async performance
- **Code quality** - All Clippy lints resolved, improved documentation

### Fixed
- **Subscription notification consistency** - Fixed subscription state management across server methods
- **Memory management** - Resolved significant drop issues and reduced contention
- **Type system improvements** - Better pattern matching and enum handling

## [0.2.0] - 2025-01-15

### Added
- Additional server features and improvements
- Enhanced client capabilities
- Better error handling and validation

## [0.1.0] - 2024-01-26

### Added
- Initial release of pmcp (Pragmatic Model Context Protocol) SDK
- Complete client implementation with async/await support
- Complete server implementation with handler registration
- Full MCP protocol support including:
  - Initialize/initialized handshake
  - Tool listing and calling
  - Resource listing and reading
  - Prompt management
  - Progress notifications
  - Request cancellation
  - Error handling with proper JSON-RPC error codes
- Transport layer abstraction with stdio implementation
- Comprehensive type system for protocol messages
- Property-based testing with 28 property tests
- Test coverage at 84.83% (exceeding 80% target)
- 12 working examples demonstrating all major features
- 5 benchmark suites for performance testing
- Full documentation with 42 passing doctests
- pmat quality standards compliance:
  - No unwrap() calls in library code
  - Comprehensive error handling
  - Async-first design
  - Zero unsafe code

### Infrastructure
- Makefile with quality targets (lint, test, coverage, bench)
- CI/CD ready with coverage reporting
- Benchmark suite for performance tracking
- Property testing for protocol invariants

[Unreleased]: https://github.com/paiml/pmcp/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/paiml/pmcp/releases/tag/v0.1.0
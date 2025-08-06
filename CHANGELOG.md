# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.6] - 2025-08-06

### Added
- **OIDC Discovery Support** - Full OpenID Connect configuration discovery
  - `OidcDiscoveryMetadata` struct for OAuth 2.0/OIDC server metadata
  - `OidcDiscoveryClient` with automatic retry on CORS/network errors
  - Token exchange client with explicit JSON accept headers
  - Comprehensive `client::auth` module with helpers for OAuth flows
  - New example: `20_oidc_discovery` demonstrating OIDC discovery and token exchange
  
- **Transport Response Isolation** - Enhanced safety for concurrent transports
  - `TransportId` type for unique transport identification
  - Protocol-level request-response correlation per transport
  - `complete_request_for_transport` method for transport-specific completion
  - Prevents responses being routed to wrong transport instances
  - Property tests ensuring transport isolation invariants

- **Enhanced Testing**
  - 5 new property tests for transport isolation
  - 10+ unit tests for OIDC discovery and auth
  - Integration tests for concurrent transport operations
  - 135+ doctests with comprehensive examples

### Changed
- Updated to align with TypeScript SDK v1.17.1 features
- Added `reqwest` as a required dependency for HTTP client functionality
- Enhanced error handling with proper retry logic for auth operations

### Fixed
- Token exchange now explicitly sets `Accept: application/json` header
- Improved error messages for authentication failures
- Fixed potential race conditions in multi-transport scenarios

## [0.6.4] - 2025-08-01

### Added
- **Comprehensive doctests for advanced SDK types** - Major documentation enhancement
  - Transport layer types (TransportMessage, MessageMetadata, MessagePriority, SendOptions)
  - Utility types - Message batching and debouncing with complete examples
  - Parallel processing utilities (BatchProcessor, ParallelBatchConfig, BatchMetrics)
  - Context propagation system (RequestContext, ContextPropagator, context macros)
  - Middleware system (Middleware trait, LoggingMiddleware, AuthMiddleware, RetryMiddleware, MiddlewareChain)
  - Added 65+ new doctest examples covering complex async patterns and middleware usage
  - Updated documentation todo tracker with completion status

### Fixed
- Fixed all doctest compilation issues with proper type annotations
- Corrected import paths in transport layer doctests
- Fixed RequestId type issues in parallel processing examples
- Improved code formatting throughout doctest examples

## [0.6.3] - 2025-08-01

### Added
- **Comprehensive doctests for core SDK types** - Improved documentation with realistic examples
  - Core constants (protocol versions, timeouts) with usage examples
  - Authentication types (AuthInfo methods, OAuth flow examples)
  - Capability types (client/server capability helpers)
  - Error types (error creation methods, error codes)
  - All doctests include real-world usage patterns
  - Added 85+ new doctest examples across the SDK

### Fixed
- **CI pipeline improvements** - Enhanced build reliability
  - Fixed cargo-deny configuration to use v2 format
  - Added missing tools (cargo-nextest, cargo-audit) to CI
  - Properly skip feature-gated examples in CI checks
  - Fixed all clippy pedantic warnings

## [0.6.0] - 2025-08-01

### Added
- **Session management for HTTP/SSE transports** - Complete session lifecycle management
  - Session creation, validation, and destruction
  - Configurable session timeouts and limits
  - Session persistence and authentication tracking
  - Cookie and header-based session ID extraction
  - Session middleware for HTTP requests
  - Automatic expired session cleanup
- **Advanced reconnection logic** - Robust connection recovery with exponential backoff
  - Configurable retry strategies with jitter
  - Circuit breaker pattern implementation
  - Connection state management
  - Success threshold tracking for stable connections
  - Comprehensive reconnection callbacks
  - Statistics tracking for monitoring
- **Enhanced error handling and recovery** - Sophisticated error recovery mechanisms
  - Multiple recovery strategies (retry, fallback, circuit breaker)
  - Policy-based error handling configuration
  - Recovery executors with handler registration
  - Automatic retry with fixed or exponential backoff
  - Fallback handler support for graceful degradation
- **Request context propagation** - Distributed tracing and correlation
  - Request context with trace and span IDs
  - Task-local context storage
  - HTTP header conversion for propagation
  - Context inheritance for nested operations
  - Baggage support for custom metadata
  - Integration with W3C Trace Context standard
- **Advanced logging with correlation IDs** - Structured logging with request tracking
  - Correlation layer for automatic context injection
  - Multiple log formats (JSON, pretty, compact)
  - Structured log entries with metadata
  - Error details capture with stack traces
  - Integration with tracing ecosystem
  - Helper macros for correlated logging

### Improved
- **Error handling** - Removed Clone from Error types for better performance
- **Type safety** - Added type aliases for complex callback types
- **Code organization** - New error and shared submodules
  - `src/error/recovery.rs` for recovery mechanisms
  - `src/shared/session.rs` for session management
  - `src/shared/reconnect.rs` for reconnection logic
  - `src/shared/context.rs` for context propagation
  - `src/shared/logging.rs` for structured logging
- **Documentation** - Added comprehensive documentation for all new features

### Fixed
- **Compilation issues** - Fixed various type mismatches and missing imports
- **Test failures** - Fixed test compilation errors in context and session modules
- **Clippy warnings** - Addressed numerous clippy lints for better code quality

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
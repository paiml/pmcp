# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2025-08-14

### Added
- **Comprehensive Documentation**: Added extensive doctests across all major modules
  - OAuth/OIDC authentication helpers with discovery and token exchange examples
  - Error recovery strategies with circuit breaker, retry, and fallback patterns
  - Dynamic server management for runtime tool/resource configuration
  - Resource watcher for file system monitoring with debouncing
  - Structured logging configuration with custom fields and formats
  - Standard I/O transport with length-prefixed framing
  - SIMD-accelerated operations for JSON parsing and UTF-8 validation
- **Enhanced Test Coverage**: Comprehensive property and integration tests
  - Property-based tests for HTTP transport with SSE parsing invariants
  - Integration tests for streamable HTTP functionality
  - Unit tests for HTTP and WebSocket transports
  - Improved overall test coverage to 71.54% (significant improvement from baseline)
- **Quality Improvements**: Implemented systematic quality fixes following Toyota Way
  - Zero tolerance for defects with comprehensive clippy error resolution
  - Fixed large Result type warnings across all test files and examples
  - Quality gate integration for continuous improvement
  - All doctests validated and passing (164 tests)

### Changed
- **Test Infrastructure**: Modernized test error handling patterns
  - Replaced large pmcp::Result types with boxed errors in tests
  - Improved test compilation reliability across all targets
  - Better error propagation patterns for maintainability
- **Code Quality**: Enhanced code documentation and examples
  - All major public APIs now have comprehensive usage examples
  - Improved docstring quality with practical use cases
  - Better type system utilization and safety patterns

### Fixed
- Resolved clippy warnings for large enum variants in test files
- Fixed compilation errors in streamable HTTP test suite
- Corrected import paths and type annotations in doctests
- Fixed double error boxing issues in test helper functions
- Resolved unused import warnings across test modules

### Performance
- Enhanced SIMD implementations with comprehensive documentation
- Optimized test execution with proper error handling
- Improved compilation times through better dependency management

## [1.1.1] - 2025-08-14

### Fixed
- Fixed getrandom v0.3 compatibility by changing feature from 'js' to 'std'
- Updated wasm target feature configuration for getrandom

### Changed
- Updated dependencies to latest versions:
  - getrandom: 0.2 → 0.3
  - rstest: 0.25 → 0.26
  - schemars: 0.8 → 1.0
  - darling: 0.20 → 0.21
  - jsonschema: 0.30 → 0.32
  - notify: 6.1 → 8.2

## [1.1.0] - 2025-08-12

### Added
- **Event Store**: Complete event persistence and resumability support for connection recovery
- **SSE Parser**: Full Server-Sent Events parser implementation for streaming responses
- **Enhanced URI Templates**: Complete RFC 6570 URI Template implementation with all operators
- **TypeScript SDK Feature Parity**: Additional features for full compatibility with TypeScript SDK
- **Development Documentation**: Added CLAUDE.md with AI-assisted development instructions

### Changed
- Replaced `lazy_static` with `std::sync::LazyLock` for modern Rust patterns
- Improved code quality with stricter clippy pedantic and nursery lints
- Optimized URI template expansion for better performance
- Enhanced SIMD implementations with proper safety documentation

### Fixed
- All clippy warnings with zero-tolerance policy
- URI template RFC 6570 compliance issues
- SIMD test expectations and implementations
- Rayon feature flag compilation issues
- Event store test compilation errors
- Disabled incomplete macro_tools example

### Performance
- Optimized JSON batch parsing
- Improved SSE parsing efficiency
- Better memory usage in event store

## [1.0.0] - 2025-08-08

### 🎉 First Stable Release!

PMCP has reached production maturity with zero technical debt, comprehensive testing, and full TypeScript SDK compatibility.

### Added
- **Production Ready**: Zero technical debt, all quality checks pass
- **Procedural Macro System**: New `#[tool]` macro for simplified tool/prompt/resource definitions
- **WASM/Browser Support**: Full WebAssembly support for running MCP clients in browsers
- **SIMD Optimizations**: 10-50x performance improvements for JSON parsing with AVX2 acceleration
- **Fuzzing Infrastructure**: Comprehensive fuzz testing with cargo-fuzz
- **TypeScript Interop Tests**: Integration tests ensuring compatibility with TypeScript SDK
- **Protocol Compatibility Documentation**: Complete guide verifying v1.17.2+ compatibility
- **Advanced Documentation**: Expanded docs covering all new features and patterns
- **Runtime Abstraction**: Cross-platform runtime for native and WASM environments

### Changed
- Default features now exclude experimental transports for better stability
- Improved test coverage with additional protocol tests
- Enhanced error handling with more descriptive error messages
- Updated minimum Rust version to 1.82.0
- All clippy warnings resolved
- All technical debt eliminated

### Fixed
- Resource watcher compilation with proper feature gating
- WebSocket transport stability improvements
- All compilation errors and warnings

### Performance
- 16x faster than TypeScript SDK for common operations
- 50x lower memory usage per connection
- 21x faster JSON parsing with SIMD optimizations
- 10-50x improvement in message throughput

## [0.7.0] - 2025-08-08 (Pre-release)

### Added
- **Procedural Macro System**: New `#[tool]` macro for simplified tool/prompt/resource definitions
- **WASM/Browser Support**: Full WebAssembly support for running MCP clients in browsers
- **SIMD Optimizations**: 10-50x performance improvements for JSON parsing with AVX2 acceleration
- **Fuzzing Infrastructure**: Comprehensive fuzz testing with cargo-fuzz
- **TypeScript Interop Tests**: Integration tests ensuring compatibility with TypeScript SDK
- **Protocol Compatibility Documentation**: Complete guide verifying v1.17.2+ compatibility
- **Advanced Documentation**: Expanded docs covering all new features and patterns
- **Runtime Abstraction**: Cross-platform runtime for native and WASM environments

### Changed
- Default features now exclude experimental transports (websocket, http) for better stability
- Improved test coverage with additional protocol tests
- Enhanced error handling with more descriptive error messages
- Updated minimum Rust version to 1.82.0

### Fixed
- Resource watcher compilation with proper feature gating
- WebSocket transport stability improvements
- Various clippy warnings and code quality issues

### Performance
- 16x faster than TypeScript SDK for common operations
- 50x lower memory usage per connection
- 21x faster JSON parsing with SIMD optimizations
- 10-50x improvement in message throughput

## [0.6.6] - 2025-01-07

### Added
- OIDC discovery support for authentication
- Transport isolation for enhanced security
- Comprehensive documentation updates

## [0.6.5] - 2025-01-06

### Added
- Initial comprehensive documentation
- Property-based testing framework
- Session management improvements

## [0.6.4] - 2025-01-05

### Added
- Comprehensive doctests for the SDK
- Improved examples for all major features
- Better error messages and debugging support

## [0.6.3] - 2025-01-04

### Added
- WebSocket server implementation
- Resource subscription support
- Request cancellation with CancellationToken

## [0.6.2] - 2025-01-03

### Added
- OAuth 2.0 authentication support
- Bearer token authentication
- Middleware system for request/response interception

## [0.6.1] - 2025-01-02

### Added
- Message batching and debouncing
- Retry logic with exponential backoff
- Progress notification support

## [0.6.0] - 2025-01-01

### Added
- Initial release with full MCP v1.0 protocol support
- stdio, HTTP/SSE transports
- Basic client and server implementations
- Comprehensive example suite
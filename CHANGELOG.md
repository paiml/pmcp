# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
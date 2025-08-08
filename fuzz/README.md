# PMCP Fuzzing Infrastructure

This directory contains the fuzzing infrastructure for the PMCP (Protocol Model Context Protocol) SDK, implementing comprehensive fuzz testing for protocol parsing, JSON-RPC message handling, transport layer, and authentication flows.

## Overview

Fuzzing is a critical component of our PMAT (Performance, Maintainability, Availability, Testability) quality gates. It helps discover edge cases, security vulnerabilities, and unexpected behavior in our protocol implementation.

## Requirements

- Rust nightly toolchain: `rustup install nightly`
- cargo-fuzz: `cargo install cargo-fuzz`
- (Optional) llvm-tools for coverage: `rustup component add llvm-tools-preview`

## Fuzz Targets

### 1. **protocol_parsing**
Tests the robustness of protocol message parsing including:
- JSON-RPC message parsing
- Request/Response/Notification deserialization
- Protocol-specific types (InitializeRequest, CallToolRequest, etc.)
- Message framing and chunking
- Malformed JSON handling

### 2. **jsonrpc_handling**
Focuses on JSON-RPC protocol internals:
- Request/Response correlation
- Batch request processing
- Error code handling
- ID management (numeric, string, null)
- Parameter validation

### 3. **transport_layer**
Tests transport-level operations:
- Message framing and fragmentation
- WebSocket frame handling
- Compression/decompression
- Connection state management
- Buffer overflow protection
- Flow control

### 4. **auth_flows**
Validates authentication mechanisms:
- OAuth2 flow simulation
- PKCE (Proof Key for Code Exchange) validation
- JWT token parsing
- API key validation
- Token expiry and refresh
- Authentication state machine

## Usage

### Quick Start

```bash
# Run all fuzz targets for 1 minute each
./run-fuzz.sh all 60

# Run a specific target continuously
./run-fuzz.sh continuous protocol_parsing

# Run CI fuzzing (5 minutes per target)
./run-fuzz.sh ci
```

### Manual Fuzzing

```bash
# Run with nightly toolchain
rustup run nightly cargo fuzz run protocol_parsing

# Run for a specific duration
rustup run nightly cargo fuzz run jsonrpc_handling -- -max_total_time=300

# Run with custom options
rustup run nightly cargo fuzz run transport_layer -- \
    -max_total_time=3600 \
    -max_len=8192 \
    -print_final_stats=1
```

### Corpus Management

```bash
# Minimize corpus (remove redundant inputs)
./run-fuzz.sh minimize protocol_parsing

# Add seed inputs
echo '{"jsonrpc":"2.0","id":1,"method":"test"}' > \
    fuzz/corpus/protocol_parsing/seed1.json
```

### Coverage Analysis

```bash
# Generate coverage report
./run-fuzz.sh coverage jsonrpc_handling

# View HTML report
open fuzz/coverage/jsonrpc_handling/html/index.html
```

## CI Integration

The fuzzing infrastructure is integrated into CI through `.github/workflows/fuzz.yml`:

- **Daily runs**: 5 minutes per target
- **PR checks**: Quick fuzzing on relevant changes
- **24-hour runs**: Manual trigger for comprehensive testing
- **Corpus caching**: Preserves discovered inputs across runs

## Security Considerations

Fuzzing helps identify:
- **Memory safety issues**: Buffer overflows, use-after-free
- **Protocol violations**: Invalid state transitions
- **DoS vectors**: Resource exhaustion, infinite loops
- **Injection attacks**: Header/parameter injection
- **Parser differentials**: Inconsistent parsing behavior

## Crash Reproduction

When a crash is found:

1. The input is saved to `fuzz/artifacts/<target>/crash-<hash>`
2. Reproduce with: `cargo fuzz run <target> fuzz/artifacts/<target>/crash-<hash>`
3. Debug with: `RUST_BACKTRACE=1 cargo fuzz run <target> <crash-file>`
4. Minimize with: `cargo fuzz tmin <target> <crash-file>`

## Performance Metrics

Target coverage goals:
- **Line coverage**: > 70%
- **Branch coverage**: > 60%
- **Execution speed**: > 1000 exec/s
- **Corpus growth**: Stabilization within 24 hours

## Best Practices

1. **Add seed inputs**: Provide valid protocol messages as seeds
2. **Regular runs**: Schedule daily fuzzing in CI
3. **Corpus sharing**: Commit minimized corpus to repository
4. **Crash fixes**: Fix all discovered crashes immediately
5. **Coverage monitoring**: Track coverage trends over time

## Troubleshooting

### Out of Memory
```bash
# Limit memory usage
cargo fuzz run <target> -- -rss_limit_mb=2048
```

### Slow Execution
```bash
# Limit input size
cargo fuzz run <target> -- -max_len=1024
```

### Address Sanitizer Issues
```bash
# Disable leak detection
cargo fuzz run <target> -- -detect_leaks=0
```

## Advanced Configuration

### Custom Mutators
Implement custom mutators in `fuzz_targets/` for domain-specific fuzzing.

### Structured Fuzzing
Use `arbitrary` derive for structured input generation.

### Dictionary-Based Fuzzing
Add protocol-specific tokens to `fuzz/dictionary/<target>.dict`.

## Contributing

When adding new fuzz targets:
1. Create the target in `fuzz_targets/`
2. Add to `Cargo.toml` `[[bin]]` section
3. Update `run-fuzz.sh` script
4. Add to CI workflow
5. Document in this README

## References

- [cargo-fuzz documentation](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [LibFuzzer documentation](https://llvm.org/docs/LibFuzzer.html)
- [Arbitrary crate](https://docs.rs/arbitrary)
- [PMCP Protocol Specification](../docs/)
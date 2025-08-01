# PMCP Documentation Update Todo List

This is a granular list of all SDK functions that need doctests and examples. Each item can be processed individually.

## Core Constants (src/lib.rs)

- [x] Add doctest example for `LATEST_PROTOCOL_VERSION`
- [x] Add doctest example for `DEFAULT_PROTOCOL_VERSION`
- [x] Add doctest example for `SUPPORTED_PROTOCOL_VERSIONS`
- [x] Add doctest example for `DEFAULT_REQUEST_TIMEOUT_MS`

## Authentication Types (src/types/auth.rs)

- [x] Add doctest example for `AuthInfo::none()`
- [x] Add doctest example for `AuthInfo::bearer()`
- [x] Add doctest example for `AuthInfo::oauth2()`
- [x] Add doctest example for `AuthInfo::is_required()`
- [x] Add doctest example for `AuthInfo::authorization_header()`
- [x] Add comprehensive OAuth flow example for `OAuthInfo`

## Capability Types (src/types/capabilities.rs)

### ClientCapabilities
- [x] Add doctest example for `ClientCapabilities::minimal()`
- [x] Add doctest example for `ClientCapabilities::full()`
- [x] Add doctest example for `ClientCapabilities::supports_tools()`
- [x] Add doctest example for `ClientCapabilities::supports_prompts()`
- [x] Add doctest example for `ClientCapabilities::supports_resources()`
- [x] Add doctest example for `ClientCapabilities::supports_sampling()`

### ServerCapabilities
- [x] Add doctest example for `ServerCapabilities::minimal()`
- [x] Add doctest example for `ServerCapabilities::tools_only()`
- [x] Add doctest example for `ServerCapabilities::prompts_only()`
- [x] Add doctest example for `ServerCapabilities::resources_only()`
- [x] Add doctest example for `ServerCapabilities::provides_tools()`
- [x] Add doctest example for `ServerCapabilities::provides_prompts()`
- [x] Add doctest example for `ServerCapabilities::provides_resources()`

## Error Types (src/error/mod.rs)

### Error Creation Methods
- [x] Add doctest example for `Error::protocol()`
- [x] Add doctest example for `Error::protocol_msg()`
- [x] Add doctest example for `Error::protocol_with_data()`
- [x] Add doctest example for `Error::validation()`
- [x] Add doctest example for `Error::internal()`
- [x] Add doctest example for `Error::not_found()`
- [x] Add doctest example for `Error::parse()`
- [x] Add doctest example for `Error::invalid_request()`
- [x] Add doctest example for `Error::method_not_found()`
- [x] Add doctest example for `Error::invalid_params()`
- [x] Add doctest example for `Error::authentication()`
- [x] Add doctest example for `Error::capability()`
- [x] Add doctest example for `Error::resource_not_found()`
- [x] Add doctest example for `Error::cancelled()`
- [x] Add doctest example for `Error::from_jsonrpc_error()`
- [x] Add doctest example for `Error::is_error_code()`
- [x] Add doctest example for `Error::error_code()`

### ErrorCode Methods
- [x] Add doctest example for `ErrorCode::other()`
- [x] Add doctest example for `ErrorCode::as_i32()`
- [x] Add doctest example for `ErrorCode::from_i32()`

## Transport Layer (src/shared/transport.rs)

- [x] Add specific usage examples for `Transport` trait methods
- [x] Add examples for `TransportMessage` enum variants
- [x] Add usage example for `MessageMetadata`
- [x] Add usage example for `MessagePriority`
- [x] Add usage example for `SendOptions`

## Utility Types (src/utils/batching.rs)

### MessageBatcher
- [x] Add doctest example for `MessageBatcher::new()`
- [x] Add doctest example for `MessageBatcher::add()`
- [x] Add doctest example for `MessageBatcher::start_timer()`
- [x] Add doctest example for `MessageBatcher::receive_batch()`

### MessageDebouncer
- [x] Add doctest example for `MessageDebouncer::new()`
- [x] Add doctest example for `MessageDebouncer::add()`
- [x] Add doctest example for `MessageDebouncer::receive()`
- [x] Add doctest example for `MessageDebouncer::flush()`

## Parallel Processing (src/utils/parallel_batch.rs)

- [x] Add usage example for `ParallelBatchConfig`
- [x] Add comprehensive example for `BatchProcessor`
- [x] Add usage example for `BatchMetrics`
- [x] Add doctest example for `rate_limited_processor()`
- [x] Add doctest example for `process_batch_parallel()`
- [x] Add doctest example for `process_batch_parallel_stateful()`

## Context Types (src/shared/context.rs)

- [x] Add comprehensive example for `RequestContext`
- [x] Add usage example for `ClientInfo`
- [x] Add usage example for `ContextPropagator`
- [x] Add example for `with_context!` macro
- [x] Add example for `context_or_new!` macro

## Middleware Types (src/shared/middleware.rs)

- [x] Add implementation example for `Middleware` trait
- [x] Add usage example for `LoggingMiddleware`
- [x] Add usage example for `AuthMiddleware`
- [x] Add usage example for `RetryMiddleware`
- [x] Add comprehensive example for `MiddlewareChain`

## Protocol Types

### JSON-RPC Types (src/types/jsonrpc.rs)
- [x] Add examples for JSONRPC request/response construction
- [x] Add examples for error handling with JSONRPC types

### Completable Types (src/types/completable.rs)
- [x] Add comprehensive example for `completable()` builder
- [x] Add examples for argument completion

### Elicitation Types (src/types/elicitation.rs)
- [x] Add example for elicitation request handling
- [x] Add example for user input validation

## Client Builder (src/client/mod.rs)

- [x] Add doctest example for `ClientBuilder::new()`
- [x] Add doctest example for `ClientBuilder::enforce_strict_capabilities()`
- [x] Add doctest example for `ClientBuilder::debounced_notifications()`
- [x] Add comprehensive builder pattern example

## Server Methods (src/server/mod.rs)

- [ ] Add doctest example for `Server::has_tool()`
- [ ] Add doctest example for `Server::has_prompt()`
- [ ] Add examples for server management methods

## Reconnection Logic (src/shared/reconnect.rs)

- [ ] Add comprehensive example for `ReconnectManager`
- [ ] Add usage example for `ReconnectConfig`
- [ ] Add example for reconnection callbacks

## Session Management (src/shared/session.rs)

- [ ] Add comprehensive example for `SessionManager`
- [ ] Add usage example for `SessionConfig`
- [ ] Add example for session lifecycle management

## Progress

Total items: 100+
Completed: 65+
Remaining: 35+

### Recently Completed (Current Session)
- [x] All Core Constants (4 items)
- [x] All Authentication Types (6 items)
- [x] All Capability Types (13 items)
- [x] Most Error Types (11 items)
- [x] All Transport Layer types (5 items)
- [x] All Utility Types - Batching (8 items)
- [x] All Parallel Processing (6 items)
- [x] All Context Types (5 items)
- [x] All Middleware Types (5 items)

### Next Priority Items
- [ ] Complete remaining Error Types (6 items)
- [ ] Protocol Types - JSON-RPC (2 items)
- [ ] Protocol Types - Completable/Elicitation (3 items)
- [ ] Client Builder (4 items)
- [ ] Server Methods (3 items)
- [ ] Reconnection Logic (3 items)
- [ ] Session Management (3 items)

## Notes

- Each item should include a working doctest that demonstrates the function's usage
- Examples should show realistic use cases, not just trivial calls
- Include error handling where appropriate
- Cross-reference related functionality in examples
- Ensure all examples compile and run successfully
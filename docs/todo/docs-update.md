# PMCP Documentation Update Todo List

This is a granular list of all SDK functions that need doctests and examples. Each item can be processed individually.

## Core Constants (src/lib.rs)

- [ ] Add doctest example for `LATEST_PROTOCOL_VERSION`
- [ ] Add doctest example for `DEFAULT_PROTOCOL_VERSION`
- [ ] Add doctest example for `SUPPORTED_PROTOCOL_VERSIONS`
- [ ] Add doctest example for `DEFAULT_REQUEST_TIMEOUT_MS`

## Authentication Types (src/types/auth.rs)

- [ ] Add doctest example for `AuthInfo::none()`
- [ ] Add doctest example for `AuthInfo::bearer()`
- [ ] Add doctest example for `AuthInfo::oauth2()`
- [ ] Add doctest example for `AuthInfo::is_required()`
- [ ] Add doctest example for `AuthInfo::authorization_header()`
- [ ] Add comprehensive OAuth flow example for `OAuthInfo`

## Capability Types (src/types/capabilities.rs)

### ClientCapabilities
- [ ] Add doctest example for `ClientCapabilities::minimal()`
- [ ] Add doctest example for `ClientCapabilities::full()`
- [ ] Add doctest example for `ClientCapabilities::supports_tools()`
- [ ] Add doctest example for `ClientCapabilities::supports_prompts()`
- [ ] Add doctest example for `ClientCapabilities::supports_resources()`
- [ ] Add doctest example for `ClientCapabilities::supports_sampling()`

### ServerCapabilities
- [ ] Add doctest example for `ServerCapabilities::minimal()`
- [ ] Add doctest example for `ServerCapabilities::tools_only()`
- [ ] Add doctest example for `ServerCapabilities::prompts_only()`
- [ ] Add doctest example for `ServerCapabilities::resources_only()`
- [ ] Add doctest example for `ServerCapabilities::provides_tools()`
- [ ] Add doctest example for `ServerCapabilities::provides_prompts()`
- [ ] Add doctest example for `ServerCapabilities::provides_resources()`

## Error Types (src/error/mod.rs)

### Error Creation Methods
- [ ] Add doctest example for `Error::protocol()`
- [ ] Add doctest example for `Error::protocol_msg()`
- [ ] Add doctest example for `Error::protocol_with_data()`
- [ ] Add doctest example for `Error::validation()`
- [ ] Add doctest example for `Error::internal()`
- [ ] Add doctest example for `Error::not_found()`
- [ ] Add doctest example for `Error::parse()`
- [ ] Add doctest example for `Error::invalid_request()`
- [ ] Add doctest example for `Error::method_not_found()`
- [ ] Add doctest example for `Error::invalid_params()`
- [ ] Add doctest example for `Error::authentication()`
- [ ] Add doctest example for `Error::capability()`
- [ ] Add doctest example for `Error::resource_not_found()`
- [ ] Add doctest example for `Error::cancelled()`
- [ ] Add doctest example for `Error::from_jsonrpc_error()`
- [ ] Add doctest example for `Error::is_error_code()`
- [ ] Add doctest example for `Error::error_code()`

### ErrorCode Methods
- [ ] Add doctest example for `ErrorCode::other()`
- [ ] Add doctest example for `ErrorCode::as_i32()`
- [ ] Add doctest example for `ErrorCode::from_i32()`

## Transport Layer (src/shared/transport.rs)

- [ ] Add specific usage examples for `Transport` trait methods
- [ ] Add examples for `TransportMessage` enum variants
- [ ] Add usage example for `MessageMetadata`
- [ ] Add usage example for `MessagePriority`
- [ ] Add usage example for `SendOptions`

## Utility Types (src/utils/batching.rs)

### MessageBatcher
- [ ] Add doctest example for `MessageBatcher::new()`
- [ ] Add doctest example for `MessageBatcher::add()`
- [ ] Add doctest example for `MessageBatcher::start_timer()`
- [ ] Add doctest example for `MessageBatcher::receive_batch()`

### MessageDebouncer
- [ ] Add doctest example for `MessageDebouncer::new()`
- [ ] Add doctest example for `MessageDebouncer::add()`
- [ ] Add doctest example for `MessageDebouncer::receive()`
- [ ] Add doctest example for `MessageDebouncer::flush()`

## Parallel Processing (src/utils/parallel_batch.rs)

- [ ] Add usage example for `ParallelBatchConfig`
- [ ] Add comprehensive example for `BatchProcessor`
- [ ] Add usage example for `BatchMetrics`
- [ ] Add doctest example for `rate_limited_processor()`
- [ ] Add doctest example for `process_batch_parallel()`
- [ ] Add doctest example for `process_batch_parallel_stateful()`

## Context Types (src/shared/context.rs)

- [ ] Add comprehensive example for `RequestContext`
- [ ] Add usage example for `ClientInfo`
- [ ] Add usage example for `ContextPropagator`
- [ ] Add example for `with_context!` macro
- [ ] Add example for `context_or_new!` macro

## Middleware Types (src/shared/middleware.rs)

- [ ] Add implementation example for `Middleware` trait
- [ ] Add usage example for `LoggingMiddleware`
- [ ] Add usage example for `AuthMiddleware`
- [ ] Add usage example for `RetryMiddleware`
- [ ] Add comprehensive example for `MiddlewareChain`

## Protocol Types

### JSON-RPC Types (src/types/jsonrpc.rs)
- [ ] Add examples for JSONRPC request/response construction
- [ ] Add examples for error handling with JSONRPC types

### Completable Types (src/types/completable.rs)
- [ ] Add comprehensive example for `completable()` builder
- [ ] Add examples for argument completion

### Elicitation Types (src/types/elicitation.rs)
- [ ] Add example for elicitation request handling
- [ ] Add example for user input validation

## Client Builder (src/client/mod.rs)

- [ ] Add doctest example for `ClientBuilder::new()`
- [ ] Add doctest example for `ClientBuilder::enforce_strict_capabilities()`
- [ ] Add doctest example for `ClientBuilder::debounced_notifications()`
- [ ] Add comprehensive builder pattern example

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
Completed: 0
Remaining: 100+

## Notes

- Each item should include a working doctest that demonstrates the function's usage
- Examples should show realistic use cases, not just trivial calls
- Include error handling where appropriate
- Cross-reference related functionality in examples
- Ensure all examples compile and run successfully
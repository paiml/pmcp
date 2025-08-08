//! Utility modules for the MCP SDK.

pub mod batching;
pub mod parallel_batch;
pub mod validation;

#[cfg(feature = "simd")]
pub mod json_simd;

pub use batching::{BatchingConfig, DebouncingConfig, MessageBatcher, MessageDebouncer};
pub use parallel_batch::{
    process_batch_parallel, process_batch_parallel_stateful, BatchProcessor, ParallelBatchConfig,
};

#[cfg(feature = "simd")]
pub use json_simd::{parse_json_batch, parse_json_fast, pretty_print_fast, serialize_json_fast};

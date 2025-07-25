//! Shared components used by both client and server.

pub mod protocol;
pub mod protocol_helpers;
pub mod stdio;
pub mod transport;
pub mod uri_template;

// Re-export commonly used types
pub use protocol::{ProgressCallback, Protocol, ProtocolOptions, RequestOptions};
pub use protocol_helpers::{
    create_notification, create_request, parse_notification, parse_request,
};
pub use stdio::StdioTransport;
pub use transport::{Transport, TransportMessage};

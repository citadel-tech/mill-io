//! Core types for Mill-RPC: wire protocol, codecs, errors, and dispatcher traits.

pub mod codec;
pub mod context;
pub mod error;
pub mod protocol;

pub use codec::{Codec, CodecType};
pub use context::RpcContext;
pub use error::{RpcError, RpcStatus};
pub use protocol::{Flags, Frame, FrameHeader, MessageType};

/// Trait for dispatching RPC calls to handler methods.
///
/// This is auto-implemented by the `#[mill_rpc::service]` macro for any type
/// that implements the generated `{Service}Server` trait.
pub trait ServiceDispatch: Send + Sync + 'static {
    fn dispatch(
        &self,
        ctx: &RpcContext,
        method_id: u16,
        args: &[u8],
        codec: &Codec,
    ) -> Result<Vec<u8>, RpcError>;
}

/// Trait for client-side RPC transport.
///
/// Abstracts the mechanism of sending a request and receiving a response.
/// The main `mill-rpc` crate provides an implementation built on `mill-net`.
pub trait RpcTransport: Send + Sync + 'static {
    /// Send a request and wait for a response.
    /// Returns the raw response payload bytes.
    fn call(&self, service_id: u16, method_id: u16, payload: Vec<u8>) -> Result<Vec<u8>, RpcError>;
}

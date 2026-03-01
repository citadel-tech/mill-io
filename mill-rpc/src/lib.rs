//! Mill-RPC: An Axum-inspired RPC framework built on Mill-IO.
//!
//! # Quick Start
//!
//! ```ignore
//! // Define a service — generates a `calculator` module
//! mill_rpc::service! {
//!     service Calculator {
//!         fn add(a: i32, b: i32) -> i32;
//!     }
//! }
//!
//! // Server side
//! struct MyCalc;
//! impl calculator::Service for MyCalc {
//!     fn add(&self, _ctx: &RpcContext, a: i32, b: i32) -> i32 { a + b }
//! }
//!
//! // Register
//! RpcServer::builder()
//!     .service(calculator::server(MyCalc))
//!     .build(&event_loop)?;
//!
//! // Client side
//! let client = calculator::Client::new(transport, codec, 0);
//! client.add(2, 3)?;
//! ```

pub mod client;
pub mod server;

pub mod prelude;

// Re-exports from core
pub use mill_rpc_core::{
    Codec, CodecType, Frame, Flags, FrameHeader, MessageType,
    RpcContext, RpcError, RpcStatus,
    RpcTransport, ServiceDispatch,
};

// Re-export the service! macro
pub use mill_rpc_macros::service;

pub use client::RpcClient;
pub use server::RpcServer;

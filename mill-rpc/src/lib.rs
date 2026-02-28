//! Mill-RPC: An Axum-inspired RPC framework built on Mill-IO.
//!
//! # Quick Start
//!
//! ```ignore
//! use mill_rpc::prelude::*;
//!
//! #[mill_rpc::service]
//! trait Calculator {
//!     fn add(a: i32, b: i32) -> i32;
//! }
//!
//! struct MyCalc;
//! impl CalculatorServer for MyCalc {
//!     fn add(&self, _ctx: &RpcContext, a: i32, b: i32) -> i32 { a + b }
//! }
//! ```

pub mod client;
pub mod server;

pub mod prelude;

// Re-exports
pub use mill_rpc_core::{
    Codec, CodecType, Flags, Frame, FrameHeader, MessageType, RpcContext, RpcError, RpcStatus,
    RpcTransport, ServiceDispatch,
};
pub use mill_rpc_macros::service;

pub use client::RpcClient;
pub use server::RpcServer;

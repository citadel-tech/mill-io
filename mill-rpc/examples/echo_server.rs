//! Echo RPC server.
//!
//! Run with: cargo run --example echo_server
//! Then connect with: cargo run --example echo_client

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

mill_rpc::service! {
    #[server]
    service Echo {
        fn echo(message: String) -> String;
        fn echo_uppercase(message: String) -> String;
        fn echo_repeat(message: String, times: u32) -> String;
        fn request_count() -> u64;
    }
}

struct EchoImpl {
    counter: AtomicU64,
}

impl EchoImpl {
    fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }
}

impl echo::Service for EchoImpl {
    fn echo(&self, _ctx: &RpcContext, message: String) -> String {
        self.counter.fetch_add(1, Ordering::Relaxed);
        println!("  echo: {:?}", message);
        message
    }

    fn echo_uppercase(&self, _ctx: &RpcContext, message: String) -> String {
        self.counter.fetch_add(1, Ordering::Relaxed);
        let upper = message.to_uppercase();
        println!("  echo_uppercase: {:?} -> {:?}", message, upper);
        upper
    }

    fn echo_repeat(&self, _ctx: &RpcContext, message: String, times: u32) -> String {
        self.counter.fetch_add(1, Ordering::Relaxed);
        let result = message.repeat(times as usize);
        println!("  echo_repeat({:?}, {}) -> {:?}", message, times, result);
        result
    }

    fn request_count(&self, _ctx: &RpcContext) -> u64 {
        let count = self.counter.load(Ordering::Relaxed);
        println!("  request_count -> {}", count);
        count
    }
}

fn main() {
    env_logger::init();
    let event_loop = Arc::new(EventLoop::new(4, 1024, 100).unwrap());

    let addr = "127.0.0.1:9002".parse().unwrap();
    let _server = RpcServer::builder()
        .bind(addr)
        .service(echo::server(EchoImpl::new()))
        .build(&event_loop)
        .expect("Failed to start echo server");

    println!("Echo server listening on {}", addr);
    event_loop.run().unwrap();
}

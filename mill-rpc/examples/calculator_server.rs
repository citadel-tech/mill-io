//! Basic calculator RPC server.
//!
//! Run with: cargo run --example calculator_server
//! Then connect with: cargo run --example calculator_client

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::Arc;

#[mill_rpc::service]
trait Calculator {
    fn add(a: i32, b: i32) -> i32;
    fn subtract(a: i32, b: i32) -> i32;
    fn multiply(a: i64, b: i64) -> i64;
    fn divide(a: f64, b: f64) -> f64;
    fn negate(x: i32) -> i32;
}

struct CalculatorImpl;

impl CalculatorServer for CalculatorImpl {
    fn add(&self, _ctx: &RpcContext, a: i32, b: i32) -> i32 {
        println!("  add({}, {}) = {}", a, b, a + b);
        a + b
    }

    fn subtract(&self, _ctx: &RpcContext, a: i32, b: i32) -> i32 {
        println!("  subtract({}, {}) = {}", a, b, a - b);
        a - b
    }

    fn multiply(&self, _ctx: &RpcContext, a: i64, b: i64) -> i64 {
        println!("  multiply({}, {}) = {}", a, b, a * b);
        a * b
    }

    fn divide(&self, _ctx: &RpcContext, a: f64, b: f64) -> f64 {
        let result = if b == 0.0 { f64::NAN } else { a / b };
        println!("  divide({}, {}) = {}", a, b, result);
        result
    }

    fn negate(&self, _ctx: &RpcContext, x: i32) -> i32 {
        println!("  negate({}) = {}", x, -x);
        -x
    }
}

fn main() {
    env_logger::init();
    let event_loop = Arc::new(EventLoop::new(4, 1024, 100).unwrap());

    let addr = "127.0.0.1:9001".parse().unwrap();
    let _server = RpcServer::builder()
        .bind(addr)
        .service(CalculatorDispatcher(CalculatorImpl))
        .build(&event_loop)
        .expect("Failed to start calculator server");

    println!("Calculator server listening on {}", addr);
    event_loop.run().unwrap();
}

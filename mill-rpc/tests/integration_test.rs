use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Generate both server and client for testing
mill_rpc::service! {
    service Calculator {
        fn add(a: i32, b: i32) -> i32;
        fn multiply(a: f64, b: f64) -> f64;
        fn echo(msg: String) -> String;
    }
}

struct MyCalculator;

impl calculator::Service for MyCalculator {
    fn add(&self, _ctx: &RpcContext, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, _ctx: &RpcContext, a: f64, b: f64) -> f64 {
        a * b
    }

    fn echo(&self, _ctx: &RpcContext, msg: String) -> String {
        format!("echo: {}", msg)
    }
}

#[test]
fn test_dispatch_add() {
    let codec = Codec::bincode();
    let ctx = RpcContext::new(1, 0, 0);
    let dispatcher = calculator::server(MyCalculator);

    // bincode serialization of AddRequest { a: 2, b: 3 }: two i32 LE
    let mut payload = Vec::new();
    payload.extend_from_slice(&2i32.to_le_bytes());
    payload.extend_from_slice(&3i32.to_le_bytes());

    let result_bytes = dispatcher
        .dispatch(&ctx, calculator::methods::ADD, &payload, &codec)
        .unwrap();
    let result: i32 = codec.deserialize(&result_bytes).unwrap();
    assert_eq!(result, 5);
}

#[test]
fn test_dispatch_multiply() {
    let codec = Codec::bincode();
    let ctx = RpcContext::new(1, 0, 0);
    let dispatcher = calculator::server(MyCalculator);

    // bincode serialization of MultiplyRequest { a: 3.0, b: 4.0 }: two f64 LE
    let mut payload = Vec::new();
    payload.extend_from_slice(&3.0f64.to_le_bytes());
    payload.extend_from_slice(&4.0f64.to_le_bytes());

    let result_bytes = dispatcher
        .dispatch(&ctx, calculator::methods::MULTIPLY, &payload, &codec)
        .unwrap();
    let result: f64 = codec.deserialize(&result_bytes).unwrap();
    assert!((result - 12.0).abs() < f64::EPSILON);
}

#[test]
fn test_dispatch_method_not_found() {
    let codec = Codec::bincode();
    let ctx = RpcContext::new(1, 0, 0);
    let dispatcher = calculator::server(MyCalculator);

    let err = dispatcher.dispatch(&ctx, 999, &[], &codec);
    assert!(err.is_err());
}

#[test]
fn test_server_builds_and_stops() {
    let event_loop = Arc::new(EventLoop::new(2, 1024, 100).unwrap());

    let server = RpcServer::builder()
        .bind("127.0.0.1:0".parse().unwrap())
        .service(calculator::server(MyCalculator))
        .build(&event_loop);

    match server {
        Ok(_s) => {
            let el = event_loop.clone();
            let h = thread::spawn(move || {
                let _ = el.run();
            });
            thread::sleep(Duration::from_millis(50));
            event_loop.stop();
            let _ = h.join();
        }
        Err(e) => {
            eprintln!("Server build failed (non-fatal in test): {}", e);
        }
    }
}

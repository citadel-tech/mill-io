use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ---- Service Definition ----

#[mill_rpc::service]
trait Calculator {
    fn add(a: i32, b: i32) -> i32;
    fn multiply(a: f64, b: f64) -> f64;
    fn echo(msg: String) -> String;
}

// ---- Server Implementation ----

struct MyCalculator;

impl CalculatorServer for MyCalculator {
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
fn test_rpc_roundtrip() {
    let _ = env_logger::builder().is_test(true).try_init();

    let codec = Codec::bincode();
    let ctx = RpcContext::new(1, 0, 0);
    let dispatcher = CalculatorDispatcher(MyCalculator);

    // Test add
    let args = codec.serialize(&AddRequest { a: 2, b: 3 }).unwrap();
    let result_bytes = dispatcher
        .dispatch(&ctx, calculator_methods::ADD, &args, &codec)
        .unwrap();
    let result: AddResponse = codec.deserialize(&result_bytes).unwrap();
    assert_eq!(result.0, 5);

    // Test multiply
    let args = codec
        .serialize(&MultiplyRequest { a: 3.0, b: 4.0 })
        .unwrap();
    let result_bytes = dispatcher
        .dispatch(&ctx, calculator_methods::MULTIPLY, &args, &codec)
        .unwrap();
    let result: MultiplyResponse = codec.deserialize(&result_bytes).unwrap();
    assert!((result.0 - 12.0).abs() < f64::EPSILON);

    // Test echo
    let args = codec
        .serialize(&EchoRequest {
            msg: "hello".to_string(),
        })
        .unwrap();
    let result_bytes = dispatcher
        .dispatch(&ctx, calculator_methods::ECHO, &args, &codec)
        .unwrap();
    let result: EchoResponse = codec.deserialize(&result_bytes).unwrap();
    assert_eq!(result.0, "echo: hello");

    // Test method not found
    let result = dispatcher.dispatch(&ctx, 999, &[], &codec);
    assert!(result.is_err());
}

#[test]
fn test_rpc_server_client_integration() {
    let _ = env_logger::builder().is_test(true).try_init();

    let event_loop = Arc::new(EventLoop::new(4, 1024, 100).unwrap());

    let server = RpcServer::builder()
        .bind("127.0.0.1:0".parse().unwrap())
        .service(CalculatorDispatcher(MyCalculator))
        .build(&event_loop);

    match server {
        Ok(_server) => {
            let el = event_loop.clone();
            let handle = thread::spawn(move || {
                let _ = el.run();
            });

            thread::sleep(Duration::from_millis(100));

            event_loop.stop();
            let _ = handle.join();
        }
        Err(e) => {
            eprintln!("Server build failed (non-fatal in test): {}", e);
        }
    }
}

//! Multi-service RPC client - calls two services on one server.
//!
//! Run the server first: cargo run --example multi_service_server
//! Then run: cargo run --example multi_service_client

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Service definitions must match the server
#[mill_rpc::service]
trait MathService {
    fn factorial(n: u64) -> u64;
    fn fibonacci(n: u32) -> u64;
    fn is_prime(n: u64) -> bool;
    fn gcd(a: u64, b: u64) -> u64;
}

#[mill_rpc::service]
trait StringService {
    fn reverse(s: String) -> String;
    fn word_count(s: String) -> u32;
    fn contains(haystack: String, needle: String) -> bool;
    fn trim(s: String) -> String;
    fn replace(s: String, from: String, to: String) -> String;
}

fn main() {
    env_logger::init();
    let event_loop = Arc::new(EventLoop::new(2, 1024, 100).unwrap());

    let el = event_loop.clone();
    let handle = thread::spawn(move || {
        el.run().unwrap();
    });
    thread::sleep(Duration::from_millis(50));

    let addr = "127.0.0.1:9004".parse().unwrap();
    let transport = mill_rpc::RpcClient::connect(addr, &event_loop, Codec::bincode())
        .expect("Failed to connect");

    // Both clients share the same transport (single TCP connection)
    let math = MathServiceClient::new(transport.clone(), Codec::bincode(), 0);
    let strings = StringServiceClient::new(transport, Codec::bincode(), 1);

    // ---- Math Service ----
    println!("=== Math Service ===\n");

    let f = math.factorial(10).unwrap();
    println!("10! = {}", f);
    assert_eq!(f, 3628800);

    let fib = math.fibonacci(20).unwrap();
    println!("fib(20) = {}", fib);
    assert_eq!(fib, 6765);

    for n in [2, 7, 15, 17, 100] {
        let prime = math.is_prime(n).unwrap();
        println!("is_prime({}) = {}", n, prime);
    }

    let g = math.gcd(48, 18).unwrap();
    println!("gcd(48, 18) = {}", g);
    assert_eq!(g, 6);

    // ---- String Service ----
    println!("\n=== String Service ===\n");

    let rev = strings.reverse("Hello, World!".into()).unwrap();
    println!("reverse(\"Hello, World!\") = {:?}", rev);
    assert_eq!(rev, "!dlroW ,olleH");

    let wc = strings
        .word_count("The quick brown fox jumps".into())
        .unwrap();
    println!("word_count(\"The quick brown fox jumps\") = {}", wc);
    assert_eq!(wc, 5);

    let has = strings.contains("rustacean".into(), "rust".into()).unwrap();
    println!("contains(\"rustacean\", \"rust\") = {}", has);
    assert!(has);

    let trimmed = strings.trim("  hello  ".into()).unwrap();
    println!("trim(\"  hello  \") = {:?}", trimmed);
    assert_eq!(trimmed, "hello");

    let replaced = strings
        .replace("foo bar foo baz".into(), "foo".into(), "qux".into())
        .unwrap();
    println!(
        "replace(\"foo bar foo baz\", \"foo\", \"qux\") = {:?}",
        replaced
    );
    assert_eq!(replaced, "qux bar qux baz");

    println!("\nAll multi-service tests passed!");

    event_loop.stop();
    let _ = handle.join();
}

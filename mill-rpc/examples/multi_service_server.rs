//! Multi-service RPC server - hosts multiple services on a single port.
//!
//! Demonstrates service composition: a math service (service_id=0) and
//! a string utility service (service_id=1) on the same server.
//!
//! Run with: cargo run --example multi_service_server
//! Then connect with: cargo run --example multi_service_client

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::Arc;

// ---------- Service 1: MathService ----------

#[mill_rpc::service]
trait MathService {
    fn factorial(n: u64) -> u64;
    fn fibonacci(n: u32) -> u64;
    fn is_prime(n: u64) -> bool;
    fn gcd(a: u64, b: u64) -> u64;
}

struct MathImpl;

impl MathServiceServer for MathImpl {
    fn factorial(&self, _ctx: &RpcContext, n: u64) -> u64 {
        let result = (1..=n).product();
        println!("  [math] factorial({}) = {}", n, result);
        result
    }

    fn fibonacci(&self, _ctx: &RpcContext, n: u32) -> u64 {
        let result = match n {
            0 => 0,
            1 => 1,
            _ => {
                let (mut a, mut b) = (0u64, 1u64);
                for _ in 2..=n {
                    let tmp = a + b;
                    a = b;
                    b = tmp;
                }
                b
            }
        };
        println!("  [math] fibonacci({}) = {}", n, result);
        result
    }

    fn is_prime(&self, _ctx: &RpcContext, n: u64) -> bool {
        let result = if n < 2 {
            false
        } else if n < 4 {
            true
        } else if n % 2 == 0 {
            false
        } else {
            let mut i = 3;
            while i * i <= n {
                if n % i == 0 {
                    return false;
                }
                i += 2;
            }
            true
        };
        println!("  [math] is_prime({}) = {}", n, result);
        result
    }

    fn gcd(&self, _ctx: &RpcContext, mut a: u64, mut b: u64) -> u64 {
        let (orig_a, orig_b) = (a, b);
        while b != 0 {
            let tmp = b;
            b = a % b;
            a = tmp;
        }
        println!("  [math] gcd({}, {}) = {}", orig_a, orig_b, a);
        a
    }
}

// ---------- Service 2: StringService ----------

#[mill_rpc::service]
trait StringService {
    fn reverse(s: String) -> String;
    fn word_count(s: String) -> u32;
    fn contains(haystack: String, needle: String) -> bool;
    fn trim(s: String) -> String;
    fn replace(s: String, from: String, to: String) -> String;
}

struct StringImpl;

impl StringServiceServer for StringImpl {
    fn reverse(&self, _ctx: &RpcContext, s: String) -> String {
        let result: String = s.chars().rev().collect();
        println!("  [str] reverse({:?}) = {:?}", s, result);
        result
    }

    fn word_count(&self, _ctx: &RpcContext, s: String) -> u32 {
        let count = s.split_whitespace().count() as u32;
        println!("  [str] word_count({:?}) = {}", s, count);
        count
    }

    fn contains(&self, _ctx: &RpcContext, haystack: String, needle: String) -> bool {
        let result = haystack.contains(&needle);
        println!(
            "  [str] contains({:?}, {:?}) = {}",
            haystack, needle, result
        );
        result
    }

    fn trim(&self, _ctx: &RpcContext, s: String) -> String {
        let result = s.trim().to_string();
        println!("  [str] trim({:?}) = {:?}", s, result);
        result
    }

    fn replace(&self, _ctx: &RpcContext, s: String, from: String, to: String) -> String {
        let result = s.replace(&from, &to);
        println!(
            "  [str] replace({:?}, {:?}, {:?}) = {:?}",
            s, from, to, result
        );
        result
    }
}

fn main() {
    env_logger::init();
    let event_loop = Arc::new(EventLoop::new(4, 1024, 100).unwrap());

    let addr = "127.0.0.1:9004".parse().unwrap();
    let _server = RpcServer::builder()
        .bind(addr)
        .service(MathServiceDispatcher(MathImpl)) // service_id = 0
        .service(StringServiceDispatcher(StringImpl)) // service_id = 1
        .build(&event_loop)
        .expect("Failed to start multi-service server");

    println!("Multi-service server listening on {}", addr);
    println!("  Service 0: MathService   (factorial, fibonacci, is_prime, gcd)");
    println!("  Service 1: StringService (reverse, word_count, contains, trim, replace)");
    event_loop.run().unwrap();
}

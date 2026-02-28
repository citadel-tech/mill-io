//! Concurrent clients stress test - multiple threads sending RPC calls simultaneously.
//!
//! This example starts an embedded server and spawns N client threads,
//! each making M requests. Demonstrates thread-safety and concurrent dispatch.
//!
//! Run with: cargo run --example concurrent_clients

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[mill_rpc::service]
trait Counter {
    fn increment() -> u64;
    fn get() -> u64;
    fn add(n: u64) -> u64;
}

struct AtomicCounter {
    value: AtomicU64,
}

impl AtomicCounter {
    fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }
}

impl CounterServer for AtomicCounter {
    fn increment(&self, _ctx: &RpcContext) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn get(&self, _ctx: &RpcContext) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    fn add(&self, _ctx: &RpcContext, n: u64) -> u64 {
        self.value.fetch_add(n, Ordering::SeqCst) + n
    }
}

fn main() {
    env_logger::init();

    let num_clients = 4;
    let requests_per_client = 100;

    // --- Start embedded server ---
    let server_el = Arc::new(EventLoop::new(4, 1024, 100).unwrap());
    let addr = "127.0.0.1:9005".parse().unwrap();

    let _server = RpcServer::builder()
        .bind(addr)
        .service(CounterDispatcher(AtomicCounter::new()))
        .build(&server_el)
        .expect("Failed to start server");

    let sel = server_el.clone();
    let server_thread = thread::spawn(move || {
        sel.run().unwrap();
    });

    thread::sleep(Duration::from_millis(100));
    println!(
        "Server started. Spawning {} clients, {} requests each...\n",
        num_clients, requests_per_client
    );

    // --- Spawn client threads ---
    let start = Instant::now();
    let mut handles = Vec::new();

    for client_id in 0..num_clients {
        let handle = thread::spawn(move || {
            let client_el = Arc::new(EventLoop::new(1, 256, 50).unwrap());

            let cel = client_el.clone();
            let el_thread = thread::spawn(move || {
                cel.run().unwrap();
            });

            thread::sleep(Duration::from_millis(20));

            let transport =
                mill_rpc::RpcClient::connect(addr, &client_el, Codec::bincode()).unwrap();
            let counter = CounterClient::new(transport, Codec::bincode(), 0);

            let mut local_results = Vec::new();
            for _ in 0..requests_per_client {
                let val = counter.increment().unwrap();
                local_results.push(val);
            }

            println!(
                "  Client {} finished: first={}, last={}",
                client_id,
                local_results.first().unwrap(),
                local_results.last().unwrap()
            );

            client_el.stop();
            let _ = el_thread.join();

            local_results
        });
        handles.push(handle);
    }

    // --- Collect results ---
    let mut all_values: Vec<u64> = Vec::new();
    for handle in handles {
        let values = handle.join().unwrap();
        all_values.extend(values);
    }

    let elapsed = start.elapsed();

    // Every increment should have returned a unique value
    all_values.sort();
    all_values.dedup();

    let total_expected = (num_clients * requests_per_client) as usize;
    println!("\n--- Results ---");
    println!("Total requests: {}", total_expected);
    println!("Unique values:  {}", all_values.len());
    println!("Time elapsed:   {:?}", elapsed);
    println!(
        "Throughput:     {:.0} req/s",
        total_expected as f64 / elapsed.as_secs_f64()
    );

    assert_eq!(
        all_values.len(),
        total_expected,
        "Expected all increments to produce unique values (no lost updates)"
    );

    println!("\nConcurrency test passed! No lost updates.");

    server_el.stop();
    let _ = server_thread.join();
}

//! Concurrent clients stress test.
//!
//! Run with: cargo run --example concurrent_clients

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// Both sides in one binary
mill_rpc::service! {
    service Counter {
        fn increment() -> u64;
        fn get() -> u64;
    }
}

struct AtomicCounter {
    value: AtomicU64,
}

impl counter::Service for AtomicCounter {
    fn increment(&self, _ctx: &RpcContext) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn get(&self, _ctx: &RpcContext) -> u64 {
        self.value.load(Ordering::SeqCst)
    }
}

fn main() {
    env_logger::init();

    let num_clients = 4;
    let requests_per_client = 100;

    let server_el = Arc::new(EventLoop::new(4, 1024, 100).unwrap());
    let addr = "127.0.0.1:9005".parse().unwrap();

    let _server = RpcServer::builder()
        .bind(addr)
        .service(counter::server(AtomicCounter {
            value: AtomicU64::new(0),
        }))
        .build(&server_el)
        .expect("Failed to start server");

    let sel = server_el.clone();
    let server_thread = thread::spawn(move || { sel.run().unwrap(); });
    thread::sleep(Duration::from_millis(100));

    println!(
        "Spawning {} clients, {} requests each...\n",
        num_clients, requests_per_client
    );

    let start = Instant::now();
    let mut handles = Vec::new();

    for client_id in 0..num_clients {
        let handle = thread::spawn(move || {
            let client_el = Arc::new(EventLoop::new(1, 256, 50).unwrap());
            let cel = client_el.clone();
            let el_thread = thread::spawn(move || { cel.run().unwrap(); });
            thread::sleep(Duration::from_millis(20));

            let transport = RpcClient::connect(addr, &client_el).unwrap();
            let client = counter::Client::new(transport, Codec::bincode(), 0);

            let mut results = Vec::new();
            for _ in 0..requests_per_client {
                results.push(client.increment().unwrap());
            }

            println!(
                "  Client {} done: first={}, last={}",
                client_id,
                results.first().unwrap(),
                results.last().unwrap()
            );

            client_el.stop();
            let _ = el_thread.join();
            results
        });
        handles.push(handle);
    }

    let mut all: Vec<u64> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();

    let elapsed = start.elapsed();
    all.sort();
    all.dedup();

    let total = (num_clients * requests_per_client) as usize;
    println!("\n--- Results ---");
    println!("Total requests: {}", total);
    println!("Unique values:  {}", all.len());
    println!("Time:           {:?}", elapsed);
    println!("Throughput:     {:.0} req/s", total as f64 / elapsed.as_secs_f64());

    assert_eq!(all.len(), total, "No lost updates");
    println!("\nConcurrency test passed!");

    server_el.stop();
    let _ = server_thread.join();
}

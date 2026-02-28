//! Key-value store RPC client.
//!
//! Run the server first: cargo run --example kv_server
//! Then run: cargo run --example kv_client

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[mill_rpc::service]
trait KeyValue {
    fn get(key: String) -> Option<String>;
    fn set(key: String, value: String) -> Option<String>;
    fn delete(key: String) -> bool;
    fn keys() -> Vec<String>;
    fn len() -> u64;
    fn clear() -> u64;
}

fn main() {
    env_logger::init();
    let event_loop = Arc::new(EventLoop::new(2, 1024, 100).unwrap());

    let el = event_loop.clone();
    let handle = thread::spawn(move || {
        el.run().unwrap();
    });
    thread::sleep(Duration::from_millis(50));

    let addr = "127.0.0.1:9003".parse().unwrap();
    let transport = mill_rpc::RpcClient::connect(addr, &event_loop, Codec::bincode())
        .expect("Failed to connect to KV server");

    let kv = KeyValueClient::new(transport, Codec::bincode(), 0);

    println!("=== Key-Value Store Client ===\n");

    // Initially empty
    let len = kv.len().unwrap();
    println!("Initial store size: {}", len);
    assert_eq!(len, 0);

    // Set some keys
    let old = kv.set("name".into(), "Alice".into()).unwrap();
    println!("SET name=Alice (old: {:?})", old);
    assert!(old.is_none());

    let old = kv.set("city".into(), "Berlin".into()).unwrap();
    println!("SET city=Berlin (old: {:?})", old);

    let old = kv.set("lang".into(), "Rust".into()).unwrap();
    println!("SET lang=Rust (old: {:?})", old);

    // Get values
    let val = kv.get("name".into()).unwrap();
    println!("GET name -> {:?}", val);
    assert_eq!(val, Some("Alice".to_string()));

    let val = kv.get("missing".into()).unwrap();
    println!("GET missing -> {:?}", val);
    assert_eq!(val, None);

    // List keys
    let mut keys = kv.keys().unwrap();
    keys.sort();
    println!("KEYS -> {:?}", keys);
    assert_eq!(keys.len(), 3);

    // Overwrite
    let old = kv.set("name".into(), "Bob".into()).unwrap();
    println!("SET name=Bob (old: {:?})", old);
    assert_eq!(old, Some("Alice".to_string()));

    let val = kv.get("name".into()).unwrap();
    println!("GET name -> {:?}", val);
    assert_eq!(val, Some("Bob".to_string()));

    // Delete
    let existed = kv.delete("city".into()).unwrap();
    println!("DEL city -> existed: {}", existed);
    assert!(existed);

    let existed = kv.delete("city".into()).unwrap();
    println!("DEL city -> existed: {}", existed);
    assert!(!existed);

    // Final size
    let len = kv.len().unwrap();
    println!("Store size: {}", len);
    assert_eq!(len, 2);

    // Clear
    let removed = kv.clear().unwrap();
    println!("CLEAR -> removed {} entries", removed);
    assert_eq!(removed, 2);

    let len = kv.len().unwrap();
    println!("Store size after clear: {}", len);
    assert_eq!(len, 0);

    println!("\nAll KV tests passed!");

    event_loop.stop();
    let _ = handle.join();
}

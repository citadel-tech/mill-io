//! In-memory key-value store RPC server.
//!
//! Run with: cargo run --example kv_server
//! Then connect with: cargo run --example kv_client

use mill_io::EventLoop;
use mill_rpc::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

mill_rpc::service! {
    #[server]
    service KeyValue {
        fn get(key: String) -> Option<String>;
        fn set(key: String, value: String) -> Option<String>;
        fn delete(key: String) -> bool;
        fn keys() -> Vec<String>;
        fn len() -> u64;
        fn clear() -> u64;
    }
}

struct KvStore {
    data: RwLock<HashMap<String, String>>,
}

impl KvStore {
    fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl key_value::Service for KvStore {
    fn get(&self, _ctx: &RpcContext, key: String) -> Option<String> {
        let data = self.data.read().unwrap();
        let result = data.get(&key).cloned();
        println!("  GET {:?} -> {:?}", key, result);
        result
    }

    fn set(&self, _ctx: &RpcContext, key: String, value: String) -> Option<String> {
        let mut data = self.data.write().unwrap();
        let old = data.insert(key.clone(), value.clone());
        println!("  SET {:?} = {:?} (old: {:?})", key, value, old);
        old
    }

    fn delete(&self, _ctx: &RpcContext, key: String) -> bool {
        let mut data = self.data.write().unwrap();
        let existed = data.remove(&key).is_some();
        println!("  DEL {:?} -> existed: {}", key, existed);
        existed
    }

    fn keys(&self, _ctx: &RpcContext) -> Vec<String> {
        let data = self.data.read().unwrap();
        let keys: Vec<String> = data.keys().cloned().collect();
        println!("  KEYS -> {:?}", keys);
        keys
    }

    fn len(&self, _ctx: &RpcContext) -> u64 {
        let data = self.data.read().unwrap();
        let len = data.len() as u64;
        println!("  LEN -> {}", len);
        len
    }

    fn clear(&self, _ctx: &RpcContext) -> u64 {
        let mut data = self.data.write().unwrap();
        let count = data.len() as u64;
        data.clear();
        println!("  CLEAR -> removed {} entries", count);
        count
    }
}

fn main() {
    env_logger::init();
    let event_loop = Arc::new(EventLoop::new(4, 1024, 100).unwrap());

    let addr = "127.0.0.1:9003".parse().unwrap();
    let _server = RpcServer::builder()
        .bind(addr)
        .service(key_value::server(KvStore::new()))
        .build(&event_loop)
        .expect("Failed to start KV server");

    println!("Key-Value server listening on {}", addr);
    event_loop.run().unwrap();
}

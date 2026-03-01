# mill-rpc

An RPC framework built on [`mill-io`](../mill-io) and [`mill-net`](../mill-net). Define services declaratively, get type-safe clients and servers — no async runtime required.

## Features

- **Zero async**: Handlers are plain synchronous functions
- **Macro-driven**: `mill_rpc::service!` generates a module with server trait, client struct, and dispatch logic
- **Selective generation**: Use `#[server]`, `#[client]`, or both (default)
- **Multi-service**: Host multiple services on a single port with automatic routing
- **Pluggable codecs**: Bincode by default, extensible
- **Binary wire protocol**: Efficient framing with one-way calls and ping/pong

## Quick Start

### Define a service

```rust
mill_rpc::service! {
    service Calculator {
        fn add(a: i32, b: i32) -> i32;
        fn multiply(a: i64, b: i64) -> i64;
    }
}
```

This generates a `calculator` module containing:
- `calculator::Service`: trait to implement on the server
- `calculator::server(impl)`: wraps your impl for registration
- `calculator::Client`: struct with typed RPC methods
- `calculator::methods`: method ID constants

### Server

```rust
struct MyCalc;

impl calculator::Service for MyCalc {
    fn add(&self, _ctx: &RpcContext, a: i32, b: i32) -> i32 { a + b }
    fn multiply(&self, _ctx: &RpcContext, a: i64, b: i64) -> i64 { a * b }
}

fn main() {
    let event_loop = Arc::new(EventLoop::new(4, 1024, 100).unwrap());

    let _server = RpcServer::builder()
        .bind("127.0.0.1:9001".parse().unwrap())
        .service(calculator::server(MyCalc))
        .build(&event_loop)
        .unwrap();

    event_loop.run().unwrap();
}
```

### Client

```rust
let transport = RpcClient::connect(addr, &event_loop, Codec::bincode()).unwrap();
let client = calculator::Client::new(transport, Codec::bincode(), 0);

let sum = client.add(10, 25).unwrap();     // 35
let prod = client.multiply(7, 8).unwrap(); // 56
```

## Selective Generation

Generate only what you need:

```rust
// Server crate: no client code generated
mill_rpc::service! {
    #[server]
    service Calculator {
        fn add(a: i32, b: i32) -> i32;
    }
}

// Client crate: no server code generated
mill_rpc::service! {
    #[client]
    service Calculator {
        fn add(a: i32, b: i32) -> i32;
    }
}

// Both (default): for tests, examples, or single-binary apps
mill_rpc::service! {
    service Calculator {
        fn add(a: i32, b: i32) -> i32;
    }
}
```

## Multi-Service Server

```rust
mill_rpc::service! {
    #[server]
    service MathService {
        fn factorial(n: u64) -> u64;
    }
}

mill_rpc::service! {
    #[server]
    service StringService {
        fn reverse(s: String) -> String;
    }
}

let _server = RpcServer::builder()
    .bind(addr)
    .service(math_service::server(MathImpl))       // service_id = 0
    .service(string_service::server(StringImpl))    // service_id = 1
    .build(&event_loop)?;

// Client side: share one connection
let math = math_service::Client::new(transport.clone(), codec, 0);
let strings = string_service::Client::new(transport, codec, 1);
```

## Examples

```bash
# Terminal 1                              # Terminal 2
cargo run --example calculator_server     cargo run --example calculator_client
cargo run --example echo_server           cargo run --example echo_client
cargo run --example kv_server             cargo run --example kv_client
cargo run --example multi_service_server  cargo run --example multi_service_client

# Self-contained stress test
cargo run --example concurrent_clients
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../LICENSE) for details.

# mill-rpc

An RPC framework built on top of [`mill-io`](../mill-io) and [`mill-net`](../mill-net). Define services as Rust traits, get type-safe clients and servers for free - no async runtime required.

## Features

- **Zero async** - Handlers are plain synchronous functions, no `async/await` needed
- **Macro-driven** - `#[mill_rpc::service]` generates server traits, client structs, and dispatch logic from a single trait definition
- **Type-safe** - Compile-time checked request/response types and method signatures
- **Multi-service** - Host multiple services on a single server with automatic routing
- **Pluggable codecs** - Bincode by default, extensible to JSON, MessagePack, CBOR, etc.
- **Binary wire protocol** - Efficient framing with support for one-way calls, ping/pong, and request cancellation

## Installation

```toml
[dependencies]
mill-rpc = { path = "../mill-rpc" }
```

## Quick Start

### 1. Define a service

```rust
use mill_rpc::prelude::*;

#[mill_rpc::service]
trait Calculator {
    fn add(a: i32, b: i32) -> i32;
    fn multiply(a: i64, b: i64) -> i64;
}
```

This single trait generates:
- `CalculatorServer` - trait you implement on the server
- `CalculatorClient` - struct with typed RPC methods
- `CalculatorDispatcher` - wrapper that implements `ServiceDispatch`
- Per-method request/response types with serde derives
- `calculator_methods` module with method ID constants

### 2. Implement the server

```rust
struct MyCalculator;

impl CalculatorServer for MyCalculator {
    fn add(&self, _ctx: &RpcContext, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, _ctx: &RpcContext, a: i64, b: i64) -> i64 {
        a * b
    }
}
```

### 3. Start the server

```rust
use mill_io::EventLoop;
use std::sync::Arc;

fn main() {
    let event_loop = Arc::new(EventLoop::new(4, 1024, 100).unwrap());

    let _server = RpcServer::builder()
        .bind("127.0.0.1:9001".parse().unwrap())
        .service(CalculatorDispatcher(MyCalculator))
        .build(&event_loop)
        .expect("Failed to start server");

    event_loop.run().unwrap();
}
```

### 4. Call from a client

```rust
use mill_io::EventLoop;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let event_loop = Arc::new(EventLoop::new(2, 1024, 100).unwrap());

    // Run event loop in background
    let el = event_loop.clone();
    thread::spawn(move || el.run().unwrap());
    thread::sleep(Duration::from_millis(50));

    let transport = mill_rpc::RpcClient::connect(
        "127.0.0.1:9001".parse().unwrap(),
        &event_loop,
        Codec::bincode(),
    ).unwrap();

    let client = CalculatorClient::new(transport, Codec::bincode(), 0);

    let sum = client.add(10, 25).unwrap();
    println!("10 + 25 = {}", sum); // 35

    let product = client.multiply(7, 8).unwrap();
    println!("7 * 8 = {}", product); // 56

    event_loop.stop();
}
```

## Multi-Service Server

Register multiple services on a single port. Each service gets an auto-assigned service ID.

```rust
#[mill_rpc::service]
trait MathService {
    fn factorial(n: u64) -> u64;
}

#[mill_rpc::service]
trait StringService {
    fn reverse(s: String) -> String;
}

// Server
let _server = RpcServer::builder()
    .bind(addr)
    .service(MathServiceDispatcher(MathImpl))       // service_id = 0
    .service(StringServiceDispatcher(StringImpl))    // service_id = 1
    .build(&event_loop)?;

// Client - both share a single TCP connection
let math = MathServiceClient::new(transport.clone(), Codec::bincode(), 0);
let strings = StringServiceClient::new(transport, Codec::bincode(), 1);

math.factorial(10)?;           // 3628800
strings.reverse("hello")?;    // "olleh"
```

## Wire Protocol

Mill-RPC uses a compact binary frame format:

```text
+--------+--------+-------+--------+-----------+---------+
| Magic  | Version| Flags | MsgType| PayloadLen| Payload |
| 2B     | 1B     | 1B    | 1B     | 4B (LE)   | N bytes |
+--------+--------+-------+--------+-----------+---------+
```

Request payloads carry routing info:

```text
+------------+-----------+-----------+---------+
| RequestID  | ServiceID | MethodID  | Args    |
| 8B (LE)    | 2B (LE)   | 2B (LE)   | N bytes |
+------------+-----------+-----------+---------+
```

**Message types:** Request, Response, Error, Ping, Pong, Cancel

**Flags:** Compressed payload, One-way (fire-and-forget)

## Examples

Run any example pair (server first, then client):

```bash
# Terminal 1
cargo run --example calculator_server

# Terminal 2
cargo run --example calculator_client
```

you will find all examples [here](./examples/).

## Error Handling

Mill-RPC uses structured errors with gRPC-style status codes:

| Code | Status            | Description                 |
| ---- | ----------------- | --------------------------- |
| 0    | OK                | Success                     |
| 2    | INVALID_ARGUMENT  | Bad request parameters      |
| 3    | NOT_FOUND         | Service or method not found |
| 8    | INTERNAL          | Server-side error           |
| 9    | UNAVAILABLE       | Connection failure          |
| 10   | DEADLINE_EXCEEDED | Request timeout             |

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../LICENSE) for details.

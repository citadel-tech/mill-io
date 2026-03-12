//! Benchmark comparison: Legacy hand-rolled RPC vs Mill-RPC.
//!
//! The legacy approach mirrors how coinswap's maker RPC works:
//! - `TcpListener` with `set_nonblocking(true)`
//! - Busy-poll loop with `sleep(HEART_BEAT_INTERVAL)`
//! - Manual `serde_cbor` serialization of request/response enums
//! - Single-threaded, one request at a time
//!
//! Mill-RPC uses:
//! - mill-net's reactor-based TcpServer
//! - Auto-generated dispatch from `mill_rpc::service!`
//! - Thread-pool for concurrent request handling
//! - Binary framing protocol with bincode

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct UtxoEntry {
    txid: String,
    vout: u32,
    value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct BalanceInfo {
    confirmed: u64,
    unconfirmed: u64,
    total: u64,
}

/// Simulated state that both servers share.
struct SharedState {
    utxos: Vec<UtxoEntry>,
    balances: BalanceInfo,
    counter: RwLock<u64>,
}

impl SharedState {
    fn new() -> Self {
        let utxos: Vec<UtxoEntry> = (0..20)
            .map(|i| UtxoEntry {
                txid: format!(
                    "abcdef{:04x}abcdef{:04x}abcdef{:04x}abcdef{:04x}",
                    i, i, i, i
                ),
                vout: i,
                value: (i as u64 + 1) * 100_000,
            })
            .collect();

        Self {
            utxos,
            balances: BalanceInfo {
                confirmed: 5_000_000,
                unconfirmed: 200_000,
                total: 5_200_000,
            },
            counter: RwLock::new(0),
        }
    }
}

/// Legacy RPC (mirrors coinswap's approach)
mod legacy {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub enum RpcMsgReq {
        Ping,
        GetUtxos,
        GetBalances,
        Increment,
        Echo(String),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum RpcMsgResp {
        Pong,
        UtxoResp { utxos: Vec<UtxoEntry> },
        BalancesResp(BalanceInfo),
        IncrementResp(u64),
        EchoResp(String),
        ServerError(String),
    }

    /// Length-prefixed message: 4-byte LE length + cbor payload (mirrors coinswap's read_message/send_message).
    pub fn send_message(stream: &mut TcpStream, msg: &RpcMsgResp) -> std::io::Result<()> {
        let data = serde_cbor::to_vec(msg).unwrap();
        let len = (data.len() as u32).to_le_bytes();
        stream.write_all(&len)?;
        stream.write_all(&data)?;
        stream.flush()?;
        Ok(())
    }

    pub fn read_message(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn send_request(stream: &mut TcpStream, req: &RpcMsgReq) -> std::io::Result<()> {
        let data = serde_cbor::to_vec(req).unwrap();
        let len = (data.len() as u32).to_le_bytes();
        stream.write_all(&len)?;
        stream.write_all(&data)?;
        stream.flush()?;
        Ok(())
    }

    pub fn read_response(stream: &mut TcpStream) -> std::io::Result<RpcMsgResp> {
        let data = read_message(stream)?;
        Ok(serde_cbor::from_slice(&data).unwrap())
    }

    fn handle_request(state: &Arc<SharedState>, socket: &mut TcpStream) -> std::io::Result<()> {
        let msg_bytes = read_message(socket)?;
        let rpc_request: RpcMsgReq = serde_cbor::from_slice(&msg_bytes).unwrap();

        let resp = match rpc_request {
            RpcMsgReq::Ping => RpcMsgResp::Pong,
            RpcMsgReq::GetUtxos => RpcMsgResp::UtxoResp {
                utxos: state.utxos.clone(),
            },
            RpcMsgReq::GetBalances => RpcMsgResp::BalancesResp(state.balances.clone()),
            RpcMsgReq::Increment => {
                let mut counter = state.counter.write().unwrap();
                *counter += 1;
                RpcMsgResp::IncrementResp(*counter)
            }
            RpcMsgReq::Echo(msg) => RpcMsgResp::EchoResp(msg),
        };

        send_message(socket, &resp)?;
        Ok(())
    }

    /// Start the legacy server and return (addr, shutdown_flag).
    pub fn start_server(state: Arc<SharedState>) -> (SocketAddr, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        listener.set_nonblocking(true).unwrap();

        thread::spawn(move || {
            while !shutdown_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(20)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(20)))
                            .unwrap();
                        if let Err(e) = handle_request(&state, &mut stream) {
                            let _ = send_message(
                                &mut stream,
                                &RpcMsgResp::ServerError(format!("{e:?}")),
                            );
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => {}
                }
                // Mirrors HEART_BEAT_INTERVAL in coinswap (typically 3 seconds,
                // but we use 1ms here so the benchmark isn't dominated by sleep)
                thread::sleep(Duration::from_millis(1));
            }
        });

        // Wait for server to be ready
        thread::sleep(Duration::from_millis(50));
        (addr, shutdown)
    }

    /// Make a single request-response roundtrip (one TCP connection per call,
    /// same as coinswap's client pattern).
    pub fn call(addr: SocketAddr, req: &RpcMsgReq) -> RpcMsgResp {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        send_request(&mut stream, req).unwrap();
        read_response(&mut stream).unwrap()
    }
}

/// Mill-RPC alternative
mod mill {
    use super::*;
    use mill_io::EventLoop;
    use mill_rpc::prelude::*;

    mill_rpc::service! {
        service BenchService {
            fn ping() -> ();
            fn get_utxos() -> Vec<UtxoEntry>;
            fn get_balances() -> BalanceInfo;
            fn increment() -> u64;
            fn echo(msg: String) -> String;
        }
    }

    pub struct BenchServiceImpl {
        pub state: Arc<SharedState>,
    }

    impl bench_service::Service for BenchServiceImpl {
        fn ping(&self, _ctx: &RpcContext) {}

        fn get_utxos(&self, _ctx: &RpcContext) -> Vec<UtxoEntry> {
            self.state.utxos.clone()
        }

        fn get_balances(&self, _ctx: &RpcContext) -> BalanceInfo {
            self.state.balances.clone()
        }

        fn increment(&self, _ctx: &RpcContext) -> u64 {
            let mut counter = self.state.counter.write().unwrap();
            *counter += 1;
            *counter
        }

        fn echo(&self, _ctx: &RpcContext, msg: String) -> String {
            msg
        }
    }

    /// Make a call using mill-rpc client.
    pub fn make_client(addr: SocketAddr, event_loop: &Arc<EventLoop>) -> bench_service::Client {
        let transport = RpcClient::connect(addr, event_loop).unwrap();
        bench_service::Client::new(transport, Codec::bincode(), 0)
    }
}

fn bench_ping(c: &mut Criterion) {
    let mut group = c.benchmark_group("ping_roundtrip");
    group.throughput(Throughput::Elements(1));

    let state = Arc::new(SharedState::new());

    // --- Legacy ---
    let (legacy_addr, legacy_shutdown) = legacy::start_server(state.clone());

    group.bench_function("legacy", |b| {
        b.iter(|| {
            let resp = legacy::call(legacy_addr, &legacy::RpcMsgReq::Ping);
            black_box(resp);
        });
    });

    legacy_shutdown.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(50));

    // --- Mill-RPC ---
    let mill_state = Arc::new(SharedState::new());
    let mill_el = Arc::new(mill_io::EventLoop::new(4, 1024, 100).unwrap());

    let svc = mill::BenchServiceImpl { state: mill_state };

    let mill_addr: SocketAddr = "127.0.0.1:19876".parse().unwrap();
    let _mill_server = mill_rpc::RpcServer::builder()
        .bind(mill_addr)
        .service(mill::bench_service::server(svc))
        .build(&mill_el);

    match _mill_server {
        Ok(_server) => {
            let el = mill_el.clone();
            thread::spawn(move || {
                let _ = el.run();
            });
            thread::sleep(Duration::from_millis(100));

            // Create a persistent client
            let client_el = Arc::new(mill_io::EventLoop::new(1, 256, 50).unwrap());
            let cel = client_el.clone();
            thread::spawn(move || {
                let _ = cel.run();
            });
            thread::sleep(Duration::from_millis(50));

            let client = mill::make_client(mill_addr, &client_el);

            group.bench_function("mill_rpc", |b| {
                b.iter(|| {
                    let resp = client.ping();
                    black_box(resp).unwrap();
                });
            });

            client_el.stop();
            mill_el.stop();
        }
        Err(e) => {
            eprintln!("Mill-RPC server failed to start (skipping): {}", e);
        }
    }

    group.finish();
}

fn bench_get_utxos(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_utxos_roundtrip");
    group.throughput(Throughput::Elements(1));

    let state = Arc::new(SharedState::new());

    // --- Legacy ---
    let (legacy_addr, legacy_shutdown) = legacy::start_server(state.clone());

    group.bench_function("legacy", |b| {
        b.iter(|| {
            let resp = legacy::call(legacy_addr, &legacy::RpcMsgReq::GetUtxos);
            black_box(resp);
        });
    });

    legacy_shutdown.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(50));

    // --- Mill-RPC ---
    let mill_state = Arc::new(SharedState::new());
    let mill_el = Arc::new(mill_io::EventLoop::new(4, 1024, 100).unwrap());

    let mill_addr: SocketAddr = "127.0.0.1:19877".parse().unwrap();
    let svc = mill::BenchServiceImpl { state: mill_state };
    let _mill_server = mill_rpc::RpcServer::builder()
        .bind(mill_addr)
        .service(mill::bench_service::server(svc))
        .build(&mill_el);

    match _mill_server {
        Ok(_server) => {
            let el = mill_el.clone();
            thread::spawn(move || {
                let _ = el.run();
            });
            thread::sleep(Duration::from_millis(100));

            let client_el = Arc::new(mill_io::EventLoop::new(1, 256, 50).unwrap());
            let cel = client_el.clone();
            thread::spawn(move || {
                let _ = cel.run();
            });
            thread::sleep(Duration::from_millis(50));

            let client = mill::make_client(mill_addr, &client_el);

            group.bench_function("mill_rpc", |b| {
                b.iter(|| {
                    let resp = client.get_utxos();
                    black_box(resp).unwrap();
                });
            });

            client_el.stop();
            mill_el.stop();
        }
        Err(e) => {
            eprintln!("Mill-RPC server failed to start (skipping): {}", e);
        }
    }

    group.finish();
}

fn bench_echo(c: &mut Criterion) {
    let mut group = c.benchmark_group("echo_roundtrip");

    let state = Arc::new(SharedState::new());

    for size in [16, 256, 4096] {
        let msg = "x".repeat(size);
        group.throughput(Throughput::Bytes(size as u64));

        // --- Legacy ---
        let (legacy_addr, legacy_shutdown) = legacy::start_server(state.clone());

        group.bench_with_input(BenchmarkId::new("legacy", size), &msg, |b, msg| {
            b.iter(|| {
                let resp = legacy::call(legacy_addr, &legacy::RpcMsgReq::Echo(msg.clone()));
                black_box(resp);
            });
        });

        legacy_shutdown.store(true, Ordering::Relaxed);
        thread::sleep(Duration::from_millis(50));
    }

    // Mill-RPC echo with different sizes
    for size in [16, 256, 4096] {
        let msg = "x".repeat(size);

        let mill_state = Arc::new(SharedState::new());
        let mill_el = Arc::new(mill_io::EventLoop::new(4, 1024, 100).unwrap());

        let port = 19878 + size as u16;
        let mill_addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let svc = mill::BenchServiceImpl { state: mill_state };
        let _mill_server = mill_rpc::RpcServer::builder()
            .bind(mill_addr)
            .service(mill::bench_service::server(svc))
            .build(&mill_el);

        match _mill_server {
            Ok(_server) => {
                let el = mill_el.clone();
                thread::spawn(move || {
                    let _ = el.run();
                });
                thread::sleep(Duration::from_millis(100));

                let client_el = Arc::new(mill_io::EventLoop::new(1, 256, 50).unwrap());
                let cel = client_el.clone();
                thread::spawn(move || {
                    let _ = cel.run();
                });
                thread::sleep(Duration::from_millis(50));

                let client = mill::make_client(mill_addr, &client_el);

                group.bench_with_input(BenchmarkId::new("mill_rpc", size), &msg, |b, msg| {
                    b.iter(|| {
                        let resp = client.echo(msg.clone());
                        black_box(resp).unwrap();
                    });
                });

                client_el.stop();
                mill_el.stop();
            }
            Err(e) => {
                eprintln!("Mill-RPC echo server failed (skipping): {}", e);
            }
        }
    }

    group.finish();
}

fn bench_sequential_burst(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_burst");
    let burst_size = 50u64;
    group.throughput(Throughput::Elements(burst_size));

    let state = Arc::new(SharedState::new());

    // --- Legacy: each call opens a new TCP connection (coinswap pattern) ---
    let (legacy_addr, legacy_shutdown) = legacy::start_server(state.clone());

    group.bench_function("legacy_new_conn_per_call", |b| {
        b.iter(|| {
            for _ in 0..burst_size {
                let resp = legacy::call(legacy_addr, &legacy::RpcMsgReq::Ping);
                black_box(resp);
            }
        });
    });

    legacy_shutdown.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(50));

    // --- Mill-RPC: persistent connection, multiplexed ---
    let mill_state = Arc::new(SharedState::new());
    let mill_el = Arc::new(mill_io::EventLoop::new(4, 1024, 100).unwrap());

    let mill_addr: SocketAddr = "127.0.0.1:19890".parse().unwrap();
    let svc = mill::BenchServiceImpl { state: mill_state };
    let _mill_server = mill_rpc::RpcServer::builder()
        .bind(mill_addr)
        .service(mill::bench_service::server(svc))
        .build(&mill_el);

    match _mill_server {
        Ok(_server) => {
            let el = mill_el.clone();
            thread::spawn(move || {
                let _ = el.run();
            });
            thread::sleep(Duration::from_millis(100));

            let client_el = Arc::new(mill_io::EventLoop::new(1, 256, 50).unwrap());
            let cel = client_el.clone();
            thread::spawn(move || {
                let _ = cel.run();
            });
            thread::sleep(Duration::from_millis(50));

            let client = mill::make_client(mill_addr, &client_el);

            group.bench_function("mill_rpc_persistent_conn", |b| {
                b.iter(|| {
                    for _ in 0..burst_size {
                        let resp = client.ping();
                        black_box(resp).unwrap();
                    }
                });
            });

            client_el.stop();
            mill_el.stop();
        }
        Err(e) => {
            eprintln!("Mill-RPC burst server failed (skipping): {}", e);
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_ping,
    bench_get_utxos,
    bench_echo,
    bench_sequential_burst,
);
criterion_main!(benches);

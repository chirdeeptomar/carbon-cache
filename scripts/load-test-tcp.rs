#!/usr/bin/env rust-script
//! TCP Server Performance Test Suite
//!
//! High-performance load test for Carbon TCP server (binary protocol)
//!
//! ```cargo
//! [dependencies]
//! tokio = { version = "1", features = ["full"] }
//! tokio-util = { version = "0.7", features = ["codec"] }
//! futures = "0.3"
//! bytes = "1.11.0"
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};
use bytes::{Bytes, BytesMut, BufMut, Buf};

// ============================================
// CONFIGURATION CONSTANTS
// ============================================

/// Target requests per second (0 = unlimited)
const TARGET_RPS: u64 = 0;

/// Total number of requests to send
const TOTAL_REQUESTS: u64 = 10_000;

/// Number of concurrent connections/workers
const CONCURRENT_CONNECTIONS: usize = 100;

/// TCP server address
const SERVER_ADDR: &str = "127.0.0.1:5500";

/// Cache name to use for testing
const CACHE_NAME: &str = "perf-test";

/// Operations per request (PUT + GET = 2)
const OPS_PER_REQUEST: u64 = 2;

/// Connection timeout in seconds
const CONNECTION_TIMEOUT_SECS: u64 = 5;

// ============================================
// PROTOCOL IMPLEMENTATION
// ============================================

const CMD_PING: u8 = 0x00;
const CMD_PUT: u8 = 0x01;
const CMD_GET: u8 = 0x02;
const CMD_DELETE: u8 = 0x03;

const RESP_PONG: u8 = 0x00;
const RESP_OK: u8 = 0x01;
const RESP_VALUE: u8 = 0x02;
const RESP_NOT_FOUND: u8 = 0x03;
const RESP_ERROR: u8 = 0x04;

#[derive(Debug, Clone)]
enum Request {
    Ping,
    Put { cache_name: String, key: Bytes, value: Bytes },
    Get { cache_name: String, key: Bytes },
    Delete { cache_name: String, key: Bytes },
}

#[derive(Debug)]
enum Response {
    Pong,
    Ok,
    Value { value: Bytes },
    NotFound,
    Error { msg: String },
}

impl Request {
    fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();
        match self {
            Request::Ping => {
                buf.put_u8(CMD_PING);
            }
            Request::Put { cache_name, key, value } => {
                buf.put_u8(CMD_PUT);
                // Encode cache_name
                let cache_name_bytes = cache_name.as_bytes();
                buf.put_u32(cache_name_bytes.len() as u32);
                buf.put_slice(cache_name_bytes);
                // Encode key and value
                buf.put_u32(key.len() as u32);
                buf.put_u32(value.len() as u32);
                buf.put_slice(key);
                buf.put_slice(value);
            }
            Request::Get { cache_name, key } => {
                buf.put_u8(CMD_GET);
                // Encode cache_name
                let cache_name_bytes = cache_name.as_bytes();
                buf.put_u32(cache_name_bytes.len() as u32);
                buf.put_slice(cache_name_bytes);
                // Encode key
                buf.put_u32(key.len() as u32);
                buf.put_slice(key);
            }
            Request::Delete { cache_name, key } => {
                buf.put_u8(CMD_DELETE);
                // Encode cache_name
                let cache_name_bytes = cache_name.as_bytes();
                buf.put_u32(cache_name_bytes.len() as u32);
                buf.put_slice(cache_name_bytes);
                // Encode key
                buf.put_u32(key.len() as u32);
                buf.put_slice(key);
            }
        }
        buf.freeze()
    }
}

impl Response {
    fn decode(mut buf: Bytes) -> Result<Self, String> {
        if buf.is_empty() {
            return Err("Empty response".to_string());
        }

        let resp_type = buf.get_u8();
        match resp_type {
            RESP_PONG => Ok(Response::Pong),
            RESP_OK => Ok(Response::Ok),
            RESP_VALUE => {
                if buf.remaining() < 4 {
                    return Err("Invalid VALUE response: missing length".to_string());
                }
                let value_len = buf.get_u32() as usize;
                if buf.remaining() < value_len {
                    return Err("Invalid VALUE response: value too short".to_string());
                }
                let value = buf.copy_to_bytes(value_len);
                Ok(Response::Value { value })
            }
            RESP_NOT_FOUND => Ok(Response::NotFound),
            RESP_ERROR => {
                if buf.remaining() < 4 {
                    return Err("Invalid ERROR response: missing length".to_string());
                }
                let msg_len = buf.get_u32() as usize;
                if buf.remaining() < msg_len {
                    return Err("Invalid ERROR response: message too short".to_string());
                }
                let msg_bytes = buf.copy_to_bytes(msg_len);
                let msg = String::from_utf8(msg_bytes.to_vec())
                    .map_err(|e| format!("Invalid error message UTF-8: {}", e))?;
                Ok(Response::Error { msg })
            }
            _ => Err(format!("Unknown response type: {}", resp_type)),
        }
    }
}

// ============================================
// STATISTICS TRACKING
// ============================================

#[derive(Debug)]
struct LoadTestStats {
    total_requests: u64,
    successful_puts: u64,
    successful_gets: u64,
    failed: u64,
    duration_ms: u64,
    connection_setup_ms: u64,
}

impl LoadTestStats {
    fn print_summary(&self) {
        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║         TCP Server Performance Test Results             ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        println!("Connection Setup:    {} ms", self.connection_setup_ms);
        println!("Test Duration:       {} ms ({:.2} seconds)",
                 self.duration_ms, self.duration_ms as f64 / 1000.0);
        println!();
        println!("Total Operations:    {}", self.total_requests);
        println!("  PUT operations:    {}", self.successful_puts);
        println!("  GET operations:    {}", self.successful_gets);
        println!("  Failed:            {} ({:.2}%)",
                 self.failed,
                 (self.failed as f64 / self.total_requests as f64) * 100.0);
        println!();

        let rps = if self.duration_ms > 0 {
            (self.total_requests as f64 / (self.duration_ms as f64 / 1000.0)) as u64
        } else {
            0
        };

        let avg_latency = if self.total_requests > 0 {
            self.duration_ms as f64 / self.total_requests as f64
        } else {
            0.0
        };

        println!("Performance Metrics:");
        println!("  Throughput:        {} ops/sec", rps);
        println!("  Avg Latency:       {:.2} ms", avg_latency);
        println!("  Success Rate:      {:.2}%",
                 ((self.total_requests - self.failed) as f64 / self.total_requests as f64) * 100.0);
        println!();
    }
}

// ============================================
// TCP CLIENT HELPERS
// ============================================

async fn create_tcp_client(addr: &str) -> Result<Framed<TcpStream, LengthDelimitedCodec>, Box<dyn std::error::Error>> {
    let stream = tokio::time::timeout(
        Duration::from_secs(CONNECTION_TIMEOUT_SECS),
        TcpStream::connect(addr)
    ).await??;

    stream.set_nodelay(true)?;

    let codec = LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(8 * 1024 * 1024)
        .new_codec();

    Ok(Framed::new(stream, codec))
}

async fn send_request(
    framed: &mut Framed<TcpStream, LengthDelimitedCodec>,
    request: Request
) -> Result<Response, Box<dyn std::error::Error>> {
    framed.send(request.encode()).await?;

    if let Some(frame) = framed.next().await {
        let response = Response::decode(frame?.freeze())?;
        Ok(response)
    } else {
        Err("No response received".into())
    }
}

// ============================================
// MAIN PERFORMANCE TEST
// ============================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║      TCP Server Performance Test Suite v1.0             ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Print configuration
    println!("Configuration:");
    println!("  Server Address:      tcp://{}", SERVER_ADDR);
    println!("  Cache Name:          {}", CACHE_NAME);
    println!("  Total Requests:      {}", TOTAL_REQUESTS);
    println!("  Concurrent Conns:    {}", CONCURRENT_CONNECTIONS);
    println!("  Target RPS:          {}", if TARGET_RPS == 0 { "Unlimited".to_string() } else { TARGET_RPS.to_string() });
    println!("  Operations/Request:  {} (PUT + GET)", OPS_PER_REQUEST);
    println!("  Total Operations:    {}", TOTAL_REQUESTS * OPS_PER_REQUEST);
    println!();

    // Check if server is running
    println!("Testing connection to TCP server...");
    let test_start = Instant::now();
    match tokio::time::timeout(
        Duration::from_secs(CONNECTION_TIMEOUT_SECS),
        TcpStream::connect(SERVER_ADDR)
    ).await {
        Ok(Ok(_)) => {
            let conn_time = test_start.elapsed();
            println!("✓ Server is reachable ({:.2} ms)", conn_time.as_millis());
        }
        Ok(Err(_)) | Err(_) => {
            eprintln!("✗ Cannot connect to TCP server at {}", SERVER_ADDR);
            eprintln!();
            eprintln!("Start the carbon server with:");
            eprintln!("  cargo run --release --bin carbon-server");
            eprintln!();
            eprintln!("Or TCP server only:");
            eprintln!("  cargo run --release --bin server-tcp");
            eprintln!();
            eprintln!("Then create the cache:");
            eprintln!("  curl -X POST http://localhost:8080/admin/caches \\");
            eprintln!("    -u admin:admin123 \\");
            eprintln!("    -H 'Content-Type: application/json' \\");
            eprintln!("    -d '{{\"name\": \"{}\", \"max_capacity\": 100000}}'", CACHE_NAME);
            return Err("Server not reachable".into());
        }
    }
    println!();

    // Shared atomic counters
    let put_success = Arc::new(AtomicU64::new(0));
    let get_success = Arc::new(AtomicU64::new(0));
    let fail_count = Arc::new(AtomicU64::new(0));
    let request_counter = Arc::new(AtomicU64::new(0));

    println!("Starting performance test...");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let start = Instant::now();

    // Spawn concurrent workers
    let mut tasks = JoinSet::new();
    let requests_per_worker = TOTAL_REQUESTS / CONCURRENT_CONNECTIONS as u64;

    for worker_id in 0..CONCURRENT_CONNECTIONS {
        let put_success = put_success.clone();
        let get_success = get_success.clone();
        let fail_count = fail_count.clone();
        let request_counter = request_counter.clone();

        tasks.spawn(async move {
            // Each worker creates its own connection
            let mut client = match create_tcp_client(SERVER_ADDR).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Worker {} failed to connect: {}", worker_id, e);
                    return;
                }
            };

            for i in 0..requests_per_worker {
                // Generate unique key per request
                let key = format!("key-{}-{}", worker_id, i);
                let value = format!("value-{}-{}", worker_id, i);

                // PUT operation
                let put_request = Request::Put {
                    cache_name: CACHE_NAME.to_string(),
                    key: Bytes::from(key.clone()),
                    value: Bytes::from(value.clone()),
                };

                match send_request(&mut client, put_request).await {
                    Ok(Response::Ok) => {
                        put_success.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(Response::Error { msg }) => {
                        if msg.contains("Cache not found") {
                            eprintln!("\n✗ Cache '{}' not found. Create it first!", CACHE_NAME);
                            eprintln!("  curl -X POST http://localhost:8080/admin/caches \\");
                            eprintln!("    -u admin:admin123 \\");
                            eprintln!("    -H 'Content-Type: application/json' \\");
                            eprintln!("    -d '{{\"name\": \"{}\", \"max_capacity\": 100000}}'", CACHE_NAME);
                        }
                        fail_count.fetch_add(1, Ordering::Relaxed);
                        return; // Exit worker on cache error
                    }
                    Ok(resp) => {
                        eprintln!("PUT unexpected response: {:?}", resp);
                        fail_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        eprintln!("PUT error: {}", e);
                        fail_count.fetch_add(1, Ordering::Relaxed);
                    }
                }

                // GET operation (verify PUT worked)
                let get_request = Request::Get {
                    cache_name: CACHE_NAME.to_string(),
                    key: Bytes::from(key),
                };

                match send_request(&mut client, get_request).await {
                    Ok(Response::Value { .. }) => {
                        get_success.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(resp) => {
                        eprintln!("GET unexpected response: {:?}", resp);
                        fail_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        eprintln!("GET error: {}", e);
                        fail_count.fetch_add(1, Ordering::Relaxed);
                    }
                }

                request_counter.fetch_add(1, Ordering::Relaxed);

                // Print progress every 1000 requests
                let count = request_counter.load(Ordering::Relaxed);
                if count % 1000 == 0 {
                    let elapsed = start.elapsed().as_millis();
                    let current_rps = if elapsed > 0 {
                        ((count * OPS_PER_REQUEST) as f64 / (elapsed as f64 / 1000.0)) as u64
                    } else {
                        0
                    };
                    print!("\rProgress: {}/{} requests ({} ops/sec)     ",
                           count, TOTAL_REQUESTS, current_rps);
                }

                // Rate limiting if TARGET_RPS is set
                if TARGET_RPS > 0 {
                    let target_delay_micros = (1_000_000.0 / (TARGET_RPS as f64 / CONCURRENT_CONNECTIONS as f64)) as u64;
                    tokio::time::sleep(Duration::from_micros(target_delay_micros)).await;
                }
            }
        });
    }

    // Wait for all workers to complete
    while let Some(_) = tasks.join_next().await {}

    let duration = start.elapsed();
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Collect and print stats
    let stats = LoadTestStats {
        total_requests: TOTAL_REQUESTS * OPS_PER_REQUEST,
        successful_puts: put_success.load(Ordering::Relaxed),
        successful_gets: get_success.load(Ordering::Relaxed),
        failed: fail_count.load(Ordering::Relaxed),
        duration_ms: duration.as_millis() as u64,
        connection_setup_ms: test_start.elapsed().as_millis() as u64,
    };

    stats.print_summary();

    println!("Note: Each request performed PUT + GET operations with unique keys");
    println!("This tests realistic TCP cache behavior under load.");
    println!();

    Ok(())
}

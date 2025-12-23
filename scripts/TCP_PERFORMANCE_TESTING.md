# TCP Server Performance Testing

High-performance load testing tool for Carbon's TCP server (binary protocol on port 5500).

## Quick Start

### Prerequisites

1. **Install rust-script** (if not already installed):

```bash
cargo install rust-script
```

2. **Start the server**:

```bash
# Option 1: Carbon server (HTTP + TCP)
cargo run --release --bin carbon-server

# Option 2: TCP server only
cargo run --release --bin server-tcp
```

3. **Create the test cache**:

```bash
curl -X POST http://localhost:8080/admin/caches \
  -u admin:admin123 \
  -H "Content-Type: application/json" \
  -d '{"name": "perf_test", "max_capacity": 100000}'
```

### Run the Performance Test

```bash
./scripts/load-test-tcp.rs
```

## Configuration

Edit the constants at the top of `load-test-tcp.rs`:

```rust
/// Target requests per second (0 = unlimited)
const TARGET_RPS: u64 = 0;

/// Total number of requests to send
const TOTAL_REQUESTS: u64 = 10_000;

/// Number of concurrent connections/workers
const CONCURRENT_CONNECTIONS: usize = 100;

/// TCP server address
const SERVER_ADDR: &str = "127.0.0.1:5500";

/// Cache name to use for testing
const CACHE_NAME: &str = "perf_test";
```

## Example Output

```
╔══════════════════════════════════════════════════════════╗
║      TCP Server Performance Test Suite v1.0             ║
╚══════════════════════════════════════════════════════════╝

Configuration:
  Server Address:      tcp://127.0.0.1:5500
  Cache Name:          perf_test
  Total Requests:      10000
  Concurrent Conns:    100
  Target RPS:          Unlimited
  Operations/Request:  2 (PUT + GET)
  Total Operations:    20000

Testing connection to TCP server...
✓ Server is reachable (2.34 ms)

Starting performance test...
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Progress: 10000/10000 requests (12961 ops/sec)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

╔══════════════════════════════════════════════════════════╗
║         TCP Server Performance Test Results             ║
╚══════════════════════════════════════════════════════════╝

Connection Setup:    2 ms
Test Duration:       1543 ms (1.54 seconds)

Total Operations:    20000
  PUT operations:    10000
  GET operations:    10000
  Failed:            0 (0.00%)

Performance Metrics:
  Throughput:        12961 ops/sec
  Avg Latency:       0.08 ms
  Success Rate:      100.00%

Note: Each request performed PUT + GET operations with unique keys
This tests realistic TCP cache behavior under load.
```

## Test Scenarios

### Scenario 1: Maximum Throughput Test

```rust
const TARGET_RPS: u64 = 0;  // Unlimited
const TOTAL_REQUESTS: u64 = 50_000;
const CONCURRENT_CONNECTIONS: usize = 200;
```

**Tests**: Server's maximum throughput capacity

### Scenario 2: Sustained Load Test

```rust
const TARGET_RPS: u64 = 5000;  // 5K ops/sec
const TOTAL_REQUESTS: u64 = 100_000;
const CONCURRENT_CONNECTIONS: usize = 50;
```

**Tests**: Stability under sustained load

### Scenario 3: High Concurrency Test

```rust
const TARGET_RPS: u64 = 0;
const TOTAL_REQUESTS: u64 = 10_000;
const CONCURRENT_CONNECTIONS: usize = 500;
```

**Tests**: Connection handling with many concurrent clients

### Scenario 4: Rate-Limited Test

```rust
const TARGET_RPS: u64 = 1000;  // 1K ops/sec
const TOTAL_REQUESTS: u64 = 60_000;
const CONCURRENT_CONNECTIONS: usize = 10;
```

**Tests**: Consistent performance at controlled rate

## What Gets Tested

-   ✅ **Binary Protocol**: Tests actual TCP binary protocol (length-delimited framing)
-   ✅ **PUT Operations**: Write operations with unique keys
-   ✅ **GET Operations**: Read operations to verify writes
-   ✅ **Connection Handling**: Each worker maintains its own connection
-   ✅ **Concurrency**: Validates server handles multiple concurrent connections
-   ✅ **Error Handling**: Captures and reports protocol errors
-   ✅ **Throughput**: Measures operations per second
-   ✅ **Latency**: Average latency per operation

## Performance Targets

Based on TCP binary protocol characteristics:

| Metric           | Target           |
| ---------------- | ---------------- |
| Throughput (max) | > 10,000 ops/sec |
| Latency (avg)    | < 1 ms           |
| Latency (P95)    | < 5 ms           |
| Latency (P99)    | < 10 ms          |
| Success Rate     | > 99.9%          |
| Concurrent Conns | 500+             |

## Comparing TCP vs HTTP Performance

Run both tests to compare protocols:

```bash
# TCP Test (binary protocol)
./scripts/load-test-tcp.rs

# HTTP Test (JSON over HTTP)
./scripts/load-test-unique-keys.rs
```

**Expected Results**:

| Metric        | TCP (Binary)    | HTTP (JSON)          |
| ------------- | --------------- | -------------------- |
| Throughput    | 10,000+ ops/sec | 5,000-10,000 ops/sec |
| Latency (avg) | < 0.1 ms        | 0.5-2 ms             |
| Protocol      | Binary          | JSON/HTTP            |
| Overhead      | Minimal         | Higher               |

## Troubleshooting

### Error: "Cannot connect to TCP server"

**Solution**:

```bash
# Check if server is running
lsof -i :5500

# Start server
cargo run --release --bin carbon-server
```

### Error: "Cache 'perf_test' not found"

**Solution**:

```bash
# Create the cache
curl -X POST http://localhost:8080/admin/caches \
  -u admin:admin123 \
  -H "Content-Type: application/json" \
  -d '{"name": "perf_test", "max_capacity": 100000}'
```

### Low Throughput Results

**Solutions**:

-   Use release build: `cargo run --release --bin carbon-server`
-   Increase concurrency: `const CONCURRENT_CONNECTIONS: usize = 200;`
-   Check system resources: `top` or `htop`
-   Increase file descriptor limit: `ulimit -n 10000`

### Connection Timeout Errors

**Solutions**:

-   Increase timeout: `const CONNECTION_TIMEOUT_SECS: u64 = 10;`
-   Reduce concurrency to avoid overwhelming server
-   Check network connectivity

## Monitoring During Tests

### Terminal 1: Run the server

```bash
cargo run --release --bin carbon-server
```

### Terminal 2: Monitor memory

```bash
./scripts/monitor-memory.sh
```

### Terminal 3: Run performance test

```bash
./scripts/load-test-tcp.rs
```

## Customization

### Test Different Operations

Edit the worker loop to test DELETE operations:

```rust
// Add DELETE test
let delete_request = Request::Delete {
    cache_name: CACHE_NAME.to_string(),
    key: Bytes::from(key),
};

match send_request(&mut client, delete_request).await {
    Ok(Response::Ok) => {
        delete_success.fetch_add(1, Ordering::Relaxed);
    }
    // ... handle errors
}
```

### Add Custom Metrics

Track additional metrics:

```rust
let latencies = Arc::new(Mutex::new(Vec::new()));

// In worker:
let op_start = Instant::now();
send_request(&mut client, put_request).await?;
let latency = op_start.elapsed().as_micros();
latencies.lock().unwrap().push(latency);
```

### Test Specific Key Patterns

```rust
// Sequential keys
let key = format!("key-{}", i);

// Random keys
let key = format!("key-{}", rand::random::<u64>());

// Fixed key (cache hit test)
let key = "fixed-key".to_string();
```

## Related Documentation

-   [HTTP Load Testing](LOAD_TESTING.md)
-   [TCP Protocol Specification](../server-tcp/PROTOCOL.md)
-   [Carbon Server Architecture](../carbon-server/)
-   [Server TCP README](../server-tcp/README.md)

## Technical Details

### Protocol Implementation

The test implements the full TCP binary protocol:

```
PUT Request Format:
  [0x01][cache_name_len: u32][cache_name][key_len: u32][value_len: u32][key][value]

GET Request Format:
  [0x02][cache_name_len: u32][cache_name][key_len: u32][key]

Response Format:
  OK:        [0x01]
  VALUE:     [0x02][value_len: u32][value]
  ERROR:     [0x04][msg_len: u32][message]
```

### Architecture

-   **Async I/O**: Tokio runtime for non-blocking operations
-   **Connection Pooling**: One connection per worker (persistent)
-   **Length-Delimited Framing**: 4-byte big-endian length prefix
-   **Atomic Counters**: Thread-safe statistics tracking
-   **Worker Model**: Concurrent workers with independent connections

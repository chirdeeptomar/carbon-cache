# Load Testing Scripts

This directory contains several load testing scripts for benchmarking the Carbon server's session management and cache performance.

## Prerequisites

All scripts require the server to be running:

```bash
cargo run --release --bin server-http
```

## Available Scripts

### 1. Session Performance Benchmark (Graduated Concurrency)

**File**: [bench-session-performance.sh](bench-session-performance.sh)

Tests session reuse performance under different concurrency levels (10, 50, 100, 200 parallel requests).

```bash
./scripts/bench-session-performance.sh
```

**What it tests**:
- Sequential requests baseline
- Low concurrency (10 parallel)
- Medium concurrency (50 parallel)
- High concurrency (100 parallel)
- Very high concurrency (200 parallel)

**Features**:
- Cross-platform (macOS & Linux)
- Uses same cache key for all requests
- Reports throughput, latency, and session reuse efficiency

**Expected output**:
```
=== Session Management Performance Benchmark ===

Scenario 1: Sequential Requests (Baseline)
✓ Sequential: 2847 ms total (56 ms avg per request)

=== Performance Summary ===
Scenario                       Total (ms)    Avg/Req (ms)
--------                       ----------    -------------
Sequential (50 req)                  2847               56
Low Concurrency (10 req)              312               31
Medium Concurrency (50 req)           823               16
...
```

---

### 2. Extreme Load Test with oha (Same Key)

**File**: [load-test-1m-rps.sh](load-test-1m-rps.sh)

Uses `oha` for extreme load testing. All requests hit the same cache key.

```bash
./scripts/load-test-1m-rps.sh
```

**Requirements**: Install `oha` first
```bash
# macOS
brew install oha

# Or via Cargo
cargo install oha
```

**Configuration**:
- Duration: 1 minute
- Target: 100 queries/sec per worker
- Concurrency: 1000 connections
- Protocol: HTTP/2

**What it tests**:
- Extreme concurrency handling
- Session reuse at scale
- Memory consumption under load

**Limitations**:
- Uses same cache key for all requests
- Doesn't test cache eviction or varied key patterns

---

### 3. Simple Load Test with Unique Keys (Recommended)

**File**: [load-test-simple.sh](load-test-simple.sh)

Simple bash-based load test that generates unique keys per request. **No additional dependencies required.**

```bash
./scripts/load-test-simple.sh
```

**Configuration** (edit file to customize):
- `TOTAL_REQUESTS`: Number of requests (default: 1000)
- `CONCURRENCY`: Parallel workers (default: 50)

**What it tests**:
- Realistic cache behavior with different keys
- Session reuse across varied requests
- Cache insertion performance

**Features**:
- Each request uses unique key (`key-1`, `key-2`, ..., `key-N`)
- No external dependencies beyond `curl`
- Cross-platform (macOS & Linux)
- Reports success rate, throughput, latency

**Example output**:
```
=== Simple Load Test: Unique Keys Per Request ===

Configuration:
  URL:             http://localhost:8080/cache/test/[unique-key]
  Total requests:  1000
  Concurrency:     50
  Auth:            Basic Auth (admin)

=== Load Test Results ===
Duration:          5234 ms (5.23 seconds)
Total requests:    1000
Successful:        998 (99.80%)
Failed:            2 (0.20%)
Throughput:        191 req/sec
Avg latency:       5.23 ms
```

---

### 4. Advanced Rust Load Test with Unique Keys

**File**: [load-test-unique-keys.rs](load-test-unique-keys.rs)

High-performance Rust-based load test with unique keys per request.

**Requirements**: Install `rust-script`
```bash
cargo install rust-script
```

**Run**:
```bash
./scripts/run-load-test-unique-keys.sh
```

**Configuration** (edit the `.rs` file):
- `total_requests`: Number of requests (default: 10,000)
- `concurrency`: Number of workers (default: 100)

**What it tests**:
- High-throughput unique key insertion
- Connection pooling efficiency
- Session reuse at scale

**Features**:
- Async/await with Tokio
- HTTP connection pooling
- Real-time progress updates
- Detailed statistics

**Advantages over simple script**:
- 10-100x higher throughput
- Lower overhead per request
- Better connection reuse
- Scales to millions of requests

---

### 5. Memory Monitoring

**File**: [monitor-memory.sh](monitor-memory.sh)

Monitors server memory consumption in real-time during load tests.

```bash
# Terminal 1: Start monitoring
./scripts/monitor-memory.sh

# Terminal 2: Run any load test
./scripts/load-test-simple.sh
```

**Output**:
```
=== Memory Monitoring for server-http (PID: 12345) ===

Time(s)   RSS(MB)   VSZ(MB)   CPU%
-------   -------   -------   ----
0         142       2304      15.2
1         156       2304      87.3
2         168       2304      95.1
...
```

**Metrics**:
- **RSS**: Resident Set Size (actual RAM used)
- **VSZ**: Virtual Memory Size (total allocated)
- **CPU%**: CPU usage percentage

---

## Recommended Testing Workflow

### Quick Test (Development)
```bash
# Test basic session performance
./scripts/bench-session-performance.sh
```

### Realistic Load Test (Pre-Production)
```bash
# Terminal 1: Monitor memory
./scripts/monitor-memory.sh

# Terminal 2: Run realistic load with unique keys
./scripts/load-test-simple.sh
```

### Extreme Load Test (Production Capacity)
```bash
# Requires oha
brew install oha

# Terminal 1: Monitor memory
./scripts/monitor-memory.sh

# Terminal 2: Run extreme load
./scripts/load-test-1m-rps.sh
```

### Custom High-Throughput Test
```bash
# Edit load-test-unique-keys.rs to set:
# - total_requests: 100_000
# - concurrency: 500

cargo install rust-script
./scripts/run-load-test-unique-keys.sh
```

---

## Interpreting Results

### Memory Usage
- **Good**: RSS stays < 200 MB for 10K requests
- **Excellent**: RSS stays < 100 MB (session caching working)
- **Problem**: RSS grows linearly with requests (potential leak)

### Throughput
- **Sequential**: ~20-50 req/sec (baseline, includes Argon2 on first request)
- **Low concurrency** (10): ~50-100 req/sec
- **Medium concurrency** (50): ~100-200 req/sec
- **High concurrency** (100+): 200-500 req/sec (session reuse critical)

### Session Reuse
With optimized session caching:
- **First request**: ~250 ms (Argon2 password hashing)
- **Subsequent requests**: <5 ms (cached session lookup)
- **Target**: >95% of requests should reuse sessions

### Latency
- **P50** (median): <10 ms
- **P95**: <50 ms
- **P99**: <100 ms

---

## Troubleshooting

### "Server not running" error
```bash
# Start the server first
cargo run --release --bin server-http
```

### "oha not installed" error
```bash
# Install oha
brew install oha
# or
cargo install oha
```

### "rust-script not installed" error
```bash
# Install rust-script
cargo install rust-script

# Or use the simple bash version instead
./scripts/load-test-simple.sh
```

### High failure rate
- Check server logs for errors
- Reduce concurrency level
- Increase server resources
- Check network/firewall settings

### Memory keeps growing
- Check for session leaks
- Verify TTL configuration
- Monitor cache eviction
- Review authentication middleware

---

## Summary Table

| Script | Unique Keys | Dependencies | Max RPS | Best For |
|--------|-------------|--------------|---------|----------|
| `bench-session-performance.sh` | No | curl | ~100 | Session reuse testing |
| `load-test-1m-rps.sh` | No | oha | 100K+ | Extreme concurrency |
| `load-test-simple.sh` | Yes | curl | ~1K | Quick realistic tests |
| `load-test-unique-keys.rs` | Yes | rust-script | 10K+ | High-throughput testing |
| `monitor-memory.sh` | N/A | ps | N/A | Memory profiling |

**Recommendation**: Start with `load-test-simple.sh` for realistic testing with unique keys and no dependencies.

# TCP Protocol Documentation

## Overview

The Carbon TCP server uses a **simple binary protocol** with length-delimited framing and manual Bytes encoding/decoding. This approach:

- ✅ Uses `bytes::Bytes` directly (zero-copy, aligns with HTTP layer)
- ✅ No Serde/Bincode serialization overhead
- ✅ Simple to understand and debug
- ✅ Efficient for high-performance use cases

## Protocol Layers

### Layer 1: TCP Stream

Raw TCP connection providing reliable, ordered byte stream.

```
┌─────────────────────────────────────┐
│   TCP Stream (raw bytes)            │
└─────────────────────────────────────┘
```

### Layer 2: Length-Delimited Framing

Uses `tokio_util::codec::LengthDelimitedCodec` to split the stream into frames.

```
Frame Format:
┌──────────────────┬───────────────────┐
│  Length (4 bytes)│  Payload (N bytes)│
│   (big-endian)   │                   │
└──────────────────┴───────────────────┘

Configuration:
- Length field: 4 bytes
- Max frame size: 8 MB
- Byte order: Big-endian
```

**Example:**
```
Input bytes:  [0x00, 0x00, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05]
              └─────────────────┘ └────────────────────────────┘
                  Length = 5           Payload (5 bytes)

LengthDelimitedCodec extracts → [0x01, 0x02, 0x03, 0x04, 0x05]
```

### Layer 3: Protocol Messages

After framing, each payload is decoded into `Request` or `Response` enums.

## Message Format

### Request Messages

All requests start with a 1-byte command identifier.

#### PING (0x00)

```
┌────┐
│0x00│
└────┘

No payload.
```

**Example:**
```rust
Request::Ping.encode() → Bytes: [0x00]
```

#### PUT (0x01)

```
┌────┬─────────────────┬────────────┬────────────┬──────────────┬─────────┬───────────┐
│0x01│cache_name_len(4)│cache_name  │key_len (4) │value_len (4) │key bytes│value bytes│
└────┴─────────────────┴────────────┴────────────┴──────────────┴─────────┴───────────┘

- cache_name_len: u32 (big-endian)
- cache_name: UTF-8 string (variable length)
- key_len: u32 (big-endian)
- value_len: u32 (big-endian)
- key bytes: variable length
- value bytes: variable length
```

**Example:**
```rust
Request::Put {
    cache_name: "test_cache".to_string(),  // 10 bytes
    key: Bytes::from("hello"),             // 5 bytes
    value: Bytes::from("world"),           // 5 bytes
}.encode()

→ Bytes: [
    0x01,                          // Command: PUT
    0x00, 0x00, 0x00, 0x0A,       // cache_name_len = 10
    0x74, 0x65, 0x73, 0x74, 0x5F, // "test_cache" (part 1)
    0x63, 0x61, 0x63, 0x68, 0x65, // "test_cache" (part 2)
    0x00, 0x00, 0x00, 0x05,       // key_len = 5
    0x00, 0x00, 0x00, 0x05,       // value_len = 5
    0x68, 0x65, 0x6C, 0x6C, 0x6F, // "hello"
    0x77, 0x6F, 0x72, 0x6C, 0x64  // "world"
]
```

#### GET (0x02)

```
┌────┬─────────────────┬────────────┬────────────┬─────────┐
│0x02│cache_name_len(4)│cache_name  │key_len (4) │key bytes│
└────┴─────────────────┴────────────┴────────────┴─────────┘

- cache_name_len: u32 (big-endian)
- cache_name: UTF-8 string (variable length)
- key_len: u32 (big-endian)
- key bytes: variable length
```

**Example:**
```rust
Request::Get {
    cache_name: "test_cache".to_string(),  // 10 bytes
    key: Bytes::from("hello"),             // 5 bytes
}.encode()

→ Bytes: [
    0x02,                          // Command: GET
    0x00, 0x00, 0x00, 0x0A,       // cache_name_len = 10
    0x74, 0x65, 0x73, 0x74, 0x5F, // "test_cache" (part 1)
    0x63, 0x61, 0x63, 0x68, 0x65, // "test_cache" (part 2)
    0x00, 0x00, 0x00, 0x05,       // key_len = 5
    0x68, 0x65, 0x6C, 0x6C, 0x6F  // "hello"
]
```

#### DELETE (0x03)

```
┌────┬─────────────────┬────────────┬────────────┬─────────┐
│0x03│cache_name_len(4)│cache_name  │key_len (4) │key bytes│
└────┴─────────────────┴────────────┴────────────┴─────────┘
```

Same format as GET, but with command byte 0x03.

### Response Messages

All responses start with a 1-byte response type identifier.

#### PONG (0x00)

```
┌────┐
│0x00│
└────┘
```

#### OK (0x01)

```
┌────┐
│0x01│
└────┘
```

#### VALUE (0x02)

```
┌────┬──────────────┬───────────┐
│0x02│value_len (4) │value bytes│
└────┴──────────────┴───────────┘
```

**Example:**
```rust
Response::Value {
    value: Bytes::from("world"),
}.encode()

→ Bytes: [
    0x02,                          // Response: VALUE
    0x00, 0x00, 0x00, 0x05,       // value_len = 5
    0x77, 0x6F, 0x72, 0x6C, 0x64  // "world"
]
```

#### NOT_FOUND (0x03)

```
┌────┐
│0x03│
└────┘
```

#### ERROR (0x04)

```
┌────┬────────────┬──────────┐
│0x04│msg_len (4) │msg bytes │
└────┴────────────┴──────────┘
```

Error message is UTF-8 encoded string.

## Complete Flow Example

### Client sends PING

```
Step 1: Encode Request
Request::Ping → Bytes [0x00]

Step 2: LengthDelimitedCodec adds frame header
[0x00, 0x00, 0x00, 0x01, 0x00]
 └─────────────────┘ └──┘
    Length = 1        PING

Step 3: Send over TCP
TCP stream → [0x00, 0x00, 0x00, 0x01, 0x00]
```

### Server receives and responds

```
Step 1: LengthDelimitedCodec extracts frame
TCP stream → [0x00, 0x00, 0x00, 0x01, 0x00]
Codec extracts → [0x00]

Step 2: Decode to Request
Bytes [0x00] → Request::Ping

Step 3: Process request
Request::Ping → Response::Pong

Step 4: Encode response
Response::Pong → Bytes [0x00]

Step 5: LengthDelimitedCodec adds frame header
[0x00, 0x00, 0x00, 0x01, 0x00]

Step 6: Send over TCP
```

## Implementation Details

### Encoding (Request/Response → Bytes)

Uses `bytes::BytesMut` for building:

```rust
let mut buf = BytesMut::new();
buf.put_u8(CMD_PING);  // Write command byte
buf.put_u32(key_len);  // Write u32 (big-endian)
buf.put_slice(&key);   // Write byte slice
buf.freeze()           // Convert to Bytes
```

### Decoding (Bytes → Request/Response)

Uses `bytes::Bytes` with `Buf` trait:

```rust
let cmd = buf.get_u8();        // Read 1 byte
let key_len = buf.get_u32();   // Read u32 (big-endian)
let key = buf.copy_to_bytes(key_len); // Extract bytes (zero-copy!)
```

**Key operations:**
- `buf.get_u8()` - Read and advance cursor
- `buf.get_u32()` - Read u32 in big-endian
- `buf.remaining()` - Check remaining bytes
- `buf.copy_to_bytes(n)` - Extract n bytes (zero-copy slice)

## Testing the Protocol

### Start the server

```bash
cargo run --bin server-tcp
```

### Run the example client

```bash
cargo run --example simple_client
```

Expected output:
```
Connected to server at 127.0.0.1:5500

=== Testing PING ===
Response: Pong

=== Testing PUT ===
Response: Ok

=== Testing GET ===
Response: Value { value: b"world" }
Value as string: world

=== Testing GET (non-existent key) ===
Response: NotFound

=== Testing DELETE ===
Response: Ok

=== Verifying deletion ===
Response: NotFound

✅ All tests completed!
```

## Writing Custom Clients

### Basic client structure

```rust
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use server_tcp::{Request, Response};

async fn connect() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to server
    let stream = TcpStream::connect("127.0.0.1:5500").await?;
    stream.set_nodelay(true)?;

    // 2. Set up codec (same config as server!)
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(8 * 1024 * 1024)
        .new_codec();

    let mut framed = Framed::new(stream, codec);

    // 3. Send request
    let request = Request::Ping;
    framed.send(request.encode()).await?;

    // 4. Receive response
    if let Some(frame) = framed.next().await {
        let response = Response::decode(frame?.freeze())?;
        println!("Response: {:?}", response);
    }

    Ok(())
}
```

### Key points for client implementation

1. **Use the same codec configuration** as the server
   - 4-byte length field
   - Big-endian
   - 8 MB max frame size

2. **Encode before sending**
   ```rust
   let bytes = request.encode();
   framed.send(bytes).await?;
   ```

3. **Decode after receiving**
   ```rust
   let frame = framed.next().await.unwrap()?;
   let response = Response::decode(frame.freeze())?;
   ```

4. **Handle errors**
   ```rust
   match Response::decode(frame.freeze()) {
       Ok(resp) => println!("{:?}", resp),
       Err(e) => eprintln!("Decode error: {}", e),
   }
   ```

## Performance Characteristics

### Zero-Copy Operations

The protocol uses `Bytes::copy_to_bytes()` which creates a zero-copy slice:

```rust
// This doesn't copy the data, just creates a new Bytes handle
// pointing to the same memory region
let key = buf.copy_to_bytes(key_len);
```

### Memory Layout

When storing in HashMap:

```rust
HashMap<Vec<u8>, Bytes>
        ↑        ↑
        |        └─ Zero-copy reference-counted buffer
        └─ Owned vector (copied once)
```

Keys are converted to `Vec<u8>` (one copy), but values remain as `Bytes` (zero-copy).

### Comparison to Bincode

| Aspect | Bytes Protocol | Bincode |
|--------|---------------|---------|
| Serialization | Manual (explicit) | Automatic |
| Performance | Zero-copy | Serialize/deserialize |
| Protocol visibility | Clear, readable | Opaque |
| Dependencies | bytes only | serde + bincode |
| Debugging | Easy to inspect bytes | Need to understand format |

## Cache Management

The TCP protocol requires cache names to be specified for all data operations (PUT, GET, DELETE). Caches must be created before they can be used.

### Creating Caches

Caches are managed via the HTTP admin API. Use the following HTTP endpoints:

**Create a cache:**
```bash
curl -X POST http://localhost:3000/admin/caches \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my_cache",
    "max_capacity": 1000,
    "eviction_policy": "LRU"
  }'
```

**List all caches:**
```bash
curl http://localhost:3000/admin/caches
```

**Describe a specific cache:**
```bash
curl http://localhost:3000/admin/caches/my_cache
```

**Delete a cache:**
```bash
curl -X DELETE http://localhost:3000/admin/caches/my_cache
```

### Using Caches via TCP

Once a cache is created via HTTP, you can access it via TCP:

```rust
// This will work if "my_cache" was created via HTTP admin API
let put_req = Request::Put {
    cache_name: "my_cache".to_string(),
    key: Bytes::from("key1"),
    value: Bytes::from("value1"),
};
```

### Cache Not Found Error

If you try to access a cache that doesn't exist, you'll receive an error response:

```rust
Response::Error {
    msg: "Cache not found: my_cache".to_string()
}
```

**Solution**: Create the cache first using the HTTP admin API.

### Workflow Example

```bash
# Step 1: Start HTTP server (for admin API)
cargo run --bin server-http

# Step 2: Create a cache
curl -X POST http://localhost:3000/admin/caches \
  -H "Content-Type: application/json" \
  -d '{"name": "test_cache", "max_capacity": 1000}'

# Step 3: Start TCP server
cargo run --bin server-tcp

# Step 4: Use TCP client
cargo run --example simple_client
```

## Error Handling

### Protocol errors

```rust
Request::decode(buf) → Result<Request, String>
Response::decode(buf) → Result<Response, String>
```

Common errors:
- "Empty buffer" - No data received
- "Invalid PUT: missing length fields" - Incomplete message
- "Unknown command: 0xFF" - Invalid command byte

### Server error responses

Server sends `Response::Error` for protocol errors:

```rust
Response::Error {
    msg: "Failed to decode: Invalid command".to_string()
}
```

## Summary

The Bytes-based protocol provides:

1. **Simple binary format** - Easy to understand and implement
2. **Length-delimited framing** - Handles message boundaries automatically
3. **Zero-copy operations** - Efficient memory usage with `Bytes`
4. **Type-safe encoding/decoding** - Compile-time guarantees
5. **Aligns with HTTP layer** - Both use `Bytes` for values

The protocol is production-ready and easy to test!

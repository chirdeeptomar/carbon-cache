use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use server_tcp::{Request, Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server
    let stream = TcpStream::connect("127.0.0.1:5500").await?;
    stream.set_nodelay(true)?;

    // Set up the same codec as the server
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(8 * 1024 * 1024)
        .new_codec();

    let mut framed = Framed::new(stream, codec);

    println!("Connected to server at 127.0.0.1:5500");
    println!("Note: Make sure to create a cache named 'test_cache' first via HTTP admin API");
    println!("Example: curl -X POST http://localhost:3000/admin/caches -H 'Content-Type: application/json' -d '{{\"name\": \"test_cache\", \"max_capacity\": 1000}}'");

    let cache_name = "test-timed";

    // Test PING
    println!("\n=== Testing PING ===");
    let ping_req = Request::Ping;
    framed.send(ping_req.encode()).await?;

    if let Some(frame) = framed.next().await {
        let response = Response::decode(frame?.freeze())?;
        println!("Response: {:?}", response);
    }

    // // Test PUT
    // println!("\n=== Testing PUT ===");
    // let put_req = Request::Put {
    //     cache_name: cache_name.to_string(),
    //     key: Bytes::from("hello"),
    //     value: Bytes::from("world"),
    // };
    // framed.send(put_req.encode()).await?;

    // if let Some(frame) = framed.next().await {
    //     let response = Response::decode(frame?.freeze())?;
    //     println!("Response: {:?}", response);
    //     if matches!(response, Response::Error { .. }) {
    //         println!("\n⚠️  Hint: Create cache '{}' first using HTTP admin API", cache_name);
    //         return Ok(());
    //     }
    // }

    // Test GET
    println!("\n=== Testing GET ===");
    let get_req = Request::Get {
        cache_name: cache_name.to_string(),
        key: Bytes::from("1"),
    };
    framed.send(get_req.encode()).await?;

    if let Some(frame) = framed.next().await {
        let response = Response::decode(frame?.freeze())?;
        println!("Response: {:?}", response);

        if let Response::Value { value } = response {
            println!("Value as string: {}", String::from_utf8_lossy(&value));
        }
    }

    // Test GET (not found)
    println!("\n=== Testing GET (non-existent key) ===");
    let get_req = Request::Get {
        cache_name: cache_name.to_string(),
        key: Bytes::from("nonexistent"),
    };
    framed.send(get_req.encode()).await?;

    if let Some(frame) = framed.next().await {
        let response = Response::decode(frame?.freeze())?;
        println!("Response: {:?}", response);
    }

    // Test DELETE
    println!("\n=== Testing DELETE ===");
    let delete_req = Request::Delete {
        cache_name: cache_name.to_string(),
        key: Bytes::from("hello"),
    };
    framed.send(delete_req.encode()).await?;

    if let Some(frame) = framed.next().await {
        let response = Response::decode(frame?.freeze())?;
        println!("Response: {:?}", response);
    }

    // Verify deletion
    println!("\n=== Verifying deletion ===");
    let get_req = Request::Get {
        cache_name: cache_name.to_string(),
        key: Bytes::from("hello"),
    };
    framed.send(get_req.encode()).await?;

    if let Some(frame) = framed.next().await {
        let response = Response::decode(frame?.freeze())?;
        println!("Response: {:?}", response);
    }

    println!("\n✅ All tests completed!");
    Ok(())
}

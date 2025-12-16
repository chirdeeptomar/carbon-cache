use bytes::Bytes;
use carbon::planes::data::{
    cache_operations::CacheOperationsService,
    operation::CacheOperations,
};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use std::sync::Arc;
use crate::protocol::{Request, Response};
use tracing::info;

pub async fn process_connection(
    socket: TcpStream,
    cache_ops: Arc<CacheOperationsService<Vec<u8>, Bytes>>
) -> Result<(), Box<dyn std::error::Error>> {
    socket.set_nodelay(true).ok();

    // Build a length-delimited codec with a 4-byte big-endian length prefix.
    // This handles framing - splitting the TCP stream into discrete messages
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(8 * 1024 * 1024)
        .new_codec();

    // Wrap the socket with the codec - now we get BytesMut frames instead of raw bytes
    let mut framed = Framed::new(socket, codec);

    // Process each frame (message) from the client
    while let Some(frame_result) = framed.next().await {
        // LengthDelimitedCodec gives us BytesMut
        let frame = frame_result?;

        // Convert BytesMut to Bytes and decode into our Request enum
        let request = match Request::decode(frame.freeze()) {
            Ok(req) => req,
            Err(e) => {
                tracing::error!("Failed to decode request: {}", e);
                let error_resp = Response::Error { msg: e };
                framed.send(error_resp.encode()).await?;
                continue;
            }
        };

        info!("Received request: {:?}", request);

        // Process the request and generate a response
        let response = match request {
            Request::Ping => Response::Pong,

            Request::Put { cache_name, key, value } => {
                match cache_ops.put(&cache_name, key.to_vec(), value).await {
                    Ok(_) => Response::Ok,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {}", name) }
                    }
                    Err(e) => {
                        Response::Error { msg: format!("Put failed: {}", e) }
                    }
                }
            }

            Request::Get { cache_name, key } => {
                match cache_ops.get(&cache_name, &key.to_vec()).await {
                    Ok(get_resp) if get_resp.found => {
                        Response::Value { value: get_resp.message }
                    }
                    Ok(_) => {
                        Response::NotFound
                    }
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {}", name) }
                    }
                    Err(e) => {
                        Response::Error { msg: format!("Get failed: {}", e) }
                    }
                }
            }

            Request::Delete { cache_name, key } => {
                match cache_ops.delete(&cache_name, &key.to_vec()).await {
                    Ok(_) => Response::Ok,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {}", name) }
                    }
                    Err(e) => {
                        Response::Error { msg: format!("Delete failed: {}", e) }
                    }
                }
            }
        };

        // Encode the response and send it back
        framed.send(response.encode()).await?;
    }

    Ok(())
}

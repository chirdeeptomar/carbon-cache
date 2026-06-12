use crate::protocol::{Request, Response};
use bytes::Bytes;
use carbon::planes::data::operation::CacheOperations;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::info;

fn make_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(8 * 1024 * 1024)
        .new_codec()
}

pub async fn process_connection(
    socket: TcpStream,
    ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
) -> Result<(), Box<dyn std::error::Error>> {
    socket.set_nodelay(true).ok();

    let mut framed = Framed::new(socket, make_codec());

    while let Some(frame_result) = framed.next().await {
        let frame = frame_result?;

        let request = match Request::decode(frame.freeze()) {
            Ok(req) => req,
            Err(e) => {
                tracing::error!("Failed to decode request: {}", e);
                framed.send(Response::Error { msg: e }.encode()).await?;
                continue;
            }
        };

        info!("Received request: {:?}", request);

        let response = match request {
            Request::Ping => Response::Pong,

            Request::Put { cache_name, key, value } => {
                match ops.put(&cache_name, key.to_vec(), value).await {
                    Ok(_) => Response::Ok,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {name}") }
                    }
                    Err(e) => Response::Error { msg: format!("Put failed: {e}") },
                }
            }

            Request::Get { cache_name, key } => {
                match ops.get(&cache_name, &key.to_vec()).await {
                    Ok(r) if r.found => Response::Value { value: r.message },
                    Ok(_) => Response::NotFound,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {name}") }
                    }
                    Err(e) => Response::Error { msg: format!("Get failed: {e}") },
                }
            }

            Request::Delete { cache_name, key } => {
                match ops.delete(&cache_name, &key.to_vec()).await {
                    Ok(_) => Response::Ok,
                    Err(shared::Error::CacheNotFound(name)) => {
                        Response::Error { msg: format!("Cache not found: {name}") }
                    }
                    Err(e) => Response::Error { msg: format!("Delete failed: {e}") },
                }
            }
        };

        framed.send(response.encode()).await?;
    }

    Ok(())
}

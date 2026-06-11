use crate::protocol::{Request, Response};
use bytes::Bytes;
use carbon::planes::data::operation::CacheOperations;
use carbon_raft::node::RaftCacheNode;
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

/// Forward a raw TCP frame to the leader's TCP address and return the response frame.
async fn forward_frame_to_leader(leader_tcp_addr: &str, raw_frame: Bytes) -> Result<Bytes, String> {
    let addr = leader_tcp_addr.replace("0.0.0.0", "127.0.0.1");
    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("connect to leader TCP {addr}: {e}"))?;
    stream.set_nodelay(true).ok();

    let mut framed = Framed::new(stream, make_codec());
    framed
        .send(raw_frame)
        .await
        .map_err(|e| format!("send frame to leader: {e}"))?;

    match framed.next().await {
        Some(Ok(resp)) => Ok(resp.freeze()),
        Some(Err(e)) => Err(format!("receive from leader: {e}")),
        None => Err("leader closed connection".to_string()),
    }
}

pub async fn process_connection(
    socket: TcpStream,
    raft_node: Arc<RaftCacheNode>,
) -> Result<(), Box<dyn std::error::Error>> {
    socket.set_nodelay(true).ok();

    let mut framed = Framed::new(socket, make_codec());

    while let Some(frame_result) = framed.next().await {
        let frame = frame_result?;
        let raw = frame.clone().freeze();

        let request = match Request::decode(raw.clone()) {
            Ok(req) => req,
            Err(e) => {
                tracing::error!("Failed to decode request: {}", e);
                framed.send(Response::Error { msg: e }.encode()).await?;
                continue;
            }
        };

        info!("Received request: {:?}", request);

        // For write commands, forward to leader if we are not it.
        let is_write = matches!(request, Request::Put { .. } | Request::Delete { .. });
        if is_write && !raft_node.is_leader() {
            if let Some(leader) = raft_node.get_leader_node() {
                match forward_frame_to_leader(&leader.tcp_addr, raw).await {
                    Ok(resp_bytes) => framed.send(resp_bytes).await?,
                    Err(e) => {
                        framed
                            .send(Response::Error { msg: format!("leader forward failed: {e}") }.encode())
                            .await?
                    }
                }
                continue;
            }
        }

        let ops: &dyn CacheOperations<Vec<u8>, Bytes> = &*raft_node;

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

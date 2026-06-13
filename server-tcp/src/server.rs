use crate::{
    forward_to_leader, make_codec,
    protocol::{Request, Response},
};
use bytes::Bytes;
use carbon::planes::data::operation::CacheOperations;
use carbon_raft::node::RaftCacheNode;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tracing::info;

pub async fn process_connection(
    socket: TcpStream,
    cache_ops: Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
    raft_node: Option<Arc<RaftCacheNode>>,
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

            Request::Get { cache_name, key } => {
                match cache_ops.get(&cache_name, &key.to_vec()).await {
                    Ok(r) if r.found => Response::Value { value: r.message },
                    Ok(_) => Response::NotFound,
                    Err(shared::Error::CacheNotFound(name)) => Response::Error {
                        msg: format!("Cache not found: {name}"),
                    },
                    Err(e) => Response::Error {
                        msg: format!("Get failed: {e}"),
                    },
                }
            }

            Request::Put {
                cache_name,
                key,
                value,
            } => {
                if let Some(raft) = &raft_node {
                    if raft.is_leader() {
                        do_put(&cache_ops, cache_name, key, value).await
                    } else {
                        forward_to_leader(
                            raft,
                            Request::Put {
                                cache_name,
                                key,
                                value,
                            },
                        )
                        .await
                    }
                } else {
                    do_put(&cache_ops, cache_name, key, value).await
                }
            }

            Request::Delete { cache_name, key } => {
                if let Some(raft) = &raft_node {
                    if raft.is_leader() {
                        do_delete(&cache_ops, cache_name, key).await
                    } else {
                        forward_to_leader(raft, Request::Delete { cache_name, key }).await
                    }
                } else {
                    do_delete(&cache_ops, cache_name, key).await
                }
            }
        };

        framed.send(response.encode()).await?;
    }

    Ok(())
}

async fn do_put(
    cache_ops: &Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
    cache_name: String,
    key: Bytes,
    value: Bytes,
) -> Response {
    match cache_ops.put(&cache_name, key.to_vec(), value).await {
        Ok(_) => Response::Ok,
        Err(shared::Error::CacheNotFound(name)) => Response::Error {
            msg: format!("Cache not found: {name}"),
        },
        Err(e) => Response::Error {
            msg: format!("Put failed: {e}"),
        },
    }
}

async fn do_delete(
    cache_ops: &Arc<dyn CacheOperations<Vec<u8>, Bytes>>,
    cache_name: String,
    key: Bytes,
) -> Response {
    match cache_ops.delete(&cache_name, &key.to_vec()).await {
        Ok(_) => Response::Ok,
        Err(shared::Error::CacheNotFound(name)) => Response::Error {
            msg: format!("Cache not found: {name}"),
        },
        Err(e) => Response::Error {
            msg: format!("Delete failed: {e}"),
        },
    }
}

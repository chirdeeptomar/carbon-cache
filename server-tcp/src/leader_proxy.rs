use std::sync::Arc;

use carbon_raft::node::RaftCacheNode;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::info;

use crate::{Request, Response};

pub async fn forward_to_leader(raft: &Arc<RaftCacheNode>, req: Request) -> Response {
    let leader = match raft.get_leader_node() {
        Some(n) => n,
        None => {
            return Response::Error {
                msg: "no leader elected".to_string(),
            };
        }
    };

    let addr = leader.tcp_addr.replace("0.0.0.0", "127.0.0.1");
    info!("Forwarding TCP request to leader at {}", addr);

    let stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            return Response::Error {
                msg: format!("leader unreachable: {e}"),
            };
        }
    };
    stream.set_nodelay(true).ok();

    let mut framed = Framed::new(stream, make_codec());

    if let Err(e) = framed.send(req.encode()).await {
        return Response::Error {
            msg: format!("failed to send to leader: {e}"),
        };
    }

    match framed.next().await {
        Some(Ok(frame)) => match Response::decode(frame.freeze()) {
            Ok(resp) => resp,
            Err(e) => Response::Error {
                msg: format!("invalid response from leader: {e}"),
            },
        },
        Some(Err(e)) => Response::Error {
            msg: format!("leader read error: {e}"),
        },
        None => Response::Error {
            msg: "leader closed connection".to_string(),
        },
    }
}

pub fn make_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .length_field_length(4)
        .max_frame_length(8 * 1024 * 1024)
        .new_codec()
}

use std::sync::Arc;

use openraft::{
    error::{InstallSnapshotError, RPCError, RaftError},
    network::{RPCOption, RaftNetwork, RaftNetworkFactory},
    raft::{
        AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
        InstallSnapshotResponse, VoteRequest, VoteResponse,
    },
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::rpc::{RaftRpcMessage, RaftRpcResponse};
use crate::types::{CarbonNode, NodeId, TypeConfig};

/// Persistent TCP connection to a single peer node.
///
/// The connection is created lazily on the first RPC call and reused for
/// subsequent calls. If the connection drops (write or read error), it is
/// cleared so the next call re-dials.
pub struct CarbonRaftNetwork {
    target_addr: String,
    conn: Arc<Mutex<Option<TcpStream>>>,
}

impl CarbonRaftNetwork {
    /// Send a message and return the response, reconnecting if necessary.
    async fn send(&self, msg: &RaftRpcMessage) -> Result<RaftRpcResponse, String> {
        let mut guard = self.conn.lock().await;
        let stream = match &mut *guard {
            Some(s) => s,
            slot @ None => {
                let s = TcpStream::connect(&self.target_addr)
                    .await
                    .map_err(|e| format!("connect to {}: {}", self.target_addr, e))?;
                slot.insert(s)
            }
        };

        let payload = serde_json::to_vec(msg).map_err(|e| e.to_string())?;
        let len = payload.len() as u32;

        if let Err(e) = stream.write_all(&len.to_be_bytes()).await {
            *guard = None;
            return Err(format!("write len: {e}"));
        }
        if let Err(e) = stream.write_all(&payload).await {
            *guard = None;
            return Err(format!("write payload: {e}"));
        }

        let mut len_buf = [0u8; 4];
        if let Err(e) = stream.read_exact(&mut len_buf).await {
            *guard = None;
            return Err(format!("read response len: {e}"));
        }
        let resp_len = u32::from_be_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        if let Err(e) = stream.read_exact(&mut resp_buf).await {
            *guard = None;
            return Err(format!("read response: {e}"));
        }

        serde_json::from_slice(&resp_buf).map_err(|e| e.to_string())
    }
}

/// Creates a [`CarbonRaftNetwork`] for each peer when openraft needs to dial a node.
pub struct CarbonRaftNetworkFactory;

impl RaftNetworkFactory<TypeConfig> for CarbonRaftNetworkFactory {
    type Network = CarbonRaftNetwork;

    async fn new_client(&mut self, _target: NodeId, node: &CarbonNode) -> Self::Network {
        CarbonRaftNetwork {
            target_addr: node.raft_addr.clone(),
            conn: Arc::new(Mutex::new(None)),
        }
    }
}

impl RaftNetwork<TypeConfig> for CarbonRaftNetwork {
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfig>,
        _opt: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, CarbonNode, RaftError<NodeId>>>
    {
        match self.send(&RaftRpcMessage::AppendEntries(req)).await {
            Ok(RaftRpcResponse::AppendEntries(resp)) => Ok(resp),
            Ok(other) => Err(RPCError::Network(openraft::error::NetworkError::new(
                &std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unexpected response: {:?}", other),
                ),
            ))),
            Err(e) => Err(RPCError::Network(openraft::error::NetworkError::new(
                &std::io::Error::other(e),
            ))),
        }
    }

    async fn vote(
        &mut self,
        req: VoteRequest<NodeId>,
        _opt: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, CarbonNode, RaftError<NodeId>>> {
        match self.send(&RaftRpcMessage::Vote(req)).await {
            Ok(RaftRpcResponse::Vote(resp)) => Ok(resp),
            Ok(other) => Err(RPCError::Network(openraft::error::NetworkError::new(
                &std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unexpected response: {:?}", other),
                ),
            ))),
            Err(e) => Err(RPCError::Network(openraft::error::NetworkError::new(
                &std::io::Error::other(e),
            ))),
        }
    }

    async fn install_snapshot(
        &mut self,
        req: InstallSnapshotRequest<TypeConfig>,
        _opt: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, CarbonNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
        match self.send(&RaftRpcMessage::InstallSnapshot(req)).await {
            Ok(RaftRpcResponse::InstallSnapshot(resp)) => Ok(resp),
            Ok(other) => Err(RPCError::Network(openraft::error::NetworkError::new(
                &std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unexpected response: {:?}", other),
                ),
            ))),
            Err(e) => Err(RPCError::Network(openraft::error::NetworkError::new(
                &std::io::Error::other(e),
            ))),
        }
    }
}

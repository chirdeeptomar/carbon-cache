//! Wire protocol for inter-node Raft RPC communication.
//!
//! All messages are serialized as JSON and framed with a 4-byte big-endian
//! length prefix over a persistent TCP connection. The same framing is used
//! in both directions (request and response).
//!
//! Frame layout:
//! ```text
//! ┌──────────────────┬──────────────────────────┐
//! │  length: u32 BE  │  JSON payload (N bytes)  │
//! └──────────────────┴──────────────────────────┘
//! ```

use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use serde::{Deserialize, Serialize};

use crate::types::{NodeId, TypeConfig};

/// Messages a node can send to a peer over the Raft RPC channel.
///
/// The first three variants are standard openraft RPCs forwarded verbatim.
/// `Join` and `Leave` are Carbon-specific extensions for cluster membership.
#[derive(Serialize, Deserialize, Debug)]
pub enum RaftRpcMessage {
    AppendEntries(AppendEntriesRequest<TypeConfig>),
    Vote(VoteRequest<NodeId>),
    InstallSnapshot(InstallSnapshotRequest<TypeConfig>),
    /// Sent by a new node to a seed to request admission into the cluster.
    /// The seed (or the leader it redirects to) will call `add_learner` then
    /// `change_membership` to promote the joining node to a voter.
    Join {
        node_id: NodeId,
        raft_addr: String,
        http_addr: String,
        tcp_addr: String,
    },
    /// Sent by a node that is shutting down to remove itself from membership.
    /// The receiver (or the leader it redirects to) will call `change_membership`
    /// to remove the departing node from the voter set.
    Leave {
        node_id: NodeId,
    },
}

/// Responses returned over the Raft RPC channel.
#[derive(Serialize, Deserialize, Debug)]
pub enum RaftRpcResponse {
    AppendEntries(AppendEntriesResponse<NodeId>),
    Vote(VoteResponse<NodeId>),
    InstallSnapshot(InstallSnapshotResponse<NodeId>),
    /// Membership change committed — the joining node is now a voter.
    Joined,
    /// Membership change committed — the departing node has been removed.
    Left,
    /// The receiver is not the leader. The caller should retry the request
    /// at `leader_addr` (the leader's `raft_addr`).
    NotLeader {
        leader_addr: String,
    },
    /// An unexpected error occurred; the message describes the cause.
    Error {
        message: String,
    },
}

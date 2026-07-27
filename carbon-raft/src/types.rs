use carbon::auth::{Role, User};
use carbon::domain::CacheConfig;
use openraft::declare_raft_types;
use serde::{Deserialize, Serialize};

/// Unique node identifier. Must be stable across restarts for a given node.
pub type NodeId = u64;

/// Per-node metadata stored in Raft membership.
///
/// Carried inside every membership log entry so any node in the cluster can
/// find the three addresses it needs to talk to a peer:
/// - `raft_addr` — internal Raft RPC port (TCP, not exposed to clients)
/// - `http_addr` — HTTP API port (used by followers to forward writes to the leader)
/// - `tcp_addr` — binary cache protocol port (for client connections)
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct CarbonNode {
    pub raft_addr: String,
    pub http_addr: String,
    pub tcp_addr: String,
}

impl std::fmt::Display for CarbonNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.raft_addr)
    }
}

/// Every mutation that flows through the Raft log.
///
/// Reads are deliberately absent — they are served directly from the local
/// state machine replica without going through consensus.
///
/// Variants are grouped by plane:
/// - **Cache control plane**: create/drop a named cache namespace
/// - **Cache data plane**: key-value writes within a cache
/// - **Auth control plane**: user and role lifecycle
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RaftLogEntry {
    // Cache control plane
    CreateCache(CacheConfig),
    DropCache { name: String },

    // Cache data plane — writes only
    Put { cache_name: String, key: Vec<u8>, value: Vec<u8> },
    Delete { cache_name: String, key: Vec<u8> },

    // Auth control plane
    CreateUser(User),
    UpdateUser(User),
    DeleteUser { id: String },
    CreateRole(Role),
    UpdateRole(Role),
    DeleteRole { id: String },
}

/// The value returned to the caller after a log entry is applied.
///
/// Each variant corresponds to a [`RaftLogEntry`] variant and carries enough
/// information for the HTTP handler to build its response without re-reading
/// the state machine.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RaftResponse {
    Ok,
    Written { created: bool },
    Deleted { existed: bool },
    CacheCreated { created: bool },
    CacheDropped { dropped: bool },
    UserCreated { user: User },
    UserUpdated { user: User },
    UserDeleted { existed: bool },
    RoleCreated { role: Role },
    RoleUpdated { role: Role },
    RoleDeleted { existed: bool },
}

// Binds all Carbon-specific types into the generic openraft engine.
declare_raft_types!(
    pub TypeConfig:
        D  = RaftLogEntry,
        R  = RaftResponse,
        NodeId = NodeId,
        Node = CarbonNode,
        Entry = openraft::Entry<TypeConfig>,
        SnapshotData = std::io::Cursor<Vec<u8>>,
        AsyncRuntime = openraft::TokioRuntime,
);

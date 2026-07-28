use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use carbon::domain::response::admin::{
    CreateCacheResponse, DescribeCacheResponse, DropCacheResponse, ListCachesResponse,
};
use carbon::domain::response::{DeleteResponse, GetResponse, PutResponse};
use carbon::domain::{CacheConfig, CacheInfo};
use carbon::planes::control::operation::AdminOperations;
use carbon::planes::data::operation::CacheOperations;
use carbon::ports::CacheStore;
use openraft::storage::Adaptor;
use openraft::{Config, Raft};
use shared::Error as SharedError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use carbon::auth::{Role, User};

use crate::log_store::CarbonRaftStorage;
use crate::network::CarbonRaftNetworkFactory;
use crate::rpc::{RaftRpcMessage, RaftRpcResponse};
use crate::state_machine::CacheStateMachine;
use crate::types::{CarbonNode, NodeId, RaftLogEntry, RaftResponse, TypeConfig};

/// Type alias for the openraft `Raft` instance parameterised with Carbon's type config.
pub type CarbonRaft = Raft<TypeConfig>;

/// The central handle for a single cluster node.
///
/// Owns the openraft instance and a shared reference to the in-memory state
/// machine. Created once at server startup via [`RaftCacheNode::new`] and
/// stored in `AppState`.
///
/// # Responsibilities
/// - Starts the Raft RPC TCP listener (binds `raft_addr`)
/// - Bootstraps or joins the cluster on first run
/// - Exposes write operations that go through Raft consensus
/// - Serves read operations directly from the local state machine replica
/// - Implements [`CacheOperations`] so HTTP handlers can use it as a drop-in
pub struct RaftCacheNode {
    pub raft: Arc<CarbonRaft>,
    pub state: Arc<RwLock<CacheStateMachine>>,
}

impl RaftCacheNode {
    /// Create a new node, start the RPC listener, and begin bootstrap/join.
    ///
    /// Returns immediately after spawning background tasks; the node may not
    /// be fully joined to the cluster by the time this returns.
    pub async fn new(
        node_id: NodeId,
        raft_addr: String,
        http_addr: String,
        tcp_addr: String,
        seeds: Vec<String>,
        storage: CarbonRaftStorage,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let state = storage.sm.clone();

        let config = Arc::new(
            Config {
                heartbeat_interval: 250,
                election_timeout_min: 299,
                election_timeout_max: 500,
                ..Default::default()
            }
            .validate()?,
        );

        let (log_store, sm_store) = Adaptor::new(storage);

        let raft = Arc::new(
            Raft::new(
                node_id,
                config,
                CarbonRaftNetworkFactory,
                log_store,
                sm_store,
            )
            .await?,
        );

        let node = Arc::new(RaftCacheNode {
            raft: raft.clone(),
            state,
        });

        let raft_for_listener = raft.clone();
        let addr = raft_addr.clone();
        tokio::spawn(async move {
            run_rpc_listener(raft_for_listener, addr).await;
        });

        let raft_for_join = raft.clone();
        tokio::spawn(async move {
            bootstrap(
                raft_for_join,
                node_id,
                raft_addr,
                http_addr,
                tcp_addr,
                seeds,
            )
            .await;
        });

        Ok(node)
    }

    /// Submit a write through Raft consensus.
    ///
    /// Must only be called on the leader. Followers should use
    /// [`get_leader_node`](Self::get_leader_node) to obtain the leader's
    /// address and proxy the request over HTTP instead.
    pub async fn write(
        &self,
        entry: RaftLogEntry,
    ) -> Result<RaftResponse, Box<dyn std::error::Error>> {
        let resp = self.raft.client_write(entry).await?;
        Ok(resp.data)
    }

    /// Returns `true` if this node is the current Raft leader.
    pub fn is_leader(&self) -> bool {
        let m = self.raft.metrics().borrow().clone();
        m.current_leader == Some(m.id)
    }

    /// Returns the [`CarbonNode`] metadata for the current leader, or `None`
    /// if no leader has been elected yet.
    pub fn get_leader_node(&self) -> Option<CarbonNode> {
        let metrics = self.raft.metrics().borrow().clone();
        metrics.current_leader.and_then(|leader_id| {
            metrics
                .membership_config
                .nodes()
                .find(|(id, _)| **id == leader_id)
                .map(|(_, node)| node.clone())
        })
    }

    pub fn get_all_nodes(&self) -> Vec<(NodeId, CarbonNode)> {
        let metrics = self.raft.metrics().borrow().clone();
        metrics
            .membership_config
            .nodes()
            .map(|(id, node)| (*id, node.clone()))
            .collect()
    }

    /// Remove a node from cluster membership.
    ///
    /// Only succeeds on the leader. Returns an error string on failure (not leader,
    /// node not in membership, or Raft error). Callers should check `is_leader()`
    /// or handle the error and retry against the correct node.
    pub async fn remove_node(&self, target_id: NodeId) -> Result<(), String> {
        if !self.is_leader() {
            return Err("not the leader".into());
        }
        let metrics = self.raft.metrics().borrow().clone();
        let current: Vec<NodeId> = metrics.membership_config.voter_ids().collect();
        if !current.contains(&target_id) {
            return Err(format!("node {target_id} is not a cluster member"));
        }
        let new_members: Vec<NodeId> = current.into_iter().filter(|id| *id != target_id).collect();
        self.raft
            .change_membership(new_members, false)
            .await
            .map(|_| ())
            .map_err(|e| format!("change_membership: {e}"))
    }

    /// Remove this node from cluster membership before shutdown.
    ///
    /// If this node is the leader, it steps down directly via `change_membership`.
    /// If it is a follower, it sends a [`RaftRpcMessage::Leave`] RPC to the leader.
    /// Either way, after this returns the voter set no longer includes this node.
    pub async fn leave_cluster(&self) -> Result<(), String> {
        let my_id = self.node_id();
        let metrics = self.raft.metrics().borrow().clone();
        let voter_count = metrics.membership_config.voter_ids().count();

        // Refuse to leave if doing so would drop the cluster below quorum (2 voters).
        // A single-node leader removing itself destroys the cluster permanently.
        if voter_count <= 2 {
            return Err(format!(
                "refusing to leave: only {voter_count} voter(s) in cluster, removal would break quorum"
            ));
        }

        if self.is_leader() {
            self.remove_node(my_id).await
        } else {
            match self.get_leader_node() {
                Some(leader) => send_leave(leader.raft_addr, my_id).await,
                None => Err("no leader elected — cannot send Leave".into()),
            }
        }
    }

    pub fn current_leader_id(&self) -> Option<NodeId> {
        self.raft.metrics().borrow().current_leader
    }

    pub fn node_id(&self) -> NodeId {
        self.raft.metrics().borrow().id
    }

    // --- Auth reads (served locally from state machine) ---

    pub async fn get_user_by_id(&self, id: &str) -> Option<User> {
        self.state.read().await.get_user_by_id(id).cloned()
    }

    pub async fn get_user_by_username(&self, username: &str) -> Option<User> {
        self.state
            .read()
            .await
            .get_user_by_username(username)
            .cloned()
    }

    pub async fn username_exists(&self, username: &str) -> bool {
        self.state.read().await.username_exists(username)
    }

    pub async fn list_users(&self) -> Vec<User> {
        self.state.read().await.list_users()
    }

    pub async fn get_role_by_id(&self, id: &str) -> Option<Role> {
        self.state.read().await.get_role_by_id(id).cloned()
    }

    pub async fn get_role_by_name(&self, name: &str) -> Option<Role> {
        self.state.read().await.get_role_by_name(name).cloned()
    }

    pub async fn list_roles(&self) -> Vec<Role> {
        self.state.read().await.list_roles()
    }

    pub async fn get_roles_by_ids(&self, ids: &[String]) -> Vec<Role> {
        self.state.read().await.get_roles_by_ids(ids)
    }
}

/// Implement CacheOperations so RaftCacheNode can be used directly as a drop-in
/// for the existing HTTP handlers. PUT/DELETE go through Raft consensus; GET
/// is served locally from the state machine.
#[async_trait]
impl CacheOperations<Vec<u8>, Bytes> for RaftCacheNode {
    async fn put(
        &self,
        cache_name: &str,
        key: Vec<u8>,
        value: Bytes,
    ) -> shared::Result<PutResponse> {
        let entry = RaftLogEntry::Put {
            cache_name: cache_name.to_string(),
            key,
            value: value.to_vec(),
        };
        match self.write(entry).await {
            Ok(RaftResponse::Written { created }) => Ok(PutResponse::new(
                created,
                if created { "inserted" } else { "updated" },
            )),
            Ok(_) => Ok(PutResponse::new(false, "ok")),
            Err(e) => Err(SharedError::Internal(e.to_string())),
        }
    }

    async fn get(&self, cache_name: &str, key: &Vec<u8>) -> shared::Result<GetResponse<Bytes>> {
        let sm = self.state.read().await;
        if !sm.configs.contains_key(cache_name) {
            return Err(SharedError::CacheNotFound(cache_name.to_string()));
        }
        match sm.get(cache_name, key) {
            Some(v) => Ok(GetResponse::new(true, Bytes::from(v))),
            None => Err(SharedError::KeyNotFound),
        }
    }

    async fn delete(&self, cache_name: &str, key: &Vec<u8>) -> shared::Result<DeleteResponse> {
        let entry = RaftLogEntry::Delete {
            cache_name: cache_name.to_string(),
            key: key.clone(),
        };
        match self.write(entry).await {
            Ok(RaftResponse::Deleted { existed }) => Ok(DeleteResponse::new(existed)),
            Ok(_) => Ok(DeleteResponse::new(false)),
            Err(e) => Err(SharedError::Internal(e.to_string())),
        }
    }
}

#[async_trait]
impl AdminOperations<Vec<u8>, Bytes> for RaftCacheNode {
    async fn create_cache(
        &self,
        config: CacheConfig,
        _store: Arc<dyn CacheStore<Vec<u8>, Bytes>>,
    ) -> shared::Result<CreateCacheResponse> {
        match self.write(RaftLogEntry::CreateCache(config)).await {
            Ok(RaftResponse::CacheCreated { created }) => Ok(CreateCacheResponse::new(
                created,
                if created {
                    "Cache created".to_string()
                } else {
                    "Cache already exists".to_string()
                },
            )),
            Ok(_) => Ok(CreateCacheResponse::new(false, "ok".to_string())),
            Err(e) => Err(SharedError::Internal(e.to_string())),
        }
    }

    async fn drop_cache(&self, name: &str) -> shared::Result<DropCacheResponse> {
        match self
            .write(RaftLogEntry::DropCache {
                name: name.to_string(),
            })
            .await
        {
            Ok(RaftResponse::CacheDropped { dropped }) => Ok(DropCacheResponse::new(dropped)),
            Ok(_) => Ok(DropCacheResponse::new(false)),
            Err(e) => Err(SharedError::Internal(e.to_string())),
        }
    }

    async fn list_caches(&self) -> shared::Result<ListCachesResponse> {
        let sm = self.state.read().await;
        let caches: Vec<CacheInfo> = sm
            .list_configs()
            .into_iter()
            .map(|c| CacheInfo::from_config(&c))
            .collect();
        Ok(ListCachesResponse::new(caches))
    }

    async fn describe_cache(&self, name: &str) -> shared::Result<DescribeCacheResponse> {
        let sm = self.state.read().await;
        match sm.describe_config(name) {
            Some(config) => Ok(DescribeCacheResponse::new(CacheInfo::from_config(&config))),
            None => Err(SharedError::CacheNotFound(name.to_string())),
        }
    }
}

/// Bootstrap or join the cluster.
///
/// Called once from a background task after the RPC listener is up.
/// - **No seeds**: initialises a single-node cluster (node becomes leader).
/// - **With seeds**: sends a [`RaftRpcMessage::Join`] to each seed in order
///   until one succeeds. The seed forwards the request to the leader, which
///   adds this node as a learner and then promotes it to voter.
///
/// The 150 ms initial sleep gives the RPC listener time to bind before
/// outbound join attempts begin.
async fn bootstrap(
    raft: Arc<CarbonRaft>,
    node_id: NodeId,
    raft_addr: String,
    http_addr: String,
    tcp_addr: String,
    seeds: Vec<String>,
) {
    tokio::time::sleep(Duration::from_millis(150)).await;

    if seeds.is_empty() {
        info!("No seeds — initializing single-node cluster");
        let mut members = BTreeMap::new();
        members.insert(
            node_id,
            CarbonNode {
                raft_addr,
                http_addr,
                tcp_addr,
            },
        );
        match raft.initialize(members).await {
            Ok(_) => info!(
                node_id = node_id,
                "cluster membership changed: single-node cluster initialized"
            ),
            Err(e) => warn!("initialize failed (already initialized?): {e}"),
        }
        return;
    }

    info!("Joining cluster via seeds: {:?}", seeds);
    // Retry indefinitely with capped exponential backoff. The node may start before
    // its seeds are ready, or seeds may restart — keep trying until one accepts us.
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        for seed_addr in &seeds {
            match send_join(
                seed_addr.clone(),
                node_id,
                raft_addr.clone(),
                http_addr.clone(),
                tcp_addr.clone(),
            )
            .await
            {
                Ok(()) => {
                    info!("Joined via seed {seed_addr}");
                    return;
                }
                Err(e) => warn!("Join via {seed_addr} failed (attempt {attempt}): {e}"),
            }
        }
        // Backoff: 300ms, 600ms, 900ms … capped at 5s
        let delay_ms = (300 * attempt).min(5000);
        tokio::time::sleep(Duration::from_millis(delay_ms as u64)).await;
    }
}

/// Send a `Join` RPC to `seed_addr`, following `NotLeader` redirects recursively
/// until the leader accepts the request or an error occurs.
fn send_join(
    seed_addr: String,
    node_id: NodeId,
    raft_addr: String,
    http_addr: String,
    tcp_addr: String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> {
    Box::pin(async move {
        let mut stream = TcpStream::connect(&seed_addr)
            .await
            .map_err(|e| format!("connect {seed_addr}: {e}"))?;

        let msg = RaftRpcMessage::Join {
            node_id,
            raft_addr: raft_addr.clone(),
            http_addr: http_addr.clone(),
            tcp_addr: tcp_addr.clone(),
        };
        write_frame(&mut stream, &msg)
            .await
            .map_err(|e| e.to_string())?;

        match read_frame(&mut stream).await.map_err(|e| e.to_string())? {
            RaftRpcResponse::Joined => Ok(()),
            RaftRpcResponse::NotLeader { leader_addr } => {
                send_join(leader_addr, node_id, raft_addr, http_addr, tcp_addr).await
            }
            RaftRpcResponse::Error { message } => Err(message),
            other => Err(format!("unexpected: {other:?}")),
        }
    })
}

/// Accept inbound Raft RPC connections on `addr` and spawn a handler per connection.
async fn run_rpc_listener(raft: Arc<CarbonRaft>, addr: String) {
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind Raft RPC on {addr}: {e}");
            return;
        }
    };
    info!("Raft RPC listening on {addr}");

    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let raft = raft.clone();
                tokio::spawn(async move { handle_conn(raft, stream).await });
            }
            Err(e) => error!("accept error: {e}"),
        }
    }
}

/// Drive a single RPC connection: read frames in a loop until the peer disconnects.
async fn handle_conn(raft: Arc<CarbonRaft>, mut stream: TcpStream) {
    loop {
        let msg = match read_frame(&mut stream).await {
            Ok(m) => m,
            Err(_) => break,
        };
        let resp = dispatch(&raft, msg).await;
        if write_frame(&mut stream, &resp).await.is_err() {
            break;
        }
    }
}

async fn dispatch(raft: &CarbonRaft, msg: RaftRpcMessage) -> RaftRpcResponse {
    match msg {
        RaftRpcMessage::AppendEntries(req) => match raft.append_entries(req).await {
            Ok(r) => RaftRpcResponse::AppendEntries(r),
            Err(e) => RaftRpcResponse::Error {
                message: e.to_string(),
            },
        },
        RaftRpcMessage::Vote(req) => match raft.vote(req).await {
            Ok(r) => RaftRpcResponse::Vote(r),
            Err(e) => RaftRpcResponse::Error {
                message: e.to_string(),
            },
        },
        RaftRpcMessage::InstallSnapshot(req) => {
            use openraft::Snapshot;
            use std::io::Cursor;
            let snapshot = Snapshot {
                meta: req.meta.clone(),
                snapshot: Box::new(Cursor::new(req.data)),
            };
            match raft.install_full_snapshot(req.vote, snapshot).await {
                Ok(r) => {
                    RaftRpcResponse::InstallSnapshot(openraft::raft::InstallSnapshotResponse {
                        vote: r.vote,
                    })
                }
                Err(e) => RaftRpcResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        RaftRpcMessage::Join {
            node_id,
            raft_addr,
            http_addr,
            tcp_addr,
        } => handle_join(raft, node_id, raft_addr, http_addr, tcp_addr).await,
        RaftRpcMessage::Leave { node_id } => handle_leave(raft, node_id).await,
    }
}

/// Handle an inbound `Join` request on the leader.
///
/// If this node is not the leader, returns [`RaftRpcResponse::NotLeader`] with
/// the leader's `raft_addr` so the joining node can retry directly.
///
/// On success: adds the new node as a learner, then promotes it to voter via
/// `change_membership`. The existing voter set is preserved — only the new
/// node is appended.
async fn handle_join(
    raft: &CarbonRaft,
    node_id: NodeId,
    raft_addr: String,
    http_addr: String,
    tcp_addr: String,
) -> RaftRpcResponse {
    let metrics = raft.metrics().borrow().clone();

    if let Some(leader_id) = metrics.current_leader {
        if leader_id != metrics.id {
            if let Some(leader_addr) = metrics
                .membership_config
                .nodes()
                .find(|(id, _)| **id == leader_id)
                .map(|(_, n)| n.raft_addr.clone())
            {
                return RaftRpcResponse::NotLeader { leader_addr };
            }
        }
    }

    let node = CarbonNode {
        raft_addr,
        http_addr,
        tcp_addr,
    };
    if let Err(e) = raft.add_learner(node_id, node, true).await {
        return RaftRpcResponse::Error {
            message: format!("add_learner: {e}"),
        };
    }

    // Re-read membership after add_learner commits and retry change_membership
    // if the cluster is mid-change (two joins arriving close together).
    for attempt in 1u32..=5 {
        let current: Vec<NodeId> = raft
            .metrics()
            .borrow()
            .membership_config
            .voter_ids()
            .collect();
        let mut new_members = current;
        if !new_members.contains(&node_id) {
            new_members.push(node_id);
        }
        match raft.change_membership(new_members.clone(), false).await {
            Ok(_) => {
                info!(
                    node_id = node_id,
                    members = ?new_members,
                    "cluster membership changed: node joined"
                );
                return RaftRpcResponse::Joined;
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("already undergoing a configuration change") && attempt < 5 {
                    tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                    continue;
                }
                return RaftRpcResponse::Error {
                    message: format!("change_membership: {e}"),
                };
            }
        }
    }
    RaftRpcResponse::Error {
        message: "change_membership: too many retries".into(),
    }
}

/// Handle an inbound `Leave` request.
///
/// If this node is not the leader, returns [`RaftRpcResponse::NotLeader`] so the
/// caller can retry at the leader's `raft_addr`. On success the departing node
/// is removed from the voter set via `change_membership`.
async fn handle_leave(raft: &CarbonRaft, node_id: NodeId) -> RaftRpcResponse {
    let metrics = raft.metrics().borrow().clone();

    if let Some(leader_id) = metrics.current_leader {
        if leader_id != metrics.id {
            if let Some(leader_addr) = metrics
                .membership_config
                .nodes()
                .find(|(id, _)| **id == leader_id)
                .map(|(_, n)| n.raft_addr.clone())
            {
                return RaftRpcResponse::NotLeader { leader_addr };
            }
        }
    }

    let current: Vec<NodeId> = metrics.membership_config.voter_ids().collect();
    let new_members: Vec<NodeId> = current.into_iter().filter(|id| *id != node_id).collect();

    match raft.change_membership(new_members.clone(), false).await {
        Ok(_) => {
            info!(
                node_id = node_id,
                members = ?new_members,
                "cluster membership changed: node left"
            );
            RaftRpcResponse::Left
        }
        Err(e) => RaftRpcResponse::Error {
            message: format!("change_membership: {e}"),
        },
    }
}

/// Send a `Leave` RPC to `leader_addr`, following `NotLeader` redirects until
/// the leader accepts or an error occurs.
fn send_leave(
    leader_addr: String,
    node_id: NodeId,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> {
    Box::pin(async move {
        let mut stream = TcpStream::connect(&leader_addr)
            .await
            .map_err(|e| format!("connect {leader_addr}: {e}"))?;

        write_frame(&mut stream, &RaftRpcMessage::Leave { node_id })
            .await
            .map_err(|e| e.to_string())?;

        match read_frame(&mut stream).await.map_err(|e| e.to_string())? {
            RaftRpcResponse::Left => Ok(()),
            RaftRpcResponse::NotLeader { leader_addr } => send_leave(leader_addr, node_id).await,
            RaftRpcResponse::Error { message } => Err(message),
            other => Err(format!("unexpected: {other:?}")),
        }
    })
}

/// Write a length-prefixed JSON frame to `stream`.
async fn write_frame<T: serde::Serialize>(
    stream: &mut TcpStream,
    msg: &T,
) -> Result<(), std::io::Error> {
    let payload = serde_json::to_vec(msg).map_err(std::io::Error::other)?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&payload).await?;
    Ok(())
}

/// Read a length-prefixed JSON frame from `stream`.
async fn read_frame<T: serde::de::DeserializeOwned>(
    stream: &mut TcpStream,
) -> Result<T, std::io::Error> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

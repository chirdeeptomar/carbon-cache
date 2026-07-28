use std::io::Cursor;
use std::ops::RangeBounds;
use std::sync::Arc;

use openraft::storage::{RaftLogReader, RaftSnapshotBuilder, RaftStorage};
use openraft::{
    Entry, EntryPayload, LogId, LogState, OptionalSend, Snapshot, SnapshotMeta,
    StorageError, StorageIOError, StoredMembership, Vote,
};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde_json;
use tokio::sync::RwLock;

use crate::state_machine::CacheStateMachine;
use crate::types::{CarbonNode, NodeId, RaftLogEntry, RaftResponse, TypeConfig};

/// redb table: log index (u64) → serialized [`Entry<TypeConfig>`] (JSON bytes).
const LOG_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("raft_log");
/// redb table: string key → serialized metadata value (JSON bytes).
/// Keys: [`KEY_VOTE`] and [`KEY_COMMITTED`].
const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("raft_meta");
const KEY_VOTE: &str = "vote";
const KEY_COMMITTED: &str = "committed";

/// openraft storage backend backed by a redb database file.
///
/// Each node stores its Raft log and metadata in a single redb file at
/// `./data/raft/{node_id}/raft.redb` (configurable via `CARBON_CLUSTER_DATA_DIR`).
///
/// The state machine ([`CacheStateMachine`]) is kept in memory and shared via
/// an `Arc<RwLock<_>>` with the HTTP handlers so reads bypass the log entirely.
/// Snapshots serialize the full state machine to JSON for log compaction and
/// catch-up replication to new or lagging nodes.
pub struct CarbonRaftStorage {
    db: Arc<Database>,
    pub sm: Arc<RwLock<CacheStateMachine>>,
    snapshot_idx: u64,
    current_snapshot: Option<Snapshot<TypeConfig>>,
}

impl CarbonRaftStorage {
    pub fn new(db: Arc<Database>) -> Result<Self, redb::Error> {
        let write_txn = db.begin_write()?;
        {
            write_txn.open_table(LOG_TABLE)?;
            write_txn.open_table(META_TABLE)?;
        }
        write_txn.commit()?;
        Ok(Self {
            db,
            sm: Arc::new(RwLock::new(CacheStateMachine::default())),
            snapshot_idx: 0,
            current_snapshot: None,
        })
    }

    /// Convenience constructor that opens (or creates) the redb file at `path`.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, redb::Error> {
        let db = Arc::new(Database::create(path)?);
        Self::new(db)
    }
}

fn io_err(e: impl std::fmt::Display) -> StorageError<NodeId> {
    let err = std::io::Error::other(e.to_string());
    StorageError::IO {
        source: StorageIOError::read(&err),
    }
}

impl RaftLogReader<TypeConfig> for CarbonRaftStorage {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + std::fmt::Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(io_err)?;
        let table = read_txn
            .open_table(LOG_TABLE)
            .map_err(io_err)?;
        let mut entries = Vec::new();
        for r in table
            .range(range)
            .map_err(io_err)?
        {
            let (_, v) = r.map_err(io_err)?;
            let entry = serde_json::from_slice::<Entry<TypeConfig>>(v.value())
                .map_err(io_err)?;
            entries.push(entry);
        }
        Ok(entries)
    }
}

impl RaftSnapshotBuilder<TypeConfig> for CarbonRaftStorage {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        let sm = self.sm.read().await;
        let data = serde_json::to_vec(&*sm).map_err(io_err)?;

        let last_applied = sm.last_applied_log;
        let last_membership = sm.last_membership.clone();
        drop(sm);

        self.snapshot_idx += 1;
        let snapshot_id = format!(
            "{}-{}-{}",
            last_applied
                .map(|l| l.leader_id.to_string())
                .unwrap_or_default(),
            last_applied.map(|l| l.index).unwrap_or(0),
            self.snapshot_idx
        );

        let meta = SnapshotMeta {
            last_log_id: last_applied,
            last_membership,
            snapshot_id,
        };

        let snapshot = Snapshot {
            meta: meta.clone(),
            snapshot: Box::new(Cursor::new(data.clone())),
        };

        self.current_snapshot = Some(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(data)),
        });

        Ok(snapshot)
    }
}

impl RaftStorage<TypeConfig> for CarbonRaftStorage {
    type LogReader = Self;
    type SnapshotBuilder = Self;

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(io_err)?;
        {
            let mut table = write_txn
                .open_table(META_TABLE)
                .map_err(io_err)?;
            let bytes = serde_json::to_vec(vote).map_err(io_err)?;
            table
                .insert(KEY_VOTE, bytes.as_slice())
                .map_err(io_err)?;
        }
        write_txn
            .commit()
            .map_err(io_err)?;
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(io_err)?;
        let table = read_txn
            .open_table(META_TABLE)
            .map_err(io_err)?;
        match table
            .get(KEY_VOTE)
            .map_err(io_err)?
        {
            None => Ok(None),
            Some(v) => Ok(serde_json::from_slice(v.value())
                .map_err(io_err)?),
        }
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogId<NodeId>>,
    ) -> Result<(), StorageError<NodeId>> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(io_err)?;
        {
            let mut table = write_txn
                .open_table(META_TABLE)
                .map_err(io_err)?;
            let bytes = serde_json::to_vec(&committed).map_err(io_err)?;
            table
                .insert(KEY_COMMITTED, bytes.as_slice())
                .map_err(io_err)?;
        }
        write_txn
            .commit()
            .map_err(io_err)?;
        Ok(())
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(io_err)?;
        let table = read_txn
            .open_table(META_TABLE)
            .map_err(io_err)?;
        match table
            .get(KEY_COMMITTED)
            .map_err(io_err)?
        {
            None => Ok(None),
            Some(v) => Ok(serde_json::from_slice(v.value())
                .map_err(io_err)?),
        }
    }

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(io_err)?;
        let table = read_txn
            .open_table(LOG_TABLE)
            .map_err(io_err)?;

        let last = match table
            .last()
            .map_err(io_err)?
        {
            Some((_, v)) => {
                let entry: Entry<TypeConfig> = serde_json::from_slice(v.value())
                    .map_err(io_err)?;
                Some(entry.log_id)
            }
            None => None,
        };

        let first = match table
            .first()
            .map_err(io_err)?
        {
            Some((_, v)) => {
                let entry: Entry<TypeConfig> = serde_json::from_slice(v.value())
                    .map_err(io_err)?;
                Some(entry.log_id)
            }
            None => None,
        };

        let purged = match first {
            None => None,
            Some(f) if f.index == 0 => None,
            Some(f) => Some(LogId::new(f.leader_id, f.index - 1)),
        };

        Ok(LogState {
            last_purged_log_id: purged,
            last_log_id: last,
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        Self {
            db: self.db.clone(),
            sm: self.sm.clone(),
            snapshot_idx: self.snapshot_idx,
            current_snapshot: self.current_snapshot.clone(),
        }
    }

    async fn append_to_log<I>(&mut self, entries: I) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + OptionalSend,
    {
        let entries: Vec<_> = entries.into_iter().collect();
        let write_txn = self
            .db
            .begin_write()
            .map_err(io_err)?;
        {
            let mut table = write_txn
                .open_table(LOG_TABLE)
                .map_err(io_err)?;
            for entry in &entries {
                let bytes = serde_json::to_vec(entry)
                    .map_err(io_err)?;
                table
                    .insert(entry.log_id.index, bytes.as_slice())
                    .map_err(io_err)?;
            }
        }
        write_txn
            .commit()
            .map_err(io_err)?;
        Ok(())
    }

    async fn delete_conflict_logs_since(
        &mut self,
        log_id: LogId<NodeId>,
    ) -> Result<(), StorageError<NodeId>> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(io_err)?;
        {
            let mut table = write_txn
                .open_table(LOG_TABLE)
                .map_err(io_err)?;
            let mut to_delete: Vec<u64> = Vec::new();
            for r in table
                .range(log_id.index..)
                .map_err(io_err)?
            {
                let (k, _) = r.map_err(io_err)?;
                to_delete.push(k.value());
            }
            for idx in to_delete {
                table
                    .remove(idx)
                    .map_err(io_err)?;
            }
        }
        write_txn
            .commit()
            .map_err(io_err)?;
        Ok(())
    }

    async fn purge_logs_upto(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(io_err)?;
        {
            let mut table = write_txn
                .open_table(LOG_TABLE)
                .map_err(io_err)?;
            let mut to_delete: Vec<u64> = Vec::new();
            for r in table
                .range(..=log_id.index)
                .map_err(io_err)?
            {
                let (k, _) = r.map_err(io_err)?;
                to_delete.push(k.value());
            }
            for idx in to_delete {
                table
                    .remove(idx)
                    .map_err(io_err)?;
            }
        }
        write_txn
            .commit()
            .map_err(io_err)?;
        Ok(())
    }

    async fn last_applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, CarbonNode>), StorageError<NodeId>>
    {
        let sm = self.sm.read().await;
        Ok((sm.last_applied_log, sm.last_membership.clone()))
    }

    async fn apply_to_state_machine(
        &mut self,
        entries: &[Entry<TypeConfig>],
    ) -> Result<Vec<RaftResponse>, StorageError<NodeId>> {
        let mut sm = self.sm.write().await;
        let mut responses = Vec::new();

        for entry in entries {
            sm.last_applied_log = Some(entry.log_id);

            let resp = match &entry.payload {
                EntryPayload::Blank => RaftResponse::Ok,
                EntryPayload::Normal(cmd) => match cmd {
                    RaftLogEntry::CreateCache(config) => {
                        let created = !sm.configs.contains_key(&config.name);
                        sm.data.entry(config.name.clone()).or_default();
                        sm.configs.insert(config.name.clone(), config.clone());
                        RaftResponse::CacheCreated { created }
                    }
                    RaftLogEntry::DropCache { name } => {
                        let dropped = sm.configs.remove(name).is_some();
                        sm.data.remove(name);
                        RaftResponse::CacheDropped { dropped }
                    }
                    RaftLogEntry::Put {
                        cache_name,
                        key,
                        value,
                    } => {
                        let inner = sm.data.entry(cache_name.clone()).or_default();
                        let created = !inner.contains_key(key);
                        inner.insert(key.clone(), value.clone());
                        RaftResponse::Written { created }
                    }
                    RaftLogEntry::Delete { cache_name, key } => {
                        let existed = sm
                            .data
                            .get_mut(cache_name)
                            .and_then(|m| m.remove(key))
                            .is_some();
                        RaftResponse::Deleted { existed }
                    }
                    RaftLogEntry::CreateUser(user) => {
                        sm.users_by_username
                            .insert(user.username.clone(), user.id.clone());
                        sm.users.insert(user.id.clone(), user.clone());
                        RaftResponse::UserCreated { user: user.clone() }
                    }
                    RaftLogEntry::UpdateUser(user) => {
                        if let Some(old_username) =
                            sm.users.get(&user.id).map(|u| u.username.clone())
                        {
                            sm.users_by_username.remove(&old_username);
                        }
                        sm.users_by_username
                            .insert(user.username.clone(), user.id.clone());
                        sm.users.insert(user.id.clone(), user.clone());
                        RaftResponse::UserUpdated { user: user.clone() }
                    }
                    RaftLogEntry::DeleteUser { id } => {
                        let existed = if let Some(u) = sm.users.remove(id) {
                            sm.users_by_username.remove(&u.username);
                            true
                        } else {
                            false
                        };
                        RaftResponse::UserDeleted { existed }
                    }
                    RaftLogEntry::CreateRole(role) => {
                        sm.roles_by_name
                            .insert(role.name.clone(), role.id.clone());
                        sm.roles.insert(role.id.clone(), role.clone());
                        RaftResponse::RoleCreated { role: role.clone() }
                    }
                    RaftLogEntry::UpdateRole(role) => {
                        if let Some(old_name) =
                            sm.roles.get(&role.id).map(|r| r.name.clone())
                        {
                            sm.roles_by_name.remove(&old_name);
                        }
                        sm.roles_by_name
                            .insert(role.name.clone(), role.id.clone());
                        sm.roles.insert(role.id.clone(), role.clone());
                        RaftResponse::RoleUpdated { role: role.clone() }
                    }
                    RaftLogEntry::DeleteRole { id } => {
                        let existed = if let Some(r) = sm.roles.remove(id) {
                            sm.roles_by_name.remove(&r.name);
                            true
                        } else {
                            false
                        };
                        RaftResponse::RoleDeleted { existed }
                    }
                },
                EntryPayload::Membership(membership) => {
                    sm.last_membership =
                        StoredMembership::new(Some(entry.log_id), membership.clone());
                    RaftResponse::Ok
                }
            };
            responses.push(resp);
        }

        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        Self {
            db: self.db.clone(),
            sm: self.sm.clone(),
            snapshot_idx: self.snapshot_idx,
            current_snapshot: self.current_snapshot.clone(),
        }
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, CarbonNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<NodeId>> {
        let new_sm: CacheStateMachine = serde_json::from_slice(snapshot.get_ref())
            .map_err(io_err)?;
        let mut sm = self.sm.write().await;
        *sm = new_sm;
        self.current_snapshot = Some(Snapshot {
            meta: meta.clone(),
            snapshot: Box::new(Cursor::new(snapshot.into_inner())),
        });
        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfig>>, StorageError<NodeId>> {
        Ok(self.current_snapshot.clone())
    }
}

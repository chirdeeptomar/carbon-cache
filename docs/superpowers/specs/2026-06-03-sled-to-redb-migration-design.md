# Sled → redb Migration Design

**Date:** 2026-06-03  
**Branch:** feature/admin-ui  
**Scope:** Replace sled embedded database with redb across existing storage layer. No new features.

---

## Context

Carbon Cache uses sled 0.34 for two persistent storage areas: auth (users/roles) and cache configuration. Sled has been effectively unmaintained since 2022. redb is a modern, actively maintained embedded database with ACID transactions, better performance, and a clean API. This migration is a prerequisite for the upcoming raft log implementation, which will also use redb.

---

## Goals

- Swap sled for redb with no behavioral changes to callers
- Rename structs/files from `Sled*` to `Redb*` so names are honest
- Preserve all existing repository traits (`UserRepository`, `RoleRepository`) — callers are unaffected
- Keep JSON serialization format (no data format change)
- All existing tests continue to pass

---

## Architecture

redb uses typed table definitions and explicit write transactions. The database handle is wrapped in `Arc<redb::Database>` for cheap cloning. The two auth repositories (`RedbUserRepository`, `RedbRoleRepository`) share a single `Arc<Database>` instance rather than opening separate databases as the sled versions did.

### Table Definitions

```rust
const USERS: TableDefinition<&str, &[u8]>            = TableDefinition::new("users");
const USERS_BY_USERNAME: TableDefinition<&str, &str> = TableDefinition::new("users_by_username");
const ROLES: TableDefinition<&str, &[u8]>            = TableDefinition::new("roles");
const ROLES_BY_NAME: TableDefinition<&str, &str>     = TableDefinition::new("roles_by_name");
```

Keys are `&str`, primary values are JSON blobs (`&[u8]`), index values are `&str` IDs.

### Transaction Model

sled did implicit per-op writes with explicit `flush()` for durability. redb requires an explicit `write_txn.commit()`. Multi-step operations (e.g. `create` writes to both primary and index table) now commit atomically — this is safer than the sled approach. The `flush()` calls in `SledPersistence` are replaced by `commit()`.

---

## Files Changed

| Old file | New file | Change |
|---|---|---|
| `carbon/src/auth/sled_repository.rs` | `carbon/src/auth/redb_repository.rs` | Full rewrite to redb API |
| `carbon/src/persistence/sled_store.rs` | `carbon/src/persistence/redb_store.rs` | Full rewrite to redb API |
| `carbon/src/auth/error.rs` | (same) | Replace `From<sled::Error>` with `From<redb::Error>` |
| `carbon/src/auth/mod.rs` | (same) | Rename module ref and re-exports |
| `carbon/src/persistence/mod.rs` | (same) | Rename module ref and re-export |
| `Cargo.toml` (workspace) | (same) | Replace `sled = "0.34"` with `redb = "2"` |
| `carbon/Cargo.toml` | (same) | Replace `sled.workspace` with `redb.workspace` |

### Struct Renames

| Old | New |
|---|---|
| `SledUserRepository` | `RedbUserRepository` |
| `SledRoleRepository` | `RedbRoleRepository` |
| `SledPersistence` | `RedbPersistence` |

---

## Error Handling

`From<sled::Error> for AuthError` in `error.rs` is replaced with `From<redb::Error> for AuthError`, mapping to the same `AuthError::StorageError(String)` variant. `RedbPersistence` continues to use inline `map_err` string formatting — no `From` impl needed there.

---

## Testing

- Existing unit tests in `redb_repository.rs` and `redb_store.rs` are updated to use the new struct names and `redb` temp file paths
- Tests use `tempfile::TempDir` — same pattern, just a `.redb` file extension for clarity
- Run: `cargo test -p carbon`

---

## Out of Scope

- Migrating existing sled data (no data migration — dev/test environments only)
- Raft log implementation (next phase)
- Changes to `UserRepository` or `RoleRepository` traits
- Any changes to callers outside the storage layer

//! `carbon-raft` — distributed consensus layer for Carbon.
//!
//! Wraps [openraft](https://docs.rs/openraft) with Carbon-specific types and wires
//! together all the pieces needed to run a multi-node cluster:
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | [`types`] | Core type aliases and the Raft log entry / response enums |
//! | [`rpc`] | Wire protocol — length-prefixed JSON frames over raw TCP |
//! | [`node`] | [`RaftCacheNode`]: public API, bootstrap, and RPC listener |
//! | [`network`] | openraft `RaftNetworkFactory` impl — dials peers over TCP |
//! | [`log_store`] | openraft `RaftStorage` impl — persists log and metadata to redb |
//! | [`state_machine`] | In-memory state replicated by the log: caches + auth |
//! | [`auth_repository`] | Read-only auth repository views backed by the state machine |
//! | [`api`] | Optional axum router exposing `/raft/metrics` (unused in carbon-server) |

pub mod api;
pub mod auth_repository;
pub mod log_store;
pub mod network;
pub mod node;
pub mod rpc;
pub mod state_machine;
pub mod types;

pub use auth_repository::{RaftRoleRepository, RaftUserRepository};

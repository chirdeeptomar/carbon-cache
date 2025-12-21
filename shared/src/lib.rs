// shared/src/lib.rs

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("not found")]
    NotFound,
    #[error("cache not found: {0}")]
    CacheNotFound(String),
    #[error("internal: {0}")]
    Internal(String),
}

// Keep old alias for backwards compatibility
pub type CacheError = Error;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct TtlMs(pub u64);

pub mod config;

// shared/src/lib.rs
pub type Key = Vec<u8>;
pub type Value = Vec<u8>;

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("not found")]
    NotFound,
    #[error("internal: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;

#[derive(Clone, Copy, Debug)]
pub struct TtlMs(pub u64);

use crate::models::CreateCacheRequest;
use carbon::domain::{CacheConfig, CacheEvictionStrategy, EvictionAlgorithm};

// Constants for validation ranges
const MIN_MEM_BYTES: u64 = 1_048_576; // 1 MB
const MAX_MEM_BYTES: u64 = 1_099_511_627_776; // 1 TB
const MAX_SHARDS: u32 = 256;
const DEFAULT_TTL_MS: u64 = 1_800_000; // 30 minutes
const DEFAULT_SHARDS: u32 = 16; // Default to 16 shards

#[derive(Debug)]
pub enum ValidationError {
    MissingRequiredField {
        field: &'static str,
        backend: &'static str,
    },
    InvalidCacheName {
        reason: &'static str,
    },
    InvalidBackendType(String),
    InvalidPolicy(String),
    OutOfRange {
        field: &'static str,
        value: u64,
        min: u64,
        max: u64,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MissingRequiredField { field, backend } => {
                write!(
                    f,
                    "Missing required field '{}' for {} cache",
                    field, backend
                )
            }
            ValidationError::InvalidCacheName { reason } => {
                write!(f, "Invalid cache name: {}", reason)
            }
            ValidationError::InvalidBackendType(backend) => {
                write!(
                    f,
                    "Invalid backend type '{}'. Must be 'ttl', 'size', or 'storage'",
                    backend
                )
            }
            ValidationError::InvalidPolicy(policy) => {
                write!(
                    f,
                    "Invalid eviction policy '{}'. Must be 'lru', 'sieve', or 'tinylfu'",
                    policy
                )
            }
            ValidationError::OutOfRange {
                field,
                value,
                min,
                max,
            } => {
                write!(
                    f,
                    "Field '{}' value {} is out of range (min: {}, max: {})",
                    field, value, min, max
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

pub struct CacheConfigFactory;

impl CacheConfigFactory {
    /// Main entry point: validates request and returns a CacheConfig or validation error
    pub fn from_request(req: CreateCacheRequest) -> Result<CacheConfig, ValidationError> {
        // Parse and validate backend type
        let backend = Self::parse_backend(&req.eviction)?;

        // Parse policy
        let policy = Self::parse_policy(&req.policy)?;

        // Validate based on backend type
        Self::validate_for_backend(&req, backend)?;

        // Validate common fields
        Self::validate_common_fields(&req)?;

        // Build config with validated and defaulted values
        Ok(Self::build_config(req, backend, policy))
    }

    fn parse_backend(eviction: &str) -> Result<CacheEvictionStrategy, ValidationError> {
        match eviction.to_lowercase().as_str() {
            "ttl" => Ok(CacheEvictionStrategy::TimeBound),
            "size" => Ok(CacheEvictionStrategy::SizeBounded),
            "storage" => Ok(CacheEvictionStrategy::OverflowToDisk),
            _ => Err(ValidationError::InvalidBackendType(eviction.to_string())),
        }
    }

    fn parse_policy(policy: &str) -> Result<EvictionAlgorithm, ValidationError> {
        if policy.is_empty() {
            return Ok(EvictionAlgorithm::TinyLfu); // Default
        }

        match policy.to_lowercase().as_str() {
            "lru" => Ok(EvictionAlgorithm::Lru),
            "sieve" => Ok(EvictionAlgorithm::Sieve),
            "tinylfu" => Ok(EvictionAlgorithm::TinyLfu),
            _ => Err(ValidationError::InvalidPolicy(policy.to_string())),
        }
    }

    fn validate_for_backend(
        req: &CreateCacheRequest,
        backend: CacheEvictionStrategy,
    ) -> Result<(), ValidationError> {
        match backend {
            CacheEvictionStrategy::TimeBound => {
                // TTL cache - all fields optional, will use defaults
                // If mem_bytes is provided, validate it
                if let Some(mem_bytes) = req.mem_bytes {
                    if mem_bytes < MIN_MEM_BYTES || mem_bytes > MAX_MEM_BYTES {
                        return Err(ValidationError::OutOfRange {
                            field: "mem_bytes",
                            value: mem_bytes,
                            min: MIN_MEM_BYTES,
                            max: MAX_MEM_BYTES,
                        });
                    }
                }
                Ok(())
            }
            CacheEvictionStrategy::SizeBounded => {
                // Size cache - mem_bytes is required
                let mem_bytes = req.mem_bytes.ok_or(ValidationError::MissingRequiredField {
                    field: "mem_bytes",
                    backend: "SizeBounded",
                })?;

                // Validate range
                if mem_bytes < MIN_MEM_BYTES || mem_bytes > MAX_MEM_BYTES {
                    return Err(ValidationError::OutOfRange {
                        field: "mem_bytes",
                        value: mem_bytes,
                        min: MIN_MEM_BYTES,
                        max: MAX_MEM_BYTES,
                    });
                }

                Ok(())
            }
            CacheEvictionStrategy::OverflowToDisk => {
                // Storage cache - mem_bytes and disk_path required
                let mem_bytes = req.mem_bytes.ok_or(ValidationError::MissingRequiredField {
                    field: "mem_bytes",
                    backend: "OverflowToDisk",
                })?;

                if req.disk_path.is_none() || req.disk_path.as_ref().unwrap().is_empty() {
                    return Err(ValidationError::MissingRequiredField {
                        field: "disk_path",
                        backend: "OverflowToDisk",
                    });
                }

                // Validate range
                if mem_bytes < MIN_MEM_BYTES || mem_bytes > MAX_MEM_BYTES {
                    return Err(ValidationError::OutOfRange {
                        field: "mem_bytes",
                        value: mem_bytes,
                        min: MIN_MEM_BYTES,
                        max: MAX_MEM_BYTES,
                    });
                }

                Ok(())
            }
        }
    }

    fn validate_common_fields(req: &CreateCacheRequest) -> Result<(), ValidationError> {
        // Validate name
        if req.name.is_empty() {
            return Err(ValidationError::InvalidCacheName {
                reason: "cache name cannot be empty",
            });
        }

        // Validate name contains only alphanumeric, hyphens, underscores
        if !req
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ValidationError::InvalidCacheName {
                reason: "cache name must contain only alphanumeric characters, hyphens, or underscores",
            });
        }

        // Validate shards range if provided
        if let Some(shards) = req.shards {
            if shards as u32 > MAX_SHARDS {
                return Err(ValidationError::OutOfRange {
                    field: "shards",
                    value: shards as u64,
                    min: 1,
                    max: MAX_SHARDS as u64,
                });
            }
        }

        Ok(())
    }

    fn build_config(
        req: CreateCacheRequest,
        backend: CacheEvictionStrategy,
        policy: EvictionAlgorithm,
    ) -> CacheConfig {
        // Apply defaults based on backend type
        let default_ttl_ms = match backend {
            CacheEvictionStrategy::TimeBound => {
                // For TTL caches, default to 30 minutes if not specified
                Some(req.default_ttl_ms.unwrap_or(DEFAULT_TTL_MS))
            }
            _ => {
                // For other cache types, only set if explicitly provided
                req.default_ttl_ms
            }
        };

        // Default shards to 16 if not provided
        let shards = req.shards.or(Some(DEFAULT_SHARDS as u8));

        CacheConfig::with_backend(
            req.name,
            backend,
            policy,
            req.mem_bytes,
            req.disk_path,
            shards,
            default_ttl_ms,
            req.max_value_bytes,
            req.description,
            req.tags,
        )
    }
}

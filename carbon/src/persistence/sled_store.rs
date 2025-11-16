use crate::domain::CacheConfig;
use shared::{Error, Result};
use std::path::Path;

/// Sled-based persistence for cache configurations
pub struct SledPersistence {
    db: sled::Db,
}

impl SledPersistence {
    /// Create a new Sled persistence layer
    /// Creates the parent directory if it doesn't exist
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Internal(format!("Failed to create directory: {}", e)))?;
        }

        let db = sled::open(path)
            .map_err(|e| Error::Internal(format!("Failed to open Sled database: {}", e)))?;

        Ok(Self { db })
    }

    /// Save a cache configuration
    pub fn save_config(&self, config: &CacheConfig) -> Result<()> {
        let key = config.name.as_bytes();
        let value = serde_json::to_vec(config)
            .map_err(|e| Error::Internal(format!("Failed to serialize config: {}", e)))?;

        self.db
            .insert(key, value)
            .map_err(|e| Error::Internal(format!("Failed to save config: {}", e)))?;

        self.db
            .flush()
            .map_err(|e| Error::Internal(format!("Failed to flush database: {}", e)))?;

        Ok(())
    }

    /// Load all cache configurations
    pub fn load_all(&self) -> Result<Vec<CacheConfig>> {
        let mut configs = Vec::new();

        for result in self.db.iter() {
            let (_, value) = result
                .map_err(|e| Error::Internal(format!("Failed to iterate database: {}", e)))?;

            let config: CacheConfig = serde_json::from_slice(&value)
                .map_err(|e| Error::Internal(format!("Failed to deserialize config: {}", e)))?;

            configs.push(config);
        }

        Ok(configs)
    }

    /// Delete a cache configuration by name
    pub fn delete_config(&self, name: &str) -> Result<bool> {
        let key = name.as_bytes();
        let removed = self
            .db
            .remove(key)
            .map_err(|e| Error::Internal(format!("Failed to delete config: {}", e)))?
            .is_some();

        self.db
            .flush()
            .map_err(|e| Error::Internal(format!("Failed to flush database: {}", e)))?;

        Ok(removed)
    }

    /// Get a single cache configuration by name
    pub fn get_config(&self, name: &str) -> Result<Option<CacheConfig>> {
        let key = name.as_bytes();
        let value = self
            .db
            .get(key)
            .map_err(|e| Error::Internal(format!("Failed to get config: {}", e)))?;

        match value {
            Some(bytes) => {
                let config: CacheConfig = serde_json::from_slice(&bytes).map_err(|e| {
                    Error::Internal(format!("Failed to deserialize config: {}", e))
                })?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EvictionAlgorithm;
    use std::collections::HashMap;

    #[test]
    fn test_sled_persistence_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.sled");

        let persistence = SledPersistence::new(&db_path).unwrap();

        let config = CacheConfig::new(
            "test-cache",
            Some(1024 * 1024),
            None,
            Some(4),
            EvictionAlgorithm::TinyLfu,
            None,
            None,
            Some("Test cache".to_string()),
            Some(HashMap::from([
                ("env".to_string(), "test".to_string()),
                ("team".to_string(), "dev".to_string()),
            ])),
        );

        // Save
        persistence.save_config(&config).unwrap();

        // Load all
        let loaded = persistence.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "test-cache");
        assert_eq!(loaded[0].description, Some("Test cache".to_string()));

        // Get specific
        let fetched = persistence.get_config("test-cache").unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "test-cache");

        // Delete
        let deleted = persistence.delete_config("test-cache").unwrap();
        assert!(deleted);

        // Verify deletion
        let loaded_after = persistence.load_all().unwrap();
        assert_eq!(loaded_after.len(), 0);
    }
}

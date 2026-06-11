use crate::domain::CacheConfig;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use shared::{Error, Result};
use std::path::Path;

const CONFIGS: TableDefinition<&str, &[u8]> = TableDefinition::new("configs");

pub struct RedbPersistence {
    db: Database,
}

impl RedbPersistence {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Internal(format!("Failed to create directory: {}", e)))?;
        }
        let db = Database::create(path)
            .map_err(|e| Error::Internal(format!("Failed to open redb database: {}", e)))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| Error::Internal(format!("Failed to begin transaction: {}", e)))?;
        {
            write_txn
                .open_table(CONFIGS)
                .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
        }
        write_txn
            .commit()
            .map_err(|e| Error::Internal(format!("Failed to commit: {}", e)))?;
        Ok(Self { db })
    }

    pub fn save_config(&self, config: &CacheConfig) -> Result<()> {
        let value = serde_json::to_vec(config)
            .map_err(|e| Error::Internal(format!("Failed to serialize config: {}", e)))?;
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| Error::Internal(format!("Failed to begin transaction: {}", e)))?;
        {
            let mut table = write_txn
                .open_table(CONFIGS)
                .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
            table
                .insert(config.name.as_str(), value.as_slice())
                .map_err(|e| Error::Internal(format!("Failed to insert config: {}", e)))?;
        }
        write_txn
            .commit()
            .map_err(|e| Error::Internal(format!("Failed to commit: {}", e)))?;
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<CacheConfig>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| Error::Internal(format!("Failed to begin read: {}", e)))?;
        let table = read_txn
            .open_table(CONFIGS)
            .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
        let mut configs = Vec::new();
        for entry in table
            .iter()
            .map_err(|e| Error::Internal(format!("Failed to iterate: {}", e)))?
        {
            let (_, v) =
                entry.map_err(|e| Error::Internal(format!("Failed to read entry: {}", e)))?;
            let config: CacheConfig = serde_json::from_slice(v.value())
                .map_err(|e| Error::Internal(format!("Failed to deserialize config: {}", e)))?;
            configs.push(config);
        }
        Ok(configs)
    }

    pub fn delete_config(&self, name: &str) -> Result<bool> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| Error::Internal(format!("Failed to begin transaction: {}", e)))?;
        let removed = {
            let mut table = write_txn
                .open_table(CONFIGS)
                .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
            table
                .remove(name)
                .map_err(|e| Error::Internal(format!("Failed to delete config: {}", e)))?
                .is_some()
        };
        write_txn
            .commit()
            .map_err(|e| Error::Internal(format!("Failed to commit: {}", e)))?;
        Ok(removed)
    }

    pub fn get_config(&self, name: &str) -> Result<Option<CacheConfig>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| Error::Internal(format!("Failed to begin read: {}", e)))?;
        let table = read_txn
            .open_table(CONFIGS)
            .map_err(|e| Error::Internal(format!("Failed to open table: {}", e)))?;
        match table
            .get(name)
            .map_err(|e| Error::Internal(format!("Failed to get config: {}", e)))?
        {
            Some(v) => {
                let config: CacheConfig = serde_json::from_slice(v.value())
                    .map_err(|e| Error::Internal(format!("Failed to deserialize config: {}", e)))?;
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
    fn test_save_load_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = RedbPersistence::new(dir.path().join("test.redb")).unwrap();

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

        store.save_config(&config).unwrap();

        let loaded = store.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "test-cache");
        assert_eq!(loaded[0].description, Some("Test cache".to_string()));

        let fetched = store.get_config("test-cache").unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "test-cache");

        let deleted = store.delete_config("test-cache").unwrap();
        assert!(deleted);

        let loaded_after = store.load_all().unwrap();
        assert_eq!(loaded_after.len(), 0);
    }

    #[test]
    fn test_delete_nonexistent_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let store = RedbPersistence::new(dir.path().join("test.redb")).unwrap();
        let deleted = store.delete_config("nope").unwrap();
        assert!(!deleted);
    }
}

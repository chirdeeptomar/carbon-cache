use crate::domain::CacheConfig;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use shared::{Error, Result};
use std::path::Path;

const CONFIGS: TableDefinition<&str, &[u8]> = TableDefinition::new("configs");

pub struct RedbPersistence {
    db: Database,
}

/// Wrap a persistence-layer failure into `Error::Internal` with context.
fn persist_err<E: std::fmt::Display>(ctx: &'static str) -> impl FnOnce(E) -> Error {
    move |e| Error::Internal(format!("Failed to {ctx}: {e}"))
}

impl RedbPersistence {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(persist_err("create directory"))?;
        }
        let db = Database::create(path).map_err(persist_err("open redb database"))?;
        let write_txn = db.begin_write().map_err(persist_err("begin transaction"))?;
        {
            write_txn
                .open_table(CONFIGS)
                .map_err(persist_err("open table"))?;
        }
        write_txn.commit().map_err(persist_err("commit"))?;
        Ok(Self { db })
    }

    pub fn save_config(&self, config: &CacheConfig) -> Result<()> {
        let value = serde_json::to_vec(config).map_err(persist_err("serialize config"))?;
        let write_txn = self
            .db
            .begin_write()
            .map_err(persist_err("begin transaction"))?;
        {
            let mut table = write_txn
                .open_table(CONFIGS)
                .map_err(persist_err("open table"))?;
            table
                .insert(config.name.as_str(), value.as_slice())
                .map_err(persist_err("insert config"))?;
        }
        write_txn.commit().map_err(persist_err("commit"))?;
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<CacheConfig>> {
        let read_txn = self.db.begin_read().map_err(persist_err("begin read"))?;
        let table = read_txn
            .open_table(CONFIGS)
            .map_err(persist_err("open table"))?;
        let mut configs = Vec::new();
        for entry in table.iter().map_err(persist_err("iterate"))? {
            let (_, v) = entry.map_err(persist_err("read entry"))?;
            let config: CacheConfig =
                serde_json::from_slice(v.value()).map_err(persist_err("deserialize config"))?;
            configs.push(config);
        }
        Ok(configs)
    }

    pub fn delete_config(&self, name: &str) -> Result<bool> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(persist_err("begin transaction"))?;
        let removed = {
            let mut table = write_txn
                .open_table(CONFIGS)
                .map_err(persist_err("open table"))?;
            table
                .remove(name)
                .map_err(persist_err("delete config"))?
                .is_some()
        };
        write_txn.commit().map_err(persist_err("commit"))?;
        Ok(removed)
    }

    pub fn get_config(&self, name: &str) -> Result<Option<CacheConfig>> {
        let read_txn = self.db.begin_read().map_err(persist_err("begin read"))?;
        let table = read_txn
            .open_table(CONFIGS)
            .map_err(persist_err("open table"))?;
        match table.get(name).map_err(persist_err("get config"))? {
            Some(v) => {
                let config: CacheConfig = serde_json::from_slice(v.value())
                    .map_err(persist_err("deserialize config"))?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CacheEvictionStrategy, CacheOptions, EvictionAlgorithm};

    #[test]
    fn test_save_load_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = RedbPersistence::new(dir.path().join("test.redb")).unwrap();

        let options = CacheOptions {
            mem_bytes: Some(1024),
            disk_path: None,
            shards: None,
            policy: EvictionAlgorithm::Lru,
            default_ttl_ms: Some(1000),
            max_value_bytes: Some(512),
            backend: Some(CacheEvictionStrategy::SizeBounded),
        };

        let config = CacheConfig::new(
            "test-cache",
            Option::Some("Random Description".to_string()),
            None,
            options,
        );

        store.save_config(&config).unwrap();

        let loaded = store.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "test-cache");
        assert_eq!(
            loaded[0].description,
            Some("Random Description".to_string())
        );

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

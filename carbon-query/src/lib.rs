mod indexes;

use serde::{Deserialize, Serialize};
use ordered_float::OrderedFloat;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    Hash,
    Range,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub field_type: FieldType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Bytes,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[derive(Eq)]
#[derive(Hash)]
pub enum FieldValue {
    String(String),
    Integer(i64),
    Float(OrderedFloat<f64>),
    Boolean(bool),
    Bytes(Vec<u8>),
    Null,
}

/// Defines the Index
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IndexDefinition {
    /// Name of the index
    pub name: String,

    /// Field to create an index on
    pub field: String,

    /// Type of the index
    pub index_type: IndexType,

    /// Is it key of the index unique
    pub unique: bool,
}

pub trait Index<K>: Send + Sync {
    /// Insert a key-value pair
    fn insert(&mut self, key: K, field_value: FieldValue);

    /// Remove a key
    fn remove(&mut self, key: &K);

    /// Query for exact match
    fn get(&self, key: &K) -> Option<Vec<FieldValue>>;
}

pub struct QueryableConfig {
    fields: Vec<FieldDefinition>,
    indexes: Vec<IndexDefinition>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation_of_queryable_config() {
        let config = QueryableConfig {
            fields: vec![FieldDefinition {
                name: "name".to_string(),
                field_type: FieldType::String,
            }],
            indexes: vec![IndexDefinition {
                field: "name".to_string(),
                index_type: IndexType::Hash,
                unique: false,
                name: "idx_name".to_string(),
            }],
        };
        assert_ne!(config.fields.len(), 0);
    }
}

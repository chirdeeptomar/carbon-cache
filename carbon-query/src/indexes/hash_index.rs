use crate::{FieldValue, Index};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

/// Simple index for storing Key and set of values
/// Example: [IndexedField: vec![Key]]
/// Imagine indexing fruits by colour: ["red", ["apple", "strawberry"], []]
struct HashIndex<K: Eq + Hash> {
    name: String,
    map: HashMap<K, HashSet<FieldValue>>,
}

impl<K: Eq + Hash> HashIndex<K> {
    fn new(idx_name: &str) -> Self {
        HashIndex {
            name: idx_name.to_string(),
            map: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash + Sync + Send + Debug> Index<K> for HashIndex<K> {
    fn insert(&mut self, key: K, field_value: FieldValue) {
        match self.map.entry(key) {
            Entry::Occupied(k) => {
                log::info!("Duplicate key: {:#?}, append to HashSet", k.key());
            }
            Entry::Vacant(k) => {
                log::info!("Unique key: {:#?}, add to HashSet", k.key());
                self.map.insert(k.key(), HashSet::from([field_value]));
            }
        }
    }

    fn remove(&mut self, key: &K) {
        todo!()
    }

    fn get(&self, value: &K) -> Option<Vec<FieldValue>> {
        self.map.get(value).map(|set| set.iter().cloned().collect())
    }
}

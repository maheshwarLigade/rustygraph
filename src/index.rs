use crate::error::{GraphError, Result};
use crate::id::NodeId;
use crate::value::PropertyValue;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::RangeBounds;

/// Key identifying an index on a specific Label and Property key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IndexKey {
    pub label: String,
    pub property_key: String,
}

impl IndexKey {
    pub fn new<S1: Into<String>, S2: Into<String>>(label: S1, property_key: S2) -> Self {
        Self {
            label: label.into(),
            property_key: property_key.into(),
        }
    }
}

/// An in-memory secondary index backed by a BTreeMap, providing exact and range lookups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertyIndex {
    entries: BTreeMap<PropertyValue, HashSet<NodeId>>,
}

impl PropertyIndex {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Insert a mapping from value to node_id.
    pub fn insert(&mut self, val: PropertyValue, node_id: NodeId) {
        self.entries.entry(val).or_default().insert(node_id);
    }

    /// Remove a mapping for a specific value and node_id.
    pub fn remove(&mut self, val: &PropertyValue, node_id: NodeId) -> bool {
        if let Some(set) = self.entries.get_mut(val) {
            let removed = set.remove(&node_id);
            if set.is_empty() {
                self.entries.remove(val);
            }
            removed
        } else {
            false
        }
    }

    /// Find node IDs matching an exact value.
    pub fn find_exact(&self, val: &PropertyValue) -> Vec<NodeId> {
        self.entries
            .get(val)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Find node IDs within a given value range.
    pub fn find_range<R>(&self, range: R) -> Vec<NodeId>
    where
        R: RangeBounds<PropertyValue>,
    {
        let mut results = Vec::new();
        for (_, set) in self.entries.range(range) {
            results.extend(set.iter().copied());
        }
        results
    }

    /// Returns the number of distinct values indexed.
    pub fn distinct_values(&self) -> usize {
        self.entries.len()
    }

    /// Returns total number of indexed entries.
    pub fn total_entries(&self) -> usize {
        self.entries.values().map(|set| set.len()).sum()
    }

    /// Clear all entries from the index.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Manages property indices and unique constraints across the graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexManager {
    indices: HashMap<IndexKey, PropertyIndex>,
    unique_constraints: HashSet<IndexKey>,
}

impl IndexManager {
    pub fn new() -> Self {
        Self {
            indices: HashMap::new(),
            unique_constraints: HashSet::new(),
        }
    }

    /// Register a secondary index on a (label, property_key) pair.
    pub fn create_index(&mut self, label: &str, property_key: &str) -> bool {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        if self.indices.contains_key(&key) {
            false
        } else {
            self.indices.insert(key, PropertyIndex::new());
            true
        }
    }

    /// Drop a secondary index on a (label, property_key) pair.
    pub fn drop_index(&mut self, label: &str, property_key: &str) -> bool {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        // Cannot drop index if it is backing a unique constraint
        if self.unique_constraints.contains(&key) {
            return false;
        }
        self.indices.remove(&key).is_some()
    }

    /// Check if an index exists on (label, property_key).
    pub fn has_index(&self, label: &str, property_key: &str) -> bool {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        self.indices.contains_key(&key)
    }

    /// Register a unique constraint on (label, property_key).
    /// Automatically ensures an index exists to back the constraint.
    pub fn create_unique_constraint(&mut self, label: &str, property_key: &str) -> bool {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        self.indices.entry(key.clone()).or_default();
        self.unique_constraints.insert(key)
    }

    /// Drop a unique constraint. Note: the underlying index remains active.
    pub fn drop_unique_constraint(&mut self, label: &str, property_key: &str) -> bool {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        self.unique_constraints.remove(&key)
    }

    /// Check if a unique constraint exists on (label, property_key).
    pub fn has_unique_constraint(&self, label: &str, property_key: &str) -> bool {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        self.unique_constraints.contains(&key)
    }

    /// Validate that setting a value for (label, property_key) does not violate uniqueness.
    pub fn validate_unique(
        &self,
        label: &str,
        property_key: &str,
        val: &PropertyValue,
        current_node_id: Option<NodeId>,
    ) -> Result<()> {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };

        if self.unique_constraints.contains(&key) {
            if let Some(idx) = self.indices.get(&key) {
                let existing = idx.find_exact(val);
                for id in existing {
                    if Some(id) != current_node_id {
                        return Err(GraphError::ConstraintViolation(format!(
                            "Unique constraint violated on (:{label} {{{property_key}: {val}}}) - already held by Node {id}"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Index a property value for a node if an index exists for (label, key).
    pub fn index_property(
        &mut self,
        label: &str,
        property_key: &str,
        val: PropertyValue,
        node_id: NodeId,
    ) {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        if let Some(idx) = self.indices.get_mut(&key) {
            idx.insert(val, node_id);
        }
    }

    /// Remove an indexed property value for a node if an index exists for (label, key).
    pub fn deindex_property(
        &mut self,
        label: &str,
        property_key: &str,
        val: &PropertyValue,
        node_id: NodeId,
    ) {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        if let Some(idx) = self.indices.get_mut(&key) {
            idx.remove(val, node_id);
        }
    }

    /// Query an exact match using an existing index, if present.
    pub fn find_exact(
        &self,
        label: &str,
        property_key: &str,
        val: &PropertyValue,
    ) -> Option<Vec<NodeId>> {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        self.indices.get(&key).map(|idx| idx.find_exact(val))
    }

    /// Query a range using an existing index, if present.
    pub fn find_range<R>(
        &self,
        label: &str,
        property_key: &str,
        range: R,
    ) -> Option<Vec<NodeId>>
    where
        R: RangeBounds<PropertyValue>,
    {
        let key = IndexKey {
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        self.indices.get(&key).map(|idx| idx.find_range(range))
    }

    /// Clear all index entries and constraints.
    pub fn clear(&mut self) {
        self.indices.clear();
        self.unique_constraints.clear();
    }
}

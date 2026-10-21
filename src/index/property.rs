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
        for (_, nodes) in self.entries.range(range) {
            results.extend(nodes.iter().copied());
        }
        results
    }

    /// Number of distinct property values in this index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the index has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all index entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Manager maintaining secondary indices and unique constraints across labels and properties.
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

    /// Create a secondary index on a (label, property_key) pair.
    pub fn create_index(&mut self, label: &str, property_key: &str) -> bool {
        let key = IndexKey::new(label, property_key);
        if self.indices.contains_key(&key) {
            false
        } else {
            self.indices.insert(key, PropertyIndex::new());
            true
        }
    }

    /// Drop an existing secondary index.
    pub fn drop_index(&mut self, label: &str, property_key: &str) -> bool {
        let key = IndexKey::new(label, property_key);
        self.indices.remove(&key).is_some()
    }

    /// Returns true if an index exists on the given label and property key.
    pub fn has_index(&self, label: &str, property_key: &str) -> bool {
        let key = IndexKey::new(label, property_key);
        self.indices.contains_key(&key)
    }

    /// Create a unique constraint on a (label, property_key) pair.
    pub fn create_unique_constraint(&mut self, label: &str, property_key: &str) -> Result<()> {
        let key = IndexKey::new(label, property_key);
        if !self.indices.contains_key(&key) {
            self.indices.insert(key.clone(), PropertyIndex::new());
        }
        self.unique_constraints.insert(key);
        Ok(())
    }

    /// Drop a unique constraint.
    pub fn drop_unique_constraint(&mut self, label: &str, property_key: &str) -> bool {
        let key = IndexKey::new(label, property_key);
        self.unique_constraints.remove(&key)
    }

    /// Returns true if a unique constraint exists on the given label and property key.
    pub fn has_unique_constraint(&self, label: &str, property_key: &str) -> bool {
        let key = IndexKey::new(label, property_key);
        self.unique_constraints.contains(&key)
    }

    /// Validate that a property value satisfies any unique constraint for the given label.
    pub fn validate_unique(
        &self,
        label: &str,
        property_key: &str,
        val: &PropertyValue,
        current_node: Option<NodeId>,
    ) -> Result<()> {
        let key = IndexKey::new(label, property_key);
        if self.unique_constraints.contains(&key) {
            if let Some(index) = self.indices.get(&key) {
                let existing = index.find_exact(val);
                for existing_id in existing {
                    if Some(existing_id) != current_node {
                        return Err(GraphError::ConstraintViolation(format!(
                            "Unique constraint violated: (:{label} {{{property_key}: {val:?}}}) already exists on node {}",
                            existing_id.0
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Index a property value for a node.
    pub fn index_property(&mut self, label: &str, property_key: &str, val: PropertyValue, node_id: NodeId) {
        let key = IndexKey::new(label, property_key);
        if let Some(index) = self.indices.get_mut(&key) {
            index.insert(val, node_id);
        }
    }

    /// Unindex a property value for a node.
    pub fn unindex_property(&mut self, label: &str, property_key: &str, val: &PropertyValue, node_id: NodeId) {
        let key = IndexKey::new(label, property_key);
        if let Some(index) = self.indices.get_mut(&key) {
            index.remove(val, node_id);
        }
    }

    /// Lookup node IDs matching an exact property value using an index.
    pub fn find_exact(&self, label: &str, property_key: &str, val: &PropertyValue) -> Option<Vec<NodeId>> {
        let key = IndexKey::new(label, property_key);
        self.indices.get(&key).map(|idx| idx.find_exact(val))
    }

    /// Lookup node IDs within a property value range using an index.
    pub fn find_range<R>(&self, label: &str, property_key: &str, range: R) -> Option<Vec<NodeId>>
    where
        R: RangeBounds<PropertyValue>,
    {
        let key = IndexKey::new(label, property_key);
        self.indices.get(&key).map(|idx| idx.find_range(range))
    }

    /// Unindex a property value for a node (alias for unindex_property).
    pub fn deindex_property(&mut self, label: &str, property_key: &str, val: &PropertyValue, node_id: NodeId) {
        self.unindex_property(label, property_key, val, node_id);
    }

    /// Clear all secondary indices and unique constraints.
    pub fn clear(&mut self) {
        self.indices.clear();
        self.unique_constraints.clear();
    }

    /// Rebuild all property indices from scratch.
    pub fn clear_all_indices(&mut self) {
        for idx in self.indices.values_mut() {
            idx.clear();
        }
    }
}

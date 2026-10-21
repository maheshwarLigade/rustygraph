use crate::id::{NodeId, RelId};
use crate::value::PropertyValue;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ops::Index;

/// Traversal direction for graph relationships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// Follow outgoing edges from source to target: `(source)->(target)`.
    Outgoing,
    /// Follow incoming edges from target to source: `(source)<-(target)`.
    Incoming,
    /// Traverse in either direction (undirected view).
    Both,
}

impl Direction {
    /// Invert the direction: Outgoing becomes Incoming and vice-versa. Both remains Both.
    #[inline]
    pub fn reverse(self) -> Self {
        match self {
            Self::Outgoing => Self::Incoming,
            Self::Incoming => Self::Outgoing,
            Self::Both => Self::Both,
        }
    }

    /// Check if this direction permits outgoing traversal.
    #[inline]
    pub fn includes_outgoing(self) -> bool {
        matches!(self, Self::Outgoing | Self::Both)
    }

    /// Check if this direction permits incoming traversal.
    #[inline]
    pub fn includes_incoming(self) -> bool {
        matches!(self, Self::Incoming | Self::Both)
    }
}

/// A container for key-value properties associated with a Node or Relationship.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Properties {
    entries: HashMap<String, PropertyValue>,
}

impl Properties {
    /// Create an empty property map.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Set a property key to a value.
    pub fn insert<K: Into<String>, V: Into<PropertyValue>>(&mut self, key: K, val: V) -> Option<PropertyValue> {
        self.entries.insert(key.into(), val.into())
    }

    /// Get a reference to a property value by key.
    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        self.entries.get(key)
    }

    /// Get a mutable reference to a property value by key.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut PropertyValue> {
        self.entries.get_mut(key)
    }

    /// Remove a property by key.
    pub fn remove(&mut self, key: &str) -> Option<PropertyValue> {
        self.entries.remove(key)
    }

    /// Check if a property key exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns the number of properties.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if there are no properties.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterator over (key, value) pairs.
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, PropertyValue> {
        self.entries.iter()
    }

    /// Iterator over keys.
    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, String, PropertyValue> {
        self.entries.keys()
    }

    /// Iterator over values.
    pub fn values(&self) -> std::collections::hash_map::Values<'_, String, PropertyValue> {
        self.entries.values()
    }
}

impl Index<&str> for Properties {
    type Output = PropertyValue;

    fn index(&self, key: &str) -> &Self::Output {
        self.get(key).unwrap_or(&PropertyValue::Null)
    }
}

impl<K: Into<String>, V: Into<PropertyValue>> FromIterator<(K, V)> for Properties {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut props = Self::new();
        for (k, v) in iter {
            props.insert(k, v);
        }
        props
    }
}

/// A Node (Vertex) in the property graph, containing an ID, zero or more labels, and properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub labels: HashSet<String>,
    pub properties: Properties,
}

impl Node {
    /// Create a new Node with the given ID.
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            labels: HashSet::new(),
            properties: Properties::new(),
        }
    }

    /// Builder method to add a label.
    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        self.labels.insert(label.into());
        self
    }

    /// Builder method to set a property.
    pub fn with_property<K: Into<String>, V: Into<PropertyValue>>(mut self, key: K, val: V) -> Self {
        self.properties.insert(key, val);
        self
    }

    /// Returns true if this node has the specified label.
    #[inline]
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.contains(label)
    }

    /// Add a label to the node. Returns true if the label was not previously present.
    pub fn add_label<S: Into<String>>(&mut self, label: S) -> bool {
        self.labels.insert(label.into())
    }

    /// Remove a label from the node. Returns true if the label was present.
    pub fn remove_label(&mut self, label: &str) -> bool {
        self.labels.remove(label)
    }

    /// Get a property value by key.
    #[inline]
    pub fn prop(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Set a property key and value.
    #[inline]
    pub fn set_prop<K: Into<String>, V: Into<PropertyValue>>(&mut self, key: K, val: V) -> Option<PropertyValue> {
        self.properties.insert(key, val)
    }

    /// Remove a property by key.
    #[inline]
    pub fn remove_prop(&mut self, key: &str) -> Option<PropertyValue> {
        self.properties.remove(key)
    }
}

/// A Relationship (Edge) connecting two nodes, with a specific type and properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: RelId,
    pub source: NodeId,
    pub target: NodeId,
    pub rel_type: String,
    pub properties: Properties,
}

impl Relationship {
    /// Create a new Relationship.
    pub fn new<S: Into<String>>(id: RelId, source: NodeId, target: NodeId, rel_type: S) -> Self {
        Self {
            id,
            source,
            target,
            rel_type: rel_type.into(),
            properties: Properties::new(),
        }
    }

    /// Builder method to add a property.
    pub fn with_property<K: Into<String>, V: Into<PropertyValue>>(mut self, key: K, val: V) -> Self {
        self.properties.insert(key, val);
        self
    }

    /// Given one endpoint of the relationship, returns the other endpoint.
    #[inline]
    pub fn other_node(&self, node: NodeId) -> Option<NodeId> {
        if node == self.source {
            Some(self.target)
        } else if node == self.target {
            Some(self.source)
        } else {
            None
        }
    }

    /// Returns true if this relationship is a self-loop (source == target).
    #[inline]
    pub fn is_self_loop(&self) -> bool {
        self.source == self.target
    }

    /// Returns true if the relationship matches the given type.
    #[inline]
    pub fn matches_type(&self, rel_type: &str) -> bool {
        self.rel_type == rel_type
    }

    /// Get a property value by key.
    #[inline]
    pub fn prop(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Set a property key and value.
    #[inline]
    pub fn set_prop<K: Into<String>, V: Into<PropertyValue>>(&mut self, key: K, val: V) -> Option<PropertyValue> {
        self.properties.insert(key, val)
    }

    /// Remove a property by key.
    #[inline]
    pub fn remove_prop(&mut self, key: &str) -> Option<PropertyValue> {
        self.properties.remove(key)
    }
}

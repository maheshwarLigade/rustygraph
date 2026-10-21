use serde::{Deserialize, Serialize};
use std::fmt;

/// Strongly typed identifier for a Node in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    /// Create a new NodeId.
    #[inline]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the raw numeric ID.
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.0)
    }
}

impl From<u64> for NodeId {
    #[inline]
    fn from(val: u64) -> Self {
        Self(val)
    }
}

impl From<NodeId> for u64 {
    #[inline]
    fn from(val: NodeId) -> Self {
        val.0
    }
}

/// Strongly typed identifier for a Relationship (Edge) in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelId(pub u64);

impl RelId {
    /// Create a new RelId.
    #[inline]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the raw numeric ID.
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RelId({})", self.0)
    }
}

impl From<u64> for RelId {
    #[inline]
    fn from(val: u64) -> Self {
        Self(val)
    }
}

impl From<RelId> for u64 {
    #[inline]
    fn from(val: RelId) -> Self {
        val.0
    }
}

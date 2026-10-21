use crate::error::Result;
use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::model::Direction;
use crate::stats::GraphStats;
use crate::value::PropertyValue;
use std::sync::{Arc, RwLock};

/// A thread-safe, concurrent wrapper around `Graph` supporting concurrent readers
/// and exclusive writer access (`Send + Sync`).
#[derive(Debug, Clone)]
pub struct ConcurrentGraph {
    inner: Arc<RwLock<Graph>>,
}

impl Default for ConcurrentGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ConcurrentGraph {
    /// Create a new thread-safe ConcurrentGraph.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Graph::new())),
        }
    }

    /// Wrap an existing Graph into a ConcurrentGraph.
    pub fn from_graph(graph: Graph) -> Self {
        Self {
            inner: Arc::new(RwLock::new(graph)),
        }
    }

    /// Execute a closure with read-only access to the underlying graph.
    pub fn read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Graph) -> R,
    {
        let guard = self.inner.read().expect("lock poisoned");
        f(&guard)
    }

    /// Execute a closure with exclusive mutable access to the underlying graph.
    pub fn write<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Graph) -> R,
    {
        let mut guard = self.inner.write().expect("lock poisoned");
        f(&mut guard)
    }

    /// Returns the total number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.read(|g| g.node_count())
    }

    /// Returns the total number of relationships in the graph.
    pub fn edge_count(&self) -> usize {
        self.read(|g| g.edge_count())
    }

    /// Create a new node.
    pub fn create_node(&self) -> NodeId {
        self.write(|g| g.create_node())
    }

    /// Create a directed edge between source and target.
    pub fn create_edge<S: Into<String>>(
        &self,
        source: NodeId,
        target: NodeId,
        rel_type: S,
    ) -> Result<RelId> {
        self.write(|g| g.create_edge(source, target, rel_type))
    }

    /// Set a property on a node.
    pub fn set_node_property<K: Into<String>, V: Into<PropertyValue>>(
        &self,
        node: NodeId,
        key: K,
        val: V,
    ) -> Result<Option<PropertyValue>> {
        self.write(|g| g.set_node_property(node, key, val))
    }

    /// Degree of a node in the given direction.
    pub fn degree(&self, node: NodeId, dir: Direction) -> Result<usize> {
        self.read(|g| g.degree(node, dir))
    }

    /// Compute summary statistics for the graph.
    pub fn stats(&self) -> GraphStats {
        self.read(|g| g.stats())
    }
}

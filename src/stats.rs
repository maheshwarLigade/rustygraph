use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Summary statistics for the graph structure, degree distributions, and schema elements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphStats {
    /// Total number of active nodes.
    pub node_count: usize,
    /// Total number of active relationships.
    pub edge_count: usize,
    /// Graph density: ratio of actual edges to maximum possible edges.
    pub density: f64,
    /// Average degree per node.
    pub average_degree: f64,
    /// Number of nodes with degree 0.
    pub isolated_nodes: usize,
    /// Number of self-loop edges.
    pub self_loops: usize,
    /// Distribution of node counts per label.
    pub label_counts: HashMap<String, usize>,
    /// Distribution of relationship counts per relationship type.
    pub type_counts: HashMap<String, usize>,
}

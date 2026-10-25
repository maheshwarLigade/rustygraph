use crate::error::{GraphError, Result};
use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::{Node, Relationship};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::index::FulltextIndex;
use crate::vector::VectorIndex;

/// Serializable dump structure containing all nodes and relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDump {
    pub nodes: Vec<Node>,
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub vector_indices: Vec<VectorIndex>,
    #[serde(default)]
    pub fulltext_indices: Vec<FulltextIndex>,
}

impl Graph {
    /// Export the entire graph structure to a serializable GraphDump.
    pub fn export_dump(&self) -> GraphDump {
        GraphDump {
            nodes: self.nodes().cloned().collect(),
            relationships: self.edges().cloned().collect(),
            vector_indices: self.vector_indices().cloned().collect(),
            fulltext_indices: self.fulltext_indices().cloned().collect(),
        }
    }

    /// Import a GraphDump into a new Graph, automatically remapping node endpoints.
    pub fn import_dump(dump: GraphDump) -> Result<Self> {
        let mut graph = Self::with_capacity(dump.nodes.len(), dump.relationships.len());
        let mut id_map: HashMap<NodeId, NodeId> = HashMap::with_capacity(dump.nodes.len());

        // Recreate nodes
        for old_node in dump.nodes {
            let labels = old_node.labels;
            let props = old_node.properties;
            let new_id = graph.create_node_with(|n| {
                n.labels = labels;
                n.properties = props;
            })?;
            id_map.insert(old_node.id, new_id);
        }

        // Recreate relationships
        for old_rel in dump.relationships {
            let new_source = *id_map.get(&old_rel.source).ok_or_else(|| {
                GraphError::InvalidOperation(format!(
                    "Relationship {} references missing source node {}",
                    old_rel.id, old_rel.source
                ))
            })?;

            let new_target = *id_map.get(&old_rel.target).ok_or_else(|| {
                GraphError::InvalidOperation(format!(
                    "Relationship {} references missing target node {}",
                    old_rel.id, old_rel.target
                ))
            })?;

            let props = old_rel.properties;
            graph.create_edge_with(new_source, new_target, old_rel.rel_type, |r| {
                r.properties = props;
            })?;
        }

        // Recreate vector indices
        for v_idx in dump.vector_indices {
            let _ = graph.create_vector_index(
                &v_idx.name,
                &v_idx.label,
                &v_idx.property_key,
                v_idx.dimensions,
                v_idx.metric,
            );
        }

        // Recreate fulltext indices
        for ft_idx in dump.fulltext_indices {
            let _ = graph.create_fulltext_index(&ft_idx.name, &ft_idx.label, &ft_idx.property_key);
        }

        Ok(graph)
    }

    /// Serialize the graph to a JSON string.
    pub fn to_json(&self) -> Result<String> {
        let dump = self.export_dump();
        serde_json::to_string_pretty(&dump)
            .map_err(|e| GraphError::SerializationError(e.to_string()))
    }

    /// Deserialize a graph from a JSON string.
    pub fn from_json(json_str: &str) -> Result<Self> {
        let dump: GraphDump = serde_json::from_str(json_str)
            .map_err(|e| GraphError::SerializationError(e.to_string()))?;
        Self::import_dump(dump)
    }
}

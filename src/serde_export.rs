use crate::error::{GraphError, Result};
use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::{Node, Relationship};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serializable dump structure containing all nodes and relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDump {
    pub nodes: Vec<Node>,
    pub relationships: Vec<Relationship>,
}

impl Graph {
    /// Export the entire graph structure to a serializable GraphDump.
    pub fn export_dump(&self) -> GraphDump {
        GraphDump {
            nodes: self.nodes().cloned().collect(),
            relationships: self.edges().cloned().collect(),
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
            .map_err(|e| GraphError::SerializationError(e.to_string()))
        ?;
        Self::import_dump(dump)
    }
}

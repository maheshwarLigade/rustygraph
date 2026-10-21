use crate::error::{GraphError, Result};
use crate::id::{NodeId, RelId};
use crate::model::{Direction, Node, Relationship};
use crate::storage::SlotArena;
use crate::value::PropertyValue;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeRecord {
    node: Node,
    outgoing: SmallVec<[RelId; 8]>,
    incoming: SmallVec<[RelId; 8]>,
}

/// The core in-memory property graph engine, providing fast O(1) mutations,
/// adjacency lookups, and secondary label/type indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    nodes: SlotArena<NodeRecord>,
    relationships: SlotArena<Relationship>,
    label_index: HashMap<String, HashSet<NodeId>>,
    type_index: HashMap<String, HashSet<RelId>>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    /// Create a new, empty in-memory Graph.
    pub fn new() -> Self {
        Self {
            nodes: SlotArena::new(),
            relationships: SlotArena::new(),
            label_index: HashMap::new(),
            type_index: HashMap::new(),
        }
    }

    /// Create a Graph with pre-allocated node and relationship capacities.
    pub fn with_capacity(node_cap: usize, rel_cap: usize) -> Self {
        Self {
            nodes: SlotArena::with_capacity(node_cap),
            relationships: SlotArena::with_capacity(rel_cap),
            label_index: HashMap::new(),
            type_index: HashMap::new(),
        }
    }

    // =========================================================================
    // Node Operations
    // =========================================================================

    /// Create a new empty node.
    pub fn create_node(&mut self) -> NodeId {
        let (raw_id, _) = self.nodes.insert_with(|raw_id| NodeRecord {
            node: Node::new(NodeId(raw_id)),
            outgoing: SmallVec::new(),
            incoming: SmallVec::new(),
        });
        NodeId(raw_id)
    }

    /// Create a node with customized initialization (labels, properties).
    pub fn create_node_with<F>(&mut self, init: F) -> NodeId
    where
        F: FnOnce(&mut Node),
    {
        let (raw_id, record) = self.nodes.insert_with(|raw_id| {
            let mut node = Node::new(NodeId(raw_id));
            init(&mut node);
            NodeRecord {
                node,
                outgoing: SmallVec::new(),
                incoming: SmallVec::new(),
            }
        });
        let node_id = NodeId(raw_id);

        // Update label index for any labels assigned in init
        for label in &record.node.labels {
            self.label_index
                .entry(label.clone())
                .or_default()
                .insert(node_id);
        }

        node_id
    }

    /// Returns a reference to a Node by ID, or None if it doesn't exist.
    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.raw()).map(|rec| &rec.node)
    }

    /// Returns a mutable reference to a Node by ID, or None if it doesn't exist.
    /// Note: to modify labels, use `add_node_label` or `remove_node_label` to keep index synchronized.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.raw()).map(|rec| &mut rec.node)
    }

    /// Returns true if a node exists with the given ID.
    #[inline]
    pub fn contains_node(&self, id: NodeId) -> bool {
        self.nodes.contains(id.raw())
    }

    /// Returns total number of active nodes in the graph.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Iterate over references to all active nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().map(|(_, rec)| &rec.node)
    }

    /// Add a label to a node and update secondary index.
    pub fn add_node_label<S: Into<String>>(&mut self, id: NodeId, label: S) -> Result<bool> {
        let label_str = label.into();
        let rec = self
            .nodes
            .get_mut(id.raw())
            .ok_or_else(|| GraphError::NodeNotFound(id.raw()))?;

        let inserted = rec.node.add_label(label_str.clone());
        if inserted {
            self.label_index
                .entry(label_str)
                .or_default()
                .insert(id);
        }
        Ok(inserted)
    }

    /// Remove a label from a node and update secondary index.
    pub fn remove_node_label(&mut self, id: NodeId, label: &str) -> Result<bool> {
        let rec = self
            .nodes
            .get_mut(id.raw())
            .ok_or_else(|| GraphError::NodeNotFound(id.raw()))?;

        let removed = rec.node.remove_label(label);
        if removed {
            if let Some(set) = self.label_index.get_mut(label) {
                set.remove(&id);
                if set.is_empty() {
                    self.label_index.remove(label);
                }
            }
        }
        Ok(removed)
    }

    /// Set a property on a node.
    pub fn set_node_property<K: Into<String>, V: Into<PropertyValue>>(
        &mut self,
        id: NodeId,
        key: K,
        val: V,
    ) -> Result<Option<PropertyValue>> {
        let rec = self
            .nodes
            .get_mut(id.raw())
            .ok_or_else(|| GraphError::NodeNotFound(id.raw()))?;
        Ok(rec.node.set_prop(key, val))
    }

    /// Get a property reference from a node.
    pub fn get_node_property(&self, id: NodeId, key: &str) -> Option<&PropertyValue> {
        self.get_node(id).and_then(|n| n.prop(key))
    }

    /// Remove a property from a node.
    pub fn remove_node_property(&mut self, id: NodeId, key: &str) -> Result<Option<PropertyValue>> {
        let rec = self
            .nodes
            .get_mut(id.raw())
            .ok_or_else(|| GraphError::NodeNotFound(id.raw()))?;
        Ok(rec.node.remove_prop(key))
    }

    /// Delete a node and all incoming/outgoing relationships connected to it.
    pub fn delete_node(&mut self, id: NodeId) -> Result<Node> {
        let rec = self
            .nodes
            .remove(id.raw())
            .ok_or_else(|| GraphError::NodeNotFound(id.raw()))?;

        // Remove from label index
        for label in &rec.node.labels {
            if let Some(set) = self.label_index.get_mut(label) {
                set.remove(&id);
                if set.is_empty() {
                    self.label_index.remove(label);
                }
            }
        }

        // Cascade delete all outgoing relationships
        for rel_id in rec.outgoing {
            if let Some(rel) = self.relationships.remove(rel_id.raw()) {
                if let Some(type_set) = self.type_index.get_mut(&rel.rel_type) {
                    type_set.remove(&rel_id);
                }
                // Remove from target node's incoming adjacency if target is not self
                if rel.target != id {
                    if let Some(target_rec) = self.nodes.get_mut(rel.target.raw()) {
                        target_rec.incoming.retain(|r| *r != rel_id);
                    }
                }
            }
        }

        // Cascade delete all incoming relationships
        for rel_id in rec.incoming {
            if let Some(rel) = self.relationships.remove(rel_id.raw()) {
                if let Some(type_set) = self.type_index.get_mut(&rel.rel_type) {
                    type_set.remove(&rel_id);
                }
                // Remove from source node's outgoing adjacency if source is not self
                if rel.source != id {
                    if let Some(source_rec) = self.nodes.get_mut(rel.source.raw()) {
                        source_rec.outgoing.retain(|r| *r != rel_id);
                    }
                }
            }
        }

        Ok(rec.node)
    }

    // =========================================================================
    // Relationship Operations
    // =========================================================================

    /// Create a directed relationship from source node to target node.
    pub fn create_edge<S: Into<String>>(
        &mut self,
        source: NodeId,
        target: NodeId,
        rel_type: S,
    ) -> Result<RelId> {
        self.create_edge_with(source, target, rel_type, |_| {})
    }

    /// Create a directed relationship with custom properties initialization.
    pub fn create_edge_with<S: Into<String>, F>(
        &mut self,
        source: NodeId,
        target: NodeId,
        rel_type: S,
        init: F,
    ) -> Result<RelId>
    where
        F: FnOnce(&mut Relationship),
    {
        if !self.contains_node(source) {
            return Err(GraphError::NodeNotFound(source.raw()));
        }
        if !self.contains_node(target) {
            return Err(GraphError::NodeNotFound(target.raw()));
        }

        let type_str = rel_type.into();
        let (raw_id, _) = self.relationships.insert_with(|raw_id| {
            let mut edge = Relationship::new(RelId(raw_id), source, target, type_str.clone());
            init(&mut edge);
            edge
        });
        let edge_id = RelId(raw_id);

        // Update adjacency vectors
        let source_rec = self.nodes.get_mut(source.raw()).unwrap();
        source_rec.outgoing.push(edge_id);

        let target_rec = self.nodes.get_mut(target.raw()).unwrap();
        target_rec.incoming.push(edge_id);

        // Update relationship type index
        self.type_index
            .entry(type_str)
            .or_default()
            .insert(edge_id);

        Ok(edge_id)
    }

    /// Returns a reference to a Relationship by ID, or None if not found.
    pub fn get_edge(&self, id: RelId) -> Option<&Relationship> {
        self.relationships.get(id.raw())
    }

    /// Returns a mutable reference to a Relationship by ID, or None if not found.
    pub fn get_edge_mut(&mut self, id: RelId) -> Option<&mut Relationship> {
        self.relationships.get_mut(id.raw())
    }

    /// Returns true if a relationship with the given ID exists.
    #[inline]
    pub fn contains_edge(&self, id: RelId) -> bool {
        self.relationships.contains(id.raw())
    }

    /// Returns the total number of relationships in the graph.
    #[inline]
    pub fn edge_count(&self) -> usize {
        self.relationships.len()
    }

    /// Iterate over references to all relationships in the graph.
    pub fn edges(&self) -> impl Iterator<Item = &Relationship> {
        self.relationships.iter().map(|(_, rel)| rel)
    }

    /// Set a property on a relationship.
    pub fn set_edge_property<K: Into<String>, V: Into<PropertyValue>>(
        &mut self,
        id: RelId,
        key: K,
        val: V,
    ) -> Result<Option<PropertyValue>> {
        let rel = self
            .relationships
            .get_mut(id.raw())
            .ok_or_else(|| GraphError::RelationshipNotFound(id.raw()))?;
        Ok(rel.set_prop(key, val))
    }

    /// Get a property reference from a relationship.
    pub fn get_edge_property(&self, id: RelId, key: &str) -> Option<&PropertyValue> {
        self.get_edge(id).and_then(|r| r.prop(key))
    }

    /// Remove a property from a relationship.
    pub fn remove_edge_property(&mut self, id: RelId, key: &str) -> Result<Option<PropertyValue>> {
        let rel = self
            .relationships
            .get_mut(id.raw())
            .ok_or_else(|| GraphError::RelationshipNotFound(id.raw()))?;
        Ok(rel.remove_prop(key))
    }

    /// Delete a relationship by ID, updating source and target adjacency.
    pub fn delete_edge(&mut self, id: RelId) -> Result<Relationship> {
        let rel = self
            .relationships
            .remove(id.raw())
            .ok_or_else(|| GraphError::RelationshipNotFound(id.raw()))?;

        // Update source node's outgoing adjacency
        if let Some(src) = self.nodes.get_mut(rel.source.raw()) {
            src.outgoing.retain(|r| *r != id);
        }

        // Update target node's incoming adjacency
        if let Some(dst) = self.nodes.get_mut(rel.target.raw()) {
            dst.incoming.retain(|r| *r != id);
        }

        // Update type index
        if let Some(type_set) = self.type_index.get_mut(&rel.rel_type) {
            type_set.remove(&id);
            if type_set.is_empty() {
                self.type_index.remove(&rel.rel_type);
            }
        }

        Ok(rel)
    }

    // =========================================================================
    // Graph Query, Traversal & Adjacency Operations
    // =========================================================================

    /// Get degree of a node according to direction (Outgoing, Incoming, or Both).
    pub fn degree(&self, node: NodeId, dir: Direction) -> Result<usize> {
        let rec = self
            .nodes
            .get(node.raw())
            .ok_or_else(|| GraphError::NodeNotFound(node.raw()))?;

        match dir {
            Direction::Outgoing => Ok(rec.outgoing.len()),
            Direction::Incoming => Ok(rec.incoming.len()),
            Direction::Both => Ok(rec.outgoing.len() + rec.incoming.len()),
        }
    }

    /// Return relationship IDs adjacent to a node according to direction.
    pub fn incident_edge_ids(&self, node: NodeId, dir: Direction) -> Result<Vec<RelId>> {
        let rec = self
            .nodes
            .get(node.raw())
            .ok_or_else(|| GraphError::NodeNotFound(node.raw()))?;

        let mut list = Vec::new();
        if dir.includes_outgoing() {
            list.extend_from_slice(&rec.outgoing);
        }
        if dir.includes_incoming() {
            list.extend_from_slice(&rec.incoming);
        }
        Ok(list)
    }

    /// Iterate over references to relationships connected to a node in a given direction.
    pub fn incident_edges(
        &self,
        node: NodeId,
        dir: Direction,
    ) -> Result<Vec<&Relationship>> {
        let edge_ids = self.incident_edge_ids(node, dir)?;
        let mut edges = Vec::with_capacity(edge_ids.len());
        for id in edge_ids {
            if let Some(edge) = self.get_edge(id) {
                edges.push(edge);
            }
        }
        Ok(edges)
    }

    /// Iterate over incident relationships matching a specific relationship type.
    pub fn incident_edges_with_type(
        &self,
        node: NodeId,
        dir: Direction,
        rel_type: &str,
    ) -> Result<Vec<&Relationship>> {
        let edges = self.incident_edges(node, dir)?;
        Ok(edges
            .into_iter()
            .filter(|r| r.matches_type(rel_type))
            .collect())
    }

    /// Retrieve neighbor node IDs reachable from `node` in given direction.
    pub fn neighbors(&self, node: NodeId, dir: Direction) -> Result<Vec<NodeId>> {
        let edges = self.incident_edges(node, dir)?;
        let mut neighbors = Vec::with_capacity(edges.len());
        for edge in edges {
            if let Some(other) = edge.other_node(node) {
                neighbors.push(other);
            }
        }
        Ok(neighbors)
    }

    /// Find all node IDs bearing a given label.
    pub fn nodes_with_label(&self, label: &str) -> Vec<NodeId> {
        self.label_index
            .get(label)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Find all relationship IDs of a given relationship type.
    pub fn edges_with_type(&self, rel_type: &str) -> Vec<RelId> {
        self.type_index
            .get(rel_type)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Clear the entire graph, removing all nodes, relationships, and index entries.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.relationships.clear();
        self.label_index.clear();
        self.type_index.clear();
    }

    /// Returns true if the graph has neither nodes nor relationships.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.relationships.is_empty()
    }
}

use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::model::{Direction, Node, Relationship};
use crate::value::PropertyValue;
use std::cmp::Ordering;
use std::collections::HashMap;

/// Runtime context holding bindings for matched node and relationship aliases.
#[derive(Debug, Clone)]
pub struct QueryContext<'a> {
    graph: &'a Graph,
    node_bindings: HashMap<String, NodeId>,
    edge_bindings: HashMap<String, RelId>,
}

impl<'a> QueryContext<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        Self {
            graph,
            node_bindings: HashMap::new(),
            edge_bindings: HashMap::new(),
        }
    }

    /// Retrieve the Node bound to `alias`.
    pub fn node(&self, alias: &str) -> Option<&'a Node> {
        self.node_bindings
            .get(alias)
            .and_then(|&id| self.graph.get_node(id))
    }

    /// Retrieve the Relationship bound to `alias`.
    pub fn edge(&self, alias: &str) -> Option<&'a Relationship> {
        self.edge_bindings
            .get(alias)
            .and_then(|&id| self.graph.get_edge(id))
    }

    /// Retrieve the NodeId bound to `alias`.
    pub fn node_id(&self, alias: &str) -> Option<NodeId> {
        self.node_bindings.get(alias).copied()
    }

    /// Retrieve the RelId bound to `alias`.
    pub fn edge_id(&self, alias: &str) -> Option<RelId> {
        self.edge_bindings.get(alias).copied()
    }

    /// Get a property value from a bound node or relationship.
    pub fn prop(&self, alias: &str, key: &str) -> Option<&'a PropertyValue> {
        if let Some(n) = self.node(alias) {
            n.prop(key)
        } else if let Some(e) = self.edge(alias) {
            e.prop(key)
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct NodePattern {
    alias: String,
    label: Option<String>,
    properties: Vec<(String, PropertyValue)>,
}

#[derive(Clone)]
struct RelPattern {
    alias: Option<String>,
    rel_type: Option<String>,
    direction: Direction,
    target_node: NodePattern,
}

/// Declarative query builder providing Neo4j Cypher-like pattern matching,
/// filtering, sorting, pagination, and projection in native Rust.
pub struct QueryBuilder<'a> {
    graph: &'a Graph,
    root_node: Option<NodePattern>,
    steps: Vec<RelPattern>,
    filters: Vec<Box<dyn Fn(&QueryContext<'a>) -> bool + 'a>>,
    sorters: Vec<Box<dyn Fn(&QueryContext<'a>, &QueryContext<'a>) -> Ordering + 'a>>,
    skip_count: Option<usize>,
    limit_count: Option<usize>,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        Self {
            graph,
            root_node: None,
            steps: Vec::new(),
            filters: Vec::new(),
            sorters: Vec::new(),
            skip_count: None,
            limit_count: None,
        }
    }

    /// Begin pattern matching from a node with a given variable alias.
    pub fn match_node<S: Into<String>>(mut self, alias: S) -> Self {
        self.root_node = Some(NodePattern {
            alias: alias.into(),
            label: None,
            properties: Vec::new(),
        });
        self
    }

    /// Filter the most recently declared node pattern to have a specific label.
    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        let label_str = label.into();
        if let Some(last_step) = self.steps.last_mut() {
            last_step.target_node.label = Some(label_str);
        } else if let Some(root) = self.root_node.as_mut() {
            root.label = Some(label_str);
        }
        self
    }

    /// Filter the most recently declared node pattern to match an exact property key and value.
    pub fn with_prop<K: Into<String>, V: Into<PropertyValue>>(mut self, key: K, val: V) -> Self {
        let entry = (key.into(), val.into());
        if let Some(last_step) = self.steps.last_mut() {
            last_step.target_node.properties.push(entry);
        } else if let Some(root) = self.root_node.as_mut() {
            root.properties.push(entry);
        }
        self
    }

    /// Follow an outgoing relationship `-[rel_type]-> (target_alias)`.
    pub fn outgoing<R: Into<String>, T: Into<String>>(
        self,
        rel_type: R,
        target_alias: T,
    ) -> Self {
        self.rel(None, Some(rel_type.into()), Direction::Outgoing, target_alias.into())
    }

    /// Follow an outgoing relationship binding the edge to an alias: `-[edge_alias:rel_type]-> (target_alias)`.
    pub fn outgoing_as<E: Into<String>, R: Into<String>, T: Into<String>>(
        self,
        edge_alias: E,
        rel_type: R,
        target_alias: T,
    ) -> Self {
        self.rel(Some(edge_alias.into()), Some(rel_type.into()), Direction::Outgoing, target_alias.into())
    }

    /// Follow an incoming relationship `<-[rel_type]- (target_alias)`.
    pub fn incoming<R: Into<String>, T: Into<String>>(
        self,
        rel_type: R,
        target_alias: T,
    ) -> Self {
        self.rel(None, Some(rel_type.into()), Direction::Incoming, target_alias.into())
    }

    /// Follow an incoming relationship binding the edge to an alias: `<-[edge_alias:rel_type]- (target_alias)`.
    pub fn incoming_as<E: Into<String>, R: Into<String>, T: Into<String>>(
        self,
        edge_alias: E,
        rel_type: R,
        target_alias: T,
    ) -> Self {
        self.rel(Some(edge_alias.into()), Some(rel_type.into()), Direction::Incoming, target_alias.into())
    }

    /// Follow an undirected / bidirectional relationship `-[rel_type]- (target_alias)`.
    pub fn rel_both<R: Into<String>, T: Into<String>>(
        self,
        rel_type: R,
        target_alias: T,
    ) -> Self {
        self.rel(None, Some(rel_type.into()), Direction::Both, target_alias.into())
    }

    /// Follow a relationship with full configuration.
    pub fn rel(
        mut self,
        edge_alias: Option<String>,
        rel_type: Option<String>,
        direction: Direction,
        target_alias: String,
    ) -> Self {
        self.steps.push(RelPattern {
            alias: edge_alias,
            rel_type,
            direction,
            target_node: NodePattern {
                alias: target_alias,
                label: None,
                properties: Vec::new(),
            },
        });
        self
    }

    /// Add a WHERE filter predicate evaluated against the query context.
    pub fn filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&QueryContext<'a>) -> bool + 'a,
    {
        self.filters.push(Box::new(predicate));
        self
    }

    /// Add an ORDER BY clause sorting results by an extracted key.
    pub fn order_by<K, F>(mut self, key_fn: F, ascending: bool) -> Self
    where
        F: Fn(&QueryContext<'a>) -> K + 'a,
        K: Ord,
    {
        self.sorters.push(Box::new(move |a, b| {
            let ord = key_fn(a).cmp(&key_fn(b));
            if ascending {
                ord
            } else {
                ord.reverse()
            }
        }));
        self
    }

    /// Skip the first `n` matching rows (pagination).
    pub fn skip(mut self, n: usize) -> Self {
        self.skip_count = Some(n);
        self
    }

    /// Limit results to at most `n` rows.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit_count = Some(n);
        self
    }

    /// Execute the pattern match and return raw QueryContext rows.
    pub fn execute(self) -> Vec<QueryContext<'a>> {
        let Some(root) = &self.root_node else {
            return Vec::new();
        };

        // 1. Identify starting candidate nodes using indexes or label filtering
        let candidates: Vec<NodeId> = if let Some((first_key, first_val)) = root.properties.first() {
            if let Some(lbl) = &root.label {
                self.graph.find_nodes_by_property(lbl, first_key, first_val)
            } else {
                self.graph
                    .nodes()
                    .filter(|n| n.prop(first_key) == Some(first_val))
                    .map(|n| n.id)
                    .collect()
            }
        } else if let Some(lbl) = &root.label {
            self.graph.nodes_with_label(lbl)
        } else {
            self.graph.nodes().map(|n| n.id).collect()
        };

        // Filter root candidates by remaining property requirements
        let mut contexts = Vec::new();
        for id in candidates {
            if let Some(node) = self.graph.get_node(id) {
                if let Some(lbl) = &root.label {
                    if !node.has_label(lbl) {
                        continue;
                    }
                }
                let mut matches_all_props = true;
                for (k, v) in &root.properties {
                    if node.prop(k) != Some(v) {
                        matches_all_props = false;
                        break;
                    }
                }
                if matches_all_props {
                    let mut ctx = QueryContext::new(self.graph);
                    ctx.node_bindings.insert(root.alias.clone(), id);
                    contexts.push(ctx);
                }
            }
        }

        // 2. Expand through each relationship step in the pattern chain
        let mut step_tip_alias = root.alias.clone();
        for step in &self.steps {
            let mut next_contexts = Vec::new();
            let current_node_alias = step_tip_alias.clone();

            for ctx in contexts {
                let current_node_id = ctx.node_bindings[&current_node_alias];

                if let Ok(edges) = self.graph.incident_edges(current_node_id, step.direction) {
                    for edge in edges {
                        if let Some(t) = &step.rel_type {
                            if !edge.matches_type(t) {
                                continue;
                            }
                        }

                        if let Some(target_id) = edge.other_node(current_node_id) {
                            if let Some(target_node) = self.graph.get_node(target_id) {
                                // Check label
                                if let Some(lbl) = &step.target_node.label {
                                    if !target_node.has_label(lbl) {
                                        continue;
                                    }
                                }

                                // Check properties
                                let mut matches_props = true;
                                for (k, v) in &step.target_node.properties {
                                    if target_node.prop(k) != Some(v) {
                                        matches_props = false;
                                        break;
                                    }
                                }

                                if matches_props {
                                    let mut new_ctx = ctx.clone();
                                    new_ctx
                                        .node_bindings
                                        .insert(step.target_node.alias.clone(), target_id);
                                    if let Some(edge_alias) = &step.alias {
                                        new_ctx
                                            .edge_bindings
                                            .insert(edge_alias.clone(), edge.id);
                                    }
                                    next_contexts.push(new_ctx);
                                }
                            }
                        }
                    }
                }
            }

            contexts = next_contexts;
            step_tip_alias = step.target_node.alias.clone();
        }

        // 3. Apply WHERE filters
        if !self.filters.is_empty() {
            contexts.retain(|ctx| self.filters.iter().all(|f| f(ctx)));
        }

        // 4. Apply ORDER BY sorters
        if !self.sorters.is_empty() {
            contexts.sort_by(|a, b| {
                for sorter in &self.sorters {
                    let ord = sorter(a, b);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                Ordering::Equal
            });
        }

        // 5. Apply pagination (skip and limit)
        let skipped = if let Some(skip) = self.skip_count {
            contexts.into_iter().skip(skip).collect::<Vec<_>>()
        } else {
            contexts
        };

        if let Some(limit) = self.limit_count {
            skipped.into_iter().take(limit).collect()
        } else {
            skipped
        }
    }

    /// Execute the query and project each matched row into a customized output structure.
    pub fn project<T, F>(self, projection: F) -> Vec<T>
    where
        F: Fn(&QueryContext<'a>) -> T,
    {
        self.execute().into_iter().map(|ctx| projection(&ctx)).collect()
    }
}

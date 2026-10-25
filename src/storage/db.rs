use super::snapshot::SnapshotManager;
use super::transaction::TransactionManager;
use super::wal::{FsyncPolicy, WalManager, WalOp};
use crate::concurrent::ConcurrentGraph;
use crate::cypher::{CypherResult, is_cypher_mutation};
use crate::error::{Error, GraphError, Result};
use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::index::FulltextSearchResult;
use crate::model::{Direction, Node, Relationship};
use crate::query::cypher::QueryPlan;
use crate::value::PropertyValue;
use crate::vector::{VectorSearchResult, VectorSimilarity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Database configuration options for standalone deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub data_dir: PathBuf,
    pub fsync_policy: String, // "always", "periodic", "never"
    pub keep_snapshots: usize,
    pub auto_checkpoint_ops: u64, // trigger checkpoint every N operations
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            fsync_policy: "always".to_string(),
            keep_snapshots: 3,
            auto_checkpoint_ops: 10_000,
        }
    }
}

/// Statistics and health metrics for the standalone database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub node_count: u64,
    pub edge_count: u64,
    pub current_wal_sequence: u64,
    pub last_checkpoint_sequence: u64,
    pub uncheckpointed_ops: u64,
    pub uptime_seconds: u64,
    #[serde(default)]
    pub estimated_memory_bytes: u64,
    #[serde(default)]
    pub active_transactions: usize,
    #[serde(default)]
    pub vector_index_count: usize,
    #[serde(default)]
    pub fulltext_index_count: usize,
    #[serde(default)]
    pub property_index_count: usize,
}

/// Standalone, crash-resilient NoSQL property graph database engine with WAL durability.
pub struct StandaloneDatabase {
    config: DatabaseConfig,
    graph: Arc<ConcurrentGraph>,
    wal: Arc<WalManager>,
    snapshots: Arc<SnapshotManager>,
    tx_manager: Arc<TransactionManager>,
    last_checkpoint_seq: AtomicU64,
    ops_since_checkpoint: AtomicU64,
    start_time: std::time::Instant,
}

impl StandaloneDatabase {
    /// Opens or creates a standalone database with full WAL crash recovery.
    pub fn open(config: DatabaseConfig) -> Result<Self> {
        let data_dir = &config.data_dir;
        std::fs::create_dir_all(data_dir)?;

        let wal_dir = data_dir.join("wal");
        let snap_dir = data_dir.join("snapshots");

        let fsync_policy = match config.fsync_policy.to_lowercase().as_str() {
            "never" => FsyncPolicy::Never,
            "periodic" => FsyncPolicy::Periodic(std::time::Duration::from_millis(500)),
            _ => FsyncPolicy::Always,
        };

        let wal = Arc::new(WalManager::open(&wal_dir, fsync_policy)?);
        let snapshots = Arc::new(SnapshotManager::new(&snap_dir, config.keep_snapshots)?);
        let tx_manager = Arc::new(TransactionManager::new(Duration::from_secs(30)));

        // Step 1: Recover from latest snapshot if present
        let (mut graph, snapshot_seq) = match snapshots.load_latest_snapshot()? {
            Some((seq, g)) => (g, seq),
            None => (Graph::new(), 0),
        };

        // Step 2: Replay WAL records newer than the snapshot
        let all_records = wal.read_all_records()?;
        for rec in all_records {
            if rec.sequence > snapshot_seq {
                Self::apply_wal_op(&mut graph, rec.op)?;
            }
        }

        let concurrent_graph = Arc::new(ConcurrentGraph::from_graph(graph));

        Ok(Self {
            config,
            graph: concurrent_graph,
            wal,
            snapshots,
            tx_manager,
            last_checkpoint_seq: AtomicU64::new(snapshot_seq),
            ops_since_checkpoint: AtomicU64::new(0),
            start_time: std::time::Instant::now(),
        })
    }

    /// Returns the active database configuration.
    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    /// Returns a reference to the underlying ConcurrentGraph.
    pub fn graph(&self) -> Arc<ConcurrentGraph> {
        Arc::clone(&self.graph)
    }

    /// Returns the current statistics of the database.
    pub fn stats(&self) -> DatabaseStats {
        let (node_count, edge_count, v_count, ft_count, prop_count) = self.graph.with_read(|g| {
            (
                g.node_count(),
                g.edge_count(),
                g.vector_indices().count(),
                g.fulltext_indices().count(),
                g.property_indices().len(),
            )
        });
        let current_seq = self.wal.current_sequence();
        let last_snap = self.last_checkpoint_seq.load(Ordering::Relaxed);
        let uncheckpointed = current_seq.saturating_sub(last_snap);
        let active_tx = self.tx_manager.list_active().len();
        let estimated_mem =
            (node_count * 128 + edge_count * 96 + v_count * 1024 + ft_count * 2048 + 4096) as u64;

        DatabaseStats {
            node_count: node_count as u64,
            edge_count: edge_count as u64,
            current_wal_sequence: current_seq,
            last_checkpoint_sequence: last_snap,
            uncheckpointed_ops: uncheckpointed,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            estimated_memory_bytes: estimated_mem,
            active_transactions: active_tx,
            vector_index_count: v_count,
            fulltext_index_count: ft_count,
            property_index_count: prop_count,
        }
    }

    // =========================================================================
    // Mutations (WAL Durability + In-Memory Apply)
    // =========================================================================

    /// Creates a new node with durability guarantees.
    pub fn create_node(
        &self,
        labels: Vec<String>,
        properties: HashMap<String, PropertyValue>,
    ) -> Result<NodeId> {
        // Reserve NodeId in graph memory
        let node_id = self.graph.with_write(|g| {
            g.create_node_with(|n| {
                for l in &labels {
                    n.add_label(l);
                }
                for (k, v) in &properties {
                    n.set_prop(k, v.clone());
                }
            })
        })?;

        // Append to WAL
        let op = WalOp::CreateNode {
            id: node_id.raw(),
            labels,
            properties: properties.into_iter().collect(),
        };
        self.wal.append(op)?;
        self.track_operation();

        Ok(node_id)
    }

    /// Retrieves a node by ID.
    pub fn get_node(&self, id: NodeId) -> Option<Node> {
        self.graph.with_read(|g| g.get_node(id).cloned())
    }

    /// Deletes a node by ID with durability guarantees.
    pub fn delete_node(&self, id: NodeId) -> Result<bool> {
        let existed = self.graph.with_write(|g| g.delete_node(id).is_ok());
        if existed {
            self.wal.append(WalOp::DeleteNode { id: id.raw() })?;
            self.track_operation();
        }
        Ok(existed)
    }

    /// Creates a directed relationship between two nodes with durability guarantees.
    pub fn create_edge(
        &self,
        source: NodeId,
        target: NodeId,
        rel_type: &str,
        properties: HashMap<String, PropertyValue>,
    ) -> Result<RelId> {
        let edge_id = self.graph.with_write(|g| {
            g.create_edge_with(source, target, rel_type, |r| {
                for (k, v) in &properties {
                    r.set_prop(k, v.clone());
                }
            })
        })?;

        let op = WalOp::CreateEdge {
            id: edge_id.raw(),
            source: source.raw(),
            target: target.raw(),
            rel_type: rel_type.to_string(),
            properties: properties.into_iter().collect(),
        };
        self.wal.append(op)?;
        self.track_operation();

        Ok(edge_id)
    }

    /// Retrieves a relationship by ID.
    pub fn get_edge(&self, id: RelId) -> Option<Relationship> {
        self.graph.with_read(|g| g.get_edge(id).cloned())
    }

    /// Deletes a relationship by ID with durability guarantees.
    pub fn delete_edge(&self, id: RelId) -> Result<bool> {
        let existed = self.graph.with_write(|g| g.delete_edge(id).is_ok());
        if existed {
            self.wal.append(WalOp::DeleteEdge { id: id.raw() })?;
            self.track_operation();
        }
        Ok(existed)
    }

    /// High-throughput batch ingestion of nodes with atomic durability.
    pub fn bulk_import_nodes(
        &self,
        items: Vec<(Vec<String>, HashMap<String, PropertyValue>)>,
    ) -> Result<Vec<NodeId>> {
        let mut ids = Vec::with_capacity(items.len());
        self.graph.with_write(|g| {
            for (labels, props) in &items {
                let id = g.create_node_with(|n| {
                    for l in labels {
                        n.add_label(l);
                    }
                    for (k, v) in props {
                        n.set_prop(k, v.clone());
                    }
                })?;
                ids.push(id);
            }
            Ok::<(), Error>(())
        })?;

        for (i, (labels, props)) in items.into_iter().enumerate() {
            let op = WalOp::CreateNode {
                id: ids[i].raw(),
                labels,
                properties: props.into_iter().collect(),
            };
            self.wal.append(op)?;
            self.track_operation();
        }

        Ok(ids)
    }

    /// High-throughput batch ingestion of edges with atomic durability.
    pub fn bulk_import_edges(
        &self,
        items: Vec<(NodeId, NodeId, String, HashMap<String, PropertyValue>)>,
    ) -> Result<Vec<RelId>> {
        let mut ids = Vec::with_capacity(items.len());
        self.graph.with_write(|g| {
            for (src, dst, rel_type, props) in &items {
                let id = g.create_edge_with(*src, *dst, rel_type, |r| {
                    for (k, v) in props {
                        r.set_prop(k, v.clone());
                    }
                })?;
                ids.push(id);
            }
            Ok::<(), Error>(())
        })?;

        for (i, (src, dst, rel_type, props)) in items.into_iter().enumerate() {
            let op = WalOp::CreateEdge {
                id: ids[i].raw(),
                source: src.raw(),
                target: dst.raw(),
                rel_type,
                properties: props.into_iter().collect(),
            };
            self.wal.append(op)?;
            self.track_operation();
        }

        Ok(ids)
    }

    /// Set an automatic Time-To-Live (TTL) on a node.
    pub fn set_node_ttl(&self, node: NodeId, ttl: std::time::Duration) -> Result<()> {
        self.graph.with_write(|g| g.set_node_ttl(node, ttl))
    }

    /// Prune all nodes whose TTL has expired across the database.
    pub fn prune_expired_nodes(&self) -> Result<usize> {
        self.graph.with_write(|g| g.prune_expired_nodes())
    }

    // =========================================================================
    // Cypher & ACID Transactions
    // =========================================================================

    /// Executes an openCypher query string against the database with WAL persistence.
    pub fn cypher(&self, query_str: &str) -> Result<CypherResult> {
        self.cypher_with_params(query_str, &HashMap::new())
    }

    /// Executes a parameterized openCypher query string against the database with WAL persistence.
    pub fn cypher_with_params(
        &self,
        query_str: &str,
        params: &HashMap<String, PropertyValue>,
    ) -> Result<CypherResult> {
        let res = self
            .graph
            .with_write(|g| g.cypher_with_params(query_str, params))?;
        if is_cypher_mutation(query_str) {
            self.wal.append(WalOp::CypherQuery {
                query: query_str.to_string(),
            })?;
            self.track_operation();
        }
        Ok(res)
    }

    /// Begins a new multi-statement ACID transaction with snapshot isolation.
    pub fn begin_transaction(&self, timeout: Option<Duration>) -> Result<u64> {
        let snapshot = self.graph.with_read(|g| g.clone());
        let tx_id = self.tx_manager.begin(snapshot, timeout);
        Ok(tx_id)
    }

    /// Executes an openCypher query inside an active transaction.
    pub fn execute_tx(&self, tx_id: u64, query_str: &str) -> Result<CypherResult> {
        self.tx_manager.execute(tx_id, query_str)
    }

    /// Commits an active transaction, applying staged mutations to live database and WAL.
    pub fn commit_transaction(&self, tx_id: u64) -> Result<()> {
        let tx = self.tx_manager.remove(tx_id)?.ok_or_else(|| {
            GraphError::InvalidOperation(format!(
                "Transaction {} not found or already closed",
                tx_id
            ))
        })?;

        if tx.created_at.elapsed() > tx.timeout {
            return Err(GraphError::InvalidOperation(format!(
                "Transaction {} timed out before commit",
                tx_id
            )));
        }

        if !tx.mutating_queries.is_empty() {
            self.graph.with_write(|g| {
                for q in &tx.mutating_queries {
                    g.cypher(q)?;
                }
                Ok::<(), GraphError>(())
            })?;

            self.wal.append(WalOp::TransactionCommit {
                tx_id,
                queries: tx.mutating_queries,
            })?;
            self.track_operation();
        }

        Ok(())
    }

    /// Rolls back an active transaction, discarding all staged mutations.
    pub fn rollback_transaction(&self, tx_id: u64) -> Result<()> {
        let removed = self.tx_manager.remove(tx_id)?;
        if removed.is_none() {
            return Err(GraphError::InvalidOperation(format!(
                "Transaction {} not found or already closed",
                tx_id
            )));
        }
        Ok(())
    }

    /// Returns a reference to the TransactionManager.
    pub fn tx_manager(&self) -> Arc<TransactionManager> {
        Arc::clone(&self.tx_manager)
    }

    /// Creates an in-memory vector index with durability.
    pub fn create_vector_index(
        &self,
        name: &str,
        label: &str,
        prop_key: &str,
        dimension: usize,
        similarity: VectorSimilarity,
    ) -> Result<()> {
        self.graph
            .with_write(|g| g.create_vector_index(name, label, prop_key, dimension, similarity))?;

        let op = WalOp::CreateVectorIndex {
            name: name.to_string(),
            label: label.to_string(),
            prop_key: prop_key.to_string(),
            dimension,
            similarity,
        };
        self.wal.append(op)?;
        self.track_operation();

        Ok(())
    }

    /// Queries a vector index for nearest neighbors.
    pub fn query_vector_index(
        &self,
        name: &str,
        k: usize,
        query_vector: &[f32],
    ) -> Result<Vec<VectorSearchResult>> {
        self.graph
            .with_read(|g| g.query_vector_index(name, k, query_vector))
    }

    /// Creates an inverted full-text keyword search index on a label and property key.
    pub fn create_fulltext_index(&self, name: &str, label: &str, property_key: &str) -> Result<()> {
        self.graph
            .with_write(|g| g.create_fulltext_index(name, label, property_key))?;

        let op = WalOp::CreateFulltextIndex {
            name: name.to_string(),
            label: label.to_string(),
            property_key: property_key.to_string(),
        };
        self.wal.append(op)?;
        self.track_operation();

        Ok(())
    }

    /// Queries a full-text inverted index for top-k relevance matches using BM25/TF-IDF.
    pub fn query_fulltext_index(
        &self,
        name: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<FulltextSearchResult>> {
        self.graph
            .with_read(|g| g.query_fulltext_index(name, query, top_k))
    }

    /// Queries a full-text inverted index using prefix matching.
    pub fn query_fulltext_prefix(
        &self,
        name: &str,
        prefix: &str,
        top_k: usize,
    ) -> Result<Vec<FulltextSearchResult>> {
        self.graph
            .with_read(|g| g.query_fulltext_prefix(name, prefix, top_k))
    }

    /// Hybrid Graph Search: full-text keyword search + graph topology traversal.
    pub fn hybrid_search_and_traverse(
        &self,
        name: &str,
        query: &str,
        top_k: usize,
        depth: usize,
    ) -> Result<Vec<(FulltextSearchResult, Vec<crate::traversal::Path>)>> {
        self.graph
            .with_read(|g| g.hybrid_search_and_traverse(name, query, top_k, depth))
    }

    // =========================================================================
    // Query Optimization & EXPLAIN Execution Plan
    // =========================================================================

    /// Returns the execution plan tree for an openCypher query without executing it.
    pub fn explain_cypher(&self, query_str: &str) -> Result<QueryPlan> {
        self.graph.with_read(|g| g.explain_cypher(query_str))
    }

    // =========================================================================
    // Secondary Index Management
    // =========================================================================

    /// Creates a secondary property index on (label, property_key) with durability.
    pub fn create_property_index(&self, label: &str, property_key: &str) -> Result<bool> {
        let created = self
            .graph
            .with_write(|g| g.create_index(label, property_key))?;
        if created {
            self.wal.append(WalOp::CreatePropertyIndex {
                label: label.to_string(),
                property_key: property_key.to_string(),
            })?;
            self.track_operation();
        }
        Ok(created)
    }

    /// Drops a secondary property index with durability.
    pub fn drop_property_index(&self, label: &str, property_key: &str) -> Result<bool> {
        let dropped = self.graph.with_write(|g| g.drop_index(label, property_key));
        if dropped {
            self.wal.append(WalOp::DropPropertyIndex {
                label: label.to_string(),
                property_key: property_key.to_string(),
            })?;
            self.track_operation();
        }
        Ok(dropped)
    }

    /// Drops a vector index with durability.
    pub fn drop_vector_index(&self, name: &str) -> Result<bool> {
        let dropped = self.graph.with_write(|g| g.drop_vector_index(name));
        if dropped {
            self.wal.append(WalOp::DropVectorIndex {
                name: name.to_string(),
            })?;
            self.track_operation();
        }
        Ok(dropped)
    }

    /// Drops a full-text index with durability.
    pub fn drop_fulltext_index(&self, name: &str) -> Result<bool> {
        let dropped = self.graph.with_write(|g| g.drop_fulltext_index(name));
        if dropped {
            self.wal.append(WalOp::DropFulltextIndex {
                name: name.to_string(),
            })?;
            self.track_operation();
        }
        Ok(dropped)
    }

    /// Introspects full graph schema including labels, types, properties, and active indexes.
    pub fn schema(&self) -> serde_json::Value {
        self.graph.with_read(|g| {
            let stats = g.stats();
            let mut vector_indices_info = Vec::new();
            for v_idx in g.vector_indices() {
                vector_indices_info.push(serde_json::json!({
                    "name": v_idx.name,
                    "label": v_idx.label,
                    "property_key": v_idx.property_key,
                    "dimensions": v_idx.dimensions,
                    "metric": format!("{:?}", v_idx.metric),
                    "indexed_count": v_idx.len()
                }));
            }

            let mut fulltext_indices_info = Vec::new();
            for ft_idx in g.fulltext_indices() {
                fulltext_indices_info.push(serde_json::json!({
                    "name": ft_idx.name,
                    "label": ft_idx.label,
                    "property_key": ft_idx.property_key,
                    "indexed_documents": ft_idx.len()
                }));
            }

            let mut prop_indices_info = Vec::new();
            for (lbl, prop, is_uniq) in g.property_indices() {
                prop_indices_info.push(serde_json::json!({
                    "label": lbl,
                    "property": prop,
                    "unique": is_uniq
                }));
            }

            serde_json::json!({
                "nodes_total": stats.node_count,
                "edges_total": stats.edge_count,
                "density": stats.density,
                "average_degree": stats.average_degree,
                "isolated_nodes": stats.isolated_nodes,
                "self_loops": stats.self_loops,
                "label_counts": stats.label_counts,
                "relationship_type_counts": stats.type_counts,
                "property_indices": prop_indices_info,
                "vector_indices": vector_indices_info,
                "fulltext_indices": fulltext_indices_info
            })
        })
    }

    /// Returns list of all active indexes (property, vector, fulltext).
    pub fn list_indexes(&self) -> serde_json::Value {
        self.graph.with_read(|g| {
            let props: Vec<_> = g.property_indices().into_iter().map(|(l, p, u)| {
                serde_json::json!({ "type": "property", "label": l, "property": p, "unique": u })
            }).collect();
            let vectors: Vec<_> = g
                .vector_indices()
                .map(|v| {
                    serde_json::json!({
                        "type": "vector",
                        "name": v.name,
                        "label": v.label,
                        "property_key": v.property_key,
                        "dimensions": v.dimensions,
                        "metric": format!("{:?}", v.metric),
                        "indexed_count": v.len()
                    })
                })
                .collect();
            let fulltexts: Vec<_> = g
                .fulltext_indices()
                .map(|f| {
                    serde_json::json!({
                        "type": "fulltext",
                        "name": f.name,
                        "label": f.label,
                        "property_key": f.property_key,
                        "indexed_documents": f.len()
                    })
                })
                .collect();

            serde_json::json!({
                "property_indices": props,
                "vector_indices": vectors,
                "fulltext_indices": fulltexts
            })
        })
    }

    // =========================================================================
    // Graph Algorithms (In-Memory Engine Analytics)
    // =========================================================================

    /// Computes PageRank centrality scores for all nodes.
    pub fn run_pagerank(&self, damping: f64, max_iter: usize, tol: f64) -> Vec<(u64, f64)> {
        self.graph.with_read(|g| {
            let scores = crate::algo::pagerank(g, damping, max_iter, tol);
            let mut sorted: Vec<(u64, f64)> =
                scores.into_iter().map(|(id, s)| (id.raw(), s)).collect();
            sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
            sorted
        })
    }

    /// Computes Personalized PageRank centrality scores relative to seed nodes.
    pub fn run_personalized_pagerank(
        &self,
        seeds: &[NodeId],
        damping: f64,
        max_iter: usize,
        tol: f64,
    ) -> Vec<(u64, f64)> {
        self.graph.with_read(|g| {
            let scores = crate::algo::personalized_pagerank(g, seeds, damping, max_iter, tol);
            let mut sorted: Vec<(u64, f64)> =
                scores.into_iter().map(|(id, s)| (id.raw(), s)).collect();
            sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
            sorted
        })
    }

    /// Computes Betweenness Centrality scores for all nodes.
    pub fn run_betweenness_centrality(
        &self,
        direction: Direction,
        normalized: bool,
    ) -> Vec<(u64, f64)> {
        self.graph.with_read(|g| {
            let scores = crate::algo::betweenness_centrality(g, direction, normalized);
            let mut sorted: Vec<(u64, f64)> =
                scores.into_iter().map(|(id, s)| (id.raw(), s)).collect();
            sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
            sorted
        })
    }

    /// Computes Degree Centrality for all nodes in the given direction.
    pub fn run_degree_centrality(&self, direction: Direction) -> Vec<(u64, usize)> {
        self.graph.with_read(|g| {
            let scores = crate::algo::degree_centrality(g, direction);
            let mut sorted: Vec<(u64, usize)> =
                scores.into_iter().map(|(id, s)| (id.raw(), s)).collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            sorted
        })
    }

    /// Computes unweighted Shortest Path between source and target using BFS.
    pub fn run_shortest_path_bfs(
        &self,
        source: NodeId,
        target: NodeId,
        direction: Direction,
    ) -> Option<crate::traversal::Path> {
        self.graph
            .with_read(|g| crate::algo::shortest_path_bfs(g, source, target, direction))
    }

    /// Computes weighted Shortest Path using Dijkstra algorithm on relationship property.
    pub fn run_dijkstra(
        &self,
        source: NodeId,
        target: NodeId,
        weight_prop: &str,
        direction: Direction,
    ) -> Option<(crate::traversal::Path, f64)> {
        self.graph
            .with_read(|g| crate::algo::dijkstra_by_prop(g, source, target, direction, weight_prop))
    }

    /// Computes Connected Component clusters ("wcc" or "scc").
    pub fn run_community_detection(&self, algo: &str) -> Result<Vec<Vec<u64>>> {
        self.graph
            .with_read(|g| match algo.to_lowercase().as_str() {
                "scc" => {
                    let comps = crate::algo::strongly_connected_components(g);
                    Ok(comps
                        .into_iter()
                        .map(|set| set.into_iter().map(|id| id.raw()).collect())
                        .collect())
                }
                _ => {
                    let comps = crate::algo::weakly_connected_components(g);
                    Ok(comps
                        .into_iter()
                        .map(|set| set.into_iter().map(|id| id.raw()).collect())
                        .collect())
                }
            })
    }

    /// Computes Jaccard Similarity or Common Neighbors between two nodes.
    pub fn run_similarity(
        &self,
        n1: NodeId,
        n2: NodeId,
        metric: &str,
        direction: Direction,
    ) -> Result<serde_json::Value> {
        self.graph.with_read(|g| {
            if metric.eq_ignore_ascii_case("common_neighbors") {
                let neighbors = crate::algo::common_neighbors(g, n1, n2, direction);
                let ids: Vec<u64> = neighbors.into_iter().map(|id| id.raw()).collect();
                Ok(serde_json::json!({
                    "metric": "common_neighbors",
                    "count": ids.len(),
                    "common_neighbors": ids,
                }))
            } else {
                let score = crate::algo::jaccard_similarity(g, n1, n2, direction);
                Ok(serde_json::json!({
                    "metric": "jaccard_similarity",
                    "score": score,
                }))
            }
        })
    }

    // =========================================================================
    // Checkpointing & Persistence
    // =========================================================================

    /// Takes a point-in-time snapshot and truncates the WAL up to the snapshot sequence.
    pub fn checkpoint(&self) -> Result<u64> {
        let current_seq = self.wal.current_sequence();
        self.wal.flush()?;

        self.graph.with_read(|g| {
            self.snapshots.save_snapshot(g, current_seq)?;
            Ok::<(), Error>(())
        })?;

        self.wal.truncate_up_to(current_seq)?;
        self.last_checkpoint_seq
            .store(current_seq, Ordering::SeqCst);
        self.ops_since_checkpoint.store(0, Ordering::SeqCst);

        Ok(current_seq)
    }

    fn track_operation(&self) {
        let ops = self.ops_since_checkpoint.fetch_add(1, Ordering::Relaxed) + 1;
        if self.config.auto_checkpoint_ops > 0 && ops >= self.config.auto_checkpoint_ops {
            let _ = self.checkpoint();
        }
    }

    fn apply_wal_op(graph: &mut Graph, op: WalOp) -> Result<()> {
        match op {
            WalOp::CreateNode {
                labels, properties, ..
            } => {
                graph.create_node_with(|n| {
                    for l in labels {
                        n.add_label(l);
                    }
                    for (k, v) in properties {
                        n.set_prop(k, v);
                    }
                })?;
            }
            WalOp::DeleteNode { id } => {
                let _ = graph.delete_node(NodeId(id));
            }
            WalOp::UpdateNodeProperties { id, properties } => {
                if let Some(node) = graph.get_node_mut(NodeId(id)) {
                    for (k, v) in properties {
                        node.set_prop(k, v);
                    }
                }
            }
            WalOp::CreateEdge {
                source,
                target,
                rel_type,
                properties,
                ..
            } => {
                let _ = graph.create_edge_with(NodeId(source), NodeId(target), &rel_type, |r| {
                    for (k, v) in properties {
                        r.set_prop(k, v);
                    }
                });
            }
            WalOp::DeleteEdge { id } => {
                let _ = graph.delete_edge(RelId(id));
            }
            WalOp::UpdateEdgeProperties { id, properties } => {
                if let Some(rel) = graph.get_edge_mut(RelId(id)) {
                    for (k, v) in properties {
                        rel.set_prop(k, v);
                    }
                }
            }
            WalOp::CreateVectorIndex {
                name,
                label,
                prop_key,
                dimension,
                similarity,
            } => {
                let _ = graph.create_vector_index(&name, &label, &prop_key, dimension, similarity);
            }
            WalOp::CreateFulltextIndex {
                name,
                label,
                property_key,
            } => {
                let _ = graph.create_fulltext_index(&name, &label, &property_key);
            }
            WalOp::CreatePropertyIndex {
                label,
                property_key,
            } => {
                let _ = graph.create_index(&label, &property_key);
            }
            WalOp::DropPropertyIndex {
                label,
                property_key,
            } => {
                let _ = graph.drop_index(&label, &property_key);
            }
            WalOp::DropVectorIndex { name } => {
                let _ = graph.drop_vector_index(&name);
            }
            WalOp::DropFulltextIndex { name } => {
                let _ = graph.drop_fulltext_index(&name);
            }
            WalOp::CypherQuery { query } => {
                let _ = graph.cypher(&query);
            }
            WalOp::TransactionCommit { queries, .. } => {
                for q in queries {
                    let _ = graph.cypher(&q);
                }
            }
        }
        Ok(())
    }
}

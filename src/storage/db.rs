use crate::concurrent::ConcurrentGraph;
use crate::cypher::{is_cypher_mutation, CypherResult};
use crate::error::{Error, GraphError, Result};
use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::model::{Node, Relationship};
use crate::value::PropertyValue;
use crate::index::FulltextSearchResult;
use crate::vector::{VectorSearchResult, VectorSimilarity};
use super::snapshot::SnapshotManager;
use super::transaction::TransactionManager;
use super::wal::{FsyncPolicy, WalManager, WalOp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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
        let (node_count, edge_count) = self.graph.with_read(|g| (g.node_count(), g.edge_count()));
        let current_seq = self.wal.current_sequence();
        let last_snap = self.last_checkpoint_seq.load(Ordering::Relaxed);
        let uncheckpointed = current_seq.saturating_sub(last_snap);

        DatabaseStats {
            node_count: node_count as u64,
            edge_count: edge_count as u64,
            current_wal_sequence: current_seq,
            last_checkpoint_sequence: last_snap,
            uncheckpointed_ops: uncheckpointed,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    // =========================================================================
    // Mutations (WAL Durability + In-Memory Apply)
    // =========================================================================

    /// Creates a new node with durability guarantees.
    pub fn create_node(&self, labels: Vec<String>, properties: HashMap<String, PropertyValue>) -> Result<NodeId> {
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
    pub fn bulk_import_nodes(&self, items: Vec<(Vec<String>, HashMap<String, PropertyValue>)>) -> Result<Vec<NodeId>> {
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
    pub fn bulk_import_edges(&self, items: Vec<(NodeId, NodeId, String, HashMap<String, PropertyValue>)>) -> Result<Vec<RelId>> {
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
        let res = self.graph.with_write(|g| g.cypher_with_params(query_str, params))?;
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
        let tx = self
            .tx_manager
            .remove(tx_id)?
            .ok_or_else(|| GraphError::InvalidOperation(format!("Transaction {} not found or already closed", tx_id)))?;

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
        self.graph.with_write(|g| {
            g.create_vector_index(name, label, prop_key, dimension, similarity)
        })?;

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
    pub fn query_vector_index(&self, name: &str, k: usize, query_vector: &[f32]) -> Result<Vec<VectorSearchResult>> {
        self.graph.with_read(|g| g.query_vector_index(name, k, query_vector))
    }

    /// Creates an inverted full-text keyword search index on a label and property key.
    pub fn create_fulltext_index(
        &self,
        name: &str,
        label: &str,
        property_key: &str,
    ) -> Result<()> {
        self.graph.with_write(|g| {
            g.create_fulltext_index(name, label, property_key)
        })?;

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
        self.graph.with_read(|g| g.query_fulltext_index(name, query, top_k))
    }

    /// Queries a full-text inverted index using prefix matching.
    pub fn query_fulltext_prefix(
        &self,
        name: &str,
        prefix: &str,
        top_k: usize,
    ) -> Result<Vec<FulltextSearchResult>> {
        self.graph.with_read(|g| g.query_fulltext_prefix(name, prefix, top_k))
    }

    /// Hybrid Graph Search: full-text keyword search + graph topology traversal.
    pub fn hybrid_search_and_traverse(
        &self,
        name: &str,
        query: &str,
        top_k: usize,
        depth: usize,
    ) -> Result<Vec<(FulltextSearchResult, Vec<crate::traversal::Path>)>> {
        self.graph.with_read(|g| g.hybrid_search_and_traverse(name, query, top_k, depth))
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
        self.last_checkpoint_seq.store(current_seq, Ordering::SeqCst);
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
            WalOp::CreateNode { labels, properties, .. } => {
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
            WalOp::CreateEdge { source, target, rel_type, properties, .. } => {
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
            WalOp::CreateVectorIndex { name, label, prop_key, dimension, similarity } => {
                let _ = graph.create_vector_index(&name, &label, &prop_key, dimension, similarity);
            }
            WalOp::CreateFulltextIndex { name, label, property_key } => {
                let _ = graph.create_fulltext_index(&name, &label, &property_key);
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

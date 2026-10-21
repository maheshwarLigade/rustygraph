//! # RustyGraph
//!
//! `rusty_graph` is an in-memory property graph database engine and library written in Rust.
//! It is inspired by Neo4j's labeled property graph model, providing fast in-memory graph
//! storage, traversal, indexing, and processing without external database overhead.
//!
//! ## Quick Start
//!
//! ```rust
//! use rusty_graph::prelude::*;
//!
//! fn main() -> Result<()> {
//!     let mut graph = Graph::new();
//!
//!     // Create nodes with labels and properties
//!     let alice = graph.create_node_with(|n| {
//!         n.add_label("Person");
//!         n.set_prop("name", "Alice");
//!         n.set_prop("age", 30);
//!     })?;
//!
//!     let bob = graph.create_node_with(|n| {
//!         n.add_label("Person");
//!         n.set_prop("name", "Bob");
//!         n.set_prop("age", 28);
//!     })?;
//!
//!     // Create a directed relationship
//!     let _rel = graph.create_edge_with(alice, bob, "KNOWS", |r| {
//!         r.set_prop("since", 2021);
//!         r.set_prop("closeness", 0.95);
//!     })?;
//!
//!     // 1. Declarative Cypher-style query builder
//!     let matches: Vec<(String, String)> = graph.query()
//!         .match_node("p").with_label("Person")
//!         .outgoing("KNOWS", "friend").with_label("Person")
//!         .filter(|ctx| ctx.prop("friend", "age").and_then(|v| v.as_i64()).unwrap_or(0) < 30)
//!         .project(|ctx| {
//!             (
//!                 ctx.prop("p", "name").unwrap().as_str().unwrap().to_string(),
//!                 ctx.prop("friend", "name").unwrap().as_str().unwrap().to_string(),
//!             )
//!         });
//!     assert_eq!(matches, vec![("Alice".to_string(), "Bob".to_string())]);
//!
//!     // 2. Direct Cypher Query string execution!
//!     let cypher_res = graph.cypher(
//!         "MATCH (p:Person)-[:KNOWS]->(f:Person) WHERE f.age < 30 RETURN p.name, f.name"
//!     )?;
//!     assert_eq!(cypher_res.len(), 1);
//!
//!     Ok(())
//! }
//! ```

pub mod algo;
pub mod concurrent;
pub mod distributed;
pub use distributed as cluster;
pub use distributed::{consensus, node};
pub mod error;
pub mod ffi;
pub mod graph;
pub mod id;
pub mod index;
pub mod model;
pub mod query;
pub use query::cypher;
pub mod serde_export;
pub mod server;
pub mod stats;
pub mod storage;
pub use storage::{arena, db, snapshot, transaction, wal};
pub mod traversal;
pub mod value;
pub mod vector;

/// Convenient re-exports of common types.
pub mod prelude {
    pub use crate::algo;
    pub use crate::cluster::{
        ClusterCommand, ClusterMode, ClusterNode, ClusterNodeId, ClusterRole, LogEntry,
        VirtualNetwork,
    };
    pub use crate::concurrent::ConcurrentGraph;
    pub use crate::cypher::CypherResult;
    pub use crate::error::{GraphError, Result};
    pub use crate::graph::Graph;
    pub use crate::id::{NodeId, RelId};
    pub use crate::index::{IndexKey, IndexManager, PropertyIndex};
    pub use crate::model::{Direction, Node, Properties, Relationship};
    pub use crate::query::{QueryBuilder, QueryContext};
    pub use crate::db::{DatabaseConfig, DatabaseStats, StandaloneDatabase};
    pub use crate::distributed::{ClusterMessage, ClusterPeer, ClusterStatus, DistributedClusterNode};
    pub use crate::serde_export::GraphDump;
    pub use crate::server::{ServerConfig, StandaloneServer};
    pub use crate::snapshot::{SnapshotManager, SnapshotMetadata};
    pub use crate::stats::GraphStats;
    pub use crate::transaction::{Transaction, TransactionManager, TransactionStatus};
    pub use crate::traversal::{Path, PathSegment, TraversalOrder, Traverser, Uniqueness};
    pub use crate::value::PropertyValue;
    pub use crate::vector::{VectorIndex, VectorSearchResult, VectorSimilarity};
    pub use crate::wal::{crc32, FsyncPolicy, WalManager, WalOp, WalRecord};
}

pub use algo::{
    a_star, common_neighbors, degree_centrality, dijkstra, dijkstra_by_prop, jaccard_similarity,
    pagerank, shortest_path_bfs, strongly_connected_components, topological_sort,
    weakly_connected_components,
};
pub use concurrent::ConcurrentGraph;
pub use cypher::CypherResult;
pub use db::{DatabaseConfig, DatabaseStats, StandaloneDatabase};
pub use distributed::{ClusterPeer, ClusterStatus, DistributedClusterNode};
pub use error::{GraphError, Result};
pub use graph::Graph;
pub use id::{NodeId, RelId};
pub use index::{IndexKey, IndexManager, PropertyIndex};
pub use model::{Direction, Node, Properties, Relationship};
pub use query::{QueryBuilder, QueryContext};
pub use serde_export::GraphDump;
pub use server::{ServerConfig, StandaloneServer};
pub use snapshot::{SnapshotManager, SnapshotMetadata};
pub use stats::GraphStats;
pub use traversal::{Path, PathSegment, TraversalOrder, Traverser, Uniqueness};
pub use value::PropertyValue;
pub use vector::{VectorIndex, VectorSearchResult, VectorSimilarity};
pub use wal::{FsyncPolicy, WalManager, WalOp, WalRecord};


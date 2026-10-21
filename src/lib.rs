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
pub mod cluster;
pub mod concurrent;
pub mod cypher;
pub mod error;
pub mod ffi;
pub mod graph;
pub mod id;
pub mod index;
pub mod model;
pub mod query;
pub mod serde_export;
pub mod stats;
pub mod storage;
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
    pub use crate::serde_export::GraphDump;
    pub use crate::stats::GraphStats;
    pub use crate::traversal::{Path, PathSegment, TraversalOrder, Traverser, Uniqueness};
    pub use crate::value::PropertyValue;
    pub use crate::vector::{VectorIndex, VectorSearchResult, VectorSimilarity};
}

pub use algo::{
    a_star, common_neighbors, degree_centrality, dijkstra, dijkstra_by_prop, jaccard_similarity,
    pagerank, shortest_path_bfs, strongly_connected_components, topological_sort,
    weakly_connected_components,
};
pub use concurrent::ConcurrentGraph;
pub use cypher::CypherResult;
pub use error::{GraphError, Result};
pub use graph::Graph;
pub use id::{NodeId, RelId};
pub use index::{IndexKey, IndexManager, PropertyIndex};
pub use model::{Direction, Node, Properties, Relationship};
pub use query::{QueryBuilder, QueryContext};
pub use serde_export::GraphDump;
pub use stats::GraphStats;
pub use traversal::{Path, PathSegment, TraversalOrder, Traverser, Uniqueness};
pub use value::PropertyValue;
pub use vector::{VectorIndex, VectorSearchResult, VectorSimilarity};

#[cfg(test)]
mod tests {
    use super::prelude::*;
    use std::ops::Bound::{Excluded, Included};

    #[test]
    fn test_node_creation_and_properties() -> Result<()> {
        let mut graph = Graph::new();

        let node_id = graph.create_node_with(|n| {
            n.add_label("Person");
            n.add_label("Developer");
            n.set_prop("name", "Neo");
            n.set_prop("skills", vec!["Rust", "Distributed Systems"]);
        })?;

        let node = graph.get_node(node_id).expect("node should exist");
        assert_eq!(node.id, node_id);
        assert!(node.has_label("Person"));
        assert!(node.has_label("Developer"));
        assert_eq!(node.prop("name"), Some(&PropertyValue::String("Neo".into())));

        // Test property update
        graph
            .set_node_property(node_id, "level", 42)
            .expect("should update property");
        assert_eq!(graph.get_node_property(node_id, "level"), Some(&PropertyValue::Int(42)));

        Ok(())
    }

    #[test]
    fn test_edge_creation_and_traversal() -> Result<()> {
        let mut graph = Graph::new();

        let a = graph.create_node_with(|n| { n.add_label("City"); n.set_prop("name", "Paris"); })?;
        let b = graph.create_node_with(|n| { n.add_label("City"); n.set_prop("name", "Berlin"); })?;
        let c = graph.create_node_with(|n| { n.add_label("City"); n.set_prop("name", "Rome"); })?;

        let e1 = graph.create_edge_with(a, b, "CONNECTED_TO", |r| {
            r.set_prop("distance_km", 1050);
        })?;

        let _e2 = graph.create_edge_with(b, c, "CONNECTED_TO", |r| {
            r.set_prop("distance_km", 1180);
        })?;

        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.degree(b, Direction::Incoming)?, 1);
        assert_eq!(graph.degree(b, Direction::Outgoing)?, 1);
        assert_eq!(graph.degree(b, Direction::Both)?, 2);

        // Check neighbors
        let b_outgoing = graph.neighbors(b, Direction::Outgoing)?;
        assert_eq!(b_outgoing, vec![c]);

        let b_incoming = graph.neighbors(b, Direction::Incoming)?;
        assert_eq!(b_incoming, vec![a]);

        // Check relationship property
        let edge = graph.get_edge(e1).expect("e1 exists");
        assert_eq!(edge.prop("distance_km"), Some(&PropertyValue::Int(1050)));
        assert_eq!(edge.other_node(a), Some(b));
        assert_eq!(edge.other_node(b), Some(a));

        Ok(())
    }

    #[test]
    fn test_label_and_type_indexes() -> Result<()> {
        let mut graph = Graph::new();

        let n1 = graph.create_node_with(|n| { n.add_label("User"); })?;
        let n2 = graph.create_node_with(|n| { n.add_label("User"); })?;
        let n3 = graph.create_node_with(|n| { n.add_label("Admin"); })?;

        let r1 = graph.create_edge(n1, n2, "FOLLOWS")?;
        let r2 = graph.create_edge(n2, n3, "BLOCKS")?;

        let users = graph.nodes_with_label("User");
        assert_eq!(users.len(), 2);
        assert!(users.contains(&n1));
        assert!(users.contains(&n2));

        let admins = graph.nodes_with_label("Admin");
        assert_eq!(admins, vec![n3]);

        let follows = graph.edges_with_type("FOLLOWS");
        assert_eq!(follows, vec![r1]);

        let blocks = graph.edges_with_type("BLOCKS");
        assert_eq!(blocks, vec![r2]);

        // Dynamic label removal
        graph.remove_node_label(n1, "User")?;
        assert_eq!(graph.nodes_with_label("User").len(), 1);

        Ok(())
    }

    #[test]
    fn test_cascading_node_deletion() -> Result<()> {
        let mut graph = Graph::new();

        let n1 = graph.create_node();
        let n2 = graph.create_node();
        let n3 = graph.create_node();

        let e1 = graph.create_edge(n1, n2, "REL")?;
        let e2 = graph.create_edge(n2, n3, "REL")?;
        let e3 = graph.create_edge(n3, n1, "REL")?;

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 3);

        // Delete n2, should delete e1 and e2 automatically
        graph.delete_node(n2)?;

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert!(!graph.contains_edge(e1));
        assert!(!graph.contains_edge(e2));
        assert!(graph.contains_edge(e3));

        // Degrees should be cleanly updated
        assert_eq!(graph.degree(n1, Direction::Outgoing)?, 0);
        assert_eq!(graph.degree(n1, Direction::Incoming)?, 1);
        assert_eq!(graph.degree(n3, Direction::Outgoing)?, 1);
        assert_eq!(graph.degree(n3, Direction::Incoming)?, 0);

        Ok(())
    }

    #[test]
    fn test_generational_arena_safety() -> Result<()> {
        let mut graph = Graph::new();

        let n1 = graph.create_node();
        graph.delete_node(n1)?;

        // Trying to access deleted n1 should return None
        assert!(graph.get_node(n1).is_none());
        assert!(!graph.contains_node(n1));

        // Create new node; arena might reuse slot with a new generation
        let n2 = graph.create_node();
        assert_ne!(n1, n2); // IDs must not collide
        assert!(graph.get_node(n1).is_none());
        assert!(graph.get_node(n2).is_some());

        Ok(())
    }

    #[test]
    fn test_property_value_types_and_ordering() {
        let int_val = PropertyValue::from(100);
        let float_val = PropertyValue::from(100.0);
        assert_eq!(int_val, float_val);

        let str_val = PropertyValue::from("Rusty");
        assert_eq!(str_val.as_str(), Some("Rusty"));

        let list_val = PropertyValue::from(vec![1, 2, 3]);
        assert_eq!(list_val.as_list().unwrap().len(), 3);

        // Test JSON conversion round-trip
        let json_repr = serde_json::to_string(&list_val).unwrap();
        let deserialized: PropertyValue = serde_json::from_str(&json_repr).unwrap();
        assert_eq!(list_val, deserialized);
    }

    #[test]
    fn test_property_indexing_and_range_queries() -> Result<()> {
        let mut graph = Graph::new();

        // Create index on (:User {age})
        graph.create_index("User", "age")?;
        assert!(graph.has_index("User", "age"));

        let u1 = graph.create_node_with(|n| {
            n.add_label("User");
            n.set_prop("name", "Alice");
            n.set_prop("age", 25);
        })?;

        let u2 = graph.create_node_with(|n| {
            n.add_label("User");
            n.set_prop("name", "Bob");
            n.set_prop("age", 35);
        })?;

        let u3 = graph.create_node_with(|n| {
            n.add_label("User");
            n.set_prop("name", "Charlie");
            n.set_prop("age", 45);
        })?;

        // Exact index lookup
        let age_35 = graph.find_nodes_by_property("User", "age", &PropertyValue::from(35));
        assert_eq!(age_35, vec![u2]);

        // Range index lookup: age >= 25 && age < 45
        let age_range = graph.find_nodes_by_property_range(
            "User",
            "age",
            (Included(PropertyValue::from(25)), Excluded(PropertyValue::from(45))),
        );
        assert_eq!(age_range.len(), 2);
        assert!(age_range.contains(&u1));
        assert!(age_range.contains(&u2));
        assert!(!age_range.contains(&u3));

        // Update age of u2 to 50
        graph.set_node_property(u2, "age", 50)?;
        let age_35_after = graph.find_nodes_by_property("User", "age", &PropertyValue::from(35));
        assert!(age_35_after.is_empty());

        let age_50 = graph.find_nodes_by_property("User", "age", &PropertyValue::from(50));
        assert_eq!(age_50, vec![u2]);

        Ok(())
    }

    #[test]
    fn test_unique_constraints() -> Result<()> {
        let mut graph = Graph::new();

        // Create unique constraint on (:User {email})
        graph.create_unique_constraint("User", "email")?;
        assert!(graph.has_unique_constraint("User", "email"));

        // Creating first user succeeds
        let u1 = graph.create_node_with(|n| {
            n.add_label("User");
            n.set_prop("email", "alice@example.com");
        })?;

        // Attempting to create duplicate user violates unique constraint
        let duplicate_result = graph.create_node_with(|n| {
            n.add_label("User");
            n.set_prop("email", "alice@example.com");
        });
        assert!(duplicate_result.is_err());
        match duplicate_result {
            Err(GraphError::ConstraintViolation(msg)) => {
                assert!(msg.contains("Unique constraint violated"));
            }
            _ => panic!("Expected ConstraintViolation error"),
        }

        // Setting duplicate email on another node also fails
        let u2 = graph.create_node_with(|n| {
            n.add_label("User");
            n.set_prop("email", "bob@example.com");
        })?;

        let update_result = graph.set_node_property(u2, "email", "alice@example.com");
        assert!(update_result.is_err());

        // Updating self with the same email succeeds
        let self_update = graph.set_node_property(u1, "email", "alice@example.com");
        assert!(self_update.is_ok());

        Ok(())
    }

    #[test]
    fn test_graph_statistics() -> Result<()> {
        let mut graph = Graph::new();

        let n1 = graph.create_node_with(|n| { n.add_label("Server"); })?;
        let n2 = graph.create_node_with(|n| { n.add_label("Client"); })?;
        let _isolated = graph.create_node_with(|n| { n.add_label("Client"); })?;

        let _e1 = graph.create_edge(n1, n2, "COMMUNICATES")?;
        let _loop = graph.create_edge(n1, n1, "HEARTBEAT")?;

        let stats = graph.stats();
        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.edge_count, 2);
        assert_eq!(stats.isolated_nodes, 1);
        assert_eq!(stats.self_loops, 1);
        assert_eq!(stats.label_counts.get("Server"), Some(&1));
        assert_eq!(stats.label_counts.get("Client"), Some(&2));
        assert_eq!(stats.type_counts.get("COMMUNICATES"), Some(&1));
        assert_eq!(stats.type_counts.get("HEARTBEAT"), Some(&1));
        assert!(stats.density > 0.0);

        Ok(())
    }

    #[test]
    fn test_fluent_traversal_engine() -> Result<()> {
        let mut graph = Graph::new();

        let a = graph.create_node_with(|n| { n.set_prop("name", "A"); })?;
        let b = graph.create_node_with(|n| { n.set_prop("name", "B"); })?;
        let c = graph.create_node_with(|n| { n.set_prop("name", "C"); })?;
        let d = graph.create_node_with(|n| { n.set_prop("name", "D"); })?;
        let e = graph.create_node_with(|n| { n.set_prop("name", "E"); })?;

        graph.create_edge(a, b, "NEXT")?;
        graph.create_edge(b, c, "NEXT")?;
        graph.create_edge(c, d, "NEXT")?;
        graph.create_edge(b, e, "BRANCH")?;

        let paths_1_to_2 = graph
            .traverse(a)
            .outgoing()
            .with_type("NEXT")
            .min_depth(1)
            .max_depth(2)
            .paths();

        assert_eq!(paths_1_to_2.len(), 2);
        assert_eq!(paths_1_to_2[0].end_node(), b);
        assert_eq!(paths_1_to_2[1].end_node(), c);

        let reachable = graph.traverse(a).outgoing().with_type("NEXT").nodes();
        assert_eq!(reachable, vec![a, b, c, d]);

        Ok(())
    }

    #[test]
    fn test_traversal_cycle_prevention() -> Result<()> {
        let mut graph = Graph::new();

        let x = graph.create_node();
        let y = graph.create_node();
        let z = graph.create_node();

        graph.create_edge(x, y, "CYCLE")?;
        graph.create_edge(y, z, "CYCLE")?;
        graph.create_edge(z, x, "CYCLE")?;

        let paths = graph.traverse(x).outgoing().paths();
        assert_eq!(paths.len(), 3);

        Ok(())
    }

    #[test]
    fn test_shortest_path_and_dijkstra() -> Result<()> {
        let mut graph = Graph::new();

        let a = graph.create_node();
        let b = graph.create_node();
        let c = graph.create_node();
        let d = graph.create_node();

        let _e_ab = graph.create_edge_with(a, b, "ROAD", |r| { r.set_prop("weight", 10.0); })?;
        let _e_bd = graph.create_edge_with(b, d, "ROAD", |r| { r.set_prop("weight", 1.0); })?;
        let _e_ac = graph.create_edge_with(a, c, "ROAD", |r| { r.set_prop("weight", 2.0); })?;
        let _e_cd = graph.create_edge_with(c, d, "ROAD", |r| { r.set_prop("weight", 3.0); })?;

        let (dijkstra_path, total_weight) =
            algo::dijkstra_by_prop(&graph, a, d, Direction::Outgoing, "weight")
                .expect("weighted path exists");

        assert_eq!(total_weight, 5.0);
        assert_eq!(dijkstra_path.nodes(), vec![a, c, d]);

        Ok(())
    }

    #[test]
    fn test_pagerank_and_centrality() -> Result<()> {
        let mut graph = Graph::new();

        let hub = graph.create_node();
        let n1 = graph.create_node();
        let n2 = graph.create_node();
        let n3 = graph.create_node();

        graph.create_edge(n1, hub, "VOTES")?;
        graph.create_edge(n2, hub, "VOTES")?;
        graph.create_edge(n3, hub, "VOTES")?;

        let in_degrees = algo::degree_centrality(&graph, Direction::Incoming);
        assert_eq!(in_degrees.get(&hub), Some(&3));

        let pr = algo::pagerank(&graph, 0.85, 50, 1e-6);
        assert!(pr[&hub] > pr[&n1]);

        Ok(())
    }

    #[test]
    fn test_community_detection_and_topo_sort() -> Result<()> {
        let mut graph = Graph::new();

        let a = graph.create_node();
        let b = graph.create_node();
        graph.create_edge(a, b, "LINK")?;

        let c = graph.create_node();
        let d = graph.create_node();
        let e = graph.create_node();
        graph.create_edge(c, d, "LINK")?;
        graph.create_edge(d, e, "LINK")?;
        graph.create_edge(e, c, "LINK")?;

        let wcc = algo::weakly_connected_components(&graph);
        assert_eq!(wcc.len(), 2);

        let scc = algo::strongly_connected_components(&graph);
        assert_eq!(scc.len(), 3);

        assert!(algo::topological_sort(&graph).is_err());

        let mut dag = Graph::new();
        let t1 = dag.create_node();
        let t2 = dag.create_node();
        dag.create_edge(t1, t2, "DEP")?;
        assert_eq!(algo::topological_sort(&dag)?, vec![t1, t2]);

        Ok(())
    }

    #[test]
    fn test_similarity_algorithms() -> Result<()> {
        let mut graph = Graph::new();

        let alice = graph.create_node();
        let bob = graph.create_node();
        let m1 = graph.create_node();
        let m2 = graph.create_node();

        graph.create_edge(alice, m1, "LIKED")?;
        graph.create_edge(alice, m2, "LIKED")?;
        graph.create_edge(bob, m1, "LIKED")?;
        graph.create_edge(bob, m2, "LIKED")?;

        let sim = algo::jaccard_similarity(&graph, alice, bob, Direction::Outgoing);
        assert_eq!(sim, 1.0);

        Ok(())
    }

    #[test]
    fn test_declarative_pattern_matching_query() -> Result<()> {
        let mut graph = Graph::new();

        let alice = graph.create_node_with(|n| {
            n.add_label("Person");
            n.set_prop("name", "Alice");
            n.set_prop("age", 30);
        })?;

        let bob = graph.create_node_with(|n| {
            n.add_label("Person");
            n.set_prop("name", "Bob");
            n.set_prop("age", 24);
        })?;

        let charlie = graph.create_node_with(|n| {
            n.add_label("Person");
            n.set_prop("name", "Charlie");
            n.set_prop("age", 35);
        })?;

        let acme = graph.create_node_with(|n| {
            n.add_label("Company");
            n.set_prop("name", "Acme Corp");
        })?;

        graph.create_edge_with(alice, bob, "KNOWS", |r| { r.set_prop("since", 2020); })?;
        graph.create_edge_with(alice, charlie, "KNOWS", |r| { r.set_prop("since", 2018); })?;
        graph.create_edge(bob, acme, "WORKS_AT")?;

        let young_friends: Vec<String> = graph.query()
            .match_node("p").with_label("Person").with_prop("name", "Alice")
            .outgoing_as("r", "KNOWS", "friend").with_label("Person")
            .filter(|ctx| ctx.prop("friend", "age").and_then(|v| v.as_i64()).unwrap_or(0) < 30)
            .project(|ctx| ctx.prop("friend", "name").unwrap().as_str().unwrap().to_string());

        assert_eq!(young_friends, vec!["Bob".to_string()]);

        let company_referrals = graph.query()
            .match_node("p").with_label("Person")
            .outgoing("KNOWS", "f").with_label("Person")
            .outgoing("WORKS_AT", "c").with_label("Company")
            .project(|ctx| {
                (
                    ctx.prop("p", "name").unwrap().as_str().unwrap().to_string(),
                    ctx.prop("f", "name").unwrap().as_str().unwrap().to_string(),
                    ctx.prop("c", "name").unwrap().as_str().unwrap().to_string(),
                )
            });

        assert_eq!(company_referrals, vec![("Alice".to_string(), "Bob".to_string(), "Acme Corp".to_string())]);

        Ok(())
    }

    #[test]
    fn test_concurrent_graph_multi_threading() -> Result<()> {
        use std::sync::Arc;
        use std::thread;

        let cg = Arc::new(ConcurrentGraph::new());

        let mut handles = vec![];
        for i in 0..4 {
            let g = Arc::clone(&cg);
            handles.push(thread::spawn(move || {
                let n1 = g.create_node();
                let n2 = g.create_node();
                g.set_node_property(n1, "thread", i).unwrap();
                g.create_edge(n1, n2, "THREAD_EDGE").unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(cg.node_count(), 8);
        assert_eq!(cg.edge_count(), 4);

        Ok(())
    }

    #[test]
    fn test_json_export_and_import() -> Result<()> {
        let mut original = Graph::new();

        let a = original.create_node_with(|n| {
            n.add_label("City");
            n.set_prop("name", "Kyoto");
            n.set_prop("country", "Japan");
        })?;

        let b = original.create_node_with(|n| {
            n.add_label("City");
            n.set_prop("name", "Tokyo");
            n.set_prop("country", "Japan");
        })?;

        original.create_edge_with(a, b, "SHINKANSEN", |r| {
            r.set_prop("line", "Tokaido");
            r.set_prop("duration_mins", 135);
        })?;

        let json_data = original.to_json()?;
        assert!(json_data.contains("Kyoto"));
        assert!(json_data.contains("SHINKANSEN"));

        let imported = Graph::from_json(&json_data)?;
        assert_eq!(imported.node_count(), 2);
        assert_eq!(imported.edge_count(), 1);

        let cities = imported.nodes_with_label("City");
        assert_eq!(cities.len(), 2);

        let edges = imported.edges_with_type("SHINKANSEN");
        assert_eq!(edges.len(), 1);

        let imported_edge = imported.get_edge(edges[0]).unwrap();
        assert_eq!(imported_edge.prop("duration_mins"), Some(&PropertyValue::Int(135)));

        Ok(())
    }

    #[test]
    fn test_cypher_query_language_execution() -> Result<()> {
        let mut graph = Graph::new();

        let alice = graph.create_node_with(|n| {
            n.add_label("Person");
            n.set_prop("name", "Alice");
            n.set_prop("age", 30);
        })?;

        let bob = graph.create_node_with(|n| {
            n.add_label("Person");
            n.set_prop("name", "Bob");
            n.set_prop("age", 24);
        })?;

        let acme = graph.create_node_with(|n| {
            n.add_label("Company");
            n.set_prop("name", "Acme Corp");
        })?;

        graph.create_edge_with(alice, bob, "KNOWS", |r| { r.set_prop("since", 2020); })?;
        graph.create_edge(bob, acme, "WORKS_AT")?;

        // 1. Simple Node Match
        let res1 = graph.cypher("MATCH (p:Person) RETURN p.name")?;
        assert_eq!(res1.columns, vec!["p.name"]);
        assert_eq!(res1.rows.len(), 2);

        // 2. Pattern Match with WHERE filter
        let res2 = graph.cypher(
            "MATCH (p:Person)-[:KNOWS]->(f:Person) WHERE f.age < 30 RETURN p.name, f.name"
        )?;
        assert_eq!(res2.columns, vec!["p.name", "f.name"]);
        assert_eq!(res2.rows.len(), 1);
        assert_eq!(res2.rows[0], vec![PropertyValue::from("Alice"), PropertyValue::from("Bob")]);

        // 3. Multi-hop Pattern Match
        let res3 = graph.cypher(
            "MATCH (p:Person)-[:KNOWS]->(f:Person)-[:WORKS_AT]->(c:Company) RETURN p.name, c.name"
        )?;
        assert_eq!(res3.rows.len(), 1);
        assert_eq!(res3.rows[0], vec![PropertyValue::from("Alice"), PropertyValue::from("Acme Corp")]);

        // 4. Formatted ASCII table display test
        let formatted = format!("{res2}");
        assert!(formatted.contains("p.name"));
        assert!(formatted.contains("f.name"));
        assert!(formatted.contains("Alice"));
        assert!(formatted.contains("Bob"));

        Ok(())
    }
}

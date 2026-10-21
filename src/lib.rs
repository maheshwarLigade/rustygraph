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
//!     // Inspect graph structure
//!     assert_eq!(graph.node_count(), 2);
//!     assert_eq!(graph.edge_count(), 1);
//!     assert_eq!(graph.degree(alice, Direction::Outgoing)?, 1);
//!
//!     // Fast label lookups
//!     let people = graph.nodes_with_label("Person");
//!     assert_eq!(people.len(), 2);
//!
//!     // Fluent path traversal
//!     let paths = graph.traverse(alice).outgoing().with_type("KNOWS").paths();
//!     assert_eq!(paths.len(), 2);
//!
//!     // Graph Data Science: Shortest Path
//!     let shortest = algo::shortest_path_bfs(&graph, alice, bob, Direction::Outgoing);
//!     assert!(shortest.is_some());
//!
//!     Ok(())
//! }
//! ```

pub mod algo;
pub mod error;
pub mod graph;
pub mod id;
pub mod index;
pub mod model;
pub mod stats;
pub mod storage;
pub mod traversal;
pub mod value;

/// Convenient re-exports of common types.
pub mod prelude {
    pub use crate::algo;
    pub use crate::error::{GraphError, Result};
    pub use crate::graph::Graph;
    pub use crate::id::{NodeId, RelId};
    pub use crate::index::{IndexKey, IndexManager, PropertyIndex};
    pub use crate::model::{Direction, Node, Properties, Relationship};
    pub use crate::stats::GraphStats;
    pub use crate::traversal::{Path, PathSegment, TraversalOrder, Traverser, Uniqueness};
    pub use crate::value::PropertyValue;
}

pub use algo::{
    a_star, common_neighbors, degree_centrality, dijkstra, dijkstra_by_prop, jaccard_similarity,
    pagerank, shortest_path_bfs, strongly_connected_components, topological_sort,
    weakly_connected_components,
};
pub use error::{GraphError, Result};
pub use graph::Graph;
pub use id::{NodeId, RelId};
pub use index::{IndexKey, IndexManager, PropertyIndex};
pub use model::{Direction, Node, Properties, Relationship};
pub use stats::GraphStats;
pub use traversal::{Path, PathSegment, TraversalOrder, Traverser, Uniqueness};
pub use value::PropertyValue;

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

        // Build a pipeline: A -> B -> C -> D and B -> E
        let a = graph.create_node_with(|n| { n.set_prop("name", "A"); })?;
        let b = graph.create_node_with(|n| { n.set_prop("name", "B"); })?;
        let c = graph.create_node_with(|n| { n.set_prop("name", "C"); })?;
        let d = graph.create_node_with(|n| { n.set_prop("name", "D"); })?;
        let e = graph.create_node_with(|n| { n.set_prop("name", "E"); })?;

        graph.create_edge(a, b, "NEXT")?;
        graph.create_edge(b, c, "NEXT")?;
        graph.create_edge(c, d, "NEXT")?;
        graph.create_edge(b, e, "BRANCH")?;

        // 1. Variable depth traversal: from A with min_depth=1, max_depth=2, type="NEXT"
        let paths_1_to_2 = graph
            .traverse(a)
            .outgoing()
            .with_type("NEXT")
            .min_depth(1)
            .max_depth(2)
            .paths();

        // Should find: (A)->(B) (depth 1), and (A)->(B)->(C) (depth 2)
        assert_eq!(paths_1_to_2.len(), 2);
        assert_eq!(paths_1_to_2[0].end_node(), b);
        assert_eq!(paths_1_to_2[1].end_node(), c);

        // 2. Distinct nodes reachable from A at any depth following "NEXT"
        let reachable = graph.traverse(a).outgoing().with_type("NEXT").nodes();
        assert_eq!(reachable, vec![a, b, c, d]);

        // 3. Traversal with multiple types: ("NEXT" and "BRANCH")
        let branch_paths = graph
            .traverse(b)
            .outgoing()
            .with_types(["NEXT", "BRANCH"])
            .min_depth(1)
            .max_depth(1)
            .paths();
        assert_eq!(branch_paths.len(), 2);
        let end_nodes: Vec<_> = branch_paths.iter().map(|p| p.end_node()).collect();
        assert!(end_nodes.contains(&c));
        assert!(end_nodes.contains(&e));

        // 4. Reverse (incoming) traversal from D
        let incoming_to_d = graph.traverse(d).incoming().min_depth(1).max_depth(1).nodes();
        assert_eq!(incoming_to_d, vec![c]);

        // 5. Undirected / Both traversal from B
        let both_from_b = graph.traverse(b).both().min_depth(1).max_depth(1).nodes();
        assert_eq!(both_from_b.len(), 3); // A (incoming), C (outgoing), E (outgoing)
        assert!(both_from_b.contains(&a));
        assert!(both_from_b.contains(&c));
        assert!(both_from_b.contains(&e));

        Ok(())
    }

    #[test]
    fn test_traversal_cycle_prevention() -> Result<()> {
        let mut graph = Graph::new();

        // Create triangle cycle: X -> Y -> Z -> X
        let x = graph.create_node();
        let y = graph.create_node();
        let z = graph.create_node();

        graph.create_edge(x, y, "CYCLE")?;
        graph.create_edge(y, z, "CYCLE")?;
        graph.create_edge(z, x, "CYCLE")?;

        // With default Uniqueness::Node, traversal from X should never loop infinitely
        let paths = graph.traverse(x).outgoing().paths();
        assert_eq!(paths.len(), 3);

        Ok(())
    }

    #[test]
    fn test_shortest_path_and_dijkstra() -> Result<()> {
        let mut graph = Graph::new();

        // Build diamond graph:
        //      (A)
        //     /   \
        //    10     2
        //   /       \
        // (B)       (C)
        //   \       /
        //    1     3
        //     \   /
        //      (D)
        let a = graph.create_node();
        let b = graph.create_node();
        let c = graph.create_node();
        let d = graph.create_node();

        let _e_ab = graph.create_edge_with(a, b, "ROAD", |r| { r.set_prop("weight", 10.0); })?;
        let _e_bd = graph.create_edge_with(b, d, "ROAD", |r| { r.set_prop("weight", 1.0); })?;
        let _e_ac = graph.create_edge_with(a, c, "ROAD", |r| { r.set_prop("weight", 2.0); })?;
        let _e_cd = graph.create_edge_with(c, d, "ROAD", |r| { r.set_prop("weight", 3.0); })?;

        // 1. BFS Shortest Path (unweighted) finds a 2-hop path (e.g. A->B->D or A->C->D)
        let bfs_path = algo::shortest_path_bfs(&graph, a, d, Direction::Outgoing).expect("path exists");
        assert_eq!(bfs_path.len(), 2);
        assert_eq!(bfs_path.start_node(), a);
        assert_eq!(bfs_path.end_node(), d);

        // 2. Dijkstra Shortest Path (weighted) should choose A -> C -> D (cost 2 + 3 = 5.0) over A -> B -> D (10 + 1 = 11.0)
        let (dijkstra_path, total_weight) =
            algo::dijkstra_by_prop(&graph, a, d, Direction::Outgoing, "weight")
                .expect("weighted path exists");

        assert_eq!(total_weight, 5.0);
        assert_eq!(dijkstra_path.nodes(), vec![a, c, d]);

        // 3. A* pathfinding
        let (astar_path, astar_weight) = algo::a_star(
            &graph,
            a,
            d,
            Direction::Outgoing,
            |r| r.prop("weight").and_then(|v| v.as_f64()).unwrap_or(1.0),
            |_curr, _target| 0.0, // admissible heuristic
        )
        .expect("A* path exists");

        assert_eq!(astar_weight, 5.0);
        assert_eq!(astar_path.nodes(), vec![a, c, d]);

        Ok(())
    }

    #[test]
    fn test_pagerank_and_centrality() -> Result<()> {
        let mut graph = Graph::new();

        // Star graph where nodes 1, 2, 3 all point to central hub 0
        let hub = graph.create_node();
        let n1 = graph.create_node();
        let n2 = graph.create_node();
        let n3 = graph.create_node();

        graph.create_edge(n1, hub, "VOTES")?;
        graph.create_edge(n2, hub, "VOTES")?;
        graph.create_edge(n3, hub, "VOTES")?;

        // Degree centrality
        let in_degrees = algo::degree_centrality(&graph, Direction::Incoming);
        assert_eq!(in_degrees.get(&hub), Some(&3));
        assert_eq!(in_degrees.get(&n1), Some(&0));

        // PageRank: Hub should have the highest PageRank score
        let pr = algo::pagerank(&graph, 0.85, 50, 1e-6);
        let hub_score = pr[&hub];
        let n1_score = pr[&n1];

        assert!(hub_score > n1_score);
        assert_eq!(pr[&n1], pr[&n2]);
        assert_eq!(pr[&n2], pr[&n3]);

        Ok(())
    }

    #[test]
    fn test_community_detection_and_topo_sort() -> Result<()> {
        let mut graph = Graph::new();

        // Subgraph 1: a -> b
        let a = graph.create_node();
        let b = graph.create_node();
        graph.create_edge(a, b, "LINK")?;

        // Subgraph 2: c -> d -> e -> c (directed cycle)
        let c = graph.create_node();
        let d = graph.create_node();
        let e = graph.create_node();
        graph.create_edge(c, d, "LINK")?;
        graph.create_edge(d, e, "LINK")?;
        graph.create_edge(e, c, "LINK")?;

        // 1. Weakly Connected Components (WCC): 2 components
        let wcc = algo::weakly_connected_components(&graph);
        assert_eq!(wcc.len(), 2);

        // 2. Strongly Connected Components (SCC):
        // {a}, {b}, and {c, d, e}
        let scc = algo::strongly_connected_components(&graph);
        assert_eq!(scc.len(), 3);
        let cyclic_scc = scc.iter().find(|s| s.len() == 3).expect("cyclic component found");
        assert!(cyclic_scc.contains(&c));
        assert!(cyclic_scc.contains(&d));
        assert!(cyclic_scc.contains(&e));

        // 3. Topological sort:
        // Currently graph has a cycle in {c, d, e}, so topological sort must fail
        assert!(algo::topological_sort(&graph).is_err());

        // Create a separate DAG
        let mut dag = Graph::new();
        let task1 = dag.create_node();
        let task2 = dag.create_node();
        let task3 = dag.create_node();
        dag.create_edge(task1, task2, "DEPENDS")?;
        dag.create_edge(task2, task3, "DEPENDS")?;

        let topo = algo::topological_sort(&dag)?;
        assert_eq!(topo, vec![task1, task2, task3]);

        Ok(())
    }

    #[test]
    fn test_similarity_algorithms() -> Result<()> {
        let mut graph = Graph::new();

        let alice = graph.create_node();
        let bob = graph.create_node();
        let charlie = graph.create_node();
        let movie1 = graph.create_node();
        let movie2 = graph.create_node();
        let movie3 = graph.create_node();

        // Alice liked movie 1 and 2
        graph.create_edge(alice, movie1, "LIKED")?;
        graph.create_edge(alice, movie2, "LIKED")?;

        // Bob liked movie 1 and 2 (identical taste)
        graph.create_edge(bob, movie1, "LIKED")?;
        graph.create_edge(bob, movie2, "LIKED")?;

        // Charlie liked movie 2 and 3
        graph.create_edge(charlie, movie2, "LIKED")?;
        graph.create_edge(charlie, movie3, "LIKED")?;

        // Common neighbors: Alice and Bob have {movie1, movie2}
        let common = algo::common_neighbors(&graph, alice, bob, Direction::Outgoing);
        assert_eq!(common.len(), 2);
        assert!(common.contains(&movie1));
        assert!(common.contains(&movie2));

        // Jaccard similarity:
        // Alice & Bob: intersection 2 / union 2 = 1.0
        let sim_ab = algo::jaccard_similarity(&graph, alice, bob, Direction::Outgoing);
        assert_eq!(sim_ab, 1.0);

        // Alice & Charlie: intersection 1 ({movie2}) / union 3 ({movie1, movie2, movie3}) = 1/3 ~ 0.333
        let sim_ac = algo::jaccard_similarity(&graph, alice, charlie, Direction::Outgoing);
        assert!((sim_ac - (1.0 / 3.0)).abs() < 1e-6);

        Ok(())
    }
}

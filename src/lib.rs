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
//!     });
//!
//!     let bob = graph.create_node_with(|n| {
//!         n.add_label("Person");
//!         n.set_prop("name", "Bob");
//!         n.set_prop("age", 28);
//!     });
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
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod graph;
pub mod id;
pub mod model;
pub mod storage;
pub mod value;

/// Convenient re-exports of common types.
pub mod prelude {
    pub use crate::error::{GraphError, Result};
    pub use crate::graph::Graph;
    pub use crate::id::{NodeId, RelId};
    pub use crate::model::{Direction, Node, Properties, Relationship};
    pub use crate::value::PropertyValue;
}

pub use error::{GraphError, Result};
pub use graph::Graph;
pub use id::{NodeId, RelId};
pub use model::{Direction, Node, Properties, Relationship};
pub use value::PropertyValue;

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn test_node_creation_and_properties() {
        let mut graph = Graph::new();

        let node_id = graph.create_node_with(|n| {
            n.add_label("Person");
            n.add_label("Developer");
            n.set_prop("name", "Neo");
            n.set_prop("skills", vec!["Rust", "Distributed Systems"]);
        });

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
    }

    #[test]
    fn test_edge_creation_and_traversal() -> Result<()> {
        let mut graph = Graph::new();

        let a = graph.create_node_with(|n| { n.add_label("City"); n.set_prop("name", "Paris"); });
        let b = graph.create_node_with(|n| { n.add_label("City"); n.set_prop("name", "Berlin"); });
        let c = graph.create_node_with(|n| { n.add_label("City"); n.set_prop("name", "Rome"); });

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

        let n1 = graph.create_node_with(|n| { n.add_label("User"); });
        let n2 = graph.create_node_with(|n| { n.add_label("User"); });
        let n3 = graph.create_node_with(|n| { n.add_label("Admin"); });

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

        // Trying to access deleted n1 should return None / Error
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
}

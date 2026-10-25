use rusty_graph::prelude::*;

#[test]
fn test_node_lifecycle_and_labels() -> Result<()> {
    let mut graph = Graph::new();
    assert_eq!(graph.node_count(), 0);

    let n1 = graph.create_node();
    assert_eq!(graph.node_count(), 1);
    assert!(graph.contains_node(n1));

    // Add labels
    assert!(graph.add_node_label(n1, "Person")?);
    assert!(!graph.add_node_label(n1, "Person")?); // adding duplicate label returns false
    assert!(graph.add_node_label(n1, "Author")?);

    let node = graph.get_node(n1).expect("node exists");
    assert!(node.has_label("Person"));
    assert!(node.has_label("Author"));
    assert!(!node.has_label("Reader"));

    // Label index check
    assert_eq!(graph.nodes_with_label("Person"), vec![n1]);
    assert_eq!(graph.nodes_with_label("Author"), vec![n1]);
    assert!(graph.nodes_with_label("Reader").is_empty());

    // Remove label
    assert!(graph.remove_node_label(n1, "Author")?);
    assert!(!graph.remove_node_label(n1, "Author")?); // removing again returns false
    assert!(!graph.get_node(n1).unwrap().has_label("Author"));
    assert!(graph.nodes_with_label("Author").is_empty());

    Ok(())
}

#[test]
fn test_edge_creation_validations_and_self_loops() -> Result<()> {
    let mut graph = Graph::new();
    let n1 = graph.create_node();
    let n2 = graph.create_node();
    let invalid_node = NodeId::new(999999);

    // Non-existent source node
    let err_src = graph.create_edge(invalid_node, n2, "REL");
    assert!(matches!(err_src, Err(GraphError::NodeNotFound(999999))));

    // Non-existent target node
    let err_dst = graph.create_edge(n1, invalid_node, "REL");
    assert!(matches!(err_dst, Err(GraphError::NodeNotFound(999999))));

    // Valid directed edge
    let e1 = graph.create_edge(n1, n2, "CONNECTS")?;
    assert_eq!(graph.edge_count(), 1);
    let edge = graph.get_edge(e1).expect("edge exists");
    assert_eq!(edge.source, n1);
    assert_eq!(edge.target, n2);
    assert_eq!(edge.rel_type, "CONNECTS");
    assert!(!edge.is_self_loop());

    // Self-loop edge
    let e_loop = graph.create_edge(n1, n1, "SELF_REF")?;
    let loop_edge = graph.get_edge(e_loop).expect("loop edge exists");
    assert!(loop_edge.is_self_loop());

    // Parallel edges between n1 and n2
    let e2 = graph.create_edge(n1, n2, "ALSO_CONNECTS")?;
    assert_eq!(graph.edge_count(), 3);
    assert_ne!(e1, e2);

    Ok(())
}

#[test]
fn test_node_and_edge_property_mutations() -> Result<()> {
    let mut graph = Graph::new();
    let n = graph.create_node_with(|node| {
        node.set_prop("title", "Book");
        node.set_prop("pages", 350);
    })?;

    assert_eq!(
        graph.get_node_property(n, "title"),
        Some(&PropertyValue::from("Book"))
    );
    assert_eq!(
        graph.get_node_property(n, "pages"),
        Some(&PropertyValue::from(350))
    );
    assert_eq!(graph.get_node_property(n, "non_existent"), None);

    // Overwrite property
    let old_pages = graph.set_node_property(n, "pages", 400)?;
    assert_eq!(old_pages, Some(PropertyValue::from(350)));
    assert_eq!(
        graph.get_node_property(n, "pages"),
        Some(&PropertyValue::from(400))
    );

    // Remove property
    let removed = graph.remove_node_property(n, "title")?;
    assert_eq!(removed, Some(PropertyValue::from("Book")));
    assert_eq!(graph.get_node_property(n, "title"), None);

    // Edge properties
    let n2 = graph.create_node();
    let edge_id = graph.create_edge_with(n, n2, "READS", |r| {
        r.set_prop("rating", 4.8);
    })?;

    assert_eq!(
        graph.get_edge_property(edge_id, "rating"),
        Some(&PropertyValue::from(4.8))
    );
    graph.set_edge_property(edge_id, "rating", 5.0)?;
    assert_eq!(
        graph.get_edge_property(edge_id, "rating"),
        Some(&PropertyValue::from(5.0))
    );
    graph.remove_edge_property(edge_id, "rating")?;
    assert_eq!(graph.get_edge_property(edge_id, "rating"), None);

    Ok(())
}

#[test]
fn test_cascading_deletions_and_integrity() -> Result<()> {
    let mut graph = Graph::new();
    let n1 = graph.create_node();
    let n2 = graph.create_node();
    let n3 = graph.create_node();

    let e1 = graph.create_edge(n1, n2, "E1")?;
    let e2 = graph.create_edge(n2, n3, "E2")?;
    let e3 = graph.create_edge(n3, n1, "E3")?;
    let e_loop = graph.create_edge(n2, n2, "LOOP")?; // self loop on n2

    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 4);

    // Delete n2 - must cascade delete e1, e2, and e_loop
    let deleted_node = graph.delete_node(n2)?;
    assert_eq!(deleted_node.id, n2);
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 1); // Only e3 (n3 -> n1) remains

    assert!(!graph.contains_edge(e1));
    assert!(!graph.contains_edge(e2));
    assert!(!graph.contains_edge(e_loop));
    assert!(graph.contains_edge(e3));

    // Verify adjacency of remaining nodes n1 and n3
    assert_eq!(graph.degree(n1, Direction::Incoming)?, 1);
    assert_eq!(graph.degree(n1, Direction::Outgoing)?, 0);
    assert_eq!(graph.degree(n3, Direction::Outgoing)?, 1);
    assert_eq!(graph.degree(n3, Direction::Incoming)?, 0);

    // Explicit edge deletion
    let deleted_e3 = graph.delete_edge(e3)?;
    assert_eq!(deleted_e3.id, e3);
    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.degree(n1, Direction::Both)?, 0);
    assert_eq!(graph.degree(n3, Direction::Both)?, 0);

    Ok(())
}

#[test]
fn test_degree_and_neighbor_lookups() -> Result<()> {
    let mut graph = Graph::new();
    let center = graph.create_node();
    let out1 = graph.create_node();
    let out2 = graph.create_node();
    let in1 = graph.create_node();

    graph.create_edge(center, out1, "OUT_REL")?;
    graph.create_edge(center, out2, "OUT_REL")?;
    graph.create_edge(in1, center, "IN_REL")?;

    assert_eq!(graph.degree(center, Direction::Outgoing)?, 2);
    assert_eq!(graph.degree(center, Direction::Incoming)?, 1);
    assert_eq!(graph.degree(center, Direction::Both)?, 3);

    let out_neighbors = graph.neighbors(center, Direction::Outgoing)?;
    assert_eq!(out_neighbors.len(), 2);
    assert!(out_neighbors.contains(&out1));
    assert!(out_neighbors.contains(&out2));

    let in_neighbors = graph.neighbors(center, Direction::Incoming)?;
    assert_eq!(in_neighbors, vec![in1]);

    let both_neighbors = graph.neighbors(center, Direction::Both)?;
    assert_eq!(both_neighbors.len(), 3);

    Ok(())
}

use rusty_graph::prelude::*;
use std::ops::Bound::{Excluded, Included, Unbounded};

#[test]
fn test_index_creation_on_existing_graph() -> Result<()> {
    let mut graph = Graph::new();

    // Populate graph before creating index
    let u1 = graph.create_node_with(|n| {
        n.add_label("Customer");
        n.set_prop("country", "CA");
    })?;
    let u2 = graph.create_node_with(|n| {
        n.add_label("Customer");
        n.set_prop("country", "US");
    })?;

    // Create index on populated graph
    assert!(graph.create_index("Customer", "country")?);
    assert!(graph.has_index("Customer", "country"));
    // Calling again returns false (already exists)
    assert!(!graph.create_index("Customer", "country")?);

    // Verify index was populated from existing nodes
    let us_customers =
        graph.find_nodes_by_property("Customer", "country", &PropertyValue::from("US"));
    assert_eq!(us_customers, vec![u2]);

    let ca_customers =
        graph.find_nodes_by_property("Customer", "country", &PropertyValue::from("CA"));
    assert_eq!(ca_customers, vec![u1]);

    // Drop index
    assert!(graph.drop_index("Customer", "country"));
    assert!(!graph.has_index("Customer", "country"));
    // Dropping again returns false
    assert!(!graph.drop_index("Customer", "country"));

    Ok(())
}

#[test]
fn test_index_range_queries_and_bounds() -> Result<()> {
    let mut graph = Graph::new();
    graph.create_index("Product", "price")?;

    let _p10 = graph.create_node_with(|n| {
        n.add_label("Product");
        n.set_prop("price", 10.0);
    })?;
    let p25 = graph.create_node_with(|n| {
        n.add_label("Product");
        n.set_prop("price", 25.0);
    })?;
    let p50 = graph.create_node_with(|n| {
        n.add_label("Product");
        n.set_prop("price", 50.0);
    })?;
    let p100 = graph.create_node_with(|n| {
        n.add_label("Product");
        n.set_prop("price", 100.0);
    })?;

    // Inclusive range: 25.0 ..= 50.0
    let range1 = graph.find_nodes_by_property_range(
        "Product",
        "price",
        (
            Included(PropertyValue::from(25.0)),
            Included(PropertyValue::from(50.0)),
        ),
    );
    assert_eq!(range1.len(), 2);
    assert!(range1.contains(&p25));
    assert!(range1.contains(&p50));

    // Excluded upper bound: 25.0 .. 50.0
    let range2 = graph.find_nodes_by_property_range(
        "Product",
        "price",
        (
            Included(PropertyValue::from(25.0)),
            Excluded(PropertyValue::from(50.0)),
        ),
    );
    assert_eq!(range2, vec![p25]);

    // Unbounded upper bound: >= 50.0
    let range3 = graph.find_nodes_by_property_range(
        "Product",
        "price",
        (Included(PropertyValue::from(50.0)), Unbounded),
    );
    assert_eq!(range3.len(), 2);
    assert!(range3.contains(&p50));
    assert!(range3.contains(&p100));

    Ok(())
}

#[test]
fn test_index_maintenance_on_mutations_and_deletions() -> Result<()> {
    let mut graph = Graph::new();
    graph.create_index("Item", "status")?;

    let item = graph.create_node_with(|n| {
        n.add_label("Item");
        n.set_prop("status", "pending");
    })?;

    assert_eq!(
        graph.find_nodes_by_property("Item", "status", &PropertyValue::from("pending")),
        vec![item]
    );

    // Mutate property from "pending" to "complete"
    graph.set_node_property(item, "status", "complete")?;
    assert!(
        graph
            .find_nodes_by_property("Item", "status", &PropertyValue::from("pending"))
            .is_empty()
    );
    assert_eq!(
        graph.find_nodes_by_property("Item", "status", &PropertyValue::from("complete")),
        vec![item]
    );

    // Remove property
    graph.remove_node_property(item, "status")?;
    assert!(
        graph
            .find_nodes_by_property("Item", "status", &PropertyValue::from("complete"))
            .is_empty()
    );

    // Re-add property and delete node
    graph.set_node_property(item, "status", "complete")?;
    graph.delete_node(item)?;
    assert!(
        graph
            .find_nodes_by_property("Item", "status", &PropertyValue::from("complete"))
            .is_empty()
    );

    Ok(())
}

#[test]
fn test_unique_constraint_enforcement() -> Result<()> {
    let mut graph = Graph::new();

    // 1. Cannot create unique constraint if duplicates already exist
    let _u1 = graph.create_node_with(|n| {
        n.add_label("Account");
        n.set_prop("email", "test@test.com");
    })?;
    let _u2 = graph.create_node_with(|n| {
        n.add_label("Account");
        n.set_prop("email", "test@test.com"); // duplicate!
    })?;

    let err_constraint = graph.create_unique_constraint("Account", "email");
    assert!(matches!(
        err_constraint,
        Err(GraphError::ConstraintViolation(_))
    ));

    // Clean up graph
    graph.clear();

    // 2. Create unique constraint on clean graph
    assert!(graph.create_unique_constraint("Account", "email")?);
    assert!(graph.has_unique_constraint("Account", "email"));

    let acc1 = graph.create_node_with(|n| {
        n.add_label("Account");
        n.set_prop("email", "admin@company.com");
    })?;

    // Attempt to insert duplicate
    let dup_insert = graph.create_node_with(|n| {
        n.add_label("Account");
        n.set_prop("email", "admin@company.com");
    });
    assert!(matches!(
        dup_insert,
        Err(GraphError::ConstraintViolation(_))
    ));

    // Attempt to update another node with existing email
    let acc2 = graph.create_node_with(|n| {
        n.add_label("Account");
        n.set_prop("email", "user@company.com");
    })?;
    let dup_update = graph.set_node_property(acc2, "email", "admin@company.com");
    assert!(matches!(
        dup_update,
        Err(GraphError::ConstraintViolation(_))
    ));

    // Updating self with same email must succeed
    assert!(
        graph
            .set_node_property(acc1, "email", "admin@company.com")
            .is_ok()
    );

    // Drop unique constraint
    assert!(graph.drop_unique_constraint("Account", "email"));
    assert!(!graph.has_unique_constraint("Account", "email"));

    // Now duplicate insert succeeds
    let dup_allowed = graph.create_node_with(|n| {
        n.add_label("Account");
        n.set_prop("email", "admin@company.com");
    });
    assert!(dup_allowed.is_ok());

    Ok(())
}

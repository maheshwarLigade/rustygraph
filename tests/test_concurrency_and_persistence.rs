use rusty_graph::prelude::*;
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_graph_high_concurrency() -> Result<()> {
    let cg = Arc::new(ConcurrentGraph::new());
    let mut handles = vec![];

    // Spawn 8 worker threads
    for t_idx in 0..8 {
        let g = Arc::clone(&cg);
        handles.push(thread::spawn(move || {
            for i in 0..50 {
                let n1 = g.create_node();
                let n2 = g.create_node();
                g.set_node_property(n1, "worker", t_idx).unwrap();
                g.set_node_property(n2, "step", i).unwrap();
                g.create_edge(n1, n2, "PROCESSED_BY").unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // 8 threads * 50 iterations * 2 nodes = 800 nodes
    // 8 threads * 50 iterations = 400 edges
    assert_eq!(cg.node_count(), 800);
    assert_eq!(cg.edge_count(), 400);

    let stats = cg.stats();
    assert_eq!(stats.node_count, 800);
    assert_eq!(stats.edge_count, 400);

    Ok(())
}

#[test]
fn test_json_persistence_round_trip_and_reindexing() -> Result<()> {
    let mut original = Graph::new();

    // Create rich dataset with multiple property types
    let ny = original.create_node_with(|n| {
        n.add_label("Location");
        n.add_label("City");
        n.set_prop("name", "New York");
        n.set_prop("population", 8_336_817);
        n.set_prop("coords", vec![40.7128, -74.0060]);
    })?;

    let london = original.create_node_with(|n| {
        n.add_label("Location");
        n.add_label("City");
        n.set_prop("name", "London");
        n.set_prop("population", 8_982_000);
        n.set_prop("coords", vec![51.5074, -0.1278]);
    })?;

    original.create_edge_with(ny, london, "FLIGHT", |r| {
        r.set_prop("carrier", "BA");
        r.set_prop("duration_hours", 7.0);
    })?;

    // Export to JSON string
    let json_data = original.to_json()?;
    assert!(!json_data.is_empty());

    // Import into fresh graph
    let mut restored = Graph::from_json(&json_data)?;

    assert_eq!(restored.node_count(), 2);
    assert_eq!(restored.edge_count(), 1);

    // Verify label indexes were restored
    let cities = restored.nodes_with_label("City");
    assert_eq!(cities.len(), 2);

    // Verify relationships and properties were restored
    let flights = restored.edges_with_type("FLIGHT");
    assert_eq!(flights.len(), 1);
    let flight_edge = restored.get_edge(flights[0]).unwrap();
    assert_eq!(flight_edge.prop("carrier"), Some(&PropertyValue::from("BA")));
    assert_eq!(flight_edge.prop("duration_hours"), Some(&PropertyValue::from(7.0)));

    // Verify we can create and use property indexes on restored graph
    restored.create_index("City", "name")?;
    let found = restored.find_nodes_by_property("City", "name", &PropertyValue::from("London"));
    assert_eq!(found.len(), 1);

    Ok(())
}

#[test]
fn test_json_persistence_malformed_input() {
    let invalid_json = "{ invalid json content }";
    let res = Graph::from_json(invalid_json);
    assert!(res.is_err());
    assert!(matches!(res, Err(GraphError::SerializationError(_))));
}

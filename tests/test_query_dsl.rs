use rusty_graph::prelude::*;

#[test]
fn test_query_builder_single_node() -> Result<()> {
    let mut graph = Graph::new();
    let _a = graph.create_node_with(|n| {
        n.add_label("Person");
        n.set_prop("age", 25);
    })?;
    let _b = graph.create_node_with(|n| {
        n.add_label("Person");
        n.set_prop("age", 35);
    })?;
    let _c = graph.create_node_with(|n| {
        n.add_label("Bot");
        n.set_prop("age", 1);
    })?;

    // Match all Persons with age >= 30
    let seniors = graph
        .query()
        .match_node("p")
        .with_label("Person")
        .filter(|ctx| ctx.prop("p", "age").and_then(|v| v.as_i64()).unwrap_or(0) >= 30)
        .project(|ctx| ctx.prop("p", "age").unwrap().as_i64().unwrap());

    assert_eq!(seniors, vec![35]);

    Ok(())
}

#[test]
fn test_query_builder_multi_hop_and_edge_alias() -> Result<()> {
    let mut graph = Graph::new();

    // Alice -> Bob -> Charlie
    let alice = graph.create_node_with(|n| {
        n.set_prop("name", "Alice");
    })?;
    let bob = graph.create_node_with(|n| {
        n.set_prop("name", "Bob");
    })?;
    let charlie = graph.create_node_with(|n| {
        n.set_prop("name", "Charlie");
    })?;

    graph.create_edge_with(alice, bob, "FRIEND", |r| {
        r.set_prop("years", 5);
    })?;
    graph.create_edge_with(bob, charlie, "FRIEND", |r| {
        r.set_prop("years", 2);
    })?;

    // Multi-hop pattern: (a)-[r1:FRIEND]->(b)-[r2:FRIEND]->(c)
    let paths = graph
        .query()
        .match_node("a")
        .outgoing_as("r1", "FRIEND", "b")
        .outgoing_as("r2", "FRIEND", "c")
        .project(|ctx| {
            (
                ctx.prop("a", "name").unwrap().as_str().unwrap().to_string(),
                ctx.prop("r1", "years").unwrap().as_i64().unwrap(),
                ctx.prop("b", "name").unwrap().as_str().unwrap().to_string(),
                ctx.prop("r2", "years").unwrap().as_i64().unwrap(),
                ctx.prop("c", "name").unwrap().as_str().unwrap().to_string(),
            )
        });

    assert_eq!(
        paths,
        vec![(
            "Alice".to_string(),
            5,
            "Bob".to_string(),
            2,
            "Charlie".to_string()
        )]
    );

    Ok(())
}

#[test]
fn test_query_builder_order_by_and_pagination() -> Result<()> {
    let mut graph = Graph::new();
    for i in (1..=10).rev() {
        graph.create_node_with(|n| {
            n.add_label("Record");
            n.set_prop("seq", i);
        })?;
    }

    // Order by seq ASC, skip 2, limit 3 -> items with seq: 3, 4, 5
    let page = graph
        .query()
        .match_node("r")
        .with_label("Record")
        .order_by(
            |ctx| ctx.prop("r", "seq").and_then(|v| v.as_i64()).unwrap_or(0),
            true,
        )
        .skip(2)
        .limit(3)
        .project(|ctx| ctx.prop("r", "seq").unwrap().as_i64().unwrap());

    assert_eq!(page, vec![3, 4, 5]);

    // Order by seq DESC, limit 2 -> items with seq: 10, 9
    let top2 = graph
        .query()
        .match_node("r")
        .with_label("Record")
        .order_by(
            |ctx| ctx.prop("r", "seq").and_then(|v| v.as_i64()).unwrap_or(0),
            false,
        )
        .limit(2)
        .project(|ctx| ctx.prop("r", "seq").unwrap().as_i64().unwrap());

    assert_eq!(top2, vec![10, 9]);

    Ok(())
}

#[test]
fn test_query_builder_empty_results() -> Result<()> {
    let mut graph = Graph::new();
    graph.create_node_with(|n| {
        n.add_label("User");
    })?;

    let results = graph
        .query()
        .match_node("u")
        .with_label("NonExistentLabel")
        .project(|ctx| ctx.node_id("u").unwrap());

    assert!(results.is_empty());

    Ok(())
}

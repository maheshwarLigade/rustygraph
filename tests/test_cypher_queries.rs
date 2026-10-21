use rusty_graph::prelude::*;

#[test]
fn test_cypher_node_pattern_and_props() -> Result<()> {
    let mut graph = Graph::new();
    graph.create_node_with(|n| {
        n.add_label("Developer");
        n.set_prop("name", "Ferris");
        n.set_prop("language", "Rust");
    })?;

    let res = graph.cypher("MATCH (d:Developer {language: 'Rust'}) RETURN d.name, d.language")?;
    assert_eq!(res.columns, vec!["d.name", "d.language"]);
    assert_eq!(res.rows.len(), 1);
    assert_eq!(res.rows[0], vec![PropertyValue::from("Ferris"), PropertyValue::from("Rust")]);

    Ok(())
}

#[test]
fn test_cypher_relationships_and_directions() -> Result<()> {
    let mut graph = Graph::new();
    let a = graph.create_node_with(|n| { n.set_prop("id", "A"); })?;
    let b = graph.create_node_with(|n| { n.set_prop("id", "B"); })?;
    graph.create_edge(a, b, "CONNECTED_TO")?;

    // Outgoing
    let out_res = graph.cypher("MATCH (x)-[:CONNECTED_TO]->(y) RETURN x.id, y.id")?;
    assert_eq!(out_res.rows.len(), 1);
    assert_eq!(out_res.rows[0], vec![PropertyValue::from("A"), PropertyValue::from("B")]);

    // Incoming: (y)<-[:CONNECTED_TO]-(x)
    let in_res = graph.cypher("MATCH (y)<-[:CONNECTED_TO]-(x) RETURN x.id, y.id")?;
    assert_eq!(in_res.rows.len(), 1);
    assert_eq!(in_res.rows[0], vec![PropertyValue::from("A"), PropertyValue::from("B")]);

    Ok(())
}

#[test]
fn test_cypher_where_operators() -> Result<()> {
    let mut graph = Graph::new();
    let _n1 = graph.create_node_with(|n| { n.add_label("Score"); n.set_prop("val", 10); })?;
    let _n2 = graph.create_node_with(|n| { n.add_label("Score"); n.set_prop("val", 20); })?;
    let _n3 = graph.create_node_with(|n| { n.add_label("Score"); n.set_prop("val", 30); })?;

    // Equal
    let eq_res = graph.cypher("MATCH (s:Score) WHERE s.val = 20 RETURN s.val")?;
    assert_eq!(eq_res.rows.len(), 1);
    assert_eq!(eq_res.rows[0], vec![PropertyValue::from(20)]);

    // Greater than or equal
    let gte_res = graph.cypher("MATCH (s:Score) WHERE s.val >= 20 RETURN s.val")?;
    assert_eq!(gte_res.rows.len(), 2);

    // Less than
    let lt_res = graph.cypher("MATCH (s:Score) WHERE s.val < 20 RETURN s.val")?;
    assert_eq!(lt_res.rows.len(), 1);
    assert_eq!(lt_res.rows[0], vec![PropertyValue::from(10)]);

    // Compound AND: s.val > 10 AND s.val < 30
    let and_res = graph.cypher("MATCH (s:Score) WHERE s.val > 10 AND s.val < 30 RETURN s.val")?;
    assert_eq!(and_res.rows.len(), 1);
    assert_eq!(and_res.rows[0], vec![PropertyValue::from(20)]);

    Ok(())
}

#[test]
fn test_cypher_limit_clause() -> Result<()> {
    let mut graph = Graph::new();
    for i in 0..10 {
        graph.create_node_with(|n| { n.add_label("Num"); n.set_prop("v", i); })?;
    }

    let lim_res = graph.cypher("MATCH (n:Num) RETURN n.v LIMIT 3")?;
    assert_eq!(lim_res.rows.len(), 3);

    Ok(())
}

#[test]
fn test_cypher_syntax_error_handling() {
    let mut graph = Graph::new();

    // Missing closing parenthesis
    let err1 = graph.cypher("MATCH (a:Person RETURN a.name");
    assert!(err1.is_err());

    // Invalid keyword
    let err2 = graph.cypher("SELECT * FROM nodes");
    assert!(err2.is_err());
}

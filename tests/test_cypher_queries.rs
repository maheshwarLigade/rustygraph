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

#[test]
fn test_cypher_parameterized_queries_and_plan_caching() -> Result<()> {
    let mut graph = Graph::new();

    // Create test nodes
    graph.cypher("CREATE (p:User {name: 'Alice', age: 28, city: 'London'})")?;
    graph.cypher("CREATE (p:User {name: 'Bob', age: 34, city: 'Paris'})")?;
    graph.cypher("CREATE (p:User {name: 'Charlie', age: 42, city: 'London'})")?;

    // Parameterized WHERE clause
    let mut params = std::collections::HashMap::new();
    params.insert("target_city".to_string(), PropertyValue::from("London"));

    let res = graph.cypher_with_params(
        "MATCH (p:User) WHERE p.city = $target_city RETURN p.name, p.age ORDER BY p.age ASC",
        &params,
    )?;

    assert_eq!(res.len(), 2);
    assert_eq!(res.rows[0][0], PropertyValue::from("Alice"));
    assert_eq!(res.rows[1][0], PropertyValue::from("Charlie"));

    // Reuse the same query with different parameter
    params.insert("target_city".to_string(), PropertyValue::from("Paris"));
    let res_paris = graph.cypher_with_params(
        "MATCH (p:User) WHERE p.city = $target_city RETURN p.name, p.age ORDER BY p.age ASC",
        &params,
    )?;
    assert_eq!(res_paris.len(), 1);
    assert_eq!(res_paris.rows[0][0], PropertyValue::from("Bob"));

    // Parameterized CREATE mutation
    let mut create_params = std::collections::HashMap::new();
    create_params.insert("new_name".to_string(), PropertyValue::from("Diana"));
    create_params.insert("new_city".to_string(), PropertyValue::from("Tokyo"));
    graph.cypher_with_params("CREATE (p:User {name: $new_name, city: $new_city})", &create_params)?;

    let res_tokyo = graph.cypher("MATCH (p:User) WHERE p.city = 'Tokyo' RETURN p.name")?;
    assert_eq!(res_tokyo.len(), 1);
    assert_eq!(res_tokyo.rows[0][0], PropertyValue::from("Diana"));

    Ok(())
}

#[test]
fn test_cypher_variable_length_paths() -> Result<()> {
    let mut graph = Graph::new();

    let a = graph.create_node_with(|n| { n.add_label("Station"); n.set_prop("name", "A"); })?;
    let b = graph.create_node_with(|n| { n.add_label("Station"); n.set_prop("name", "B"); })?;
    let c = graph.create_node_with(|n| { n.add_label("Station"); n.set_prop("name", "C"); })?;
    let d = graph.create_node_with(|n| { n.add_label("Station"); n.set_prop("name", "D"); })?;

    graph.create_edge(a, b, "TRACK")?;
    graph.create_edge(b, c, "TRACK")?;
    graph.create_edge(c, d, "TRACK")?;

    // 1 to 2 hops from A should reach B and C
    let res_1_2 = graph.cypher("MATCH (a:Station {name: 'A'})-[:TRACK*1..2]->(b:Station) RETURN b.name ORDER BY b.name ASC")?;
    assert_eq!(res_1_2.len(), 2);
    assert_eq!(res_1_2.rows[0][0], PropertyValue::from("B"));
    assert_eq!(res_1_2.rows[1][0], PropertyValue::from("C"));

    // Exactly 3 hops from A should reach D
    let res_exact_3 = graph.cypher("MATCH (a:Station {name: 'A'})-[:TRACK*3]->(b:Station) RETURN b.name")?;
    assert_eq!(res_exact_3.len(), 1);
    assert_eq!(res_exact_3.rows[0][0], PropertyValue::from("D"));

    Ok(())
}

#[test]
fn test_cypher_collect_and_distinct() -> Result<()> {
    let mut graph = Graph::new();

    graph.cypher("CREATE (:Dev {name: 'Alice', role: 'Rust'})")?;
    graph.cypher("CREATE (:Dev {name: 'Bob', role: 'Rust'})")?;
    graph.cypher("CREATE (:Dev {name: 'Charlie', role: 'Go'})")?;

    // Total count vs Distinct count
    let res_count = graph.cypher("MATCH (d:Dev) RETURN count(d.role), count(DISTINCT d.role)")?;
    assert_eq!(res_count.len(), 1);
    assert_eq!(res_count.rows[0][0], PropertyValue::from(3));
    assert_eq!(res_count.rows[0][1], PropertyValue::from(2));

    // Collect list aggregation
    let res_collect = graph.cypher("MATCH (d:Dev) RETURN collect(DISTINCT d.role)")?;
    assert_eq!(res_collect.len(), 1);
    if let PropertyValue::List(list) = &res_collect.rows[0][0] {
        assert_eq!(list.len(), 2);
        assert!(list.contains(&PropertyValue::from("Rust")));
        assert!(list.contains(&PropertyValue::from("Go")));
    } else {
        panic!("Expected list from collect");
    }

    // RETURN DISTINCT
    let res_distinct = graph.cypher("MATCH (d:Dev) RETURN DISTINCT d.role")?;
    assert_eq!(res_distinct.len(), 2);

    Ok(())
}

#[test]
fn test_cypher_merge_and_upsert() -> Result<()> {
    let mut graph = Graph::new();

    // 1. First MERGE creates the node and triggers ON CREATE SET
    let r1 = graph.cypher(
        "MERGE (u:User {email: 'alice@example.com'}) \
         ON CREATE SET u.created = 1, u.logins = 1 \
         RETURN u.email, u.created, u.logins"
    )?;
    assert_eq!(r1.len(), 1);
    assert_eq!(r1.rows[0][0], PropertyValue::from("alice@example.com"));
    assert_eq!(r1.rows[0][1], PropertyValue::from(1));
    assert_eq!(r1.rows[0][2], PropertyValue::from(1));
    assert_eq!(graph.node_count(), 1);

    // 2. Second MERGE matches existing node and triggers ON MATCH SET
    let r2 = graph.cypher(
        "MERGE (u:User {email: 'alice@example.com'}) \
         ON MATCH SET u.logins = 2 \
         RETURN u.email, u.logins"
    )?;
    assert_eq!(r2.len(), 1);
    assert_eq!(r2.rows[0][0], PropertyValue::from("alice@example.com"));
    assert_eq!(r2.rows[0][1], PropertyValue::from(2));
    // Verify node count has NOT increased
    assert_eq!(graph.node_count(), 1);

    // 3. MERGE with relationship
    let r3 = graph.cypher(
        "MERGE (a:User {email: 'alice@example.com'})-[r:FOLLOWS]->(b:User {email: 'bob@example.com'}) \
         RETURN a.email, b.email"
    )?;
    assert_eq!(r3.len(), 1);
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.relationship_count(), 1);

    // 4. Repeating the relationship MERGE does not create duplicate edges or nodes
    let r4 = graph.cypher(
        "MERGE (a:User {email: 'alice@example.com'})-[r:FOLLOWS]->(b:User {email: 'bob@example.com'}) \
         RETURN a.email, b.email"
    )?;
    assert_eq!(r4.len(), 1);
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.relationship_count(), 1);

    Ok(())
}



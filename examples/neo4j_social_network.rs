use rusty_graph::prelude::*;

fn main() -> Result<()> {
    println!("=== RustyGraph: In-Memory Neo4j-Style Graph Engine ===");

    let mut graph = Graph::new();

    // 1. Setup Unique Constraints and Indexes
    println!("\n[1] Configuring schema & indexes...");
    graph.create_unique_constraint("User", "username")?;
    graph.create_index("User", "age")?;
    println!("✓ Unique constraint created on (:User {{username}})");
    println!("✓ Property index created on (:User {{age}})");

    // 2. Create Nodes (Users, Tech Stacks, Companies)
    println!("\n[2] Populating graph entities...");
    let alice = graph.create_node_with(|n| {
        n.add_label("User");
        n.add_label("Engineer");
        n.set_prop("username", "alice");
        n.set_prop("name", "Alice Chen");
        n.set_prop("age", 29);
        n.set_prop("city", "Seattle");
    })?;

    let bob = graph.create_node_with(|n| {
        n.add_label("User");
        n.add_label("DataScientist");
        n.set_prop("username", "bob");
        n.set_prop("name", "Bob Miller");
        n.set_prop("age", 34);
        n.set_prop("city", "San Francisco");
    })?;

    let charlie = graph.create_node_with(|n| {
        n.add_label("User");
        n.add_label("Engineer");
        n.set_prop("username", "charlie");
        n.set_prop("name", "Charlie Davis");
        n.set_prop("age", 25);
        n.set_prop("city", "Austin");
    })?;

    let acme_corp = graph.create_node_with(|n| {
        n.add_label("Company");
        n.set_prop("name", "Acme AI Corp");
        n.set_prop("industry", "Artificial Intelligence");
    })?;

    let rust_lang = graph.create_node_with(|n| {
        n.add_label("Technology");
        n.set_prop("name", "Rust");
        n.set_prop("paradigm", "Systems");
    })?;

    println!("✓ Created 5 nodes across labels: User, Engineer, DataScientist, Company, Technology");

    // 3. Create Directed Relationships with Properties
    println!("\n[3] Creating labeled property relationships...");
    graph.create_edge_with(alice, bob, "KNOWS", |r| {
        r.set_prop("since", 2021);
        r.set_prop("weight", 1.2);
    })?;

    graph.create_edge_with(bob, charlie, "KNOWS", |r| {
        r.set_prop("since", 2023);
        r.set_prop("weight", 0.8);
    })?;

    graph.create_edge_with(alice, charlie, "KNOWS", |r| {
        r.set_prop("since", 2019);
        r.set_prop("weight", 2.5);
    })?;

    graph.create_edge_with(alice, acme_corp, "WORKS_AT", |r| {
        r.set_prop("role", "Staff Architect");
    })?;

    graph.create_edge_with(bob, acme_corp, "WORKS_AT", |r| {
        r.set_prop("role", "Lead ML Engineer");
    })?;

    graph.create_edge(alice, rust_lang, "EXPERTISE_IN")?;
    graph.create_edge(charlie, rust_lang, "EXPERTISE_IN")?;

    println!("✓ Created 7 relationships with properties and types: KNOWS, WORKS_AT, EXPERTISE_IN");

    // 4. Test Unique Constraint Protection
    println!("\n[4] Testing unique constraint protection...");
    let duplicate_user = graph.create_node_with(|n| {
        n.add_label("User");
        n.set_prop("username", "alice"); // Duplicate username!
    });
    match duplicate_user {
        Err(GraphError::ConstraintViolation(msg)) => {
            println!("✓ Successfully rejected duplicate user: {msg}");
        }
        _ => panic!("Expected constraint violation!"),
    }

    // 5. Declarative Pattern Matching (Cypher-style)
    println!("\n[5] Declarative Pattern Match: Coworkers at Acme Corp");
    println!("    MATCH (u:User)-[:WORKS_AT]->(c:Company {{name: 'Acme AI Corp'}})");
    let coworkers: Vec<(String, String)> = graph
        .query()
        .match_node("u")
        .with_label("User")
        .outgoing("WORKS_AT", "c")
        .with_label("Company")
        .with_prop("name", "Acme AI Corp")
        .project(|ctx| {
            (
                ctx.prop("u", "name").unwrap().as_str().unwrap().to_string(),
                ctx.prop("c", "name").unwrap().as_str().unwrap().to_string(),
            )
        });

    for (user, company) in coworkers {
        println!("    -> {user} works at {company}");
    }

    // 5b. Direct Cypher Query String execution!
    println!("\n[5b] Direct Cypher Query String execution:");
    let cypher_query =
        "MATCH (u:User)-[:WORKS_AT]->(c:Company) WHERE u.age > 28 RETURN u.name, u.age, c.name";
    println!("    Query: {cypher_query}");
    let cypher_result = graph.cypher(cypher_query)?;
    println!("{cypher_result}");

    // 6. Graph Data Science: PageRank Centrality
    println!("\n[6] Running PageRank Centrality (iterative power iteration)...");
    let pr = algo::pagerank(&graph, 0.85, 50, 1e-6);
    let mut ranked_nodes: Vec<_> = pr.iter().collect();
    ranked_nodes.sort_by(|a, b| b.1.total_cmp(a.1));

    for (id, score) in ranked_nodes.iter().take(3) {
        let node = graph.get_node(**id).unwrap();
        let name = node
            .prop("name")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "Unknown".into());
        println!("    Ranked Node {id} ({name}): PageRank Score = {score:.4}");
    }

    // 7. Graph Data Science: Shortest Path (Dijkstra weighted by relationship 'weight')
    println!("\n[7] Weighted Shortest Path: Alice -> Charlie");
    if let Some((path, cost)) =
        algo::dijkstra_by_prop(&graph, alice, charlie, Direction::Outgoing, "weight")
    {
        println!(
            "    Optimal Path length: {} hops, Total Weight: {:.2}",
            path.len(),
            cost
        );
        let node_names: Vec<String> = path
            .nodes()
            .iter()
            .map(|&nid| {
                graph
                    .get_node(nid)
                    .unwrap()
                    .prop("name")
                    .unwrap()
                    .to_string()
            })
            .collect();
        println!("    Path: {}", node_names.join(" -> "));
    }

    // 8. Graph Statistics
    println!("\n[8] Graph Summary Statistics:");
    let stats = graph.stats();
    println!("    Active Nodes:        {}", stats.node_count);
    println!("    Active Edges:        {}", stats.edge_count);
    println!("    Density:             {:.4}", stats.density);
    println!("    Average Degree:      {:.2}", stats.average_degree);
    println!("    Label Breakdown:     {:?}", stats.label_counts);
    println!("    Rel Type Breakdown:  {:?}", stats.type_counts);

    // 9. JSON Serialization / Persistence
    println!("\n[9] JSON Serialization Preview (First 400 chars):");
    let json_export = graph.to_json()?;
    println!("{}", &json_export[..json_export.len().min(400)]);
    println!("    ... (truncated)");

    println!("\n=== RustyGraph Demonstration Complete! ===");
    Ok(())
}

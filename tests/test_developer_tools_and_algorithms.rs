use rusty_graph::error::Result;
use rusty_graph::graph::Graph;
use rusty_graph::server::StandaloneServer;
use rusty_graph::server::config::ServerConfig;
use rusty_graph::storage::db::DatabaseConfig;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn temp_test_dir(name: &str) -> PathBuf {
    let rand = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir = std::env::temp_dir().join(format!("rustygraph_dev_tools_{}_{}", name, rand));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn http_request(method: &str, url: &str, body: &str) -> Result<(u16, String)> {
    let stripped = url.strip_prefix("http://").unwrap_or(url);
    let mut parts = stripped.split('/');
    let host_port = parts.next().unwrap();
    let path = format!("/{}", parts.collect::<Vec<_>>().join("/"));

    let mut stream = TcpStream::connect(host_port)?;
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method,
        path,
        host_port,
        body.len(),
        body
    );
    stream.write_all(req.as_bytes())?;

    let mut resp = Vec::new();
    stream.read_to_end(&mut resp)?;
    let resp_str = String::from_utf8_lossy(&resp);

    let status = resp_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(500);

    let body_start = resp_str.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let body = resp_str[body_start..].to_string();

    Ok((status, body))
}

#[test]
fn test_cypher_explain_query_plan() -> Result<()> {
    let mut g = Graph::new();
    g.cypher(
        "CREATE (p:Person {name: 'Alice', age: 30})-[:KNOWS]->(b:Person {name: 'Bob', age: 28})",
    )?;

    // 1. In-memory Graph explain
    let plan = g.explain_cypher(
        "MATCH (p:Person)-[:KNOWS]->(b) WHERE p.age >= 18 RETURN p.name, count(b) LIMIT 10",
    )?;
    assert!(!plan.is_mutation);
    assert!(plan.text_representation.contains("ProduceResults"));
    assert!(plan.text_representation.contains("Limit"));
    assert!(plan.text_representation.contains("Aggregate"));
    assert!(plan.text_representation.contains("Filter"));
    assert!(plan.text_representation.contains("ExpandAll"));
    assert!(plan.text_representation.contains("NodeByLabelScan"));

    // 2. Mutation explain
    let mut_plan = g.explain_cypher("CREATE (u:User {name: 'Charlie'})")?;
    assert!(mut_plan.is_mutation);
    assert!(mut_plan.text_representation.contains("Create"));

    Ok(())
}

#[test]
fn test_rest_api_developer_cockpit_suite() -> Result<()> {
    let dir = temp_test_dir("cockpit_suite");
    let port = 19000 + ((std::process::id() + 42) % 1000);
    let http_addr = format!("127.0.0.1:{}", port);

    let server_config = ServerConfig {
        http_addr: http_addr.clone(),
        cluster_addr: None,
        node_id: "test-cockpit-node".to_string(),
        cluster_mode: "cp".to_string(),
        peer_addrs: Vec::new(),
        db_config: DatabaseConfig {
            data_dir: dir.clone(),
            fsync_policy: "always".to_string(),
            keep_snapshots: 2,
            auto_checkpoint_ops: 0,
        },
        auth_enabled: false,
        default_admin_password: None,
    };

    let server = StandaloneServer::start(server_config)?;
    thread::sleep(Duration::from_millis(250));

    // 1. Verify Studio UI endpoints (GET / and GET /ui)
    let (ui_status, ui_html) = http_request("GET", &format!("http://{}/", http_addr), "")?;
    assert_eq!(ui_status, 200);
    assert!(ui_html.contains("RustyGraph Studio"));
    assert!(ui_html.contains("Graph Algorithms"));
    assert!(ui_html.contains("Schema & Indexes"));

    // 2. Verify Prometheus /metrics endpoint
    let (m_status, metrics_text) =
        http_request("GET", &format!("http://{}/metrics", http_addr), "")?;
    assert_eq!(m_status, 200);
    assert!(metrics_text.contains("rustygraph_nodes_total"));
    assert!(metrics_text.contains("rustygraph_wal_sequence"));

    // 3. Create Sample Graph for Algorithms & Schema
    let sample_cypher = serde_json::json!({
        "query": "CREATE (a:Person {name: 'Alice', rank: 1})\
                  CREATE (b:Person {name: 'Bob', rank: 2})\
                  CREATE (c:Person {name: 'Charlie', rank: 3})\
                  CREATE (d:Person {name: 'Diana', rank: 4})\
                  CREATE (a)-[:KNOWS {weight: 1.5}]->(b)\
                  CREATE (b)-[:KNOWS {weight: 2.0}]->(c)\
                  CREATE (c)-[:KNOWS {weight: 1.0}]->(d)\
                  CREATE (a)-[:KNOWS {weight: 10.0}]->(d)\
                  RETURN a, b, c, d"
    })
    .to_string();

    let (cypher_st, _) = http_request(
        "POST",
        &format!("http://{}/cypher", http_addr),
        &sample_cypher,
    )?;
    assert_eq!(cypher_st, 200);

    // 4. Test POST /cypher/explain and EXPLAIN query
    let explain_payload = serde_json::json!({
        "query": "MATCH (a:Person)-[:KNOWS]->(b) WHERE a.rank > 1 RETURN a.name, b.name"
    })
    .to_string();

    let (exp_st, exp_resp) = http_request(
        "POST",
        &format!("http://{}/cypher/explain", http_addr),
        &explain_payload,
    )?;
    assert_eq!(exp_st, 200);
    let exp_json: Value = serde_json::from_str(&exp_resp)?;
    assert_eq!(exp_json["success"], true);
    assert!(
        exp_json["plan_tree"]
            .as_str()
            .unwrap()
            .contains("ExpandAll")
    );

    // EXPLAIN prefix via /cypher
    let explain_prefix_payload = serde_json::json!({
        "query": "EXPLAIN MATCH (n) RETURN n"
    })
    .to_string();
    let (exp_pfx_st, exp_pfx_resp) = http_request(
        "POST",
        &format!("http://{}/cypher", http_addr),
        &explain_prefix_payload,
    )?;
    assert_eq!(exp_pfx_st, 200);
    assert!(exp_pfx_resp.contains("\"explain\":true"));

    // 5. Test Schema & Index Endpoints
    // 5a. GET /schema
    let (schema_st, schema_resp) =
        http_request("GET", &format!("http://{}/schema", http_addr), "")?;
    assert_eq!(schema_st, 200);
    let schema_json: Value = serde_json::from_str(&schema_resp)?;
    assert_eq!(schema_json["nodes_total"], 4);
    assert_eq!(schema_json["edges_total"], 4);
    assert_eq!(schema_json["label_counts"]["Person"], 4);

    // 5b. POST /indexes/property
    let idx_payload = serde_json::json!({ "label": "Person", "property": "name" }).to_string();
    let (idx_st, idx_resp) = http_request(
        "POST",
        &format!("http://{}/indexes/property", http_addr),
        &idx_payload,
    )?;
    assert_eq!(idx_st, 200);
    assert!(idx_resp.contains("\"success\":true"));

    // 5c. GET /indexes
    let (list_idx_st, list_idx_resp) =
        http_request("GET", &format!("http://{}/indexes", http_addr), "")?;
    assert_eq!(list_idx_st, 200);
    assert!(list_idx_resp.contains("property_indices"));
    assert!(list_idx_resp.contains("Person"));

    // 5d. DELETE /indexes/property/Person/name
    let (del_idx_st, del_idx_resp) = http_request(
        "DELETE",
        &format!("http://{}/indexes/property/Person/name", http_addr),
        "",
    )?;
    assert_eq!(del_idx_st, 200);
    assert!(del_idx_resp.contains("\"success\":true"));

    // 6. Test Graph Algorithm REST Endpoints
    // 6a. PageRank
    let pr_payload = serde_json::json!({ "damping": 0.85, "max_iterations": 20 }).to_string();
    let (pr_st, pr_resp) = http_request(
        "POST",
        &format!("http://{}/algo/pagerank", http_addr),
        &pr_payload,
    )?;
    assert_eq!(pr_st, 200);
    let pr_json: Value = serde_json::from_str(&pr_resp)?;
    assert_eq!(pr_json["algorithm"], "pagerank");
    assert_eq!(pr_json["top_nodes"].as_array().unwrap().len(), 4);
    // Resolve node IDs dynamically from graph metadata
    let find_node = |name: &str| -> u64 {
        pr_json["top_nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| {
                item["properties"]["name"] == name || item["properties"]["entries"]["name"] == name
            })
            .expect(&format!("Node with name {} not found", name))["node_id"]
            .as_u64()
            .unwrap()
    };
    let alice_id = find_node("Alice");
    let bob_id = find_node("Bob");
    let diana_id = find_node("Diana");

    // 6b. Shortest Path (BFS vs Dijkstra)
    let sp_bfs_payload = serde_json::json!({
        "source": alice_id,
        "target": diana_id,
        "algorithm": "bfs",
        "direction": "outgoing"
    })
    .to_string();
    let (sp_st, sp_resp) = http_request(
        "POST",
        &format!("http://{}/algo/shortest-path", http_addr),
        &sp_bfs_payload,
    )?;
    assert_eq!(sp_st, 200);
    let sp_json: Value = serde_json::from_str(&sp_resp)?;
    assert_eq!(sp_json["found"], true);
    assert_eq!(sp_json["cost"], 1.0); // direct edge A->D is 1 hop in BFS

    let sp_dijkstra_payload = serde_json::json!({
        "source": alice_id,
        "target": diana_id,
        "algorithm": "dijkstra",
        "weight_property": "weight",
        "direction": "outgoing"
    })
    .to_string();
    let (dijk_st, dijk_resp) = http_request(
        "POST",
        &format!("http://{}/algo/shortest-path", http_addr),
        &sp_dijkstra_payload,
    )?;
    assert_eq!(dijk_st, 200);
    let dijk_json: Value = serde_json::from_str(&dijk_resp)?;
    assert_eq!(dijk_json["found"], true);
    assert_eq!(dijk_json["cost"], 4.5); // A(1.5)->B(2.0)->C(1.0)->D = 4.5 < direct A->D (10.0)

    // 6c. Centrality (Betweenness & Degree)
    let cent_payload =
        serde_json::json!({ "metric": "betweenness", "direction": "both", "normalized": false })
            .to_string();
    let (cent_st, cent_resp) = http_request(
        "POST",
        &format!("http://{}/algo/centrality", http_addr),
        &cent_payload,
    )?;
    assert_eq!(cent_st, 200);
    assert!(cent_resp.contains("betweenness"));

    let deg_payload = serde_json::json!({ "metric": "degree", "direction": "both" }).to_string();
    let (deg_st, deg_resp) = http_request(
        "POST",
        &format!("http://{}/algo/centrality", http_addr),
        &deg_payload,
    )?;
    assert_eq!(deg_st, 200);
    assert!(deg_resp.contains("degree"));

    // 6d. Community Detection (WCC)
    let wcc_payload = serde_json::json!({ "algorithm": "wcc" }).to_string();
    let (wcc_st, wcc_resp) = http_request(
        "POST",
        &format!("http://{}/algo/community", http_addr),
        &wcc_payload,
    )?;
    assert_eq!(wcc_st, 200);
    let wcc_json: Value = serde_json::from_str(&wcc_resp)?;
    assert_eq!(wcc_json["component_count"], 1); // all 4 nodes are connected

    // 6e. Similarity (Jaccard)
    let sim_payload =
        serde_json::json!({ "source": alice_id, "target": bob_id, "metric": "jaccard" })
            .to_string();
    let (sim_st, sim_resp) = http_request(
        "POST",
        &format!("http://{}/algo/similarity", http_addr),
        &sim_payload,
    )?;
    assert_eq!(sim_st, 200);
    assert!(sim_resp.contains("jaccard_similarity"));

    // 7. Test Enhanced /stats
    let (stats_st, stats_resp) = http_request("GET", &format!("http://{}/stats", http_addr), "")?;
    assert_eq!(stats_st, 200);
    let stats_json: Value = serde_json::from_str(&stats_resp)?;
    assert_eq!(stats_json["node_count"], 4);
    assert_eq!(stats_json["edge_count"], 4);
    assert!(stats_json["estimated_memory_bytes"].as_u64().unwrap() > 0);

    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

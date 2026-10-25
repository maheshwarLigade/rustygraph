#![allow(unused_imports)]
use rusty_graph::db::{DatabaseConfig, StandaloneDatabase};
use rusty_graph::error::Result;
use rusty_graph::graph::Graph;
use rusty_graph::id::NodeId;
use rusty_graph::model::Direction;
use rusty_graph::server::{ServerConfig, StandaloneServer};
use rusty_graph::traversal::Traverser;
use rusty_graph::value::PropertyValue;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn temp_test_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "rusty_graph_nosql_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    dir.push(unique);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_nested_nosql_document_paths_and_cypher() -> Result<()> {
    let mut graph = Graph::new();

    // Construct nested NoSQL document:
    // {
    //   "name": "Alice",
    //   "profile": {
    //      "tier": "enterprise",
    //      "contact": {
    //         "city": "San Francisco",
    //         "zipcode": "94107"
    //      }
    //   }
    // }
    let mut contact = BTreeMap::new();
    contact.insert("city".to_string(), PropertyValue::from("San Francisco"));
    contact.insert("zipcode".to_string(), PropertyValue::from("94107"));

    let mut profile = BTreeMap::new();
    profile.insert("tier".to_string(), PropertyValue::from("enterprise"));
    profile.insert("contact".to_string(), PropertyValue::Map(contact));

    let alice = graph.create_node_with(|n| {
        n.add_label("User");
        n.set_prop("name", "Alice");
        n.set_prop("profile", PropertyValue::Map(profile));
    })?;

    // 1. Direct path navigation on Node
    assert_eq!(
        graph.get_node_property(alice, "profile.tier"),
        Some(&PropertyValue::from("enterprise"))
    );
    assert_eq!(
        graph.get_node_property(alice, "profile.contact.city"),
        Some(&PropertyValue::from("San Francisco"))
    );
    assert_eq!(
        graph.get_node_property(alice, "profile.contact.zipcode"),
        Some(&PropertyValue::from("94107"))
    );

    // 2. Deep path mutation
    graph.set_node_property(alice, "profile.contact.city", "Oakland")?;
    assert_eq!(
        graph.get_node_property(alice, "profile.contact.city"),
        Some(&PropertyValue::from("Oakland"))
    );

    // 3. openCypher WHERE query using nested NoSQL dot-paths
    let res = graph.cypher(
        "MATCH (u:User) WHERE u.profile.contact.city = 'Oakland' RETURN u.name, u.profile.tier",
    )?;
    assert_eq!(res.rows.len(), 1);
    assert_eq!(res.rows[0][0], PropertyValue::from("Alice"));
    assert_eq!(res.rows[0][1], PropertyValue::from("enterprise"));

    // 4. openCypher SET clause with nested dot-path mutation
    let _ =
        graph.cypher("MATCH (u:User) WHERE u.name = 'Alice' SET u.profile.contact.state = 'CA'")?;
    assert_eq!(
        graph.get_node_property(alice, "profile.contact.state"),
        Some(&PropertyValue::from("CA"))
    );

    Ok(())
}

#[test]
fn test_supernode_degree_skew_pagination_and_capped_traversal() -> Result<()> {
    let mut graph = Graph::new();

    // Create a dense hub (supernode) with 200 outgoing relationships
    let hub = graph.create_node_with(|n| {
        n.add_label("Influencer");
        n.set_prop("handle", "@techlead");
    })?;

    let mut followers = Vec::new();
    for i in 0..200 {
        let f = graph.create_node_with(|n| {
            n.add_label("User");
            n.set_prop("idx", i as i64);
        })?;
        graph.create_edge(hub, f, "FOLLOWS")?;
        followers.push(f);
    }

    // 1. Supernode detection
    assert!(graph.is_supernode(hub, 50)?);
    assert!(graph.is_supernode(hub, 150)?);
    assert!(!graph.is_supernode(hub, 500)?);

    // 2. Incident edge pagination (offset + limit)
    let page1 = graph.incident_edges_paged(hub, Direction::Outgoing, Some("FOLLOWS"), 0, 10)?;
    assert_eq!(page1.len(), 10);
    assert_eq!(page1[0].target, followers[0]);

    let page2 = graph.incident_edges_paged(hub, Direction::Outgoing, Some("FOLLOWS"), 10, 15)?;
    assert_eq!(page2.len(), 15);
    assert_eq!(page2[0].target, followers[10]);

    // 3. Traversal engine supernode edge-capping to prevent OOM / freezes
    let traverser = Traverser::new(&graph, hub)
        .outgoing()
        .with_supernode_limit(5);

    let paths = traverser.paths();
    // Start node + 5 capped branch extensions
    assert_eq!(paths.len(), 6); // 1 base (len 0) + 5 1-hop paths

    Ok(())
}

#[test]
fn test_temporal_node_ttl_expiration_and_pruning() -> Result<()> {
    let mut graph = Graph::new();

    let ephemeral = graph.create_node_with(|n| {
        n.add_label("Session");
        n.set_prop("token", "xyz_secret");
    })?;

    let persistent = graph.create_node_with(|n| {
        n.add_label("Account");
        n.set_prop("username", "mahesh");
    })?;

    // Set TTL of 20 milliseconds on ephemeral node
    graph.set_node_ttl(ephemeral, Duration::from_millis(20))?;

    // Node is active immediately
    assert!(graph.contains_node(ephemeral));
    let pruned_early = graph.prune_expired_nodes()?;
    assert_eq!(pruned_early, 0);

    // Sleep past expiration
    thread::sleep(Duration::from_millis(40));

    // Prune expired nodes
    let pruned = graph.prune_expired_nodes()?;
    assert_eq!(pruned, 1);

    // Verify ephemeral node is pruned, but persistent node remains intact
    assert!(!graph.contains_node(ephemeral));
    assert!(graph.contains_node(persistent));

    Ok(())
}

#[test]
fn test_high_throughput_bulk_ingestion() -> Result<()> {
    let dir = temp_test_dir("bulk_ingestion");
    let config = DatabaseConfig {
        data_dir: dir.clone(),
        ..Default::default()
    };
    let db = StandaloneDatabase::open(config)?;

    // 1. Bulk import 50 nodes
    let mut batch_nodes = Vec::new();
    for i in 0..50 {
        let mut props = HashMap::new();
        props.insert("seq".to_string(), PropertyValue::from(i as i64));
        props.insert(
            "payload".to_string(),
            PropertyValue::from(format!("node_{}", i)),
        );
        batch_nodes.push((vec!["BatchItem".to_string()], props));
    }

    let node_ids = db.bulk_import_nodes(batch_nodes)?;
    assert_eq!(node_ids.len(), 50);

    // 2. Bulk import 49 edges connecting sequentially
    let mut batch_edges = Vec::new();
    for i in 0..49 {
        let mut props = HashMap::new();
        props.insert("weight".to_string(), PropertyValue::from(1.0f64));
        batch_edges.push((node_ids[i], node_ids[i + 1], "NEXT".to_string(), props));
    }

    let edge_ids = db.bulk_import_edges(batch_edges)?;
    assert_eq!(edge_ids.len(), 49);

    // 3. Verify graph statistics
    let stats = db.stats();
    assert_eq!(stats.node_count, 50);
    assert_eq!(stats.edge_count, 49);

    // 4. Checkpoint and reopen to verify WAL recovery of bulk items
    db.checkpoint()?;
    drop(db);

    let config2 = DatabaseConfig {
        data_dir: dir.clone(),
        ..Default::default()
    };
    let db2 = StandaloneDatabase::open(config2)?;
    let stats2 = db2.stats();
    assert_eq!(stats2.node_count, 50);
    assert_eq!(stats2.edge_count, 49);

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn test_http_bulk_and_ttl_endpoints() -> Result<()> {
    let dir = temp_test_dir("http_bulk_ttl");
    let port = 19185;

    let config = ServerConfig {
        http_addr: format!("127.0.0.1:{}", port),
        cluster_addr: None,
        db_config: DatabaseConfig {
            data_dir: dir.clone(),
            ..Default::default()
        },
        ..Default::default()
    };

    let server = StandaloneServer::start(config)?;
    thread::sleep(Duration::from_millis(150));

    // Helper closure for HTTP requests
    let send_req = |method: &str, path: &str, body: &str| -> (u16, String) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let req = format!(
            "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
            method,
            path,
            port,
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).unwrap();

        let mut resp_str = String::new();
        stream.read_to_string(&mut resp_str).unwrap();

        let status = resp_str
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u16>()
            .unwrap();

        let body_idx = resp_str.find("\r\n\r\n").unwrap() + 4;
        let body = resp_str[body_idx..].to_string();
        (status, body)
    };

    // 1. Bulk import nodes via HTTP
    let bulk_nodes_payload = r#"{
        "nodes": [
            {"labels": ["Device"], "properties": {"device_id": "iot-001"}},
            {"labels": ["Device"], "properties": {"device_id": "iot-002"}}
        ]
    }"#;
    let (status, body) = send_req("POST", "/bulk/nodes", bulk_nodes_payload);
    assert_eq!(status, 200);
    assert!(body.contains("\"success\":true"));
    assert!(body.contains("\"count\":2"));

    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let node_id = parsed["node_ids"][0].as_u64().unwrap();

    // 2. Set TTL on created node
    let ttl_payload = r#"{"ttl_seconds": 1}"#;
    let (status, body) = send_req("POST", &format!("/nodes/{}/ttl", node_id), ttl_payload);
    assert_eq!(status, 200);
    assert!(body.contains("\"success\":true"));

    // 3. Prune TTL endpoint
    let (status, body) = send_req("POST", "/ttl/prune", "");
    assert_eq!(status, 200);
    assert!(body.contains("\"success\":true"));

    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

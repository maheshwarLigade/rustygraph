use rusty_graph::cluster::ClusterMode;
use rusty_graph::db::{DatabaseConfig, StandaloneDatabase};
use rusty_graph::distributed::{ClusterPeer, DistributedClusterNode};
use rusty_graph::error::Result;
use rusty_graph::server::{ServerConfig, StandaloneServer};
use rusty_graph::value::PropertyValue;
use rusty_graph::vector::VectorSimilarity;
use rusty_graph::wal::{crc32, FsyncPolicy, WalManager, WalOp};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn temp_test_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "rusty_graph_test_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    dir.push(unique);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_wal_crc32_and_recovery() -> Result<()> {
    let dir = temp_test_dir("wal_crc");
    let wal = WalManager::open(&dir, FsyncPolicy::Always)?;

    let mut props = HashMap::new();
    props.insert("name".to_string(), PropertyValue::from("Alice"));
    props.insert("age".to_string(), PropertyValue::from(30));

    let op1 = WalOp::CreateNode {
        id: 1,
        labels: vec!["Person".to_string()],
        properties: props.clone(),
    };
    let seq1 = wal.append(op1.clone())?;
    assert_eq!(seq1, 1);

    let op2 = WalOp::DeleteNode { id: 1 };
    let seq2 = wal.append(op2.clone())?;
    assert_eq!(seq2, 2);

    wal.flush()?;

    // Read back and verify CRC32
    let records = wal.read_all_records()?;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].op, op1);
    assert_eq!(records[1].op, op2);

    // Verify CRC32 matches
    let bytes1 = serde_json::to_vec(&op1)?;
    assert_eq!(records[0].crc32, crc32(&bytes1));

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn test_standalone_database_crash_recovery() -> Result<()> {
    let dir = temp_test_dir("db_recovery");

    let config = DatabaseConfig {
        data_dir: dir.clone(),
        fsync_policy: "always".to_string(),
        keep_snapshots: 2,
        auto_checkpoint_ops: 0,
    };

    // Session 1: Create nodes, edges, vector index, and take checkpoint
    {
        let db = StandaloneDatabase::open(config.clone())?;

        let mut p1 = HashMap::new();
        p1.insert("name".to_string(), PropertyValue::from("Alice"));
        p1.insert("vec".to_string(), PropertyValue::from(vec![1.0f32, 0.0, 0.0]));
        let a_id = db.create_node(vec!["User".to_string()], p1)?;

        let mut p2 = HashMap::new();
        p2.insert("name".to_string(), PropertyValue::from("Bob"));
        let b_id = db.create_node(vec!["User".to_string()], p2)?;

        let _ = db.create_edge(a_id, b_id, "FRIENDS_WITH", HashMap::new())?;

        db.create_vector_index("user_vec", "User", "vec", 3, VectorSimilarity::Cosine)?;

        // Force a checkpoint snapshot
        let seq = db.checkpoint()?;
        assert!(seq >= 3);

        // Session 1 continued: Mutate post-checkpoint (these live in WAL)
        let mut p3 = HashMap::new();
        p3.insert("name".to_string(), PropertyValue::from("Charlie"));
        db.create_node(vec!["User".to_string()], p3)?;

        assert_eq!(db.stats().node_count, 3);
        assert_eq!(db.stats().edge_count, 1);
    } // Database closed

    // Session 2: Reopen from disk - must recover snapshot + post-checkpoint WAL replay
    {
        let db2 = StandaloneDatabase::open(config)?;
        let stats = db2.stats();

        assert_eq!(stats.node_count, 3, "All 3 nodes must be present after recovery");
        assert_eq!(stats.edge_count, 1, "Edge must be present after recovery");

        // Verify Cypher query works on recovered state
        let res = db2.cypher("MATCH (u:User) RETURN u.name")?;
        assert_eq!(res.rows.len(), 3);

        // Verify Vector search works on recovered state
        let hits = db2.query_vector_index("user_vec", 1, &[0.99, 0.01, 0.0])?;
        assert_eq!(hits.len(), 1);
        assert!(hits[0].score > 0.95);
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn test_http_rest_api_server_endpoints() -> Result<()> {
    let dir = temp_test_dir("http_server");
    let port = 18080 + (std::process::id() % 1000);
    let http_addr = format!("127.0.0.1:{}", port);

    let server_config = ServerConfig {
        http_addr: http_addr.clone(),
        cluster_addr: None,
        node_id: "test-node".to_string(),
        cluster_mode: "cp".to_string(),
        peer_addrs: Vec::new(),
        db_config: DatabaseConfig {
            data_dir: dir.clone(),
            fsync_policy: "always".to_string(),
            keep_snapshots: 2,
            auto_checkpoint_ops: 0,
        },
    };

    let server = StandaloneServer::start(server_config)?;
    thread::sleep(Duration::from_millis(200));

    // 1. Test /health
    let (status, body) = http_request("GET", &format!("http://{}/health", http_addr), "")?;
    assert_eq!(status, 200);
    assert!(body.contains("healthy"));

    // 2. Test /nodes (Create Node)
    let node_payload = serde_json::json!({
        "labels": ["Person", "Engineer"],
        "properties": {
            "name": "David",
            "skill": "Rust"
        }
    })
    .to_string();

    let (status, node_resp) = http_request("POST", &format!("http://{}/nodes", http_addr), &node_payload)?;
    assert_eq!(status, 200);
    assert!(node_resp.contains("node_id"));

    // 3. Test /cypher query
    let cypher_payload = serde_json::json!({
        "query": "MATCH (p:Person) RETURN p.name, p.skill"
    })
    .to_string();

    let (status, cypher_resp) = http_request("POST", &format!("http://{}/cypher", http_addr), &cypher_payload)?;
    assert_eq!(status, 200);
    assert!(cypher_resp.contains("David"));
    assert!(cypher_resp.contains("Rust"));

    // 4. Test /stats
    let (status, stats_resp) = http_request("GET", &format!("http://{}/stats", http_addr), "")?;
    assert_eq!(status, 200);
    assert!(stats_resp.contains("\"node_count\":1"));

    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn test_distributed_tcp_cluster_consensus() -> Result<()> {
    let dir1 = temp_test_dir("dist_node1");
    let dir2 = temp_test_dir("dist_node2");

    let p1 = 29000 + (std::process::id() % 500);
    let p2 = p1 + 1;

    let addr1 = format!("127.0.0.1:{}", p1);
    let addr2 = format!("127.0.0.1:{}", p2);

    let db1 = Arc::new(StandaloneDatabase::open(DatabaseConfig {
        data_dir: dir1.clone(),
        fsync_policy: "never".to_string(),
        keep_snapshots: 1,
        auto_checkpoint_ops: 0,
    })?);

    let db2 = Arc::new(StandaloneDatabase::open(DatabaseConfig {
        data_dir: dir2.clone(),
        fsync_policy: "never".to_string(),
        keep_snapshots: 1,
        auto_checkpoint_ops: 0,
    })?);

    // Node 1 starts as Leader
    let node1 = DistributedClusterNode::start(
        "node-1".to_string(),
        addr1.clone(),
        ClusterMode::CP,
        Arc::clone(&db1),
        Vec::new(),
    )?;

    thread::sleep(Duration::from_millis(200));

    // Node 2 joins Node 1
    let peer1 = ClusterPeer {
        id: "node-1".to_string(),
        address: addr1.clone(),
    };

    let node2 = DistributedClusterNode::start(
        "node-2".to_string(),
        addr2.clone(),
        ClusterMode::CP,
        Arc::clone(&db2),
        vec![peer1],
    )?;

    thread::sleep(Duration::from_millis(800));

    let status1 = node1.status();
    let status2 = node2.status();

    assert!(status1.is_leader, "Node 1 should be leader");
    assert!(!status2.is_leader, "Node 2 should be follower");
    assert_eq!(status2.known_leader, Some("node-1".to_string()));

    node1.stop();
    node2.stop();

    let _ = std::fs::remove_dir_all(&dir1);
    let _ = std::fs::remove_dir_all(&dir2);
    Ok(())
}

// Minimal HTTP helper for test suite
fn http_request(method: &str, url: &str, body: &str) -> std::io::Result<(u16, String)> {
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

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;

    let resp_str = String::from_utf8_lossy(&raw);
    let status = resp_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    let body_part = if let Some(idx) = resp_str.find("\r\n\r\n") {
        resp_str[idx + 4..].to_string()
    } else {
        String::new()
    };

    Ok((status, body_part))
}

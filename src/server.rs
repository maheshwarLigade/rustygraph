use crate::cluster::ClusterMode;
use crate::db::{DatabaseConfig, StandaloneDatabase};
use crate::distributed::{ClusterPeer, DistributedClusterNode};
use crate::error::{Error, Result};
use crate::id::{NodeId, RelId};
use crate::value::PropertyValue;
use crate::vector::VectorSimilarity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Response, Server, StatusCode};

/// Configuration for the standalone HTTP REST server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub http_addr: String,
    pub cluster_addr: Option<String>,
    pub node_id: String,
    pub cluster_mode: String, // "cp" or "ap"
    pub peer_addrs: Vec<String>,
    pub db_config: DatabaseConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_addr: "0.0.0.0:8000".to_string(),
            cluster_addr: Some("127.0.0.1:9001".to_string()),
            node_id: "node-1".to_string(),
            cluster_mode: "cp".to_string(),
            peer_addrs: Vec::new(),
            db_config: DatabaseConfig::default(),
        }
    }
}

// Request payload types
#[derive(Debug, Deserialize)]
struct CypherRequest {
    query: String,
}

#[derive(Debug, Deserialize)]
struct TxBeginRequest {
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CreateNodeRequest {
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Deserialize)]
struct CreateEdgeRequest {
    source: u64,
    target: u64,
    rel_type: String,
    #[serde(default)]
    properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Deserialize)]
struct CreateVectorIndexRequest {
    name: String,
    label: String,
    prop_key: String,
    dimension: usize,
    similarity: Option<String>, // "cosine", "euclidean", "dot"
}

#[derive(Debug, Deserialize)]
struct VectorSearchRequest {
    index: String,
    k: usize,
    vector: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct JoinClusterRequest {
    peer_address: String,
}

/// Standalone HTTP REST server exposing full graph, Cypher, NoSQL, and cluster APIs.
pub struct StandaloneServer {
    config: ServerConfig,
    db: Arc<StandaloneDatabase>,
    cluster: Option<Arc<DistributedClusterNode>>,
    running: Arc<AtomicBool>,
}

impl StandaloneServer {
    /// Initializes and starts the standalone HTTP and distributed cluster server.
    pub fn start(config: ServerConfig) -> Result<Arc<Self>> {
        let db = Arc::new(StandaloneDatabase::open(config.db_config.clone())?);

        let cluster = if let Some(ref c_addr) = config.cluster_addr {
            let mode = if config.cluster_mode.to_lowercase() == "ap" {
                ClusterMode::AP
            } else {
                ClusterMode::CP
            };

            let initial_peers = config
                .peer_addrs
                .iter()
                .enumerate()
                .map(|(i, addr)| ClusterPeer {
                    id: format!("peer-{}", i + 1),
                    address: addr.clone(),
                })
                .collect();

            let node = DistributedClusterNode::start(
                config.node_id.clone(),
                c_addr.clone(),
                mode,
                Arc::clone(&db),
                initial_peers,
            )?;
            Some(node)
        } else {
            None
        };

        let server_instance = Arc::new(Self {
            config: config.clone(),
            db: Arc::clone(&db),
            cluster,
            running: Arc::new(AtomicBool::new(true)),
        });

        // Start HTTP Listener
        let http_server = Server::http(&config.http_addr)
            .map_err(|e| Error::Storage(format!("Failed to bind HTTP server to {}: {}", config.http_addr, e)))?;

        let server_worker = Arc::clone(&server_instance);
        thread::spawn(move || {
            server_worker.run_http_loop(http_server);
        });

        Ok(server_instance)
    }

    /// Stops the server.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(ref c) = self.cluster {
            c.stop();
        }
    }

    /// Returns the active server configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Returns the database handle.
    pub fn db(&self) -> Arc<StandaloneDatabase> {
        Arc::clone(&self.db)
    }

    fn run_http_loop(&self, server: Server) {
        for mut request in server.incoming_requests() {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }

            let url = request.url().to_string();
            let method = request.method().clone();

            let mut body_bytes = Vec::new();
            let _ = request.as_reader().read_to_end(&mut body_bytes);

            let resp = self.dispatch_route(&method, &url, &body_bytes);

            // Add standard CORS and JSON headers
            let header_cors = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
            let header_methods = Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, DELETE, OPTIONS"[..]).unwrap();
            let header_headers = Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type"[..]).unwrap();
            let header_content_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();

            let response = Response::from_string(resp.body)
                .with_status_code(resp.status)
                .with_header(header_cors)
                .with_header(header_methods)
                .with_header(header_headers)
                .with_header(header_content_type);

            let _ = request.respond(response);
        }
    }

    fn dispatch_route(&self, method: &Method, url: &str, body: &[u8]) -> HttpResponse {
        // Handle OPTIONS preflight
        if *method == Method::Options {
            return HttpResponse::ok("{}");
        }

        let path = url.split('?').next().unwrap_or(url);

        match (method, path) {
            // Health & Stats
            (&Method::Get, "/health") => HttpResponse::ok(r#"{"status":"healthy","engine":"rusty_graph"}"#),
            (&Method::Get, "/stats") => {
                let stats = self.db.stats();
                HttpResponse::json(&stats)
            }

            // Cluster Status & Dynamic Join
            (&Method::Get, "/cluster/status") => {
                if let Some(ref c) = self.cluster {
                    HttpResponse::json(&c.status())
                } else {
                    HttpResponse::ok(r#"{"cluster_enabled":false}"#)
                }
            }
            (&Method::Post, "/cluster/join") => {
                if let Some(ref c) = self.cluster {
                    if let Ok(req) = serde_json::from_slice::<JoinClusterRequest>(body) {
                        match c.join_cluster(&req.peer_address) {
                            Ok(_) => HttpResponse::ok(r#"{"success":true,"message":"Cluster join requested"}"#),
                            Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                        }
                    } else {
                        HttpResponse::error(StatusCode(400), "Invalid JSON payload for /cluster/join")
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Clustering is disabled on this node")
                }
            }

            // openCypher Query Execution
            (&Method::Post, "/cypher") => {
                if let Ok(req) = serde_json::from_slice::<CypherRequest>(body) {
                    match self.db.cypher(&req.query) {
                        Ok(res) => HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "columns": res.columns,
                            "rows": res.rows,
                            "error": null,
                        })),
                        Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid JSON payload for /cypher (expected {\"query\": \"...\"})")
                }
            }

            // ACID Transactions
            (&Method::Post, "/tx/begin") => {
                let timeout = if !body.is_empty() {
                    serde_json::from_slice::<TxBeginRequest>(body)
                        .ok()
                        .and_then(|r| r.timeout_ms)
                        .map(std::time::Duration::from_millis)
                } else {
                    None
                };
                match self.db.begin_transaction(timeout) {
                    Ok(tx_id) => HttpResponse::json(&serde_json::json!({
                        "success": true,
                        "tx_id": tx_id,
                        "timeout_ms": timeout.unwrap_or(std::time::Duration::from_secs(30)).as_millis() as u64,
                        "message": format!("Transaction {} started with snapshot isolation", tx_id),
                    })),
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            }
            (&Method::Post, p) if p.starts_with("/tx/") && p.ends_with("/cypher") => {
                let parts: Vec<&str> = p.split('/').collect();
                if parts.len() == 4 {
                    if let Ok(tx_id) = parts[2].parse::<u64>() {
                        if let Ok(req) = serde_json::from_slice::<CypherRequest>(body) {
                            match self.db.execute_tx(tx_id, &req.query) {
                                Ok(res) => HttpResponse::json(&serde_json::json!({
                                    "success": true,
                                    "tx_id": tx_id,
                                    "columns": res.columns,
                                    "rows": res.rows,
                                    "error": null,
                                })),
                                Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                            }
                        } else {
                            HttpResponse::error(StatusCode(400), "Invalid JSON payload for /tx/:id/cypher")
                        }
                    } else {
                        HttpResponse::error(StatusCode(400), "Invalid Transaction ID in URL")
                    }
                } else {
                    HttpResponse::error(StatusCode(404), "Route not found")
                }
            }
            (&Method::Post, p) if p.starts_with("/tx/") && p.ends_with("/commit") => {
                let parts: Vec<&str> = p.split('/').collect();
                if parts.len() == 4 {
                    if let Ok(tx_id) = parts[2].parse::<u64>() {
                        match self.db.commit_transaction(tx_id) {
                            Ok(_) => HttpResponse::json(&serde_json::json!({
                                "success": true,
                                "tx_id": tx_id,
                                "message": format!("Transaction {} committed successfully", tx_id),
                            })),
                            Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                        }
                    } else {
                        HttpResponse::error(StatusCode(400), "Invalid Transaction ID in URL")
                    }
                } else {
                    HttpResponse::error(StatusCode(404), "Route not found")
                }
            }
            (&Method::Post, p) if p.starts_with("/tx/") && p.ends_with("/rollback") => {
                let parts: Vec<&str> = p.split('/').collect();
                if parts.len() == 4 {
                    if let Ok(tx_id) = parts[2].parse::<u64>() {
                        match self.db.rollback_transaction(tx_id) {
                            Ok(_) => HttpResponse::json(&serde_json::json!({
                                "success": true,
                                "tx_id": tx_id,
                                "message": format!("Transaction {} rolled back successfully", tx_id),
                            })),
                            Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                        }
                    } else {
                        HttpResponse::error(StatusCode(400), "Invalid Transaction ID in URL")
                    }
                } else {
                    HttpResponse::error(StatusCode(404), "Route not found")
                }
            }
            (&Method::Get, "/tx/list") => {
                let active = self.db.tx_manager().list_active();
                let json_list: Vec<serde_json::Value> = active
                    .into_iter()
                    .map(|(id, rem, ops)| {
                        serde_json::json!({
                            "tx_id": id,
                            "remaining_ms": rem,
                            "staged_mutations": ops,
                        })
                    })
                    .collect();
                HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "active_transactions": json_list,
                }))
            }

            // Nodes CRUD
            (&Method::Post, "/nodes") => {
                if let Ok(req) = serde_json::from_slice::<CreateNodeRequest>(body) {
                    match self.db.create_node(req.labels, req.properties) {
                        Ok(id) => HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "node_id": id.raw(),
                        })),
                        Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid JSON for /nodes")
                }
            }
            (&Method::Get, p) if p.starts_with("/nodes/") => {
                let id_str = &p[7..];
                if let Ok(id) = id_str.parse::<u64>() {
                    if let Some(node) = self.db.get_node(NodeId(id)) {
                        let out_edges: Vec<u64> = self.db.graph().read(|g| {
                            g.incident_edge_ids(NodeId(id), crate::model::Direction::Outgoing)
                                .unwrap_or_default()
                                .iter()
                                .map(|e| e.raw())
                                .collect()
                        });
                        let in_edges: Vec<u64> = self.db.graph().read(|g| {
                            g.incident_edge_ids(NodeId(id), crate::model::Direction::Incoming)
                                .unwrap_or_default()
                                .iter()
                                .map(|e| e.raw())
                                .collect()
                        });
                        HttpResponse::json(&serde_json::json!({
                            "id": node.id.raw(),
                            "labels": node.labels.iter().collect::<Vec<_>>(),
                            "properties": node.properties,
                            "outgoing_edges": out_edges,
                            "incoming_edges": in_edges,
                        }))
                    } else {
                        HttpResponse::error(StatusCode(404), &format!("Node {} not found", id))
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid Node ID")
                }
            }
            (&Method::Delete, p) if p.starts_with("/nodes/") => {
                let id_str = &p[7..];
                if let Ok(id) = id_str.parse::<u64>() {
                    match self.db.delete_node(NodeId(id)) {
                        Ok(true) => HttpResponse::ok(r#"{"success":true,"deleted":true}"#),
                        Ok(false) => HttpResponse::error(StatusCode(404), "Node not found"),
                        Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid Node ID")
                }
            }

            // Edges CRUD
            (&Method::Post, "/edges") => {
                if let Ok(req) = serde_json::from_slice::<CreateEdgeRequest>(body) {
                    match self.db.create_edge(
                        NodeId(req.source),
                        NodeId(req.target),
                        &req.rel_type,
                        req.properties,
                    ) {
                        Ok(id) => HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "edge_id": id.raw(),
                        })),
                        Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid JSON for /edges")
                }
            }
            (&Method::Get, p) if p.starts_with("/edges/") => {
                let id_str = &p[7..];
                if let Ok(id) = id_str.parse::<u64>() {
                    if let Some(edge) = self.db.get_edge(RelId(id)) {
                        HttpResponse::json(&serde_json::json!({
                            "id": edge.id.raw(),
                            "source": edge.source.raw(),
                            "target": edge.target.raw(),
                            "type": edge.rel_type,
                            "properties": edge.properties,
                        }))
                    } else {
                        HttpResponse::error(StatusCode(404), &format!("Edge {} not found", id))
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid Edge ID")
                }
            }
            (&Method::Delete, p) if p.starts_with("/edges/") => {
                let id_str = &p[7..];
                if let Ok(id) = id_str.parse::<u64>() {
                    match self.db.delete_edge(RelId(id)) {
                        Ok(true) => HttpResponse::ok(r#"{"success":true,"deleted":true}"#),
                        Ok(false) => HttpResponse::error(StatusCode(404), "Edge not found"),
                        Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid Edge ID")
                }
            }

            // Vector Database Indexing & Search
            (&Method::Post, "/vector/indexes") => {
                if let Ok(req) = serde_json::from_slice::<CreateVectorIndexRequest>(body) {
                    let metric = match req.similarity.as_deref() {
                        Some("euclidean") => VectorSimilarity::Euclidean,
                        Some("dot") => VectorSimilarity::DotProduct,
                        _ => VectorSimilarity::Cosine,
                    };
                    match self.db.create_vector_index(
                        &req.name,
                        &req.label,
                        &req.prop_key,
                        req.dimension,
                        metric,
                    ) {
                        Ok(_) => HttpResponse::ok(r#"{"success":true,"message":"Vector index created"}"#),
                        Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid JSON for /vector/indexes")
                }
            }
            (&Method::Post, "/vector/search") => {
                if let Ok(req) = serde_json::from_slice::<VectorSearchRequest>(body) {
                    match self.db.query_vector_index(&req.index, req.k, &req.vector) {
                        Ok(hits) => HttpResponse::json(&hits),
                        Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid JSON for /vector/search")
                }
            }

            // Persistence: Checkpoint, Export, Import
            (&Method::Post, "/checkpoint") => {
                match self.db.checkpoint() {
                    Ok(seq) => HttpResponse::json(&serde_json::json!({
                        "success": true,
                        "checkpointed_sequence": seq,
                    })),
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            }
            (&Method::Get, "/export") => {
                match self.db.graph().read(|g| g.to_json()) {
                    Ok(json) => HttpResponse::ok(&json),
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            }
            (&Method::Post, "/import") => {
                let json_str = String::from_utf8_lossy(body);
                match crate::graph::Graph::from_json(&json_str) {
                    Ok(restored) => {
                        self.db.graph().write(|g| {
                            *g = restored;
                        });
                        let _ = self.db.checkpoint();
                        HttpResponse::ok(r#"{"success":true,"message":"Database imported successfully"}"#)
                    }
                    Err(e) => HttpResponse::error(StatusCode(400), &format!("Failed to parse import JSON: {}", e)),
                }
            }

            _ => HttpResponse::error(StatusCode(404), &format!("Route not found: {} {}", method, path)),
        }
    }
}

struct HttpResponse {
    status: StatusCode,
    body: String,
}

impl HttpResponse {
    fn ok(body: &str) -> Self {
        Self {
            status: StatusCode(200),
            body: body.to_string(),
        }
    }

    fn json<T: Serialize>(val: &T) -> Self {
        match serde_json::to_string(val) {
            Ok(body) => Self {
                status: StatusCode(200),
                body,
            },
            Err(e) => Self::error(StatusCode(500), &e.to_string()),
        }
    }

    fn error(status: StatusCode, msg: &str) -> Self {
        let err_json = serde_json::json!({
            "success": false,
            "error": msg
        });
        Self {
            status,
            body: err_json.to_string(),
        }
    }
}

use super::dto::*;
use super::response::HttpResponse;
use crate::distributed::DistributedClusterNode;
use crate::id::{NodeId, RelId};
use crate::storage::db::StandaloneDatabase;
use crate::value::PropertyValue;
use crate::vector::VectorSimilarity;
use std::collections::HashMap;
use std::sync::Arc;
use tiny_http::{Method, StatusCode};

pub fn dispatch_route(
    db: &StandaloneDatabase,
    cluster: Option<&Arc<DistributedClusterNode>>,
    method: &Method,
    url: &str,
    body: &[u8],
) -> HttpResponse {
    // Handle OPTIONS preflight
    if *method == Method::Options {
        return HttpResponse::ok("{}");
    }

    let path = url.split('?').next().unwrap_or(url);

    match (method, path) {
        // Health & Stats
        (&Method::Get, "/health") => HttpResponse::ok(r#"{"status":"healthy","engine":"rusty_graph"}"#),
        (&Method::Get, "/stats") => {
            let stats = db.stats();
            HttpResponse::json(&stats)
        }

        // Cluster Status & Dynamic Join
        (&Method::Get, "/cluster/status") => {
            if let Some(c) = cluster {
                HttpResponse::json(&c.status())
            } else {
                HttpResponse::ok(r#"{"cluster_enabled":false}"#)
            }
        }
        (&Method::Post, "/cluster/join") => {
            if let Some(c) = cluster {
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
                match db.cypher(&req.query) {
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
            match db.begin_transaction(timeout) {
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
                        match db.execute_tx(tx_id, &req.query) {
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
                    match db.commit_transaction(tx_id) {
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
                    match db.rollback_transaction(tx_id) {
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
            let active = db.tx_manager().list_active();
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
                match db.create_node(req.labels, req.properties) {
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
                if let Some(node) = db.get_node(NodeId(id)) {
                    let out_edges: Vec<u64> = db.graph().read(|g| {
                        g.incident_edge_ids(NodeId(id), crate::model::Direction::Outgoing)
                            .unwrap_or_default()
                            .iter()
                            .map(|e| e.raw())
                            .collect()
                    });
                    let in_edges: Vec<u64> = db.graph().read(|g| {
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
        // Bulk Ingestion & NoSQL Batch APIs
        (&Method::Post, "/bulk/nodes") => {
            if let Ok(req) = serde_json::from_slice::<BulkNodesRequest>(body) {
                let items: Vec<(Vec<String>, HashMap<String, PropertyValue>)> = req
                    .nodes
                    .into_iter()
                    .map(|n| (n.labels, n.properties))
                    .collect();
                match db.bulk_import_nodes(items) {
                    Ok(ids) => {
                        let raw_ids: Vec<u64> = ids.into_iter().map(|id| id.raw()).collect();
                        HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "count": raw_ids.len(),
                            "node_ids": raw_ids,
                        }))
                    }
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Invalid JSON for /bulk/nodes (expected {\"nodes\": [...]})")
            }
        }
        (&Method::Post, "/bulk/edges") => {
            if let Ok(req) = serde_json::from_slice::<BulkEdgesRequest>(body) {
                let items: Vec<(NodeId, NodeId, String, HashMap<String, PropertyValue>)> = req
                    .edges
                    .into_iter()
                    .map(|e| (NodeId(e.source), NodeId(e.target), e.rel_type, e.properties))
                    .collect();
                match db.bulk_import_edges(items) {
                    Ok(ids) => {
                        let raw_ids: Vec<u64> = ids.into_iter().map(|id| id.raw()).collect();
                        HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "count": raw_ids.len(),
                            "edge_ids": raw_ids,
                        }))
                    }
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Invalid JSON for /bulk/edges (expected {\"edges\": [...]})")
            }
        }

        // TTL (Time-To-Live) Node Expiration
        (&Method::Post, "/ttl/prune") => {
            match db.prune_expired_nodes() {
                Ok(pruned) => HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "pruned_nodes": pruned,
                })),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Post, p) if p.starts_with("/nodes/") && p.ends_with("/ttl") => {
            let parts: Vec<&str> = p.split('/').collect();
            if parts.len() == 4 {
                if let Ok(id) = parts[2].parse::<u64>() {
                    if let Ok(req) = serde_json::from_slice::<SetTtlRequest>(body) {
                        match db.set_node_ttl(NodeId(id), std::time::Duration::from_secs(req.ttl_seconds)) {
                            Ok(_) => HttpResponse::json(&serde_json::json!({
                                "success": true,
                                "node_id": id,
                                "ttl_seconds": req.ttl_seconds,
                                "message": format!("TTL of {}s set on node {}", req.ttl_seconds, id),
                            })),
                            Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                        }
                    } else {
                        HttpResponse::error(StatusCode(400), "Invalid JSON payload for /nodes/:id/ttl (expected {\"ttl_seconds\": 60})")
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid Node ID")
                }
            } else {
                HttpResponse::error(StatusCode(404), "Route not found")
            }
        }
        (&Method::Delete, p) if p.starts_with("/nodes/") => {
            let id_str = &p[7..];
            if let Ok(id) = id_str.parse::<u64>() {
                match db.delete_node(NodeId(id)) {
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
                match db.create_edge(
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
                if let Some(edge) = db.get_edge(RelId(id)) {
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
                match db.delete_edge(RelId(id)) {
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
                match db.create_vector_index(
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
                match db.query_vector_index(&req.index, req.k, &req.vector) {
                    Ok(hits) => HttpResponse::json(&hits),
                    Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Invalid JSON for /vector/search")
            }
        }

        // Persistence: Checkpoint, Export, Import
        (&Method::Post, "/checkpoint") => {
            match db.checkpoint() {
                Ok(seq) => HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "checkpointed_sequence": seq,
                })),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Get, "/export") => {
            match db.graph().read(|g| g.to_json()) {
                Ok(json) => HttpResponse::ok(&json),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Post, "/import") => {
            let json_str = String::from_utf8_lossy(body);
            match crate::graph::Graph::from_json(&json_str) {
                Ok(restored) => {
                    db.graph().write(|g| {
                        *g = restored;
                    });
                    let _ = db.checkpoint();
                    HttpResponse::ok(r#"{"success":true,"message":"Database imported successfully"}"#)
                }
                Err(e) => HttpResponse::error(StatusCode(400), &format!("Failed to parse import JSON: {}", e)),
            }
        }

        _ => HttpResponse::error(StatusCode(404), &format!("Route not found: {} {}", method, path)),
    }
}

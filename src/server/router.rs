use super::auth::{AuthManager, Privilege, SecurityContext};
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
    auth: Option<&Arc<AuthManager>>,
    auth_header: Option<&str>,
    method: &Method,
    url: &str,
    body: &[u8],
) -> HttpResponse {
    // 1. Handle OPTIONS preflight (always public)
    if *method == Method::Options {
        return HttpResponse::ok("{}");
    }

    let path = url.split('?').next().unwrap_or(url);

    // 2. Public endpoints (no authentication required)
    match (method, path) {
        (&Method::Get, "/") | (&Method::Get, "/ui") => {
            return HttpResponse::html(super::studio::STUDIO_HTML);
        }
        (&Method::Get, "/health") => {
            return HttpResponse::ok(r#"{"status":"healthy","engine":"rusty_graph"}"#);
        }
        (&Method::Get, "/metrics") => {
            let stats = db.stats();
            let body = format!(
                "# HELP rustygraph_nodes_total Total nodes in database\n# TYPE rustygraph_nodes_total gauge\nrustygraph_nodes_total {}\n# HELP rustygraph_edges_total Total edges in database\n# TYPE rustygraph_edges_total gauge\nrustygraph_edges_total {}\n# HELP rustygraph_wal_sequence Current WAL sequence number\n# TYPE rustygraph_wal_sequence counter\nrustygraph_wal_sequence {}\n# HELP rustygraph_uptime_seconds Database uptime\n# TYPE rustygraph_uptime_seconds counter\nrustygraph_uptime_seconds {}\n# HELP rustygraph_memory_bytes Estimated memory usage\n# TYPE rustygraph_memory_bytes gauge\nrustygraph_memory_bytes {}\n# HELP rustygraph_active_transactions Number of active transactions\n# TYPE rustygraph_active_transactions gauge\nrustygraph_active_transactions {}\n",
                stats.node_count,
                stats.edge_count,
                stats.current_wal_sequence,
                stats.uptime_seconds,
                stats.estimated_memory_bytes,
                stats.active_transactions
            );
            return HttpResponse::ok(&body);
        }
        (&Method::Post, "/auth/login") => {
            if let Some(auth_mgr) = auth {
                if let Ok(req) = serde_json::from_slice::<LoginRequest>(body) {
                    match auth_mgr.login(&req.username, &req.password) {
                        Ok(session) => {
                            return HttpResponse::json(&serde_json::json!({
                                "success": true,
                                "token": session.token,
                                "username": session.username,
                                "expires_at": session.expires_at,
                                "message": "Authentication successful",
                            }));
                        }
                        Err(e) => {
                            return HttpResponse::error(StatusCode(401), &e.to_string());
                        }
                    }
                } else {
                    return HttpResponse::error(
                        StatusCode(400),
                        "Invalid JSON payload for /auth/login (expected {\"username\": \"...\", \"password\": \"...\"})",
                    );
                }
            } else {
                return HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "token": "auth_disabled",
                    "username": "admin",
                    "expires_at": 0,
                    "message": "Authentication is disabled on this server",
                }));
            }
        }
        _ => {}
    }

    // 3. Authenticate request if auth is enabled
    let ctx = match auth {
        Some(auth_mgr) if auth_mgr.is_auth_enabled() => {
            match auth_mgr.authenticate_request(auth_header) {
                Ok(c) => c,
                Err(e) => {
                    return HttpResponse::error(
                        StatusCode(401),
                        &format!("Authentication required: {}", e),
                    );
                }
            }
        }
        _ => SecurityContext::root(),
    };

    // 4. Authenticated & Authorized Route Dispatch
    match (method, path) {
        // Current Authenticated User Profile
        (&Method::Get, "/auth/me") => HttpResponse::json(&serde_json::json!({
            "username": ctx.username,
            "roles": ctx.roles,
            "privileges": ctx.privileges,
        })),

        // User Management (Privilege: UserAdmin)
        (&Method::Get, "/auth/users") => {
            if let Err(e) = ctx.require(Privilege::UserAdmin) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Some(auth_mgr) = auth {
                match auth_mgr.list_users() {
                    Ok(users) => HttpResponse::json(&users),
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            } else {
                HttpResponse::json(&serde_json::json!([]))
            }
        }
        (&Method::Post, "/auth/users") => {
            if let Err(e) = ctx.require(Privilege::UserAdmin) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Some(auth_mgr) = auth {
                if let Ok(req) = serde_json::from_slice::<CreateUserRequest>(body) {
                    let roles = if req.roles.is_empty() {
                        vec!["reader".to_string()]
                    } else {
                        req.roles
                    };
                    match auth_mgr.create_user(&req.username, &req.password, roles) {
                        Ok(_) => HttpResponse::ok(
                            r#"{"success":true,"message":"User created successfully"}"#,
                        ),
                        Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid JSON for /auth/users")
                }
            } else {
                HttpResponse::error(StatusCode(400), "Authentication manager not initialized")
            }
        }
        (&Method::Delete, p) if p.starts_with("/auth/users/") => {
            if let Err(e) = ctx.require(Privilege::UserAdmin) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let target_user = &p[12..];
            if target_user.is_empty() {
                return HttpResponse::error(StatusCode(400), "Username missing from URL");
            }
            if let Some(auth_mgr) = auth {
                match auth_mgr.delete_user(target_user) {
                    Ok(_) => HttpResponse::ok(
                        r#"{"success":true,"message":"User deleted successfully"}"#,
                    ),
                    Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Authentication manager not initialized")
            }
        }
        (&Method::Post, p) if p.starts_with("/auth/users/") && p.ends_with("/roles") => {
            if let Err(e) = ctx.require(Privilege::UserAdmin) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let parts: Vec<&str> = p.split('/').collect();
            if parts.len() == 5 {
                let target_user = parts[3];
                if let Some(auth_mgr) = auth {
                    if let Ok(req) = serde_json::from_slice::<AssignRolesRequest>(body) {
                        match auth_mgr.assign_roles(target_user, req.roles) {
                            Ok(_) => HttpResponse::ok(
                                r#"{"success":true,"message":"Roles updated successfully"}"#,
                            ),
                            Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                        }
                    } else {
                        HttpResponse::error(StatusCode(400), "Invalid JSON for assign roles")
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Authentication manager not initialized")
                }
            } else {
                HttpResponse::error(StatusCode(404), "Route not found")
            }
        }
        (&Method::Put, p) if p.starts_with("/auth/users/") && p.ends_with("/password") => {
            let parts: Vec<&str> = p.split('/').collect();
            if parts.len() == 5 {
                let target_user = parts[3];
                // Allow self-service password update or UserAdmin
                if ctx.username != target_user {
                    if let Err(e) = ctx.require(Privilege::UserAdmin) {
                        return HttpResponse::error(StatusCode(403), &e.to_string());
                    }
                }
                if let Some(auth_mgr) = auth {
                    if let Ok(req) = serde_json::from_slice::<UpdatePasswordRequest>(body) {
                        match auth_mgr.update_password(target_user, &req.password) {
                            Ok(_) => HttpResponse::ok(
                                r#"{"success":true,"message":"Password updated successfully"}"#,
                            ),
                            Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                        }
                    } else {
                        HttpResponse::error(StatusCode(400), "Invalid JSON for password update")
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Authentication manager not initialized")
                }
            } else {
                HttpResponse::error(StatusCode(404), "Route not found")
            }
        }

        // Health & Stats
        (&Method::Get, "/stats") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let stats = db.stats();
            HttpResponse::json(&stats)
        }

        // Cluster Status & Dynamic Join
        (&Method::Get, "/cluster/status") => {
            if let Err(e) = ctx.require(Privilege::DbAccess) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Some(c) = cluster {
                HttpResponse::json(&c.status())
            } else {
                HttpResponse::ok(r#"{"cluster_enabled":false}"#)
            }
        }
        (&Method::Post, "/cluster/join") => {
            if let Err(e) = ctx.require(Privilege::ClusterAdmin) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Some(c) = cluster {
                if let Ok(req) = serde_json::from_slice::<JoinClusterRequest>(body) {
                    match c.join_cluster(&req.peer_address) {
                        Ok(_) => HttpResponse::ok(
                            r#"{"success":true,"message":"Cluster join requested"}"#,
                        ),
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
                let trimmed = req.query.trim();
                if trimmed.to_uppercase().starts_with("EXPLAIN ") {
                    if let Err(e) = ctx.require(Privilege::GraphRead) {
                        return HttpResponse::error(StatusCode(403), &e.to_string());
                    }
                    let actual_query = trimmed[8..].trim();
                    match db.explain_cypher(actual_query) {
                        Ok(plan) => {
                            return HttpResponse::json(&serde_json::json!({
                                "success": true,
                                "explain": true,
                                "query": actual_query,
                                "is_mutation": plan.is_mutation,
                                "plan": plan.root_step,
                                "plan_tree": plan.text_representation,
                            }));
                        }
                        Err(e) => return HttpResponse::error(StatusCode(400), &e.to_string()),
                    }
                }

                let is_mutation = crate::query::cypher::is_cypher_mutation(&req.query);
                if is_mutation {
                    if let Err(e) = ctx.require(Privilege::GraphWrite) {
                        return HttpResponse::error(StatusCode(403), &e.to_string());
                    }
                } else {
                    if let Err(e) = ctx.require(Privilege::GraphRead) {
                        return HttpResponse::error(StatusCode(403), &e.to_string());
                    }
                }

                match db.cypher_with_params(&req.query, &req.params) {
                    Ok(res) => HttpResponse::json(&serde_json::json!({
                        "success": true,
                        "columns": res.columns,
                        "rows": res.rows,
                        "error": null,
                    })),
                    Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                }
            } else {
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid JSON payload for /cypher (expected {\"query\": \"...\"})",
                )
            }
        }
        (&Method::Post, "/cypher/explain") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Ok(req) = serde_json::from_slice::<CypherRequest>(body) {
                let trimmed = req.query.trim();
                let actual_query = if trimmed.to_uppercase().starts_with("EXPLAIN ") {
                    trimmed[8..].trim()
                } else {
                    trimmed
                };
                match db.explain_cypher(actual_query) {
                    Ok(plan) => HttpResponse::json(&serde_json::json!({
                        "success": true,
                        "explain": true,
                        "query": actual_query,
                        "is_mutation": plan.is_mutation,
                        "plan": plan.root_step,
                        "plan_tree": plan.text_representation,
                    })),
                    Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                }
            } else {
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid JSON payload for /cypher/explain (expected {\"query\": \"...\"})",
                )
            }
        }

        // ACID Transactions
        (&Method::Post, "/tx/begin") => {
            if let Err(e) = ctx.require(Privilege::TxManage) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::TxManage) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
                        HttpResponse::error(
                            StatusCode(400),
                            "Invalid JSON payload for /tx/:id/cypher",
                        )
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid Transaction ID in URL")
                }
            } else {
                HttpResponse::error(StatusCode(404), "Route not found")
            }
        }
        (&Method::Post, p) if p.starts_with("/tx/") && p.ends_with("/commit") => {
            if let Err(e) = ctx.require(Privilege::TxManage) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::TxManage) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::TxManage) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::GraphWrite) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
        (&Method::Get, p) if p.starts_with("/nodes/") && !p.ends_with("/ttl") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::GraphWrite) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid JSON for /bulk/nodes (expected {\"nodes\": [...]})",
                )
            }
        }
        (&Method::Post, "/bulk/edges") => {
            if let Err(e) = ctx.require(Privilege::GraphWrite) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid JSON for /bulk/edges (expected {\"edges\": [...]})",
                )
            }
        }

        // TTL (Time-To-Live) Node Expiration
        (&Method::Post, "/ttl/prune") => {
            if let Err(e) = ctx.require(Privilege::GraphWrite) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            match db.prune_expired_nodes() {
                Ok(pruned) => HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "pruned_nodes": pruned,
                })),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Post, p) if p.starts_with("/nodes/") && p.ends_with("/ttl") => {
            if let Err(e) = ctx.require(Privilege::GraphSchema) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let parts: Vec<&str> = p.split('/').collect();
            if parts.len() == 4 {
                if let Ok(id) = parts[2].parse::<u64>() {
                    if let Ok(req) = serde_json::from_slice::<SetTtlRequest>(body) {
                        match db.set_node_ttl(
                            NodeId(id),
                            std::time::Duration::from_secs(req.ttl_seconds),
                        ) {
                            Ok(_) => HttpResponse::json(&serde_json::json!({
                                "success": true,
                                "node_id": id,
                                "ttl_seconds": req.ttl_seconds,
                                "message": format!("TTL of {}s set on node {}", req.ttl_seconds, id),
                            })),
                            Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                        }
                    } else {
                        HttpResponse::error(
                            StatusCode(400),
                            "Invalid JSON payload for /nodes/:id/ttl (expected {\"ttl_seconds\": 60})",
                        )
                    }
                } else {
                    HttpResponse::error(StatusCode(400), "Invalid Node ID")
                }
            } else {
                HttpResponse::error(StatusCode(404), "Route not found")
            }
        }
        (&Method::Delete, p) if p.starts_with("/nodes/") => {
            if let Err(e) = ctx.require(Privilege::GraphWrite) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::GraphWrite) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::GraphWrite) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
            if let Err(e) = ctx.require(Privilege::GraphSchema) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
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
                    Ok(_) => {
                        HttpResponse::ok(r#"{"success":true,"message":"Vector index created"}"#)
                    }
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Invalid JSON for /vector/indexes")
            }
        }
        (&Method::Post, "/vector/search") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Ok(req) = serde_json::from_slice::<VectorSearchRequest>(body) {
                match db.query_vector_index(&req.index, req.k, &req.vector) {
                    Ok(hits) => HttpResponse::json(&hits),
                    Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Invalid JSON for /vector/search")
            }
        }

        // Inverted Full-Text Keyword Search
        (&Method::Post, "/fulltext/indexes") => {
            if let Err(e) = ctx.require(Privilege::GraphSchema) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Ok(req) = serde_json::from_slice::<CreateFulltextIndexRequest>(body) {
                match db.create_fulltext_index(&req.name, &req.label, &req.property_key) {
                    Ok(_) => {
                        HttpResponse::ok(r#"{"success":true,"message":"Full-text index created"}"#)
                    }
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Invalid JSON for /fulltext/indexes")
            }
        }
        (&Method::Post, "/fulltext/search") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Ok(req) = serde_json::from_slice::<FulltextSearchRequest>(body) {
                let res = if req.prefix {
                    db.query_fulltext_prefix(&req.index, &req.query, req.top_k)
                } else {
                    db.query_fulltext_index(&req.index, &req.query, req.top_k)
                };
                match res {
                    Ok(hits) => HttpResponse::json(&hits),
                    Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                }
            } else {
                HttpResponse::error(StatusCode(400), "Invalid JSON for /fulltext/search")
            }
        }

        // Schema & Index Introspection & Management
        (&Method::Get, "/schema") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            HttpResponse::json(&db.schema())
        }
        (&Method::Get, "/indexes") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            HttpResponse::json(&db.list_indexes())
        }
        (&Method::Post, "/indexes/property") => {
            if let Err(e) = ctx.require(Privilege::GraphSchema) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Ok(req) = serde_json::from_slice::<CreatePropertyIndexRequest>(body) {
                match db.create_property_index(&req.label, &req.property) {
                    Ok(true) => HttpResponse::ok(
                        r#"{"success":true,"message":"Property index created successfully"}"#,
                    ),
                    Ok(false) => HttpResponse::ok(
                        r#"{"success":false,"message":"Property index already exists"}"#,
                    ),
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            } else {
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid JSON for /indexes/property (expected {\"label\": \"...\", \"property\": \"...\"})",
                )
            }
        }
        (&Method::Delete, p) if p.starts_with("/indexes/property/") => {
            if let Err(e) = ctx.require(Privilege::GraphSchema) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let parts: Vec<&str> = p.split('/').collect();
            if parts.len() == 5 {
                let label = parts[3];
                let prop = parts[4];
                match db.drop_property_index(label, prop) {
                    Ok(true) => {
                        HttpResponse::ok(r#"{"success":true,"message":"Property index dropped"}"#)
                    }
                    Ok(false) => HttpResponse::error(StatusCode(404), "Property index not found"),
                    Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
                }
            } else {
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid URL format (expected /indexes/property/:label/:property)",
                )
            }
        }
        (&Method::Delete, p) if p.starts_with("/vector/indexes/") => {
            if let Err(e) = ctx.require(Privilege::GraphSchema) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let name = &p[16..];
            if name.is_empty() {
                return HttpResponse::error(StatusCode(400), "Missing vector index name");
            }
            match db.drop_vector_index(name) {
                Ok(true) => {
                    HttpResponse::ok(r#"{"success":true,"message":"Vector index dropped"}"#)
                }
                Ok(false) => HttpResponse::error(StatusCode(404), "Vector index not found"),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Delete, p) if p.starts_with("/fulltext/indexes/") => {
            if let Err(e) = ctx.require(Privilege::GraphSchema) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let name = &p[18..];
            if name.is_empty() {
                return HttpResponse::error(StatusCode(400), "Missing fulltext index name");
            }
            match db.drop_fulltext_index(name) {
                Ok(true) => {
                    HttpResponse::ok(r#"{"success":true,"message":"Full-text index dropped"}"#)
                }
                Ok(false) => HttpResponse::error(StatusCode(404), "Full-text index not found"),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }

        // Graph Algorithms & Analytics
        (&Method::Post, "/algo/pagerank") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let req = serde_json::from_slice::<PageRankRequest>(body).unwrap_or(PageRankRequest {
                damping: 0.85,
                max_iterations: 50,
                tolerance: 1e-6,
                seeds: None,
            });

            let ranks = if let Some(seeds) = req.seeds {
                let seed_ids: Vec<NodeId> = seeds.into_iter().map(NodeId).collect();
                db.run_personalized_pagerank(
                    &seed_ids,
                    req.damping,
                    req.max_iterations,
                    req.tolerance,
                )
            } else {
                db.run_pagerank(req.damping, req.max_iterations, req.tolerance)
            };

            let ranked_json: Vec<_> = ranks.into_iter().map(|(id, score)| {
                let node_info = db.get_node(NodeId(id));
                serde_json::json!({
                    "node_id": id,
                    "score": score,
                    "labels": node_info.as_ref().map(|n| n.labels.iter().collect::<Vec<_>>()).unwrap_or_default(),
                    "properties": node_info.map(|n| n.properties).unwrap_or_default(),
                })
            }).collect();

            HttpResponse::json(&serde_json::json!({
                "success": true,
                "algorithm": "pagerank",
                "damping": req.damping,
                "iterations": req.max_iterations,
                "count": ranked_json.len(),
                "top_nodes": ranked_json,
            }))
        }
        (&Method::Post, "/algo/shortest-path") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Ok(req) = serde_json::from_slice::<ShortestPathRequest>(body) {
                let dir = match req.direction.to_lowercase().as_str() {
                    "outgoing" => crate::model::Direction::Outgoing,
                    "incoming" => crate::model::Direction::Incoming,
                    _ => crate::model::Direction::Both,
                };
                let src = NodeId(req.source);
                let dst = NodeId(req.target);

                if req.algorithm.eq_ignore_ascii_case("dijkstra") {
                    let res = db.run_dijkstra(src, dst, &req.weight_property, dir);
                    match res {
                        Some((path, cost)) => HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "algorithm": "dijkstra",
                            "found": true,
                            "source": req.source,
                            "target": req.target,
                            "cost": cost,
                            "length": path.len(),
                            "nodes": path.nodes().iter().map(|n| n.raw()).collect::<Vec<_>>(),
                            "edges": path.edges().iter().map(|e| e.raw()).collect::<Vec<_>>(),
                            "path": path.to_string(),
                        })),
                        None => HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "algorithm": "dijkstra",
                            "found": false,
                            "source": req.source,
                            "target": req.target,
                            "cost": null,
                            "nodes": [],
                            "edges": [],
                        })),
                    }
                } else {
                    let path_opt = db.run_shortest_path_bfs(src, dst, dir);
                    match path_opt {
                        Some(path) => HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "algorithm": "bfs",
                            "found": true,
                            "source": req.source,
                            "target": req.target,
                            "cost": path.len() as f64,
                            "length": path.len(),
                            "nodes": path.nodes().iter().map(|n| n.raw()).collect::<Vec<_>>(),
                            "edges": path.edges().iter().map(|e| e.raw()).collect::<Vec<_>>(),
                            "path": path.to_string(),
                        })),
                        None => HttpResponse::json(&serde_json::json!({
                            "success": true,
                            "algorithm": "bfs",
                            "found": false,
                            "source": req.source,
                            "target": req.target,
                            "cost": null,
                            "nodes": [],
                            "edges": [],
                        })),
                    }
                }
            } else {
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid JSON for /algo/shortest-path (expected {\"source\": 1, \"target\": 2})",
                )
            }
        }
        (&Method::Post, "/algo/centrality") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let req =
                serde_json::from_slice::<CentralityRequest>(body).unwrap_or(CentralityRequest {
                    metric: "betweenness".to_string(),
                    direction: "both".to_string(),
                    normalized: true,
                });
            let dir = match req.direction.to_lowercase().as_str() {
                "outgoing" => crate::model::Direction::Outgoing,
                "incoming" => crate::model::Direction::Incoming,
                _ => crate::model::Direction::Both,
            };

            if req.metric.eq_ignore_ascii_case("degree") {
                let scores = db.run_degree_centrality(dir);
                let json_scores: Vec<_> = scores.into_iter().map(|(id, deg)| {
                    let n = db.get_node(NodeId(id));
                    serde_json::json!({
                        "node_id": id,
                        "degree": deg,
                        "labels": n.as_ref().map(|x| x.labels.iter().collect::<Vec<_>>()).unwrap_or_default(),
                        "properties": n.map(|x| x.properties).unwrap_or_default(),
                    })
                }).collect();
                HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "metric": "degree",
                    "direction": req.direction,
                    "top_nodes": json_scores,
                }))
            } else {
                let scores = db.run_betweenness_centrality(dir, req.normalized);
                let json_scores: Vec<_> = scores.into_iter().map(|(id, s)| {
                    let n = db.get_node(NodeId(id));
                    serde_json::json!({
                        "node_id": id,
                        "score": s,
                        "labels": n.as_ref().map(|x| x.labels.iter().collect::<Vec<_>>()).unwrap_or_default(),
                        "properties": n.map(|x| x.properties).unwrap_or_default(),
                    })
                }).collect();
                HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "metric": "betweenness",
                    "direction": req.direction,
                    "normalized": req.normalized,
                    "top_nodes": json_scores,
                }))
            }
        }
        (&Method::Post, "/algo/community") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let req =
                serde_json::from_slice::<CommunityRequest>(body).unwrap_or(CommunityRequest {
                    algorithm: "wcc".to_string(),
                });
            match db.run_community_detection(&req.algorithm) {
                Ok(clusters) => HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "algorithm": req.algorithm,
                    "component_count": clusters.len(),
                    "communities": clusters,
                })),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Post, "/algo/similarity") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            if let Ok(req) = serde_json::from_slice::<SimilarityRequest>(body) {
                let dir = match req.direction.to_lowercase().as_str() {
                    "outgoing" => crate::model::Direction::Outgoing,
                    "incoming" => crate::model::Direction::Incoming,
                    _ => crate::model::Direction::Both,
                };
                match db.run_similarity(NodeId(req.source), NodeId(req.target), &req.metric, dir) {
                    Ok(mut res) => {
                        res["source"] = serde_json::json!(req.source);
                        res["target"] = serde_json::json!(req.target);
                        res["success"] = serde_json::json!(true);
                        HttpResponse::json(&res)
                    }
                    Err(e) => HttpResponse::error(StatusCode(400), &e.to_string()),
                }
            } else {
                HttpResponse::error(
                    StatusCode(400),
                    "Invalid JSON for /algo/similarity (expected {\"source\": 1, \"target\": 2})",
                )
            }
        }

        // Persistence: Checkpoint, Export, Import
        (&Method::Post, "/checkpoint") => {
            if let Err(e) = ctx.require(Privilege::ServerAdmin) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            match db.checkpoint() {
                Ok(seq) => HttpResponse::json(&serde_json::json!({
                    "success": true,
                    "checkpointed_sequence": seq,
                })),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Get, "/export") => {
            if let Err(e) = ctx.require(Privilege::GraphRead) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            match db.graph().read(|g| g.to_json()) {
                Ok(json) => HttpResponse::ok(&json),
                Err(e) => HttpResponse::error(StatusCode(500), &e.to_string()),
            }
        }
        (&Method::Post, "/import") => {
            if let Err(e) = ctx.require(Privilege::ServerAdmin) {
                return HttpResponse::error(StatusCode(403), &e.to_string());
            }
            let json_str = String::from_utf8_lossy(body);
            match crate::graph::Graph::from_json(&json_str) {
                Ok(restored) => {
                    db.graph().write(|g| {
                        *g = restored;
                    });
                    let _ = db.checkpoint();
                    HttpResponse::ok(
                        r#"{"success":true,"message":"Database imported successfully"}"#,
                    )
                }
                Err(e) => HttpResponse::error(
                    StatusCode(400),
                    &format!("Failed to parse import JSON: {}", e),
                ),
            }
        }

        _ => HttpResponse::error(
            StatusCode(404),
            &format!("Route not found: {} {}", method, path),
        ),
    }
}

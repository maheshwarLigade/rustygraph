use super::protocol::*;
use crate::error::Result;
use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::Direction;
use crate::storage::db::StandaloneDatabase;
use crate::value::PropertyValue;
use serde_json::json;
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock};

/// Backend storage engine driving the MCP Server.
#[derive(Clone)]
pub enum McpBackend {
    /// In-memory graph protected by read-write lock.
    Graph(Arc<RwLock<Graph>>),
    /// Standalone persistent database backed by WAL and snapshot isolation.
    Database(Arc<StandaloneDatabase>),
    /// Remote RustyGraph server connected over HTTP REST API.
    Remote(String),
}

/// Model Context Protocol (MCP) server providing standard AI agent tools and resources.
pub struct McpServer {
    backend: McpBackend,
    name: String,
    version: String,
}

impl McpServer {
    /// Creates a new MCP server backed by the given backend.
    pub fn new(backend: McpBackend) -> Self {
        Self {
            backend,
            name: "rustygraph-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Creates an MCP server using an in-memory graph.
    pub fn with_graph(graph: Arc<RwLock<Graph>>) -> Self {
        Self::new(McpBackend::Graph(graph))
    }

    /// Creates an MCP server using a standalone persistent database.
    pub fn with_database(db: Arc<StandaloneDatabase>) -> Self {
        Self::new(McpBackend::Database(db))
    }

    /// Creates an MCP server connected to a remote RustyGraph server URL.
    pub fn with_remote(url: impl Into<String>) -> Self {
        Self::new(McpBackend::Remote(url.into()))
    }

    /// Returns the list of MCP tool definitions exposed by this server.
    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "cypher_query".to_string(),
                description: "Execute an openCypher statement against RustyGraph. Supports pattern matching, variable-length paths (*1..3), atomic MERGE upserts, list aggregations (collect, count), mutations (CREATE, SET, DELETE, DETACH), and parameters ($param).".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "openCypher query string to execute"
                        },
                        "params": {
                            "type": "object",
                            "description": "Optional key-value parameters to safely resolve $param variables in query"
                        }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "vector_search".to_string(),
                description: "Perform nearest-neighbor vector similarity search using dense embeddings (Cosine, Euclidean, or DotProduct metrics).".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "index": {
                            "type": "string",
                            "description": "Name of the vector index"
                        },
                        "vector": {
                            "type": "array",
                            "items": { "type": "number" },
                            "description": "Query embedding vector"
                        },
                        "k": {
                            "type": "integer",
                            "description": "Number of nearest neighbors to retrieve (default: 5)"
                        }
                    },
                    "required": ["index", "vector"]
                }),
            },
            ToolDefinition {
                name: "graph_rag".to_string(),
                description: "Hybrid GraphRAG retrieval: finds top-k semantic seed nodes by vector embedding, then traverses and expands n-hop relational graph topology to synthesize rich context for LLM prompt augmentation.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "index": {
                            "type": "string",
                            "description": "Name of the vector index"
                        },
                        "vector": {
                            "type": "array",
                            "items": { "type": "number" },
                            "description": "Query embedding vector"
                        },
                        "k": {
                            "type": "integer",
                            "description": "Number of seed documents to retrieve (default: 3)"
                        },
                        "hops": {
                            "type": "integer",
                            "description": "Relational traversal depth bound (default: 2)"
                        }
                    },
                    "required": ["index", "vector"]
                }),
            },
            ToolDefinition {
                name: "fulltext_search".to_string(),
                description: "Execute inverted full-text keyword search across indexed string properties with BM25 / TF-IDF relevance ranking and prefix matching.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "index": {
                            "type": "string",
                            "description": "Name of the full-text index"
                        },
                        "query": {
                            "type": "string",
                            "description": "Search keyword phrase"
                        },
                        "top_k": {
                            "type": "integer",
                            "description": "Maximum number of results to return (default: 10)"
                        },
                        "prefix": {
                            "type": "boolean",
                            "description": "Enable prefix matching for incomplete terms (default: false)"
                        }
                    },
                    "required": ["index", "query"]
                }),
            },
            ToolDefinition {
                name: "get_schema".to_string(),
                description: "Inspect the database schema, including all active node labels, relationship types, property keys, secondary property indexes, full-text indexes, vector indexes, and active entity counts.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            ToolDefinition {
                name: "run_algorithm".to_string(),
                description: "Execute Graph Data Science (GDS) algorithms directly on the graph: pagerank, personalized_pagerank (PPR with restart seeds), betweenness_centrality (Brandes $O(V \\cdot E)$), shortest_path, weakly_connected_components (WCC), strongly_connected_components (SCC), or degree_centrality.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "algorithm": {
                            "type": "string",
                            "enum": ["pagerank", "personalized_pagerank", "betweenness_centrality", "shortest_path", "wcc", "scc", "degree_centrality"],
                            "description": "Graph algorithm to execute"
                        },
                        "params": {
                            "type": "object",
                            "description": "Algorithm parameters: seeds (array of node IDs for PPR), damping_factor (number), max_iterations (integer), source (integer node ID for shortest_path), target (integer node ID for shortest_path), direction ('outgoing', 'incoming', 'both'), normalized (boolean)"
                        }
                    },
                    "required": ["algorithm"]
                }),
            },
        ]
    }

    /// Returns the list of MCP resource definitions exposed by this server.
    pub fn get_resource_definitions(&self) -> Vec<ResourceDefinition> {
        vec![
            ResourceDefinition {
                uri: "rustygraph://schema".to_string(),
                name: "RustyGraph Schema".to_string(),
                description: Some("Comprehensive summary of labels, relationship types, property indices, and metrics".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDefinition {
                uri: "rustygraph://stats".to_string(),
                name: "RustyGraph Statistics & Telemetry".to_string(),
                description: Some("Real-time database metrics including node count, relationship count, and WAL log sequence".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ]
    }

    /// Handles an incoming JSON-RPC 2.0 request and returns an optional response envelope.
    pub fn handle_request(&self, req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        let empty_params = json!({});
        let params = req.params.as_ref().unwrap_or(&empty_params);

        match req.method.as_str() {
            "initialize" => {
                let result = json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": false, "listChanged": false }
                    },
                    "serverInfo": {
                        "name": self.name,
                        "version": self.version
                    }
                });
                Some(JsonRpcResponse::success(req.id.clone(), result))
            }
            "notifications/initialized" => None,
            "ping" => Some(JsonRpcResponse::success(req.id.clone(), json!({}))),
            "tools/list" => {
                let tools = self.get_tool_definitions();
                Some(JsonRpcResponse::success(req.id.clone(), json!({ "tools": tools })))
            }
            "tools/call" => {
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

                let tool_res = self.execute_tool(tool_name, &args);
                Some(JsonRpcResponse::success(req.id.clone(), serde_json::to_value(tool_res).unwrap()))
            }
            "resources/list" => {
                let resources = self.get_resource_definitions();
                Some(JsonRpcResponse::success(req.id.clone(), json!({ "resources": resources })))
            }
            "resources/read" => {
                let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                match self.read_resource(uri) {
                    Ok(content) => Some(JsonRpcResponse::success(req.id.clone(), json!({ "contents": [content] }))),
                    Err(e) => Some(JsonRpcResponse::error(req.id.clone(), -32002, format!("Resource not found: {e}"))),
                }
            }
            other => Some(JsonRpcResponse::error(
                req.id.clone(),
                -32601,
                format!("Method '{other}' not found"),
            )),
        }
    }

    fn execute_tool(&self, name: &str, args: &serde_json::Value) -> ToolCallResult {
        match name {
            "cypher_query" => self.tool_cypher_query(args),
            "vector_search" => self.tool_vector_search(args),
            "graph_rag" => self.tool_graph_rag(args),
            "fulltext_search" => self.tool_fulltext_search(args),
            "get_schema" => self.tool_get_schema(),
            "run_algorithm" => self.tool_run_algorithm(args),
            other => ToolCallResult::error(format!("Unknown tool: '{other}'")),
        }
    }

    fn tool_cypher_query(&self, args: &serde_json::Value) -> ToolCallResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolCallResult::error("Missing required argument 'query'"),
        };

        let mut query_params = HashMap::new();
        if let Some(params_obj) = args.get("params").and_then(|v| v.as_object()) {
            for (k, v) in params_obj {
                query_params.insert(k.clone(), PropertyValue::from(v.clone()));
            }
        }

        match &self.backend {
            McpBackend::Graph(g_lock) => {
                let mut g = match g_lock.write() {
                    Ok(guard) => guard,
                    Err(e) => return ToolCallResult::error(format!("Lock acquisition failed: {e}")),
                };
                match g.cypher_with_params(query, &query_params) {
                    Ok(res) => ToolCallResult::text(res.to_ascii_table()),
                    Err(e) => ToolCallResult::error(format!("Cypher error: {e}")),
                }
            }
            McpBackend::Database(db) => {
                match db.cypher_with_params(query, &query_params) {
                    Ok(res) => ToolCallResult::text(res.to_ascii_table()),
                    Err(e) => ToolCallResult::error(format!("Cypher error: {e}")),
                }
            }
            McpBackend::Remote(url) => {
                let body = json!({ "query": query, "params": query_params }).to_string();
                match http_post(url, "/cypher", &body) {
                    Ok((status, text)) => {
                        if status == 200 {
                            ToolCallResult::text(text)
                        } else {
                            ToolCallResult::error(format!("HTTP {status}: {text}"))
                        }
                    }
                    Err(e) => ToolCallResult::error(format!("Remote HTTP failure: {e}")),
                }
            }
        }
    }

    fn tool_vector_search(&self, args: &serde_json::Value) -> ToolCallResult {
        let index = match args.get("index").and_then(|v| v.as_str()) {
            Some(idx) => idx,
            None => return ToolCallResult::error("Missing required argument 'index'"),
        };

        let vector: Vec<f32> = match args.get("vector").and_then(|v| v.as_array()) {
            Some(arr) => arr.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect(),
            None => return ToolCallResult::error("Missing required argument 'vector'"),
        };

        let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        match &self.backend {
            McpBackend::Graph(g_lock) => {
                let g = match g_lock.read() {
                    Ok(guard) => guard,
                    Err(e) => return ToolCallResult::error(format!("Lock acquisition failed: {e}")),
                };
                match g.query_vector_index(index, k, &vector) {
                    Ok(hits) => {
                        let mut output = Vec::new();
                        for hit in hits {
                            let props = g.get_node(hit.node_id).map(|n| n.properties.clone()).unwrap_or_default();
                            output.push(json!({
                                "node_id": hit.node_id.raw(),
                                "similarity_score": hit.score,
                                "properties": props
                            }));
                        }
                        ToolCallResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
                    }
                    Err(e) => ToolCallResult::error(format!("Vector search error: {e}")),
                }
            }
            McpBackend::Database(db) => {
                let res = db.graph().with_read(|g| {
                    g.query_vector_index(index, k, &vector).map(|hits| {
                        hits.into_iter().map(|hit| {
                            let props = g.get_node(hit.node_id).map(|n| n.properties.clone()).unwrap_or_default();
                            json!({
                                "node_id": hit.node_id.raw(),
                                "similarity_score": hit.score,
                                "properties": props
                            })
                        }).collect::<Vec<_>>()
                    })
                });
                match res {
                    Ok(hits) => ToolCallResult::text(serde_json::to_string_pretty(&hits).unwrap_or_default()),
                    Err(e) => ToolCallResult::error(format!("Vector search error: {e}")),
                }
            }
            McpBackend::Remote(url) => {
                let body = json!({ "index": index, "vector": vector, "k": k }).to_string();
                match http_post(url, "/vector/search", &body) {
                    Ok((status, text)) => {
                        if status == 200 { ToolCallResult::text(text) } else { ToolCallResult::error(format!("HTTP {status}: {text}")) }
                    }
                    Err(e) => ToolCallResult::error(format!("Remote HTTP failure: {e}")),
                }
            }
        }
    }

    fn tool_graph_rag(&self, args: &serde_json::Value) -> ToolCallResult {
        let index = match args.get("index").and_then(|v| v.as_str()) {
            Some(idx) => idx,
            None => return ToolCallResult::error("Missing required argument 'index'"),
        };

        let vector: Vec<f32> = match args.get("vector").and_then(|v| v.as_array()) {
            Some(arr) => arr.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect(),
            None => return ToolCallResult::error("Missing required argument 'vector'"),
        };

        let k = args.get("k").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
        let hops = args.get("hops").and_then(|v| v.as_u64()).unwrap_or(2) as usize;

        let execute_rag = |g: &Graph| -> Result<serde_json::Value> {
            let rag_hits = g.vector_search_and_traverse(index, &vector, k, hops)?;
            let mut formatted = Vec::new();
            for (hit, paths) in rag_hits {
                let seed_node = g.get_node(hit.node_id).map(|n| {
                    json!({
                        "id": n.id.raw(),
                        "labels": n.labels.iter().collect::<Vec<_>>(),
                        "properties": n.properties
                    })
                });
                let path_strings: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
                formatted.push(json!({
                    "seed_document": seed_node,
                    "similarity_score": hit.score,
                    "expanded_context_paths": path_strings,
                }));
            }
            Ok(json!({ "query_k": k, "traversal_hops": hops, "rag_context": formatted }))
        };

        match &self.backend {
            McpBackend::Graph(g_lock) => {
                let g = match g_lock.read() {
                    Ok(guard) => guard,
                    Err(e) => return ToolCallResult::error(format!("Lock acquisition failed: {e}")),
                };
                match execute_rag(&g) {
                    Ok(v) => ToolCallResult::text(serde_json::to_string_pretty(&v).unwrap_or_default()),
                    Err(e) => ToolCallResult::error(format!("GraphRAG error: {e}")),
                }
            }
            McpBackend::Database(db) => {
                match db.graph().with_read(|g| execute_rag(g)) {
                    Ok(v) => ToolCallResult::text(serde_json::to_string_pretty(&v).unwrap_or_default()),
                    Err(e) => ToolCallResult::error(format!("GraphRAG error: {e}")),
                }
            }
            McpBackend::Remote(url) => {
                let body = json!({ "index": index, "vector": vector, "k": k }).to_string();
                match http_post(url, "/vector/search", &body) {
                    Ok((status, text)) => {
                        if status == 200 { ToolCallResult::text(text) } else { ToolCallResult::error(format!("HTTP {status}: {text}")) }
                    }
                    Err(e) => ToolCallResult::error(format!("Remote HTTP failure: {e}")),
                }
            }
        }
    }

    fn tool_fulltext_search(&self, args: &serde_json::Value) -> ToolCallResult {
        let index = match args.get("index").and_then(|v| v.as_str()) {
            Some(idx) => idx,
            None => return ToolCallResult::error("Missing required argument 'index'"),
        };
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolCallResult::error("Missing required argument 'query'"),
        };
        let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let prefix = args.get("prefix").and_then(|v| v.as_bool()).unwrap_or(false);

        match &self.backend {
            McpBackend::Graph(g_lock) => {
                let g = match g_lock.read() {
                    Ok(guard) => guard,
                    Err(e) => return ToolCallResult::error(format!("Lock acquisition failed: {e}")),
                };
                let hits_res = if prefix {
                    g.query_fulltext_prefix(index, query, top_k)
                } else {
                    g.query_fulltext_index(index, query, top_k)
                };

                match hits_res {
                    Ok(hits) => {
                        let mut output = Vec::new();
                        for hit in hits {
                            let props = g.get_node(hit.node_id).map(|n| n.properties.clone()).unwrap_or_default();
                            output.push(json!({
                                "node_id": hit.node_id.raw(),
                                "relevance_score": hit.score,
                                "properties": props
                            }));
                        }
                        ToolCallResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
                    }
                    Err(e) => ToolCallResult::error(format!("Fulltext search error: {e}")),
                }
            }
            McpBackend::Database(db) => {
                let res = db.graph().with_read(|g| {
                    let hits_res = if prefix {
                        g.query_fulltext_prefix(index, query, top_k)
                    } else {
                        g.query_fulltext_index(index, query, top_k)
                    };
                    hits_res.map(|hits| {
                        hits.into_iter().map(|hit| {
                            let props = g.get_node(hit.node_id).map(|n| n.properties.clone()).unwrap_or_default();
                            json!({
                                "node_id": hit.node_id.raw(),
                                "relevance_score": hit.score,
                                "properties": props
                            })
                        }).collect::<Vec<_>>()
                    })
                });
                match res {
                    Ok(hits) => ToolCallResult::text(serde_json::to_string_pretty(&hits).unwrap_or_default()),
                    Err(e) => ToolCallResult::error(format!("Fulltext search error: {e}")),
                }
            }
            McpBackend::Remote(url) => {
                let body = json!({ "index": index, "query": query, "top_k": top_k, "prefix": prefix }).to_string();
                match http_post(url, "/fulltext/search", &body) {
                    Ok((status, text)) => {
                        if status == 200 { ToolCallResult::text(text) } else { ToolCallResult::error(format!("HTTP {status}: {text}")) }
                    }
                    Err(e) => ToolCallResult::error(format!("Remote HTTP failure: {e}")),
                }
            }
        }
    }

    fn tool_get_schema(&self) -> ToolCallResult {
        let inspect_schema = |g: &Graph| -> serde_json::Value {
            let stats = g.stats();
            let mut vector_indices_info = Vec::new();
            for v_idx in g.vector_indices() {
                vector_indices_info.push(json!({
                    "name": v_idx.name,
                    "label": v_idx.label,
                    "property_key": v_idx.property_key,
                    "dimensions": v_idx.dimensions,
                    "metric": format!("{:?}", v_idx.metric),
                    "indexed_count": v_idx.len()
                }));
            }

            let mut fulltext_indices_info = Vec::new();
            for ft_idx in g.fulltext_indices() {
                fulltext_indices_info.push(json!({
                    "name": ft_idx.name,
                    "label": ft_idx.label,
                    "property_key": ft_idx.property_key,
                    "indexed_documents": ft_idx.len()
                }));
            }

            json!({
                "nodes_total": stats.node_count,
                "relationships_total": stats.edge_count,
                "label_counts": stats.label_counts,
                "relationship_type_counts": stats.type_counts,
                "vector_indices": vector_indices_info,
                "fulltext_indices": fulltext_indices_info,
                "graph_density": stats.density,
                "average_degree": stats.average_degree,
                "isolated_nodes": stats.isolated_nodes,
            })
        };

        match &self.backend {
            McpBackend::Graph(g_lock) => {
                let g = match g_lock.read() {
                    Ok(guard) => guard,
                    Err(e) => return ToolCallResult::error(format!("Lock acquisition failed: {e}")),
                };
                ToolCallResult::text(serde_json::to_string_pretty(&inspect_schema(&g)).unwrap_or_default())
            }
            McpBackend::Database(db) => {
                let schema_val = db.graph().with_read(|g| inspect_schema(g));
                ToolCallResult::text(serde_json::to_string_pretty(&schema_val).unwrap_or_default())
            }
            McpBackend::Remote(url) => match http_get(url, "/stats") {
                Ok((status, text)) => {
                    if status == 200 { ToolCallResult::text(text) } else { ToolCallResult::error(format!("HTTP {status}: {text}")) }
                }
                Err(e) => ToolCallResult::error(format!("Remote HTTP failure: {e}")),
            },
        }
    }

    fn tool_run_algorithm(&self, args: &serde_json::Value) -> ToolCallResult {
        let algorithm = match args.get("algorithm").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return ToolCallResult::error("Missing required argument 'algorithm'"),
        };
        let params = args.get("params").cloned().unwrap_or_else(|| json!({}));

        let execute_algo = |g: &Graph| -> Result<serde_json::Value> {
            match algorithm {
                "pagerank" => {
                    let damping = params.get("damping_factor").and_then(|v| v.as_f64()).unwrap_or(0.85);
                    let max_iter = params.get("max_iterations").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                    let scores = crate::algo::pagerank(g, damping, max_iter, 1e-6);
                    let mut sorted: Vec<(u64, f64)> = scores.into_iter().map(|(id, s)| (id.raw(), s)).collect();
                    sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
                    Ok(json!({ "algorithm": "pagerank", "top_ranked_nodes": sorted }))
                }
                "personalized_pagerank" => {
                    let damping = params.get("damping_factor").and_then(|v| v.as_f64()).unwrap_or(0.85);
                    let max_iter = params.get("max_iterations").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                    let seeds: Vec<NodeId> = params.get("seeds")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|x| x.as_u64().map(NodeId)).collect())
                        .unwrap_or_default();
                    let scores = crate::algo::personalized_pagerank(g, &seeds, damping, max_iter, 1e-6);
                    let mut sorted: Vec<(u64, f64)> = scores.into_iter().map(|(id, s)| (id.raw(), s)).collect();
                    sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
                    Ok(json!({ "algorithm": "personalized_pagerank", "seeds": seeds.iter().map(|s| s.raw()).collect::<Vec<_>>(), "ranked_nodes": sorted }))
                }
                "betweenness_centrality" => {
                    let normalized = params.get("normalized").and_then(|v| v.as_bool()).unwrap_or(true);
                    let dir_str = params.get("direction").and_then(|v| v.as_str()).unwrap_or("both");
                    let dir = match dir_str.to_lowercase().as_str() {
                        "outgoing" => Direction::Outgoing,
                        "incoming" => Direction::Incoming,
                        _ => Direction::Both,
                    };
                    let scores = crate::algo::betweenness_centrality(g, dir, normalized);
                    let mut sorted: Vec<(u64, f64)> = scores.into_iter().map(|(id, s)| (id.raw(), s)).collect();
                    sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
                    Ok(json!({ "algorithm": "betweenness_centrality", "direction": dir_str, "normalized": normalized, "top_nodes": sorted }))
                }
                "shortest_path" => {
                    let source = params.get("source").and_then(|v| v.as_u64()).map(NodeId).ok_or_else(|| {
                        crate::error::GraphError::InvalidOperation("Missing required 'source' node ID".into())
                    })?;
                    let target = params.get("target").and_then(|v| v.as_u64()).map(NodeId).ok_or_else(|| {
                        crate::error::GraphError::InvalidOperation("Missing required 'target' node ID".into())
                    })?;
                    let path_opt = crate::algo::shortest_path_bfs(g, source, target, Direction::Outgoing);
                    Ok(json!({
                        "algorithm": "shortest_path",
                        "source": source.raw(),
                        "target": target.raw(),
                        "found": path_opt.is_some(),
                        "length": path_opt.as_ref().map(|p| p.len()),
                        "nodes": path_opt.as_ref().map(|p| p.nodes().into_iter().map(|n| n.raw()).collect::<Vec<_>>()),
                        "edges": path_opt.as_ref().map(|p| p.edges().into_iter().map(|e| e.raw()).collect::<Vec<_>>()),
                        "path": path_opt.as_ref().map(|p| p.to_string()),
                    }))
                }
                "wcc" => {
                    let components = crate::algo::weakly_connected_components(g);
                    let formatted: Vec<Vec<u64>> = components.into_iter().map(|set| set.into_iter().map(|id| id.raw()).collect()).collect();
                    Ok(json!({ "algorithm": "weakly_connected_components", "component_count": formatted.len(), "clusters": formatted }))
                }
                "scc" => {
                    let components = crate::algo::strongly_connected_components(g);
                    let formatted: Vec<Vec<u64>> = components.into_iter().map(|set| set.into_iter().map(|id| id.raw()).collect()).collect();
                    Ok(json!({ "algorithm": "strongly_connected_components", "component_count": formatted.len(), "components": formatted }))
                }
                "degree_centrality" => {
                    let dir_str = params.get("direction").and_then(|v| v.as_str()).unwrap_or("both");
                    let dir = match dir_str.to_lowercase().as_str() {
                        "outgoing" => Direction::Outgoing,
                        "incoming" => Direction::Incoming,
                        _ => Direction::Both,
                    };
                    let degrees = crate::algo::degree_centrality(g, dir);
                    let mut sorted: Vec<(u64, usize)> = degrees.into_iter().map(|(id, deg)| (id.raw(), deg)).collect();
                    sorted.sort_by(|a, b| b.1.cmp(&a.1));
                    Ok(json!({ "algorithm": "degree_centrality", "direction": dir_str, "top_nodes": sorted }))
                }
                other => Err(crate::error::GraphError::InvalidOperation(format!("Unsupported algorithm: '{other}'"))),
            }
        };

        match &self.backend {
            McpBackend::Graph(g_lock) => {
                let g = match g_lock.read() {
                    Ok(guard) => guard,
                    Err(e) => return ToolCallResult::error(format!("Lock acquisition failed: {e}")),
                };
                match execute_algo(&g) {
                    Ok(v) => ToolCallResult::text(serde_json::to_string_pretty(&v).unwrap_or_default()),
                    Err(e) => ToolCallResult::error(format!("Algorithm execution error: {e}")),
                }
            }
            McpBackend::Database(db) => {
                match db.graph().with_read(|g| execute_algo(g)) {
                    Ok(v) => ToolCallResult::text(serde_json::to_string_pretty(&v).unwrap_or_default()),
                    Err(e) => ToolCallResult::error(format!("Algorithm execution error: {e}")),
                }
            }
            McpBackend::Remote(_) => {
                ToolCallResult::error("Algorithm execution requires embedded graph or database backend")
            }
        }
    }

    fn read_resource(&self, uri: &str) -> Result<ResourceContent> {
        match uri {
            "rustygraph://schema" => {
                let schema_res = self.tool_get_schema();
                let text = schema_res.content.into_iter().map(|c| c.text).collect::<Vec<_>>().join("\n");
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text,
                })
            }
            "rustygraph://stats" => {
                let stats_json = match &self.backend {
                    McpBackend::Graph(g_lock) => {
                        let g = g_lock.read().unwrap();
                        serde_json::to_string_pretty(&g.stats()).unwrap_or_default()
                    }
                    McpBackend::Database(db) => {
                        serde_json::to_string_pretty(&db.stats()).unwrap_or_default()
                    }
                    McpBackend::Remote(url) => {
                        http_get(url, "/stats").map(|(_, text)| text).unwrap_or_default()
                    }
                };
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: stats_json,
                })
            }
            other => Err(crate::error::GraphError::InvalidOperation(format!("Unknown resource URI: '{other}'"))),
        }
    }

    /// Runs the standard line-delimited JSON-RPC 2.0 stdio event loop.
    pub fn run_stdio(&self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                Ok(req) => {
                    if let Some(resp) = self.handle_request(&req) {
                        let serialized = serde_json::to_string(&resp).unwrap();
                        stdout.write_all(serialized.as_bytes())?;
                        stdout.write_all(b"\n")?;
                        stdout.flush()?;
                    }
                }
                Err(err) => {
                    let err_resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {err}"));
                    let serialized = serde_json::to_string(&err_resp).unwrap();
                    stdout.write_all(serialized.as_bytes())?;
                    stdout.write_all(b"\n")?;
                    stdout.flush()?;
                }
            }
        }

        Ok(())
    }
}

// Lightweight zero-dependency HTTP client functions for remote backend
fn http_get(server_url: &str, path: &str) -> io::Result<(u16, String)> {
    let (host, port) = parse_url(server_url)?;
    let mut stream = TcpStream::connect(format!("{host}:{port}"))?;

    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes())?;
    read_http_response(stream)
}

fn http_post(server_url: &str, path: &str, body: &str) -> io::Result<(u16, String)> {
    let (host, port) = parse_url(server_url)?;
    let mut stream = TcpStream::connect(format!("{host}:{port}"))?;

    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes())?;
    read_http_response(stream)
}

fn parse_url(url: &str) -> io::Result<(String, u16)> {
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let parts: Vec<&str> = stripped.split(':').collect();
    let host = parts[0].to_string();
    let port = if parts.len() > 1 {
        parts[1]
            .split('/')
            .next()
            .unwrap_or("8000")
            .parse::<u16>()
            .unwrap_or(8000)
    } else {
        8000
    };

    Ok((host, port))
}

fn read_http_response(mut stream: TcpStream) -> io::Result<(u16, String)> {
    let mut raw_response = Vec::new();
    stream.read_to_end(&mut raw_response)?;

    let resp_str = String::from_utf8_lossy(&raw_response);
    let mut lines = resp_str.split("\r\n");

    let status_line = lines.next().unwrap_or("");
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    let body = if let Some(idx) = resp_str.find("\r\n\r\n") {
        resp_str[idx + 4..].to_string()
    } else {
        String::new()
    };

    Ok((status_code, body))
}

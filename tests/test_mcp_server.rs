use rusty_graph::mcp::McpServer;
use rusty_graph::mcp::protocol::*;
use rusty_graph::prelude::*;
use serde_json::json;
use std::sync::{Arc, RwLock};

fn create_test_mcp_server() -> McpServer {
    let graph = Graph::new();
    McpServer::with_graph(Arc::new(RwLock::new(graph)))
}

#[test]
fn test_mcp_initialize_and_ping() {
    let server = create_test_mcp_server();

    // 1. Initialize request
    let init_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0.0" }
        })),
    };

    let resp = server.handle_request(&init_req).expect("Response expected");
    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!(1)));
    assert!(resp.error.is_none());

    let result = resp.result.expect("Result expected");
    assert_eq!(
        result.get("protocolVersion").and_then(|v| v.as_str()),
        Some("2024-11-05")
    );
    assert_eq!(
        result
            .get("serverInfo")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str()),
        Some("rustygraph-mcp")
    );

    // 2. Notification (notifications/initialized) must produce None
    let notif_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: "notifications/initialized".to_string(),
        params: None,
    };
    assert!(server.handle_request(&notif_req).is_none());

    // 3. Ping request
    let ping_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "ping".to_string(),
        params: None,
    };
    let ping_resp = server
        .handle_request(&ping_req)
        .expect("Ping response expected");
    assert_eq!(ping_resp.id, Some(json!(2)));
    assert!(ping_resp.result.is_some());
}

#[test]
fn test_mcp_tools_list() {
    let server = create_test_mcp_server();

    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(10)),
        method: "tools/list".to_string(),
        params: None,
    };

    let resp = server.handle_request(&list_req).expect("Response expected");
    let result = resp.result.expect("Result expected");
    let tools = result
        .get("tools")
        .and_then(|v| v.as_array())
        .expect("tools array");

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(tool_names.contains(&"cypher_query"));
    assert!(tool_names.contains(&"vector_search"));
    assert!(tool_names.contains(&"graph_rag"));
    assert!(tool_names.contains(&"fulltext_search"));
    assert!(tool_names.contains(&"get_schema"));
    assert!(tool_names.contains(&"run_algorithm"));
}

#[test]
fn test_mcp_tool_call_cypher_query() {
    let server = create_test_mcp_server();

    // 1. Create nodes via openCypher
    let create_call = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(20)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "cypher_query",
            "arguments": {
                "query": "CREATE (p:Person {name: 'Alice', age: 30})-[:KNOWS]->(b:Person {name: 'Bob', age: 28})"
            }
        })),
    };
    let create_resp = server
        .handle_request(&create_call)
        .expect("Response expected");
    let create_res_val = create_resp.result.expect("Result expected");
    assert_eq!(create_res_val.get("isError"), Some(&json!(false)));

    // 2. Query matched nodes
    let query_call = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(21)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "cypher_query",
            "arguments": {
                "query": "MATCH (p:Person)-[:KNOWS]->(b:Person) RETURN p.name, b.name"
            }
        })),
    };
    let query_resp = server
        .handle_request(&query_call)
        .expect("Response expected");
    let query_res_val = query_resp.result.expect("Result expected");
    let text = query_res_val
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .expect("text content");

    assert!(text.contains("Alice"));
    assert!(text.contains("Bob"));

    // 3. MERGE Upsert query
    let merge_call = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(22)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "cypher_query",
            "arguments": {
                "query": "MERGE (u:Person {name: 'Alice'}) ON MATCH SET u.visited = 1 RETURN u.name, u.visited"
            }
        })),
    };
    let merge_resp = server
        .handle_request(&merge_call)
        .expect("Response expected");
    let merge_res_val = merge_resp.result.expect("Result expected");
    assert_eq!(merge_res_val.get("isError"), Some(&json!(false)));
}

#[test]
fn test_mcp_tool_call_vector_search_and_graph_rag() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    {
        let mut g = graph.write().unwrap();
        g.create_vector_index("papers", "Paper", "vec", 4, VectorSimilarity::Cosine)
            .unwrap();
        let p1 = g
            .create_node_with(|n| {
                n.add_label("Paper");
                n.set_prop("title", "Attention is All You Need");
                n.set_prop("vec", vec![1.0f32, 0.0, 0.0, 0.0]);
            })
            .unwrap();
        let p2 = g
            .create_node_with(|n| {
                n.add_label("Paper");
                n.set_prop("title", "BERT");
                n.set_prop("vec", vec![0.0f32, 1.0, 0.0, 0.0]);
            })
            .unwrap();
        g.create_edge(p1, p2, "CITES").unwrap();
    }

    let server = McpServer::with_graph(graph);

    // 1. Vector Search
    let v_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(30)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "vector_search",
            "arguments": {
                "index": "papers",
                "vector": [0.99, 0.05, 0.0, 0.0],
                "k": 1
            }
        })),
    };
    let v_resp = server.handle_request(&v_req).expect("Response expected");
    let v_res_val = v_resp.result.expect("Result expected");
    let v_text = v_res_val.get("content").unwrap().as_array().unwrap()[0]
        .get("text")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(v_text.contains("Attention is All You Need"));

    // 2. GraphRAG
    let rag_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(31)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "graph_rag",
            "arguments": {
                "index": "papers",
                "vector": [1.0, 0.0, 0.0, 0.0],
                "k": 1,
                "hops": 2
            }
        })),
    };
    let rag_resp = server.handle_request(&rag_req).expect("Response expected");
    let rag_res_val = rag_resp.result.expect("Result expected");
    let rag_text = rag_res_val.get("content").unwrap().as_array().unwrap()[0]
        .get("text")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(rag_text.contains("Attention is All You Need"));
    assert!(rag_text.contains("expanded_context_paths"));
}

#[test]
fn test_mcp_tool_call_fulltext_and_schema() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    {
        let mut g = graph.write().unwrap();
        g.create_fulltext_index("kb", "Doc", "content").unwrap();
        g.create_node_with(|n| {
            n.add_label("Doc");
            n.set_prop(
                "content",
                "RustyGraph is a high-performance native graph database in Rust",
            );
        })
        .unwrap();
    }

    let server = McpServer::with_graph(graph);

    // 1. Fulltext Search
    let ft_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(40)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "fulltext_search",
            "arguments": {
                "index": "kb",
                "query": "native graph database",
                "top_k": 5
            }
        })),
    };
    let ft_resp = server.handle_request(&ft_req).expect("Response expected");
    let ft_res_val = ft_resp.result.expect("Result expected");
    let ft_text = ft_res_val.get("content").unwrap().as_array().unwrap()[0]
        .get("text")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(ft_text.contains("relevance_score"));
    assert!(ft_text.contains("RustyGraph"));

    // 2. Get Schema
    let schema_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(41)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "get_schema",
            "arguments": {}
        })),
    };
    let schema_resp = server
        .handle_request(&schema_req)
        .expect("Response expected");
    let schema_res_val = schema_resp.result.expect("Result expected");
    let schema_text = schema_res_val.get("content").unwrap().as_array().unwrap()[0]
        .get("text")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(schema_text.contains("nodes_total"));
    assert!(schema_text.contains("Doc"));

    // 3. Resources List & Read
    let r_list = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(42)),
        method: "resources/list".to_string(),
        params: None,
    };
    let r_resp = server.handle_request(&r_list).expect("Response expected");
    let r_res_val = r_resp.result.expect("Result expected");
    let r_arr = r_res_val
        .get("resources")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(r_arr.len(), 2);

    let r_read = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(43)),
        method: "resources/read".to_string(),
        params: Some(json!({ "uri": "rustygraph://stats" })),
    };
    let r_read_resp = server.handle_request(&r_read).expect("Response expected");
    let r_read_res_val = r_read_resp.result.expect("Result expected");
    let contents = r_read_res_val
        .get("contents")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(contents.len(), 1);
    assert_eq!(
        contents[0].get("uri").unwrap().as_str(),
        Some("rustygraph://stats")
    );
}

#[test]
fn test_mcp_tool_call_run_algorithm() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let (a, _b, c) = {
        let mut g = graph.write().unwrap();
        let a = g.create_node();
        let b = g.create_node();
        let c = g.create_node();
        g.create_edge(a, b, "LINK").unwrap();
        g.create_edge(b, c, "LINK").unwrap();
        (a, b, c)
    };

    let server = McpServer::with_graph(graph);

    // 1. PageRank
    let pr_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(50)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "run_algorithm",
            "arguments": {
                "algorithm": "pagerank",
                "params": { "damping_factor": 0.85 }
            }
        })),
    };
    let pr_resp = server.handle_request(&pr_req).expect("Response expected");
    let pr_res_val = pr_resp.result.expect("Result expected");
    let pr_text = pr_res_val.get("content").unwrap().as_array().unwrap()[0]
        .get("text")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(pr_text.contains("top_ranked_nodes"));

    // 2. Betweenness Centrality (Node B is the bridge)
    let bc_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(51)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "run_algorithm",
            "arguments": {
                "algorithm": "betweenness_centrality",
                "params": { "normalized": false }
            }
        })),
    };
    let bc_resp = server.handle_request(&bc_req).expect("Response expected");
    let bc_res_val = bc_resp.result.expect("Result expected");
    let bc_text = bc_res_val.get("content").unwrap().as_array().unwrap()[0]
        .get("text")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(bc_text.contains("top_nodes"));

    // 3. Shortest Path BFS from A to C
    let sp_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(52)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "run_algorithm",
            "arguments": {
                "algorithm": "shortest_path",
                "params": {
                    "source": a.raw(),
                    "target": c.raw()
                }
            }
        })),
    };
    let sp_resp = server.handle_request(&sp_req).expect("Response expected");
    let sp_res_val = sp_resp.result.expect("Result expected");
    let sp_text = sp_res_val.get("content").unwrap().as_array().unwrap()[0]
        .get("text")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(sp_text.contains(&format!("\"source\": {}", a.raw())));
    assert!(sp_text.contains(&format!("\"target\": {}", c.raw())));
    assert!(sp_text.contains("\"length\": 2"));
    assert!(sp_text.contains(&format!("({})", a.raw())));
}

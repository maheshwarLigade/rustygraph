//! C FFI Foreign Function Interface
//!
//! Exposes a standardized C-compatible ABI (`extern "C"`) so that Go, Java, Python, C#, C/C++,
//! and other languages can link and embed `rusty_graph` directly in-process with zero network overhead.

use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::value::PropertyValue;
use crate::vector::VectorSimilarity;
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, CStr, CString};

/// Helper to safely convert a C string pointer to a Rust string slice.
unsafe fn c_str_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(ptr).to_str().ok() }
    }
}

/// Helper to convert a Rust string into a newly allocated C string pointer.
fn str_to_c_string(s: String) -> *mut c_char {
    CString::new(s)
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[derive(Serialize)]
struct CypherJsonResponse {
    pub success: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<PropertyValue>>,
    pub error: Option<String>,
}

#[derive(Serialize)]
struct VectorHitJson {
    pub node_id: u64,
    pub score: f32,
}

// =============================================================================
// Graph Lifecycle Management
// =============================================================================

/// Create a new in-memory Graph instance. Returns an opaque pointer to the Graph.
/// Must be freed using `rusty_graph_free`.
#[unsafe(no_mangle)]
pub extern "C" fn rusty_graph_new() -> *mut Graph {
    let graph = Box::new(Graph::new());
    Box::into_raw(graph)
}

/// Free a Graph instance previously allocated by `rusty_graph_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_free(ptr: *mut Graph) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Free a C-string returned by `rusty_graph` functions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

/// Returns the total number of active nodes in the graph.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_node_count(ptr: *const Graph) -> u64 {
    if ptr.is_null() {
        return 0;
    }
    unsafe { (*ptr).node_count() as u64 }
}

/// Returns the total number of active relationships in the graph.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_edge_count(ptr: *const Graph) -> u64 {
    if ptr.is_null() {
        return 0;
    }
    unsafe { (*ptr).edge_count() as u64 }
}

// =============================================================================
// Node & Edge Operations
// =============================================================================

/// Create a node with optional JSON labels (array of strings) and JSON properties (object).
/// Returns the 64-bit Node ID, or 0 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_create_node(
    ptr: *mut Graph,
    labels_json: *const c_char,
    props_json: *const c_char,
) -> u64 {
    if ptr.is_null() {
        return 0;
    }
    let graph = unsafe { &mut *ptr };

    let labels: Vec<String> = unsafe { c_str_to_str(labels_json) }
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let properties: HashMap<String, PropertyValue> = unsafe { c_str_to_str(props_json) }
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let res = graph.create_node_with(|n| {
        for l in labels {
            n.add_label(l);
        }
        for (k, v) in properties {
            n.set_prop(k, v);
        }
    });

    match res {
        Ok(node_id) => node_id.raw(),
        Err(_) => 0,
    }
}

/// Delete a node by ID. Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_delete_node(ptr: *mut Graph, node_id: u64) -> c_int {
    if ptr.is_null() {
        return -1;
    }
    let graph = unsafe { &mut *ptr };
    match graph.delete_node(NodeId(node_id)) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Create a directed relationship from `source` to `target` with a given type and properties JSON.
/// Returns the 64-bit Rel ID, or 0 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_create_edge(
    ptr: *mut Graph,
    source: u64,
    target: u64,
    rel_type: *const c_char,
    props_json: *const c_char,
) -> u64 {
    if ptr.is_null() {
        return 0;
    }
    let graph = unsafe { &mut *ptr };

    let type_str = match unsafe { c_str_to_str(rel_type) } {
        Some(s) => s,
        None => return 0,
    };

    let properties: HashMap<String, PropertyValue> = unsafe { c_str_to_str(props_json) }
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let res = graph.create_edge_with(NodeId(source), NodeId(target), type_str, |r| {
        for (k, v) in properties {
            r.set_prop(k, v);
        }
    });

    match res {
        Ok(rel_id) => rel_id.raw(),
        Err(_) => 0,
    }
}

/// Delete a relationship by ID. Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_delete_edge(ptr: *mut Graph, rel_id: u64) -> c_int {
    if ptr.is_null() {
        return -1;
    }
    let graph = unsafe { &mut *ptr };
    match graph.delete_edge(RelId(rel_id)) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

// =============================================================================
// Cypher Query Language Execution
// =============================================================================

/// Execute a Cypher query string against the graph.
/// Returns a JSON string formatted as `{"success": bool, "columns": [...], "rows": [...], "error": ...}`.
/// The caller must free the returned string using `rusty_graph_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_cypher(
    ptr: *mut Graph,
    query: *const c_char,
) -> *mut c_char {
    if ptr.is_null() {
        let err_resp = CypherJsonResponse {
            success: false,
            columns: Vec::new(),
            rows: Vec::new(),
            error: Some("Null graph pointer provided".to_string()),
        };
        return str_to_c_string(serde_json::to_string(&err_resp).unwrap());
    }

    let graph = unsafe { &mut *ptr };
    let query_str = match unsafe { c_str_to_str(query) } {
        Some(s) => s,
        None => {
            let err_resp = CypherJsonResponse {
                success: false,
                columns: Vec::new(),
                rows: Vec::new(),
                error: Some("Null query string provided".to_string()),
            };
            return str_to_c_string(serde_json::to_string(&err_resp).unwrap());
        }
    };

    match graph.cypher(query_str) {
        Ok(result) => {
            let resp = CypherJsonResponse {
                success: true,
                columns: result.columns,
                rows: result.rows,
                error: None,
            };
            str_to_c_string(serde_json::to_string(&resp).unwrap())
        }
        Err(e) => {
            let resp = CypherJsonResponse {
                success: false,
                columns: Vec::new(),
                rows: Vec::new(),
                error: Some(e.to_string()),
            };
            str_to_c_string(serde_json::to_string(&resp).unwrap())
        }
    }
}

// =============================================================================
// Vector Indexing & Search
// =============================================================================

/// Create a vector index on the graph.
/// `metric`: 0 = Cosine, 1 = Euclidean, 2 = DotProduct.
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_create_vector_index(
    ptr: *mut Graph,
    name: *const c_char,
    label: *const c_char,
    prop_key: *const c_char,
    dimension: usize,
    metric: u32,
) -> c_int {
    if ptr.is_null() {
        return -1;
    }
    let graph = unsafe { &mut *ptr };

    let index_name = match unsafe { c_str_to_str(name) } {
        Some(s) => s,
        None => return -1,
    };
    let index_label = match unsafe { c_str_to_str(label) } {
        Some(s) => s,
        None => return -1,
    };
    let index_prop = match unsafe { c_str_to_str(prop_key) } {
        Some(s) => s,
        None => return -1,
    };

    let sim = match metric {
        0 => VectorSimilarity::Cosine,
        1 => VectorSimilarity::Euclidean,
        2 => VectorSimilarity::DotProduct,
        _ => return -1,
    };

    match graph.create_vector_index(index_name, index_label, index_prop, dimension, sim) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Query a vector index for top-k nearest neighbors matching query vector.
/// Returns a JSON string array of `[{"node_id": u64, "score": f32}, ...]`.
/// Must be freed using `rusty_graph_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_query_vector_index(
    ptr: *const Graph,
    name: *const c_char,
    k: usize,
    vec_ptr: *const f32,
    vec_len: usize,
) -> *mut c_char {
    if ptr.is_null() || vec_ptr.is_null() {
        return str_to_c_string("[]".to_string());
    }
    let graph = unsafe { &*ptr };

    let index_name = match unsafe { c_str_to_str(name) } {
        Some(s) => s,
        None => return str_to_c_string("[]".to_string()),
    };

    let query_vec = unsafe { std::slice::from_raw_parts(vec_ptr, vec_len) };

    match graph.query_vector_index(index_name, k, query_vec) {
        Ok(hits) => {
            let json_hits: Vec<VectorHitJson> = hits
                .into_iter()
                .map(|h| VectorHitJson {
                    node_id: h.node_id.raw(),
                    score: h.score,
                })
                .collect();
            str_to_c_string(serde_json::to_string(&json_hits).unwrap())
        }
        Err(_) => str_to_c_string("[]".to_string()),
    }
}

// =============================================================================
// JSON Persistence Import & Export
// =============================================================================

/// Export the graph to a JSON dump string.
/// Must be freed using `rusty_graph_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_to_json(ptr: *const Graph) -> *mut c_char {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let graph = unsafe { &*ptr };
    match graph.to_json() {
        Ok(json_str) => str_to_c_string(json_str),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Import a graph from a JSON dump string. Returns a new Graph pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_graph_from_json(json_str: *const c_char) -> *mut Graph {
    let s = match unsafe { c_str_to_str(json_str) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    match Graph::from_json(s) {
        Ok(graph) => Box::into_raw(Box::new(graph)),
        Err(_) => std::ptr::null_mut(),
    }
}

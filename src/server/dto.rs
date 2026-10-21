use crate::value::PropertyValue;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct CypherRequest {
    pub query: String,
    #[serde(default)]
    pub params: HashMap<String, PropertyValue>,
}

#[derive(Debug, Deserialize)]
pub struct TxBeginRequest {
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNodeRequest {
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEdgeRequest {
    pub source: u64,
    pub target: u64,
    pub rel_type: String,
    #[serde(default)]
    pub properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVectorIndexRequest {
    pub name: String,
    pub label: String,
    pub prop_key: String,
    pub dimension: usize,
    pub similarity: Option<String>, // "cosine", "euclidean", "dot"
}

#[derive(Debug, Deserialize)]
pub struct VectorSearchRequest {
    pub index: String,
    pub k: usize,
    pub vector: Vec<f32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFulltextIndexRequest {
    pub name: String,
    pub label: String,
    pub property_key: String,
}

#[derive(Debug, Deserialize)]
pub struct FulltextSearchRequest {
    pub index: String,
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub prefix: bool,
}

fn default_top_k() -> usize {
    10
}

#[derive(Debug, Deserialize)]
pub struct JoinClusterRequest {
    pub peer_address: String,
}

#[derive(Debug, Deserialize)]
pub struct BulkNodeItem {
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Deserialize)]
pub struct BulkNodesRequest {
    pub nodes: Vec<BulkNodeItem>,
}

#[derive(Debug, Deserialize)]
pub struct BulkEdgeItem {
    pub source: u64,
    pub target: u64,
    pub rel_type: String,
    #[serde(default)]
    pub properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Deserialize)]
pub struct BulkEdgesRequest {
    pub edges: Vec<BulkEdgeItem>,
}

#[derive(Debug, Deserialize)]
pub struct SetTtlRequest {
    pub ttl_seconds: u64,
}

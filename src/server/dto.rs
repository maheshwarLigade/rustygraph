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

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordRequest {
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignRolesRequest {
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePropertyIndexRequest {
    pub label: String,
    pub property: String,
}

#[derive(Debug, Deserialize)]
pub struct PageRankRequest {
    #[serde(default = "default_damping")]
    pub damping: f64,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
    pub seeds: Option<Vec<u64>>,
}

fn default_damping() -> f64 {
    0.85
}

fn default_max_iterations() -> usize {
    50
}

fn default_tolerance() -> f64 {
    1e-6
}

#[derive(Debug, Deserialize)]
pub struct ShortestPathRequest {
    pub source: u64,
    pub target: u64,
    #[serde(default = "default_algo_bfs")]
    pub algorithm: String, // "bfs", "dijkstra"
    #[serde(default = "default_weight_prop")]
    pub weight_property: String,
    #[serde(default = "default_direction_both")]
    pub direction: String, // "outgoing", "incoming", "both"
}

fn default_algo_bfs() -> String {
    "bfs".to_string()
}

fn default_weight_prop() -> String {
    "weight".to_string()
}

fn default_direction_both() -> String {
    "both".to_string()
}

#[derive(Debug, Deserialize)]
pub struct CentralityRequest {
    #[serde(default = "default_centrality_metric")]
    pub metric: String, // "betweenness", "degree"
    #[serde(default = "default_direction_both")]
    pub direction: String,
    #[serde(default = "default_true")]
    pub normalized: bool,
}

fn default_centrality_metric() -> String {
    "betweenness".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct CommunityRequest {
    #[serde(default = "default_community_algo")]
    pub algorithm: String, // "wcc", "scc"
}

fn default_community_algo() -> String {
    "wcc".to_string()
}

#[derive(Debug, Deserialize)]
pub struct SimilarityRequest {
    pub source: u64,
    pub target: u64,
    #[serde(default = "default_similarity_metric")]
    pub metric: String, // "jaccard", "common_neighbors"
    #[serde(default = "default_direction_both")]
    pub direction: String,
}

fn default_similarity_metric() -> String {
    "jaccard".to_string()
}

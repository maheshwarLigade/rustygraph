use crate::storage::db::DatabaseConfig;
use serde::{Deserialize, Serialize};

/// Configuration for the standalone HTTP REST server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub http_addr: String,
    pub cluster_addr: Option<String>,
    pub node_id: String,
    pub cluster_mode: String, // "cp" or "ap"
    pub peer_addrs: Vec<String>,
    pub db_config: DatabaseConfig,
    pub auth_enabled: bool,
    pub default_admin_password: Option<String>,
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
            auth_enabled: false,
            default_admin_password: Some("admin".to_string()),
        }
    }
}

use crate::error::{GraphError, Result};
use crate::storage::db::DatabaseConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

/// Helper struct for deserializing TOML/JSON configuration files with flexible sections.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    pub server: Option<ServerSection>,
    #[serde(default)]
    pub cluster: Option<ClusterSection>,
    #[serde(default)]
    pub storage: Option<StorageSection>,

    // Flat format compatibility for backward-compatibility
    pub http_addr: Option<String>,
    pub cluster_addr: Option<String>,
    pub node_id: Option<String>,
    pub cluster_mode: Option<String>,
    pub peer_addrs: Option<Vec<String>>,
    pub data_dir: Option<PathBuf>,
    pub fsync_policy: Option<String>,
    pub keep_snapshots: Option<usize>,
    pub auto_checkpoint_ops: Option<u64>,
    pub auth_enabled: Option<bool>,
    pub default_admin_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerSection {
    pub http_addr: Option<String>,
    pub auth_enabled: Option<bool>,
    pub default_admin_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterSection {
    pub node_id: Option<String>,
    pub cluster_addr: Option<String>,
    pub cluster_mode: Option<String>,
    pub peer_addrs: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageSection {
    pub data_dir: Option<PathBuf>,
    pub fsync_policy: Option<String>,
    pub keep_snapshots: Option<usize>,
    pub auto_checkpoint_ops: Option<u64>,
}

impl ServerConfig {
    /// Loads configuration through the layered configuration model:
    /// 1. Default conventions (`ServerConfig::default()`)
    /// 2. Configuration file (either explicit path or auto-discovered `./rustygraph.toml`, `./rustygraph.json`)
    /// 3. Environment variables (`RUSTYGRAPH_*`)
    pub fn load_layered(explicit_config_path: Option<&Path>) -> Result<Self> {
        let mut config = Self::default();

        // 1. Check for configuration file
        let path_to_load = if let Some(p) = explicit_config_path {
            if !p.exists() {
                return Err(GraphError::Storage(format!(
                    "Specified configuration file not found: {}",
                    p.display()
                )));
            }
            Some(p.to_path_buf())
        } else if let Ok(env_path) = env::var("RUSTYGRAPH_CONFIG") {
            let p = PathBuf::from(env_path);
            if !p.exists() {
                return Err(GraphError::Storage(format!(
                    "RUSTYGRAPH_CONFIG file not found: {}",
                    p.display()
                )));
            }
            Some(p)
        } else {
            // Conventional auto-discovery locations
            let candidates = [
                Path::new("rustygraph.toml"),
                Path::new("rustygraph.json"),
                Path::new("config/rustygraph.toml"),
                Path::new("config/rustygraph.json"),
            ];
            candidates
                .iter()
                .find(|p| p.exists())
                .map(|p| p.to_path_buf())
        };

        if let Some(ref path) = path_to_load {
            config.apply_file(path)?;
        }

        // 2. Apply environment variables
        config.apply_env();

        Ok(config)
    }

    /// Read and merge a TOML or JSON file into the config.
    pub fn apply_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).map_err(|e| {
            GraphError::Storage(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        self.apply_str(
            &content,
            path.extension().and_then(|s| s.to_str()) == Some("json"),
        )
        .map_err(|e| {
            GraphError::SerializationError(format!(
                "Failed to parse config {}: {}",
                path.display(),
                e
            ))
        })
    }

    /// Parse and apply configuration from a string (TOML or JSON).
    pub fn apply_str(&mut self, content: &str, is_json: bool) -> Result<()> {
        let parsed: ConfigFile = if is_json {
            serde_json::from_str(content)
                .map_err(|e| GraphError::SerializationError(format!("JSON parse error: {}", e)))?
        } else {
            toml::from_str(content)
                .map_err(|e| GraphError::SerializationError(format!("TOML parse error: {}", e)))?
        };

        // Merge section-based settings
        if let Some(server) = parsed.server {
            if let Some(addr) = server.http_addr {
                self.http_addr = addr;
            }
            if let Some(auth) = server.auth_enabled {
                self.auth_enabled = auth;
            }
            if let Some(pass) = server.default_admin_password {
                self.default_admin_password = Some(pass);
            }
        }

        if let Some(cluster) = parsed.cluster {
            if let Some(false) = cluster.enabled {
                self.cluster_addr = None;
            } else if let Some(addr) = cluster.cluster_addr {
                self.cluster_addr = Some(addr);
            }
            if let Some(id) = cluster.node_id {
                self.node_id = id;
            }
            if let Some(mode) = cluster.cluster_mode {
                self.cluster_mode = mode;
            }
            if let Some(peers) = cluster.peer_addrs {
                self.peer_addrs = peers;
            }
        }

        if let Some(storage) = parsed.storage {
            if let Some(dir) = storage.data_dir {
                self.db_config.data_dir = dir;
            }
            if let Some(fsync) = storage.fsync_policy {
                self.db_config.fsync_policy = fsync;
            }
            if let Some(keep) = storage.keep_snapshots {
                self.db_config.keep_snapshots = keep;
            }
            if let Some(checkpoint) = storage.auto_checkpoint_ops {
                self.db_config.auto_checkpoint_ops = checkpoint;
            }
        }

        // Merge flat fallback fields
        if let Some(addr) = parsed.http_addr {
            self.http_addr = addr;
        }
        if let Some(addr) = parsed.cluster_addr {
            self.cluster_addr = Some(addr);
        }
        if let Some(id) = parsed.node_id {
            self.node_id = id;
        }
        if let Some(mode) = parsed.cluster_mode {
            self.cluster_mode = mode;
        }
        if let Some(peers) = parsed.peer_addrs {
            self.peer_addrs = peers;
        }
        if let Some(dir) = parsed.data_dir {
            self.db_config.data_dir = dir;
        }
        if let Some(fsync) = parsed.fsync_policy {
            self.db_config.fsync_policy = fsync;
        }
        if let Some(keep) = parsed.keep_snapshots {
            self.db_config.keep_snapshots = keep;
        }
        if let Some(ops) = parsed.auto_checkpoint_ops {
            self.db_config.auto_checkpoint_ops = ops;
        }
        if let Some(auth) = parsed.auth_enabled {
            self.auth_enabled = auth;
        }
        if let Some(pass) = parsed.default_admin_password {
            self.default_admin_password = Some(pass);
        }

        Ok(())
    }

    /// Overrides configuration fields with environment variables if present.
    pub fn apply_env(&mut self) {
        if let Ok(val) = env::var("RUSTYGRAPH_HTTP_ADDR").or_else(|_| env::var("RUSTYGRAPH_HTTP")) {
            if !val.trim().is_empty() {
                self.http_addr = val.trim().to_string();
            }
        }

        if let Ok(val) =
            env::var("RUSTYGRAPH_CLUSTER_ADDR").or_else(|_| env::var("RUSTYGRAPH_CLUSTER"))
        {
            let trimmed = val.trim();
            if trimmed.eq_ignore_ascii_case("none")
                || trimmed.eq_ignore_ascii_case("disabled")
                || trimmed.eq_ignore_ascii_case("off")
            {
                self.cluster_addr = None;
            } else if !trimmed.is_empty() {
                self.cluster_addr = Some(trimmed.to_string());
            }
        }

        if let Ok(val) = env::var("RUSTYGRAPH_NODE_ID") {
            if !val.trim().is_empty() {
                self.node_id = val.trim().to_string();
            }
        }

        if let Ok(val) =
            env::var("RUSTYGRAPH_CLUSTER_MODE").or_else(|_| env::var("RUSTYGRAPH_MODE"))
        {
            if !val.trim().is_empty() {
                self.cluster_mode = val.trim().to_string();
            }
        }

        if let Ok(val) =
            env::var("RUSTYGRAPH_PEERS").or_else(|_| env::var("RUSTYGRAPH_CLUSTER_PEERS"))
        {
            self.peer_addrs = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Ok(val) = env::var("RUSTYGRAPH_DATA_DIR") {
            if !val.trim().is_empty() {
                self.db_config.data_dir = PathBuf::from(val.trim());
            }
        }

        if let Ok(val) =
            env::var("RUSTYGRAPH_FSYNC").or_else(|_| env::var("RUSTYGRAPH_FSYNC_POLICY"))
        {
            if !val.trim().is_empty() {
                self.db_config.fsync_policy = val.trim().to_string();
            }
        }

        if let Ok(val) = env::var("RUSTYGRAPH_KEEP_SNAPSHOTS") {
            if let Ok(parsed) = val.trim().parse::<usize>() {
                self.db_config.keep_snapshots = parsed;
            }
        }

        if let Ok(val) = env::var("RUSTYGRAPH_AUTO_CHECKPOINT_OPS") {
            if let Ok(parsed) = val.trim().parse::<u64>() {
                self.db_config.auto_checkpoint_ops = parsed;
            }
        }

        if let Ok(val) =
            env::var("RUSTYGRAPH_AUTH_ENABLED").or_else(|_| env::var("RUSTYGRAPH_AUTH"))
        {
            match val.trim().to_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => self.auth_enabled = true,
                "0" | "false" | "no" | "off" => self.auth_enabled = false,
                _ => {}
            }
        }

        if let Ok(val) =
            env::var("RUSTYGRAPH_ADMIN_PASSWORD").or_else(|_| env::var("RUSTYGRAPH_ADMIN_PASS"))
        {
            if !val.is_empty() {
                self.default_admin_password = Some(val);
            }
        }
    }

    /// Generate an exhaustive, well-commented sample `rustygraph.toml`.
    pub fn generate_sample_toml() -> &'static str {
        r#"# =============================================================================
# RustyGraph Server Configuration
# Layered Configuration Model: CLI Flags > Env Vars > Config File > Defaults
# =============================================================================

[server]
# HTTP REST API and Web Studio bind address
http_addr = "0.0.0.0:8000"

# Enable RBAC authentication and JWT token security
auth_enabled = true

# Initial root administrator password (applied if user database is uninitialized)
default_admin_password = "admin"

[cluster]
# Unique node identifier within the distributed cluster
node_id = "node-1"

# Inter-node TCP cluster consensus bind address
cluster_addr = "127.0.0.1:9001"

# Distributed cluster consensus mode:
# - "cp": Causal Clustering with strict majority quorum writes (Neo4j-style)
# - "ap": High Availability with LWW anti-entropy and full partition tolerance
cluster_mode = "cp"

# Initial seed peers to discover and join upon boot
peer_addrs = [
    # "127.0.0.1:9002",
    # "127.0.0.1:9003"
]

[storage]
# Directory where Write-Ahead Logs (WAL) and memory snapshots are persisted
data_dir = "./data"

# Durability disk synchronization policy:
# - "always": fsync on every commit / WAL mutation (maximum durability)
# - "periodic": fsync at background interval (balanced performance)
# - "never": rely entirely on OS page cache flushing (maximum throughput)
fsync_policy = "always"

# Number of historical full-graph snapshots to retain on disk
keep_snapshots = 3

# Automatic background checkpoint trigger after N write mutations
auto_checkpoint_ops = 5000
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_convention() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.http_addr, "0.0.0.0:8000");
        assert_eq!(cfg.node_id, "node-1");
        assert_eq!(cfg.cluster_mode, "cp");
        assert_eq!(cfg.db_config.fsync_policy, "always");
        assert_eq!(cfg.db_config.data_dir, PathBuf::from("./data"));
    }

    #[test]
    fn test_parse_toml_sections() -> Result<()> {
        let toml_str = r#"
[server]
http_addr = "192.168.1.100:9999"
auth_enabled = true
default_admin_password = "secret_pass_123"

[cluster]
node_id = "cluster-master-01"
cluster_addr = "192.168.1.100:9991"
cluster_mode = "ap"
peer_addrs = ["192.168.1.101:9991", "192.168.1.102:9991"]

[storage]
data_dir = "/var/lib/rustygraph"
fsync_policy = "periodic"
keep_snapshots = 5
auto_checkpoint_ops = 25000
"#;

        let mut cfg = ServerConfig::default();
        cfg.apply_str(toml_str, false)?;

        assert_eq!(cfg.http_addr, "192.168.1.100:9999");
        assert!(cfg.auth_enabled);
        assert_eq!(
            cfg.default_admin_password,
            Some("secret_pass_123".to_string())
        );
        assert_eq!(cfg.node_id, "cluster-master-01");
        assert_eq!(cfg.cluster_addr, Some("192.168.1.100:9991".to_string()));
        assert_eq!(cfg.cluster_mode, "ap");
        assert_eq!(cfg.peer_addrs.len(), 2);
        assert_eq!(cfg.db_config.data_dir, PathBuf::from("/var/lib/rustygraph"));
        assert_eq!(cfg.db_config.fsync_policy, "periodic");
        assert_eq!(cfg.db_config.keep_snapshots, 5);
        assert_eq!(cfg.db_config.auto_checkpoint_ops, 25000);

        Ok(())
    }

    #[test]
    fn test_parse_json_config() -> Result<()> {
        let json_str = r#"{
            "server": {
                "http_addr": "127.0.0.1:8088",
                "auth_enabled": true
            },
            "cluster": {
                "node_id": "json-node-99",
                "cluster_mode": "cp"
            },
            "storage": {
                "data_dir": "./json_data",
                "fsync_policy": "never"
            }
        }"#;

        let mut cfg = ServerConfig::default();
        cfg.apply_str(json_str, true)?;

        assert_eq!(cfg.http_addr, "127.0.0.1:8088");
        assert!(cfg.auth_enabled);
        assert_eq!(cfg.node_id, "json-node-99");
        assert_eq!(cfg.cluster_mode, "cp");
        assert_eq!(cfg.db_config.data_dir, PathBuf::from("./json_data"));
        assert_eq!(cfg.db_config.fsync_policy, "never");

        Ok(())
    }
}

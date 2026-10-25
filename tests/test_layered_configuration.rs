use rusty_graph::server::ServerConfig;
use std::env;
use std::fs;
use std::path::PathBuf;

fn temp_test_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "rustygraph_config_test_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    dir.push(unique);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_default_convention_configuration() {
    let cfg = ServerConfig::default();
    assert_eq!(cfg.http_addr, "0.0.0.0:8000");
    assert_eq!(cfg.node_id, "node-1");
    assert_eq!(cfg.cluster_mode, "cp");
    assert_eq!(cfg.cluster_addr, Some("127.0.0.1:9001".to_string()));
    assert_eq!(cfg.db_config.data_dir, PathBuf::from("./data"));
    assert_eq!(cfg.db_config.fsync_policy, "always");
    assert_eq!(cfg.db_config.keep_snapshots, 3);
    assert_eq!(cfg.db_config.auto_checkpoint_ops, 10_000);
}

#[test]
fn test_toml_file_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_test_dir("toml_config");
    let config_path = dir.join("rustygraph.toml");

    let toml_data = r#"
[server]
http_addr = "127.0.0.1:9999"
auth_enabled = true
default_admin_password = "supersecretpassword"

[cluster]
node_id = "cluster-seed-1"
cluster_addr = "127.0.0.1:9991"
cluster_mode = "ap"
peer_addrs = ["127.0.0.1:9992", "127.0.0.1:9993"]

[storage]
data_dir = "/tmp/rustygraph_store"
fsync_policy = "periodic"
keep_snapshots = 7
auto_checkpoint_ops = 20000
"#;
    fs::write(&config_path, toml_data)?;

    let mut cfg = ServerConfig::default();
    cfg.apply_file(&config_path)?;

    assert_eq!(cfg.http_addr, "127.0.0.1:9999");
    assert!(cfg.auth_enabled);
    assert_eq!(cfg.default_admin_password, Some("supersecretpassword".to_string()));
    assert_eq!(cfg.node_id, "cluster-seed-1");
    assert_eq!(cfg.cluster_addr, Some("127.0.0.1:9991".to_string()));
    assert_eq!(cfg.cluster_mode, "ap");
    assert_eq!(cfg.peer_addrs, vec!["127.0.0.1:9992", "127.0.0.1:9993"]);
    assert_eq!(cfg.db_config.data_dir, PathBuf::from("/tmp/rustygraph_store"));
    assert_eq!(cfg.db_config.fsync_policy, "periodic");
    assert_eq!(cfg.db_config.keep_snapshots, 7);
    assert_eq!(cfg.db_config.auto_checkpoint_ops, 20000);

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn test_json_file_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_test_dir("json_config");
    let config_path = dir.join("rustygraph.json");

    let json_data = r#"{
        "server": {
            "http_addr": "0.0.0.0:8080",
            "auth_enabled": false
        },
        "cluster": {
            "node_id": "json-node-42",
            "cluster_mode": "cp"
        },
        "storage": {
            "data_dir": "./custom_json_data",
            "fsync_policy": "never",
            "keep_snapshots": 2,
            "auto_checkpoint_ops": 15000
        }
    }"#;
    fs::write(&config_path, json_data)?;

    let mut cfg = ServerConfig::default();
    cfg.apply_file(&config_path)?;

    assert_eq!(cfg.http_addr, "0.0.0.0:8080");
    assert!(!cfg.auth_enabled);
    assert_eq!(cfg.node_id, "json-node-42");
    assert_eq!(cfg.cluster_mode, "cp");
    assert_eq!(cfg.db_config.data_dir, PathBuf::from("./custom_json_data"));
    assert_eq!(cfg.db_config.fsync_policy, "never");
    assert_eq!(cfg.db_config.keep_snapshots, 2);
    assert_eq!(cfg.db_config.auto_checkpoint_ops, 15000);

    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn test_environment_variable_precedence() {
    let mut cfg = ServerConfig::default();

    // Set env vars
    unsafe {
        env::set_var("RUSTYGRAPH_HTTP_ADDR", "0.0.0.0:9090");
        env::set_var("RUSTYGRAPH_NODE_ID", "env-node-7");
        env::set_var("RUSTYGRAPH_CLUSTER_MODE", "ap");
        env::set_var("RUSTYGRAPH_DATA_DIR", "./env_data");
        env::set_var("RUSTYGRAPH_FSYNC", "periodic");
        env::set_var("RUSTYGRAPH_AUTH_ENABLED", "true");
        env::set_var("RUSTYGRAPH_ADMIN_PASSWORD", "env_secret_pass");
        env::set_var("RUSTYGRAPH_PEERS", "10.0.0.1:9001, 10.0.0.2:9001");
    }

    cfg.apply_env();

    assert_eq!(cfg.http_addr, "0.0.0.0:9090");
    assert_eq!(cfg.node_id, "env-node-7");
    assert_eq!(cfg.cluster_mode, "ap");
    assert_eq!(cfg.db_config.data_dir, PathBuf::from("./env_data"));
    assert_eq!(cfg.db_config.fsync_policy, "periodic");
    assert!(cfg.auth_enabled);
    assert_eq!(cfg.default_admin_password, Some("env_secret_pass".to_string()));
    assert_eq!(cfg.peer_addrs, vec!["10.0.0.1:9001", "10.0.0.2:9001"]);

    // Clean up env vars
    unsafe {
        env::remove_var("RUSTYGRAPH_HTTP_ADDR");
        env::remove_var("RUSTYGRAPH_NODE_ID");
        env::remove_var("RUSTYGRAPH_CLUSTER_MODE");
        env::remove_var("RUSTYGRAPH_DATA_DIR");
        env::remove_var("RUSTYGRAPH_FSYNC");
        env::remove_var("RUSTYGRAPH_AUTH_ENABLED");
        env::remove_var("RUSTYGRAPH_ADMIN_PASSWORD");
        env::remove_var("RUSTYGRAPH_PEERS");
    }
}

#[test]
fn test_disable_cluster_via_env() {
    let mut cfg = ServerConfig::default();
    assert!(cfg.cluster_addr.is_some());

    unsafe {
        env::set_var("RUSTYGRAPH_CLUSTER_ADDR", "disabled");
    }
    cfg.apply_env();
    assert_eq!(cfg.cluster_addr, None);

    unsafe {
        env::remove_var("RUSTYGRAPH_CLUSTER_ADDR");
    }
}

#[test]
fn test_sample_toml_validity() {
    let sample = ServerConfig::generate_sample_toml();
    let mut cfg = ServerConfig::default();
    cfg.apply_str(sample, false).expect("Sample TOML must parse without error");

    assert_eq!(cfg.http_addr, "0.0.0.0:8000");
    assert_eq!(cfg.node_id, "node-1");
    assert_eq!(cfg.cluster_mode, "cp");
    assert!(cfg.auth_enabled);
    assert_eq!(cfg.default_admin_password, Some("admin".to_string()));
}

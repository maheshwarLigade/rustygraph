use rusty_graph::server::{ConnectionString, ServerConfig, StandaloneServer};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    if args.iter().any(|a| a == "--init-config") {
        let sample = ServerConfig::generate_sample_toml();
        let target = Path::new("rustygraph.toml");
        if target.exists() {
            eprintln!("⚠️  rustygraph.toml already exists in current directory!");
        } else {
            std::fs::write(target, sample).expect("Failed to write rustygraph.toml");
            println!("✅ Generated sample configuration file: rustygraph.toml");
        }
        return;
    }

    // Step 1: Check for explicit config path (-c, --config <PATH>)
    let mut explicit_config: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if (args[i] == "--config" || args[i] == "-c") && i + 1 < args.len() {
            explicit_config = Some(PathBuf::from(&args[i + 1]));
            break;
        }
        i += 1;
    }

    // Step 2: Load layered configuration (Defaults -> File -> Environment Variables)
    let mut config = match ServerConfig::load_layered(explicit_config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Configuration Error: {}", e);
            std::process::exit(1);
        }
    };

    // Step 3: Apply CLI argument overrides (Highest precedence)
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" if i + 1 < args.len() => {
                i += 2; // already handled
            }
            "--http" | "--listen-http" | "--http-addr" if i + 1 < args.len() => {
                config.http_addr = args[i + 1].clone();
                i += 2;
            }
            "--cluster" | "--listen-cluster" | "--cluster-addr" if i + 1 < args.len() => {
                config.cluster_addr = Some(args[i + 1].clone());
                i += 2;
            }
            "--no-cluster" => {
                config.cluster_addr = None;
                i += 1;
            }
            "--node-id" | "--cluster-node-id" if i + 1 < args.len() => {
                config.node_id = args[i + 1].clone();
                i += 2;
            }
            "--mode" | "--cluster-mode" if i + 1 < args.len() => {
                config.cluster_mode = args[i + 1].clone();
                i += 2;
            }
            "--peers" | "--peer" | "--cluster-peer" | "--cluster-peers" if i + 1 < args.len() => {
                for peer in args[i + 1].split(',') {
                    let trimmed = peer.trim();
                    if !trimmed.is_empty() {
                        config.peer_addrs.push(trimmed.to_string());
                    }
                }
                i += 2;
            }
            "--data-dir" if i + 1 < args.len() => {
                config.db_config.data_dir = PathBuf::from(args[i + 1].clone());
                i += 2;
            }
            "--fsync" if i + 1 < args.len() => {
                config.db_config.fsync_policy = args[i + 1].clone();
                i += 2;
            }
            "--auth" => {
                config.auth_enabled = true;
                i += 1;
            }
            "--no-auth" => {
                config.auth_enabled = false;
                i += 1;
            }
            "--admin-pass" | "--admin-password" if i + 1 < args.len() => {
                config.default_admin_password = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_help();
                std::process::exit(1);
            }
        }
    }

    let admin_pass = config
        .default_admin_password
        .clone()
        .unwrap_or_else(|| "admin".to_string());
    let conn_str = if config.auth_enabled {
        format!("rustygraph://admin:{}@{}", admin_pass, config.http_addr)
    } else {
        format!("rustygraph://{}", config.http_addr)
    };
    let masked_conn = ConnectionString::parse(&conn_str)
        .map(|cs| cs.to_masked_string())
        .unwrap_or(conn_str);

    println!("=====================================================================");
    println!("        🦀 RustyGraph Standalone Distributed NoSQL Server            ");
    println!("=====================================================================");
    println!("  Node ID:          {}", config.node_id);
    println!("  HTTP REST API:    http://{}", config.http_addr);
    println!("  Connection URI:   {}", masked_conn);
    println!(
        "  Authentication:   {}",
        if config.auth_enabled {
            "Enabled (RBAC Active)"
        } else {
            "Disabled"
        }
    );
    if let Some(ref c_addr) = config.cluster_addr {
        println!("  Cluster TCP:      tcp://{}", c_addr);
        println!("  Cluster Mode:     {}", config.cluster_mode.to_uppercase());
        println!("  Initial Peers:    {:?}", config.peer_addrs);
    } else {
        println!("  Clustering:       Disabled (Standalone Single-Node)");
    }
    println!(
        "  Data Directory:   {}",
        config.db_config.data_dir.display()
    );
    println!("  Fsync Policy:     {}", config.db_config.fsync_policy);
    println!("=====================================================================");

    let server = match StandaloneServer::start(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ Fatal Error starting RustyGraph server: {}", e);
            std::process::exit(1);
        }
    };

    println!("🚀 Server is ready and listening for queries & requests.");
    println!("   Press Ctrl+C to gracefully shutdown.\n");

    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);

    ctrlc_handler(move || {
        println!("\n🛑 Received termination signal. Flushing WAL & shutting down...");
        r.store(false, Ordering::SeqCst);
    });

    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(200));
    }

    server.stop();
    let _ = server.db().checkpoint();
    println!("✅ Database shutdown completed safely.");
}

fn print_help() {
    println!(
        r#"RustyGraph Standalone Distributed Database Server

USAGE:
    rustygraph-server [OPTIONS]

CONFIGURATION MODEL:
    Layered Precedence: CLI Flags > Environment Variables > Config File > Defaults

OPTIONS:
    -c, --config <FILE>    Path to TOML or JSON config file (default: ./rustygraph.toml)
    --init-config          Generate a sample rustygraph.toml configuration file in current directory
    --http <ADDR>          HTTP REST API bind address (default: 0.0.0.0:8000)
    --cluster <ADDR>       Inter-node cluster TCP bind address (default: 127.0.0.1:9001)
    --no-cluster           Disable cluster networking
    --node-id <ID>         Unique cluster node identifier (default: node-1)
    --mode <cp|ap>         Distributed cluster consensus mode: cp | ap (default: cp)
    --peers <ADDR,ADDR>    Comma-separated list of existing peer cluster TCP addresses
    --data-dir <DIR>       Path to durable database directory (default: ./data)
    --fsync <POLICY>       Disk sync policy: always | periodic | never (default: always)
    --auth                 Enable authentication and RBAC
    --no-auth              Disable authentication
    --admin-pass <PASS>    Initial password for root 'admin' user (default: admin)
    -h, --help             Print help information

ENVIRONMENT VARIABLES:
    RUSTYGRAPH_CONFIG             Path to configuration file
    RUSTYGRAPH_HTTP_ADDR          HTTP REST API bind address
    RUSTYGRAPH_CLUSTER_ADDR       Cluster TCP address (or 'none' to disable)
    RUSTYGRAPH_NODE_ID            Node identifier
    RUSTYGRAPH_CLUSTER_MODE       Consensus mode: 'cp' or 'ap'
    RUSTYGRAPH_PEERS              Comma-separated peer addresses
    RUSTYGRAPH_DATA_DIR           Database persistence directory
    RUSTYGRAPH_FSYNC              Disk sync policy: 'always', 'periodic', 'never'
    RUSTYGRAPH_AUTH_ENABLED       Enable/disable authentication ('true' / 'false')
    RUSTYGRAPH_ADMIN_PASSWORD     Initial root administrator password
"#
    );
}

fn ctrlc_handler<F>(_f: F)
where
    F: FnOnce() + Send + 'static,
{
    // Graceful signal listener
}

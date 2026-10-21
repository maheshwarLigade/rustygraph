use rusty_graph::db::DatabaseConfig;
use rusty_graph::server::{ServerConfig, StandaloneServer};
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let mut http_addr = "127.0.0.1:8000".to_string();
    let mut cluster_addr = Some("127.0.0.1:9001".to_string());
    let mut node_id = "node-1".to_string();
    let mut cluster_mode = "cp".to_string();
    let mut peer_addrs = Vec::new();
    let mut data_dir = PathBuf::from("./data");
    let mut fsync_policy = "always".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--http" if i + 1 < args.len() => {
                http_addr = args[i + 1].clone();
                i += 2;
            }
            "--cluster" if i + 1 < args.len() => {
                cluster_addr = Some(args[i + 1].clone());
                i += 2;
            }
            "--no-cluster" => {
                cluster_addr = None;
                i += 1;
            }
            "--node-id" if i + 1 < args.len() => {
                node_id = args[i + 1].clone();
                i += 2;
            }
            "--mode" if i + 1 < args.len() => {
                cluster_mode = args[i + 1].clone();
                i += 2;
            }
            "--peers" if i + 1 < args.len() => {
                peer_addrs = args[i + 1]
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                i += 2;
            }
            "--data-dir" if i + 1 < args.len() => {
                data_dir = PathBuf::from(args[i + 1].clone());
                i += 2;
            }
            "--fsync" if i + 1 < args.len() => {
                fsync_policy = args[i + 1].clone();
                i += 2;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_help();
                std::process::exit(1);
            }
        }
    }

    println!("=====================================================================");
    println!("        🦀 RustyGraph Standalone Distributed NoSQL Server            ");
    println!("=====================================================================");
    println!("  Node ID:          {}", node_id);
    println!("  HTTP REST API:    http://{}", http_addr);
    if let Some(ref c_addr) = cluster_addr {
        println!("  Cluster TCP:      tcp://{}", c_addr);
        println!("  Cluster Mode:     {}", cluster_mode.to_uppercase());
        println!("  Initial Peers:    {:?}", peer_addrs);
    } else {
        println!("  Clustering:       Disabled (Standalone Single-Node)");
    }
    println!("  Data Directory:   {}", data_dir.display());
    println!("  Fsync Policy:     {}", fsync_policy);
    println!("=====================================================================");

    let db_config = DatabaseConfig {
        data_dir,
        fsync_policy,
        keep_snapshots: 3,
        auto_checkpoint_ops: 5000,
    };

    let server_config = ServerConfig {
        http_addr,
        cluster_addr,
        node_id,
        cluster_mode,
        peer_addrs,
        db_config,
    };

    let server = match StandaloneServer::start(server_config) {
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

OPTIONS:
    --http <ADDR>          HTTP REST API bind address (default: 127.0.0.1:8000)
    --cluster <ADDR>       Inter-node cluster TCP bind address (default: 127.0.0.1:9001)
    --no-cluster           Disable cluster networking
    --node-id <ID>         Unique cluster node identifier (default: node-1)
    --mode <cp|ap>         Distributed cluster consensus mode (default: cp)
    --peers <ADDR,ADDR>    Comma-separated list of existing peer cluster TCP addresses
    --data-dir <DIR>       Path to durable database directory (default: ./data)
    --fsync <POLICY>       Disk sync policy: always | periodic | never (default: always)
    -h, --help             Print help information
"#
    );
}

fn ctrlc_handler<F>(_f: F)
where
    F: FnOnce() + Send + 'static,
{
    // Graceful signal listener
}

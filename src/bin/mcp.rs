use rusty_graph::mcp::{McpBackend, McpServer};
use rusty_graph::prelude::*;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut data_dir: Option<PathBuf> = None;
    let mut server_url: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--data" if i + 1 < args.len() => {
                data_dir = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--url" if i + 1 < args.len() => {
                server_url = Some(args[i + 1].clone());
                i += 2;
            }
            "--remote" => {
                server_url = Some("http://127.0.0.1:8000".to_string());
                i += 1;
            }
            "-h" | "--help" => {
                print_mcp_help();
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                print_mcp_help();
                std::process::exit(1);
            }
        }
    }

    let backend = if let Some(url) = server_url {
        eprintln!("🦀 RustyGraph MCP Server connecting to remote daemon at: {url}");
        McpBackend::Remote(url)
    } else if let Some(dir) = data_dir {
        eprintln!(
            "🦀 RustyGraph MCP Server opening persistent database at: {}",
            dir.display()
        );
        let mut config = DatabaseConfig::default();
        config.data_dir = dir;
        let db = StandaloneDatabase::open(config).expect("Failed to open persistent database");
        McpBackend::Database(Arc::new(db))
    } else {
        eprintln!("🦀 RustyGraph MCP Server initialized with in-memory graph");
        let graph = Graph::new();
        McpBackend::Graph(Arc::new(RwLock::new(graph)))
    };

    let server = McpServer::new(backend);
    eprintln!("🦀 MCP JSON-RPC 2.0 stdio server listening on stdin/stdout...");

    if let Err(e) = server.run_stdio() {
        eprintln!("MCP server loop exited with error: {e}");
        std::process::exit(1);
    }
}

fn print_mcp_help() {
    eprintln!(
        r#"🦀 RustyGraph Model Context Protocol (MCP) Server v{}

USAGE:
    rustygraph-mcp [OPTIONS]

OPTIONS:
    --data <DIR>    Open persistent standalone database backed by WAL and snapshots
    --url <URL>     Connect to remote or local running rustygraph-server daemon
    --remote        Shorthand to connect to http://127.0.0.1:8000
    -h, --help      Print this help message

SAMPLE CLAUDE DESKTOP CONFIGURATION (claude_desktop_config.json):
    {{
      "mcpServers": {{
        "rustygraph": {{
          "command": "rustygraph-mcp",
          "args": ["--data", "/path/to/rustygraph_data"]
        }}
      }}
    }}
"#,
        env!("CARGO_PKG_VERSION")
    );
}

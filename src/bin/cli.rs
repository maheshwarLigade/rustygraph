use rusty_graph::server::ConnectionString;
use std::env;
use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut server_url = "http://127.0.0.1:8000".to_string();
    let mut exec_query: Option<String> = None;
    let mut auth_token: Option<String> = None;
    let mut auth_user: Option<String> = None;
    let mut auth_pass: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--uri" | "-u" if i + 1 < args.len() => {
                let uri_str = &args[i + 1];
                if let Ok(cs) = ConnectionString::parse(uri_str) {
                    server_url = cs.to_http_url();
                    auth_user = cs.username;
                    auth_pass = cs.password;
                } else {
                    eprintln!("Invalid connection URI: {}", uri_str);
                    std::process::exit(1);
                }
                i += 2;
            }
            "--server" if i + 1 < args.len() => {
                server_url = args[i + 1].clone();
                i += 2;
            }
            "--username" | "--user" if i + 1 < args.len() => {
                auth_user = Some(args[i + 1].clone());
                i += 2;
            }
            "--password" | "--pass" if i + 1 < args.len() => {
                auth_pass = Some(args[i + 1].clone());
                i += 2;
            }
            "--exec" if i + 1 < args.len() => {
                exec_query = Some(args[i + 1].clone());
                i += 2;
            }
            "-h" | "--help" => {
                print_cli_help();
                return;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_cli_help();
                std::process::exit(1);
            }
        }
    }

    // Attempt login if credentials were provided
    if let (Some(u), Some(p)) = (&auth_user, &auth_pass) {
        let login_payload = serde_json::json!({ "username": u, "password": p }).to_string();
        if let Ok((200, resp)) = http_post(&server_url, "/auth/login", &login_payload, None) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp) {
                if let Some(token) = v.get("token").and_then(|t| t.as_str()) {
                    auth_token = Some(format!("Bearer {}", token));
                }
            }
        }
    }

    if let Some(query) = exec_query {
        execute_cypher_and_print(&server_url, &query, None, auth_token.as_deref());
        return;
    }

    // Interactive REPL Mode
    println!("=====================================================================");
    println!("            🦀 RustyGraph Interactive Cypher REPL                    ");
    println!("=====================================================================");
    println!("  Connected to: {}", server_url);
    if let Some(u) = &auth_user {
        println!("  Authenticated as: {}", u);
    }
    println!("  Type openCypher queries and press Enter.");
    println!("  Type :help for commands, :quit or :exit to leave.\n");

    let stdin = io::stdin();
    let mut input_buffer = String::new();
    let mut active_tx: Option<u64> = None;

    loop {
        if input_buffer.is_empty() {
            if let Some(tx_id) = active_tx {
                print!("rustygraph (tx:{})> ", tx_id);
            } else if let Some(u) = &auth_user {
                print!("rustygraph ({})> ", u);
            } else {
                print!("rustygraph> ");
            }
        } else {
            print!("       ...> ");
        }
        let _ = io::stdout().flush();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() || line.is_empty() {
            println!("\nGoodbye!");
            break;
        }

        let trimmed = line.trim();

        if input_buffer.is_empty() && trimmed.starts_with(':') {
            match trimmed {
                ":quit" | ":exit" | ":q" => {
                    println!("Goodbye!");
                    break;
                }
                ":help" | ":h" => {
                    print_repl_commands();
                }
                s if s.starts_with(":login") => {
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let user = parts[1];
                        let pass = parts[2];
                        let payload =
                            serde_json::json!({ "username": user, "password": pass }).to_string();
                        match http_post(&server_url, "/auth/login", &payload, None) {
                            Ok((200, resp)) => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp) {
                                    if let Some(token) = v.get("token").and_then(|t| t.as_str()) {
                                        auth_token = Some(format!("Bearer {}", token));
                                        auth_user = Some(user.to_string());
                                        println!("✅ Successfully authenticated as '{}'.", user);
                                    }
                                }
                            }
                            Ok((st, resp)) => {
                                eprintln!("❌ Authentication failed (HTTP {}): {}", st, resp)
                            }
                            Err(e) => eprintln!("❌ Network error: {}", e),
                        }
                    } else {
                        println!("Usage: :login <username> <password>");
                    }
                }
                ":whoami" => {
                    fetch_and_print_get(
                        &server_url,
                        "/auth/me",
                        "User Security Context",
                        auth_token.as_deref(),
                    );
                }
                ":users" => {
                    fetch_and_print_get(
                        &server_url,
                        "/auth/users",
                        "Registered Database Users",
                        auth_token.as_deref(),
                    );
                }
                s if s.starts_with(":begin") => {
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    let timeout_payload = if parts.len() > 1 {
                        if let Ok(ms) = parts[1].parse::<u64>() {
                            serde_json::json!({ "timeout_ms": ms }).to_string()
                        } else {
                            "{}".to_string()
                        }
                    } else {
                        "{}".to_string()
                    };
                    match http_post(
                        &server_url,
                        "/tx/begin",
                        &timeout_payload,
                        auth_token.as_deref(),
                    ) {
                        Ok((200, resp)) => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp) {
                                if let Some(id) = v.get("tx_id").and_then(|x| x.as_u64()) {
                                    active_tx = Some(id);
                                    println!(
                                        "⚡ Transaction {} started (snapshot isolation active).",
                                        id
                                    );
                                }
                            }
                        }
                        Ok((st, resp)) => {
                            eprintln!("❌ Failed to begin transaction (HTTP {}): {}", st, resp)
                        }
                        Err(e) => eprintln!("❌ Network error: {}", e),
                    }
                }
                ":commit" => {
                    if let Some(id) = active_tx {
                        match http_post(
                            &server_url,
                            &format!("/tx/{}/commit", id),
                            "{}",
                            auth_token.as_deref(),
                        ) {
                            Ok((200, _)) => {
                                println!(
                                    "✅ Transaction {} committed successfully to WAL and live database.",
                                    id
                                );
                                active_tx = None;
                            }
                            Ok((st, resp)) => {
                                eprintln!("❌ Failed to commit transaction (HTTP {}): {}", st, resp)
                            }
                            Err(e) => eprintln!("❌ Network error: {}", e),
                        }
                    } else {
                        println!("No active transaction to commit. Use :begin to start one.");
                    }
                }
                ":rollback" => {
                    if let Some(id) = active_tx {
                        match http_post(
                            &server_url,
                            &format!("/tx/{}/rollback", id),
                            "{}",
                            auth_token.as_deref(),
                        ) {
                            Ok((200, _)) => {
                                println!(
                                    "⏪ Transaction {} rolled back (staged changes discarded).",
                                    id
                                );
                                active_tx = None;
                            }
                            Ok((st, resp)) => eprintln!(
                                "❌ Failed to rollback transaction (HTTP {}): {}",
                                st, resp
                            ),
                            Err(e) => eprintln!("❌ Network error: {}", e),
                        }
                    } else {
                        println!("No active transaction to rollback.");
                    }
                }
                ":tx" => {
                    if let Some(id) = active_tx {
                        println!("Current session active transaction: {}", id);
                    } else {
                        fetch_and_print_get(
                            &server_url,
                            "/tx/list",
                            "Active Server Transactions",
                            auth_token.as_deref(),
                        );
                    }
                }
                ":stats" => {
                    fetch_and_print_get(
                        &server_url,
                        "/stats",
                        "Database Statistics",
                        auth_token.as_deref(),
                    );
                }
                ":cluster" => {
                    fetch_and_print_get(
                        &server_url,
                        "/cluster/status",
                        "Cluster Status",
                        auth_token.as_deref(),
                    );
                }
                ":checkpoint" => {
                    fetch_and_print_post(
                        &server_url,
                        "/checkpoint",
                        "{}",
                        "Triggered Checkpoint",
                        auth_token.as_deref(),
                    );
                }
                ":export" => {
                    fetch_and_print_get(
                        &server_url,
                        "/export",
                        "Graph Export",
                        auth_token.as_deref(),
                    );
                }
                ":schema" => {
                    fetch_and_print_get(
                        &server_url,
                        "/schema",
                        "Database Schema & Topology",
                        auth_token.as_deref(),
                    );
                }
                ":indexes" => {
                    fetch_and_print_get(
                        &server_url,
                        "/indexes",
                        "Active Database Indexes",
                        auth_token.as_deref(),
                    );
                }
                s if s.starts_with(":explain ") => {
                    let query = s[9..].trim();
                    let payload = serde_json::json!({ "query": query }).to_string();
                    match http_post(
                        &server_url,
                        "/cypher/explain",
                        &payload,
                        auth_token.as_deref(),
                    ) {
                        Ok((200, resp)) => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp) {
                                if let Some(tree) = v.get("plan_tree").and_then(|t| t.as_str()) {
                                    println!("\n📊 Cypher Execution Plan Tree:\n{}", tree);
                                } else {
                                    println!(
                                        "{}",
                                        serde_json::to_string_pretty(&v).unwrap_or(resp)
                                    );
                                }
                            }
                        }
                        Ok((st, resp)) => eprintln!("❌ Explain failed (HTTP {}): {}", st, resp),
                        Err(e) => eprintln!("❌ Network error: {}", e),
                    }
                }
                s if s.starts_with(":algo") => {
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    let algo_name = if parts.len() > 1 {
                        parts[1]
                    } else {
                        "pagerank"
                    };
                    match algo_name {
                        "pagerank" | "pr" => {
                            let d = parts
                                .get(2)
                                .and_then(|x| x.parse::<f64>().ok())
                                .unwrap_or(0.85);
                            let iter = parts
                                .get(3)
                                .and_then(|x| x.parse::<usize>().ok())
                                .unwrap_or(50);
                            let body = serde_json::json!({ "damping": d, "max_iterations": iter })
                                .to_string();
                            fetch_and_print_post(
                                &server_url,
                                "/algo/pagerank",
                                &body,
                                "PageRank Results",
                                auth_token.as_deref(),
                            );
                        }
                        "shortest_path" | "sp" => {
                            let src = parts
                                .get(2)
                                .and_then(|x| x.parse::<u64>().ok())
                                .unwrap_or(0);
                            let dst = parts
                                .get(3)
                                .and_then(|x| x.parse::<u64>().ok())
                                .unwrap_or(1);
                            let body = serde_json::json!({ "source": src, "target": dst, "algorithm": "bfs" }).to_string();
                            fetch_and_print_post(
                                &server_url,
                                "/algo/shortest-path",
                                &body,
                                "Shortest Path BFS",
                                auth_token.as_deref(),
                            );
                        }
                        "community" | "wcc" => {
                            let body = serde_json::json!({ "algorithm": "wcc" }).to_string();
                            fetch_and_print_post(
                                &server_url,
                                "/algo/community",
                                &body,
                                "Connected Component Communities",
                                auth_token.as_deref(),
                            );
                        }
                        "centrality" | "betweenness" => {
                            let body =
                                serde_json::json!({ "metric": "betweenness", "direction": "both" })
                                    .to_string();
                            fetch_and_print_post(
                                &server_url,
                                "/algo/centrality",
                                &body,
                                "Betweenness Centrality",
                                auth_token.as_deref(),
                            );
                        }
                        _ => println!(
                            "Unknown algorithm '{}'. Available: pagerank, shortest_path, community, centrality",
                            algo_name
                        ),
                    }
                }
                _ => {
                    println!("Unknown command: {}. Type :help for commands.", trimmed);
                }
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        input_buffer.push_str(trimmed);
        input_buffer.push(' ');

        if trimmed.ends_with(';') || (!trimmed.ends_with('\\') && !trimmed.ends_with(',')) {
            let query = input_buffer.trim().trim_end_matches(';').trim().to_string();
            input_buffer.clear();

            if !query.is_empty() {
                execute_cypher_and_print(&server_url, &query, active_tx, auth_token.as_deref());
            }
        }
    }
}

fn print_cli_help() {
    println!(
        r#"RustyGraph CLI - Interactive Terminal Client

USAGE:
    rustygraph-cli [OPTIONS]

OPTIONS:
    --uri, -u <URI>        Connection URI: rustygraph://user:pass@host:port/db
    --server <URL>         Remote HTTP REST address (default: http://127.0.0.1:8000)
    --username <USER>      Database username
    --password <PASS>      Database user password
    --exec <QUERY>         Execute a single openCypher query and exit
    -h, --help             Print help information
"#
    );
}

fn print_repl_commands() {
    println!(
        r#"
Available REPL Commands:
  :login <user> <pass> Authenticate session with username and password
  :whoami              Display current user identity, roles, and privileges
  :users               List registered database user accounts
  :begin [timeout_ms]  Start a multi-statement ACID transaction
  :commit              Commit current transaction to WAL
  :rollback            Discard current transaction changes
  :tx                  Show active transaction info or server transaction list
  :stats               Display live database node/edge counts & WAL metrics
  :schema              Display database schema, labels, and topology
  :indexes             List all active property, vector, and full-text indexes
  :explain <query>     Display visual Cypher query execution plan tree
  :algo <name> [args]  Run graph algorithms (pagerank, shortest_path, community, centrality)
  :cluster             Display distributed cluster role, term, and peer status
  :checkpoint          Force atomic snapshot and WAL truncation
  :export              Print JSON dump of full graph topology
  :help, :h            Show this help menu
  :quit, :exit, :q     Exit the REPL
"#
    );
}

fn execute_cypher_and_print(
    server_url: &str,
    query: &str,
    tx_id: Option<u64>,
    auth_header: Option<&str>,
) {
    let payload = serde_json::json!({ "query": query }).to_string();
    let path = if let Some(id) = tx_id {
        format!("/tx/{}/cypher", id)
    } else {
        "/cypher".to_string()
    };

    let start = Instant::now();
    match http_post(server_url, &path, &payload, auth_header) {
        Ok((200, body)) => {
            let elapsed = start.elapsed();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                render_tabular_cypher_output(&json, elapsed);
            } else {
                println!("{}", body);
            }
        }
        Ok((status, body)) => {
            eprintln!("❌ Query Error (HTTP {}):", status);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(err_msg) = v.get("error").and_then(|e| e.as_str()) {
                    eprintln!("   {}", err_msg);
                } else {
                    eprintln!("   {}", body);
                }
            } else {
                eprintln!("   {}", body);
            }
            eprintln!();
        }
        Err(e) => {
            eprintln!("❌ Network / Connection Error: {}", e);
        }
    }
}

fn render_tabular_cypher_output(json: &serde_json::Value, duration: std::time::Duration) {
    let columns = json
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let rows = json
        .get("rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if columns.is_empty() {
        println!("Empty result set (0 rows) in {:.2?}", duration);
        return;
    }

    let mut widths = Vec::new();
    for col in &columns {
        widths.push(col.len().max(4));
    }

    for row in &rows {
        if let Some(cols) = row.as_array() {
            for (i, val) in cols.iter().enumerate() {
                if i < widths.len() {
                    let s = format_val(val);
                    if s.len() > widths[i] {
                        widths[i] = s.len();
                    }
                }
            }
        }
    }

    print_border(&widths, "┌", "┬", "┐");

    print!("│");
    for (i, col) in columns.iter().enumerate() {
        print!(" {:<width$} │", col, width = widths[i]);
    }
    println!();

    print_border(&widths, "├", "┼", "┤");

    for row in &rows {
        if let Some(cols) = row.as_array() {
            print!("│");
            for (i, val) in cols.iter().enumerate() {
                let s = format_val(val);
                print!(" {:<width$} │", s, width = widths[i]);
            }
            println!();
        }
    }

    print_border(&widths, "└", "┴", "┘");
    println!(
        "({} {} in {:.2?})\n",
        rows.len(),
        if rows.len() == 1 { "row" } else { "rows" },
        duration
    );
}

fn print_border(widths: &[usize], left: &str, mid: &str, right: &str) {
    print!("{}", left);
    for (i, w) in widths.iter().enumerate() {
        print!("{}", "─".repeat(*w + 2));
        if i + 1 < widths.len() {
            print!("{}", mid);
        }
    }
    println!("{}", right);
}

fn format_val(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => val.to_string(),
    }
}

fn fetch_and_print_get(server_url: &str, path: &str, title: &str, auth_header: Option<&str>) {
    match http_get(server_url, path, auth_header) {
        Ok((status, body)) => {
            println!("\n--- {} (HTTP {}) ---", title, status);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else {
                println!("{}", body);
            }
            println!();
        }
        Err(e) => eprintln!("❌ Error fetching {}: {}", path, e),
    }
}

fn fetch_and_print_post(
    server_url: &str,
    path: &str,
    body: &str,
    title: &str,
    auth_header: Option<&str>,
) {
    match http_post(server_url, path, body, auth_header) {
        Ok((status, body)) => {
            println!("\n--- {} (HTTP {}) ---", title, status);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else {
                println!("{}", body);
            }
            println!();
        }
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

// Lightweight zero-dependency HTTP client with optional Authorization header
fn http_get(server_url: &str, path: &str, auth_header: Option<&str>) -> io::Result<(u16, String)> {
    let (host, port) = parse_url(server_url)?;
    let mut stream = TcpStream::connect(format!("{}:{}", host, port))?;

    let auth_line = match auth_header {
        Some(h) => format!("Authorization: {}\r\n", h),
        None => String::new(),
    };

    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\n{}Connection: close\r\n\r\n",
        path, host, port, auth_line
    );
    stream.write_all(req.as_bytes())?;

    read_http_response(stream)
}

fn http_post(
    server_url: &str,
    path: &str,
    body: &str,
    auth_header: Option<&str>,
) -> io::Result<(u16, String)> {
    let (host, port) = parse_url(server_url)?;
    let mut stream = TcpStream::connect(format!("{}:{}", host, port))?;

    let auth_line = match auth_header {
        Some(h) => format!("Authorization: {}\r\n", h),
        None => String::new(),
    };

    let req = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
        path,
        host,
        port,
        body.len(),
        auth_line,
        body
    );
    stream.write_all(req.as_bytes())?;

    read_http_response(stream)
}

fn parse_url(url: &str) -> io::Result<(String, u16)> {
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let parts: Vec<&str> = stripped.split(':').collect();
    let host = parts[0].to_string();
    let port = if parts.len() > 1 {
        parts[1]
            .split('/')
            .next()
            .unwrap_or("8000")
            .parse::<u16>()
            .unwrap_or(8000)
    } else {
        8000
    };

    Ok((host, port))
}

fn read_http_response(mut stream: TcpStream) -> io::Result<(u16, String)> {
    let mut raw_response = Vec::new();
    stream.read_to_end(&mut raw_response)?;

    let resp_str = String::from_utf8_lossy(&raw_response);
    let mut lines = resp_str.split("\r\n");

    let status_line = lines.next().unwrap_or("");
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    let body = if let Some(idx) = resp_str.find("\r\n\r\n") {
        resp_str[idx + 4..].to_string()
    } else {
        String::new()
    };

    Ok((status_code, body))
}

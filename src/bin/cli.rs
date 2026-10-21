use std::env;
use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut server_url = "http://127.0.0.1:8000".to_string();
    let mut exec_query: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--server" if i + 1 < args.len() => {
                server_url = args[i + 1].clone();
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

    if let Some(query) = exec_query {
        execute_cypher_and_print(&server_url, &query);
        return;
    }

    // Interactive REPL Mode
    println!("=====================================================================");
    println!("            🦀 RustyGraph Interactive Cypher REPL                    ");
    println!("=====================================================================");
    println!("  Connected to: {}", server_url);
    println!("  Type openCypher queries and press Enter.");
    println!("  Type :help for commands, :quit or :exit to leave.\n");

    let stdin = io::stdin();
    let mut input_buffer = String::new();

    loop {
        if input_buffer.is_empty() {
            print!("rustygraph> ");
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
                ":stats" => {
                    fetch_and_print_get(&server_url, "/stats", "Database Statistics");
                }
                ":cluster" => {
                    fetch_and_print_get(&server_url, "/cluster/status", "Cluster Status");
                }
                ":checkpoint" => {
                    fetch_and_print_post(&server_url, "/checkpoint", "{}", "Triggered Checkpoint");
                }
                ":export" => {
                    fetch_and_print_get(&server_url, "/export", "Graph Export");
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

        // If query ends with semicolon, or single line without multi-line prompt
        if trimmed.ends_with(';') || (!trimmed.ends_with('\\') && !trimmed.ends_with(',')) {
            let query = input_buffer.trim().trim_end_matches(';').trim().to_string();
            input_buffer.clear();

            if !query.is_empty() {
                execute_cypher_and_print(&server_url, &query);
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
    --server <URL>    Server HTTP URL (default: http://127.0.0.1:8000)
    --exec <QUERY>    Execute a single Cypher query and exit
    -h, --help        Print help information
"#
    );
}

fn print_repl_commands() {
    println!(
        r#"REPL Commands:
  :stats        View node counts, edge counts, and WAL sequence metrics
  :cluster      View cluster topology, leader, and active peer nodes
  :checkpoint   Force an immediate WAL flush and on-disk snapshot checkpoint
  :export       Dump entire database topology as JSON
  :help         Show this help menu
  :quit, :exit  Exit the REPL
"#
    );
}

fn execute_cypher_and_print(server_url: &str, query: &str) {
    let start = Instant::now();
    let body = serde_json::json!({ "query": query }).to_string();

    match http_post(server_url, "/cypher", &body) {
        Ok((status, response)) => {
            let duration = start.elapsed();
            if status == 200 {
                if let Ok(res_json) = serde_json::from_str::<serde_json::Value>(&response) {
                    print_cypher_table(&res_json, duration);
                } else {
                    println!("{}", response);
                }
            } else {
                eprintln!("❌ Error (HTTP {}): {}", status, response);
            }
        }
        Err(e) => {
            eprintln!("❌ Network error connecting to {}: {}", server_url, e);
        }
    }
}

fn print_cypher_table(json: &serde_json::Value, duration: std::time::Duration) {
    let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if !success {
        let err_msg = json
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        println!("❌ Cypher Error: {}", err_msg);
        return;
    }

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

    // Determine column widths
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

    // Format top border
    print_border(&widths, "┌", "┬", "┐");

    // Header
    print!("│");
    for (i, col) in columns.iter().enumerate() {
        print!(" {:<width$} │", col, width = widths[i]);
    }
    println!();

    // Separator
    print_border(&widths, "├", "┼", "┤");

    // Rows
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

    // Bottom border
    print_border(&widths, "└", "┴", "┘");
    println!("({} {} in {:.2?})\n", rows.len(), if rows.len() == 1 { "row" } else { "rows" }, duration);
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

fn fetch_and_print_get(server_url: &str, path: &str, title: &str) {
    match http_get(server_url, path) {
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

fn fetch_and_print_post(server_url: &str, path: &str, payload: &str, title: &str) {
    match http_post(server_url, path, payload) {
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

// Lightweight zero-dependency HTTP client using std::net::TcpStream
fn http_get(server_url: &str, path: &str) -> io::Result<(u16, String)> {
    let (host, port) = parse_url(server_url)?;
    let mut stream = TcpStream::connect(format!("{}:{}", host, port))?;

    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
        path, host, port
    );
    stream.write_all(req.as_bytes())?;

    read_http_response(stream)
}

fn http_post(server_url: &str, path: &str, body: &str) -> io::Result<(u16, String)> {
    let (host, port) = parse_url(server_url)?;
    let mut stream = TcpStream::connect(format!("{}:{}", host, port))?;

    let req = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        host,
        port,
        body.len(),
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

    // Body starts after empty line \r\n\r\n
    let body = if let Some(idx) = resp_str.find("\r\n\r\n") {
        resp_str[idx + 4..].to_string()
    } else {
        String::new()
    };

    Ok((status_code, body))
}

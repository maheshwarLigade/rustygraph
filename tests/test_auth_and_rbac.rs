use rusty_graph::db::DatabaseConfig;
use rusty_graph::error::Result;
use rusty_graph::server::connection::ConnectionString;
use rusty_graph::server::crypto::{
    Sha256, base64_decode, base64_encode, hash_password, hex_encode, hmac_sha256, verify_password,
};
use rusty_graph::server::{ServerConfig, StandaloneServer};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn temp_test_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "rusty_graph_auth_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    dir.push(unique);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// Minimal HTTP client helper for integration tests
fn http_request(
    method: &str,
    url: &str,
    body: &str,
    auth_header: Option<&str>,
) -> std::io::Result<(u16, String)> {
    let stripped = url.strip_prefix("http://").unwrap_or(url);
    let mut parts = stripped.split('/');
    let host_port = parts.next().unwrap();
    let path = format!("/{}", parts.collect::<Vec<_>>().join("/"));

    let mut stream = TcpStream::connect(host_port)?;

    let auth_line = match auth_header {
        Some(h) => format!("Authorization: {}\r\n", h),
        None => String::new(),
    };

    let req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
        method,
        path,
        host_port,
        body.len(),
        auth_line,
        body
    );
    stream.write_all(req.as_bytes())?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;

    let resp_str = String::from_utf8_lossy(&raw);
    let status = resp_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    let body_part = if let Some(idx) = resp_str.find("\r\n\r\n") {
        resp_str[idx + 4..].to_string()
    } else {
        String::new()
    };

    Ok((status, body_part))
}

#[test]
fn test_connection_string_parsing_and_masking() {
    // 1. Full URI with credentials, port, path, and parameters
    let uri =
        "rustygraph://admin:Secr%40tPass%21@127.0.0.1:8000/production?timeout_ms=5000&mode=cp";
    let cs = ConnectionString::parse(uri).expect("Parse valid URI");

    assert_eq!(cs.scheme, "rustygraph");
    assert_eq!(cs.username, Some("admin".to_string()));
    assert_eq!(cs.password, Some("Secr@tPass!".to_string()));
    assert_eq!(cs.host, "127.0.0.1");
    assert_eq!(cs.port, 8000);
    assert_eq!(cs.database, "production");
    assert_eq!(cs.timeout_ms(), Some(5000));
    assert_eq!(cs.to_http_url(), "http://127.0.0.1:8000");

    // Masked display must hide password
    let masked = cs.to_masked_string();
    assert!(!masked.contains("Secr@tPass!"));
    assert!(masked.contains("admin:***@"));

    // Basic Auth header generation
    let basic_hdr = cs.to_basic_auth_header().expect("Basic auth header");
    assert!(basic_hdr.starts_with("Basic "));

    // 2. TLS scheme (rustygraphs://)
    let tls_uri = "rustygraphs://app_user:pass@remote.domain:8443/db";
    let tls_cs = ConnectionString::parse(tls_uri).expect("Parse TLS URI");
    assert!(tls_cs.is_tls());
    assert_eq!(tls_cs.to_http_url(), "https://remote.domain:8443");

    // 3. Minimal HTTP URI
    let min_cs = ConnectionString::parse("http://localhost:9000").expect("Parse minimal");
    assert_eq!(min_cs.host, "localhost");
    assert_eq!(min_cs.port, 9000);
    assert_eq!(min_cs.database, "main");
}

#[test]
fn test_cryptographic_primitives() {
    // 1. SHA-256 standard NIST test vector ("abc")
    let hash = Sha256::digest(b"abc");
    assert_eq!(
        hex_encode(&hash),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    // 2. HMAC-SHA256 standard test vector
    let hmac = hmac_sha256(b"key", b"The quick brown fox jumps over the lazy dog");
    assert_eq!(
        hex_encode(&hmac),
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
    );

    // 3. PBKDF2 password hashing & verification
    let secret = "CorrectHorseBatteryStaple!";
    let hashed = hash_password(secret);
    assert!(hashed.starts_with("pbkdf2_sha256$10000$"));
    assert!(verify_password(secret, &hashed));
    assert!(!verify_password("WrongPassword123", &hashed));

    // 4. Base64 encode & decode
    let original = "admin:P@ssword123#";
    let encoded = base64_encode(original.as_bytes());
    let decoded = base64_decode(&encoded).expect("Decode valid base64");
    assert_eq!(String::from_utf8(decoded).unwrap(), original);
}

#[test]
fn test_server_authentication_and_rbac_flow() -> Result<()> {
    let dir = temp_test_dir("auth_rbac_server");
    let port = 19400 + (std::process::id() % 500);
    let http_addr = format!("127.0.0.1:{}", port);
    let admin_pass = "SuperAdminSecret1";

    let server_config = ServerConfig {
        http_addr: http_addr.clone(),
        cluster_addr: None,
        node_id: "auth-test-node".to_string(),
        cluster_mode: "cp".to_string(),
        peer_addrs: Vec::new(),
        db_config: DatabaseConfig {
            data_dir: dir.clone(),
            fsync_policy: "never".to_string(),
            keep_snapshots: 1,
            auto_checkpoint_ops: 0,
        },
        auth_enabled: true,
        default_admin_password: Some(admin_pass.to_string()),
    };

    let server = StandaloneServer::start(server_config)?;
    thread::sleep(Duration::from_millis(200));

    let base_url = format!("http://{}", http_addr);

    // 1. Public /health should work without credentials
    let (status, health_body) = http_request("GET", &format!("{}/health", base_url), "", None)?;
    assert_eq!(status, 200);
    assert!(health_body.contains("healthy"));

    // 2. Protected /cypher query without Authorization header should fail with 401 Unauthorized
    let (status, _) = http_request(
        "POST",
        &format!("{}/cypher", base_url),
        r#"{"query": "MATCH (n) RETURN count(n)"}"#,
        None,
    )?;
    assert_eq!(status, 401, "Unauthenticated request must return 401");

    // 3. Login with invalid password returns 401
    let (status, _) = http_request(
        "POST",
        &format!("{}/auth/login", base_url),
        r#"{"username": "admin", "password": "WrongPassword"}"#,
        None,
    )?;
    assert_eq!(status, 401, "Bad password must return 401");

    // 4. Login with correct admin password returns 200 and Bearer token
    let login_payload = format!(r#"{{"username": "admin", "password": "{}"}}"#, admin_pass);
    let (status, login_resp) = http_request(
        "POST",
        &format!("{}/auth/login", base_url),
        &login_payload,
        None,
    )?;
    assert_eq!(status, 200);
    let admin_login: serde_json::Value = serde_json::from_str(&login_resp).unwrap();
    let admin_token = admin_login["token"].as_str().expect("Token present");
    let admin_bearer = format!("Bearer {}", admin_token);

    // 5. Test /auth/me with admin token
    let (status, me_resp) = http_request(
        "GET",
        &format!("{}/auth/me", base_url),
        "",
        Some(&admin_bearer),
    )?;
    assert_eq!(status, 200);
    assert!(me_resp.contains("admin"));

    // 6. Admin creates two users: 'bob' (reader) and 'charlie' (editor)
    let create_bob = r#"{"username": "bob", "password": "bobpassword", "roles": ["reader"]}"#;
    let (status, _) = http_request(
        "POST",
        &format!("{}/auth/users", base_url),
        create_bob,
        Some(&admin_bearer),
    )?;
    assert_eq!(status, 200);

    let create_charlie =
        r#"{"username": "charlie", "password": "charliepassword", "roles": ["editor"]}"#;
    let (status, _) = http_request(
        "POST",
        &format!("{}/auth/users", base_url),
        create_charlie,
        Some(&admin_bearer),
    )?;
    assert_eq!(status, 200);

    // 7. Test 'bob' (Reader) permissions
    let (status, bob_login_resp) = http_request(
        "POST",
        &format!("{}/auth/login", base_url),
        r#"{"username": "bob", "password": "bobpassword"}"#,
        None,
    )?;
    assert_eq!(status, 200);
    let bob_token = serde_json::from_str::<serde_json::Value>(&bob_login_resp).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let bob_bearer = format!("Bearer {}", bob_token);

    // Bob can execute read query (MATCH) -> 200 OK
    let (status, _) = http_request(
        "POST",
        &format!("{}/cypher", base_url),
        r#"{"query": "MATCH (n) RETURN count(n)"}"#,
        Some(&bob_bearer),
    )?;
    assert_eq!(status, 200, "Reader can execute read queries");

    // Bob CANNOT execute mutating query (CREATE) -> 403 Forbidden!
    let (status, err_resp) = http_request(
        "POST",
        &format!("{}/cypher", base_url),
        r#"{"query": "CREATE (p:Person {name: 'Bob'})"}"#,
        Some(&bob_bearer),
    )?;
    assert_eq!(status, 403, "Reader cannot execute mutations");
    assert!(err_resp.contains("Permission Denied") || err_resp.contains("GraphWrite"));

    // Bob CANNOT trigger checkpoint -> 403 Forbidden!
    let (status, _) = http_request(
        "POST",
        &format!("{}/checkpoint", base_url),
        "{}",
        Some(&bob_bearer),
    )?;
    assert_eq!(status, 403, "Reader cannot trigger checkpoint");

    // 8. Test 'charlie' (Editor) permissions
    let (status, charlie_login_resp) = http_request(
        "POST",
        &format!("{}/auth/login", base_url),
        r#"{"username": "charlie", "password": "charliepassword"}"#,
        None,
    )?;
    assert_eq!(status, 200);
    let charlie_token =
        serde_json::from_str::<serde_json::Value>(&charlie_login_resp).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();
    let charlie_bearer = format!("Bearer {}", charlie_token);

    // Charlie CAN execute mutating query (CREATE) -> 200 OK!
    let (status, create_resp) = http_request(
        "POST",
        &format!("{}/cypher", base_url),
        r#"{"query": "CREATE (p:Person {name: 'Charlie'})"}"#,
        Some(&charlie_bearer),
    )?;
    assert_eq!(status, 200, "Editor can execute mutations");
    assert!(create_resp.contains("success"));

    // Charlie can read the newly created node -> 200 OK
    let (status, read_resp) = http_request(
        "POST",
        &format!("{}/cypher", base_url),
        r#"{"query": "MATCH (p:Person) RETURN p.name"}"#,
        Some(&charlie_bearer),
    )?;
    assert_eq!(status, 200);
    assert!(read_resp.contains("Charlie"));

    // Charlie CANNOT trigger server checkpoint -> 403 Forbidden!
    let (status, _) = http_request(
        "POST",
        &format!("{}/checkpoint", base_url),
        "{}",
        Some(&charlie_bearer),
    )?;
    assert_eq!(status, 403, "Editor cannot trigger checkpoint");

    // 9. HTTP Basic Auth test with admin credentials
    let basic_auth_val = base64_encode(format!("admin:{}", admin_pass).as_bytes());
    let basic_hdr = format!("Basic {}", basic_auth_val);
    let (status, _) = http_request(
        "POST",
        &format!("{}/checkpoint", base_url),
        "{}",
        Some(&basic_hdr),
    )?;
    assert_eq!(status, 200, "Basic Auth admin can trigger checkpoint");

    // 10. Clean shutdown
    server.stop();
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

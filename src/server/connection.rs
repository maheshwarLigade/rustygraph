//! Database connection string parser and URI representation for RustyGraph.
//!
//! Supports canonical URI schemes:
//! - `rustygraph://[username:password@]host[:port][/database][?params]`
//! - `rustygraphs://[username:password@]host[:port][/database][?params]` (TLS)
//! - `http://[username:password@]host[:port][/database][?params]`
//! - `https://[username:password@]host[:port][/database][?params]`

use super::crypto::base64_encode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed RustyGraph connection configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionString {
    pub scheme: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub params: HashMap<String, String>,
}

impl Default for ConnectionString {
    fn default() -> Self {
        Self {
            scheme: "rustygraph".to_string(),
            username: None,
            password: None,
            host: "127.0.0.1".to_string(),
            port: 8000,
            database: "main".to_string(),
            params: HashMap::new(),
        }
    }
}

impl ConnectionString {
    /// Parses a database connection string URI into a structured `ConnectionString`.
    pub fn parse(uri: &str) -> Result<Self, String> {
        let uri = uri.trim();
        if uri.is_empty() {
            return Err("Connection string URI cannot be empty".to_string());
        }

        // 1. Parse scheme
        let (scheme, rest) = match uri.find("://") {
            Some(idx) => {
                let s = &uri[..idx].to_lowercase();
                let rem = &uri[idx + 3..];
                (s.to_string(), rem)
            }
            None => {
                // Default to http scheme if raw host:port provided
                ("rustygraph".to_string(), uri)
            }
        };

        match scheme.as_str() {
            "rustygraph" | "rustygraphs" | "http" | "https" => {}
            _ => {
                return Err(format!(
                    "Unsupported scheme '{}'. Supported schemes: rustygraph, rustygraphs, http, https",
                    scheme
                ));
            }
        }

        // 2. Separate query params
        let (rem_path, query_str) = match rest.find('?') {
            Some(idx) => (&rest[..idx], Some(&rest[idx + 1..])),
            None => (rest, None),
        };

        // 3. Separate userinfo from host/path
        let (userinfo, host_and_path) = match rem_path.rfind('@') {
            Some(idx) => (Some(&rem_path[..idx]), &rem_path[idx + 1..]),
            None => (None, rem_path),
        };

        // 4. Parse user & password
        let (username, password) = if let Some(ui) = userinfo {
            match ui.find(':') {
                Some(idx) => {
                    let u = percent_decode(&ui[..idx])?;
                    let p = percent_decode(&ui[idx + 1..])?;
                    (Some(u), Some(p))
                }
                None => {
                    let u = percent_decode(ui)?;
                    (Some(u), None)
                }
            }
        } else {
            (None, None)
        };

        // 5. Separate host:port from database path
        let (host_port_str, db_path) = match host_and_path.find('/') {
            Some(idx) => (&host_and_path[..idx], &host_and_path[idx + 1..]),
            None => (host_and_path, ""),
        };

        // 6. Parse host & port
        let (host, port) = parse_host_port(host_port_str, &scheme)?;

        // 7. Parse database name
        let database = if db_path.is_empty() {
            "main".to_string()
        } else {
            percent_decode(db_path)?
        };

        // 8. Parse query parameters
        let mut params = HashMap::new();
        if let Some(qs) = query_str {
            for pair in qs.split('&') {
                if pair.is_empty() {
                    continue;
                }
                let mut kv = pair.splitn(2, '=');
                let key = percent_decode(kv.next().unwrap_or(""))?;
                let val = if let Some(v) = kv.next() {
                    percent_decode(v)?
                } else {
                    "true".to_string()
                };
                params.insert(key, val);
            }
        }

        Ok(Self {
            scheme,
            username,
            password,
            host,
            port,
            database,
            params,
        })
    }

    /// Whether this connection specifies TLS/HTTPS encryption.
    pub fn is_tls(&self) -> bool {
        self.scheme == "rustygraphs" || self.scheme == "https"
    }

    /// Converts the connection target into a standard HTTP(S) base URL: `http://host:port`.
    pub fn to_http_url(&self) -> String {
        let http_scheme = if self.is_tls() { "https" } else { "http" };
        format!("{}://{}:{}", http_scheme, self.host, self.port)
    }

    /// Returns a safe-for-logs representation masking the password as `***`.
    pub fn to_masked_string(&self) -> String {
        let auth_part = match (&self.username, &self.password) {
            (Some(u), Some(_)) => format!("{}:***@", percent_encode(u)),
            (Some(u), None) => format!("{}@", percent_encode(u)),
            _ => String::new(),
        };

        let mut query_part = String::new();
        if !self.params.is_empty() {
            let mut items: Vec<_> = self.params.iter().collect();
            items.sort_by_key(|&(k, _)| k);
            let qs: Vec<String> = items
                .into_iter()
                .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
                .collect();
            query_part = format!("?{}", qs.join("&"));
        }

        format!(
            "{}://{}{}:{}/{}{}",
            self.scheme,
            auth_part,
            self.host,
            self.port,
            percent_encode(&self.database),
            query_part
        )
    }

    /// Returns the HTTP Basic Authorization header value if credentials are present.
    pub fn to_basic_auth_header(&self) -> Option<String> {
        if let (Some(u), Some(p)) = (&self.username, &self.password) {
            let combined = format!("{}:{}", u, p);
            Some(format!("Basic {}", base64_encode(combined.as_bytes())))
        } else {
            None
        }
    }

    /// Gets an optional timeout parameter in milliseconds.
    pub fn timeout_ms(&self) -> Option<u64> {
        self.params
            .get("timeout_ms")
            .and_then(|v| v.parse::<u64>().ok())
    }
}

fn parse_host_port(host_port: &str, scheme: &str) -> Result<(String, u16), String> {
    if host_port.is_empty() {
        return Err("Host cannot be empty in connection string".to_string());
    }

    let default_port = match scheme {
        "rustygraphs" | "https" => 8443,
        _ => 8000,
    };

    // Support IPv6 bracket notation: [::1]:8000
    if host_port.starts_with('[') {
        if let Some(end_bracket) = host_port.find(']') {
            let ip6 = &host_port[1..end_bracket];
            let rem = &host_port[end_bracket + 1..];
            let port = if let Some(colon) = rem.find(':') {
                rem[colon + 1..]
                    .parse::<u16>()
                    .map_err(|_| format!("Invalid port number in '{}'", host_port))?
            } else {
                default_port
            };
            return Ok((ip6.to_string(), port));
        }
    }

    match host_port.rfind(':') {
        Some(idx) => {
            let h = &host_port[..idx];
            let p = host_port[idx + 1..]
                .parse::<u16>()
                .map_err(|_| format!("Invalid port number in '{}'", host_port))?;
            let host_res = if h.is_empty() { "127.0.0.1" } else { h };
            Ok((host_res.to_string(), p))
        }
        None => Ok((host_port.to_string(), default_port)),
    }
}

/// Standard URL percent-decoding.
pub fn percent_decode(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err("Incomplete percent-encoded sequence".to_string());
            }
            let h1 = match hex_val(bytes[i + 1]) {
                Some(v) => v,
                None => {
                    return Err(format!(
                        "Invalid hex in percent-encoding: %{}{}",
                        bytes[i + 1] as char,
                        bytes[i + 2] as char
                    ));
                }
            };
            let h2 = match hex_val(bytes[i + 2]) {
                Some(v) => v,
                None => {
                    return Err(format!(
                        "Invalid hex in percent-encoding: %{}{}",
                        bytes[i + 1] as char,
                        bytes[i + 2] as char
                    ));
                }
            };
            out.push((h1 << 4) | h2);
            i += 3;
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| format!("Invalid UTF-8 in decoded string: {}", e))
}

/// Standard URL percent-encoding for unreserved characters.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_connection_string() {
        let uri =
            "rustygraph://admin:P%40ssw%3Ard@192.168.1.50:8000/graph_db?timeout_ms=5000&mode=cp";
        let cs = ConnectionString::parse(uri).expect("Parse full URI");

        assert_eq!(cs.scheme, "rustygraph");
        assert_eq!(cs.username, Some("admin".to_string()));
        assert_eq!(cs.password, Some("P@ssw:rd".to_string()));
        assert_eq!(cs.host, "192.168.1.50");
        assert_eq!(cs.port, 8000);
        assert_eq!(cs.database, "graph_db");
        assert_eq!(
            cs.params.get("timeout_ms").map(|s| s.as_str()),
            Some("5000")
        );
        assert_eq!(cs.params.get("mode").map(|s| s.as_str()), Some("cp"));
        assert_eq!(cs.to_http_url(), "http://192.168.1.50:8000");

        // Verify masked display hides the password
        let masked = cs.to_masked_string();
        assert!(!masked.contains("P@ssw:rd"));
        assert!(masked.contains("admin:***@"));
    }

    #[test]
    fn test_parse_minimal_http_uri() {
        let uri = "http://localhost:9000";
        let cs = ConnectionString::parse(uri).expect("Parse minimal URI");

        assert_eq!(cs.scheme, "http");
        assert_eq!(cs.username, None);
        assert_eq!(cs.password, None);
        assert_eq!(cs.host, "localhost");
        assert_eq!(cs.port, 9000);
        assert_eq!(cs.database, "main");
    }

    #[test]
    fn test_basic_auth_header() {
        let cs = ConnectionString::parse("rustygraph://alice:secret@127.0.0.1:8000").unwrap();
        let header = cs.to_basic_auth_header().expect("Auth header present");
        assert!(header.starts_with("Basic "));
    }
}

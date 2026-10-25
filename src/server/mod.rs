//! Standalone HTTP REST and distributed cluster server for RustyGraph.
//!
//! Exposes full graph CRUD, ACID transaction endpoints, openCypher query execution,
//! NoSQL document batch processing, vector embeddings similarity search,
//! TTL expiration pruning, and distributed cluster membership over HTTP REST.

pub mod auth;
pub mod config;
pub mod connection;
pub mod crypto;
pub mod dto;
pub mod response;
pub mod router;
pub mod studio;

pub use auth::{
    AuthManager, Privilege, Role, SecurityContext, SessionToken, UserAccount, UserInfo,
};
pub use config::ServerConfig;
pub use connection::ConnectionString;
pub use dto::*;

use crate::cluster::ClusterMode;
use crate::distributed::{ClusterPeer, DistributedClusterNode};
use crate::error::{Error, Result};
use crate::storage::db::StandaloneDatabase;
use router::dispatch_route;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use tiny_http::{Header, Response, Server};

/// Standalone HTTP REST server exposing full graph, Cypher, NoSQL, and cluster APIs.
pub struct StandaloneServer {
    config: ServerConfig,
    db: Arc<StandaloneDatabase>,
    cluster: Option<Arc<DistributedClusterNode>>,
    auth: Arc<AuthManager>,
    running: Arc<AtomicBool>,
}

impl StandaloneServer {
    /// Initializes and starts the standalone HTTP and distributed cluster server.
    pub fn start(config: ServerConfig) -> Result<Arc<Self>> {
        let db = Arc::new(StandaloneDatabase::open(config.db_config.clone())?);

        let auth = Arc::new(AuthManager::new(
            config.auth_enabled,
            Some(&config.db_config.data_dir),
            config.default_admin_password.as_deref(),
        )?);

        let cluster = if let Some(ref c_addr) = config.cluster_addr {
            let mode = if config.cluster_mode.to_lowercase() == "ap" {
                ClusterMode::AP
            } else {
                ClusterMode::CP
            };

            let initial_peers = config
                .peer_addrs
                .iter()
                .enumerate()
                .map(|(i, addr)| ClusterPeer {
                    id: format!("peer-{}", i + 1),
                    address: addr.clone(),
                })
                .collect();

            let node = DistributedClusterNode::start(
                config.node_id.clone(),
                c_addr.clone(),
                mode,
                Arc::clone(&db),
                initial_peers,
            )?;
            Some(node)
        } else {
            None
        };

        let server_instance = Arc::new(Self {
            config: config.clone(),
            db: Arc::clone(&db),
            cluster,
            auth,
            running: Arc::new(AtomicBool::new(true)),
        });

        // Start HTTP Listener
        let http_server = Server::http(&config.http_addr).map_err(|e| {
            Error::Storage(format!(
                "Failed to bind HTTP server to {}: {}",
                config.http_addr, e
            ))
        })?;

        let server_worker = Arc::clone(&server_instance);
        thread::spawn(move || {
            server_worker.run_http_loop(http_server);
        });

        Ok(server_instance)
    }

    /// Stops the server.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(ref c) = self.cluster {
            c.stop();
        }
    }

    /// Returns the active server configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Returns the database handle.
    pub fn db(&self) -> Arc<StandaloneDatabase> {
        Arc::clone(&self.db)
    }

    /// Returns the authentication manager.
    pub fn auth(&self) -> Arc<AuthManager> {
        Arc::clone(&self.auth)
    }

    fn run_http_loop(&self, server: Server) {
        for mut request in server.incoming_requests() {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }

            let url = request.url().to_string();
            let method = request.method().clone();

            let mut body_bytes = Vec::new();
            let _ = request.as_reader().read_to_end(&mut body_bytes);

            let auth_header = request
                .headers()
                .iter()
                .find(|h| {
                    h.field
                        .as_str()
                        .as_str()
                        .eq_ignore_ascii_case("Authorization")
                })
                .map(|h| h.value.as_str().to_string());

            let resp = dispatch_route(
                &self.db,
                self.cluster.as_ref(),
                Some(&self.auth),
                auth_header.as_deref(),
                &method,
                &url,
                &body_bytes,
            );

            // Add standard CORS and JSON headers
            let header_cors =
                Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
            let header_methods = Header::from_bytes(
                &b"Access-Control-Allow-Methods"[..],
                &b"GET, POST, DELETE, PUT, OPTIONS"[..],
            )
            .unwrap();
            let header_headers = Header::from_bytes(
                &b"Access-Control-Allow-Headers"[..],
                &b"Content-Type, Authorization"[..],
            )
            .unwrap();
            let header_content_type =
                Header::from_bytes(&b"Content-Type"[..], resp.content_type.as_bytes()).unwrap();

            let response = Response::from_string(resp.body)
                .with_status_code(resp.status)
                .with_header(header_cors)
                .with_header(header_methods)
                .with_header(header_headers)
                .with_header(header_content_type);

            let _ = request.respond(response);
        }
    }
}

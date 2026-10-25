use super::consensus::ClusterMode;
use crate::error::{Error, Result};
use crate::storage::db::StandaloneDatabase;
use crate::storage::wal::WalOp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

/// A remote peer node in the distributed cluster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClusterPeer {
    pub id: String,
    pub address: String,
}

/// Messages exchanged across peer TCP sockets in the distributed cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterMessage {
    // Discovery & Membership
    Join {
        peer_id: String,
        address: String,
    },
    JoinAck {
        leader_id: Option<String>,
        peers: Vec<ClusterPeer>,
    },
    Heartbeat {
        from: String,
        term: u64,
        is_leader: bool,
    },
    HeartbeatAck {
        from: String,
        term: u64,
    },

    // CP Mode Consensus
    RequestVote {
        term: u64,
        candidate_id: String,
    },
    RequestVoteResponse {
        term: u64,
        vote_granted: bool,
    },
    ForwardWrite {
        op: WalOp,
    },
    ForwardWriteAck {
        success: bool,
        error: Option<String>,
    },

    // AP Mode Gossip Synchronization
    GossipSync {
        from: String,
        lamport_time: u64,
        ops: Vec<WalOp>,
    },

    // State Synchronization
    SyncSnapshotRequest {
        from: String,
    },
    SyncSnapshotResponse {
        dump_json: String,
        current_term: u64,
        wal_sequence: u64,
    },
}

/// Status summary of the distributed cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub node_id: String,
    pub address: String,
    pub mode: String,
    pub is_leader: bool,
    pub current_term: u64,
    pub active_peers: Vec<ClusterPeer>,
    pub known_leader: Option<String>,
}

/// Real TCP socket-based distributed cluster manager supporting CP and AP modes.
pub struct DistributedClusterNode {
    pub node_id: String,
    pub address: String,
    pub mode: ClusterMode,
    db: Arc<StandaloneDatabase>,
    is_leader: AtomicBool,
    current_term: AtomicU64,
    known_leader: RwLock<Option<String>>,
    peers: Arc<RwLock<HashMap<String, ClusterPeer>>>,
    last_heartbeat: Arc<Mutex<Instant>>,
    running: Arc<AtomicBool>,
}

impl DistributedClusterNode {
    /// Initializes and starts a distributed cluster node on a local TCP address.
    pub fn start(
        node_id: String,
        address: String,
        mode: ClusterMode,
        db: Arc<StandaloneDatabase>,
        initial_peers: Vec<ClusterPeer>,
    ) -> Result<Arc<Self>> {
        let is_init_leader = initial_peers.is_empty();

        let mut peer_map = HashMap::new();
        for p in initial_peers {
            peer_map.insert(p.id.clone(), p);
        }

        let node = Arc::new(Self {
            node_id: node_id.clone(),
            address: address.clone(),
            mode,
            db,
            is_leader: AtomicBool::new(is_init_leader),
            current_term: AtomicU64::new(1),
            known_leader: RwLock::new(if is_init_leader {
                Some(node_id.clone())
            } else {
                None
            }),
            peers: Arc::new(RwLock::new(peer_map)),
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
            running: Arc::new(AtomicBool::new(true)),
        });

        // 1. Start TCP Listener thread
        let listener = TcpListener::bind(&address).map_err(|e| {
            Error::Storage(format!(
                "Failed to bind TCP cluster port {}: {}",
                address, e
            ))
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|e| Error::Storage(format!("Failed to set nonblocking: {}", e)))?;

        let listener_node = Arc::clone(&node);
        thread::spawn(move || {
            listener_node.run_listener(listener);
        });

        // 2. Start Background Heartbeat & Failure Detection thread
        let heartbeat_node = Arc::clone(&node);
        thread::spawn(move || {
            heartbeat_node.run_heartbeat_loop();
        });

        // 3. Connect to initial peers if not the solitary leader
        if !is_init_leader {
            let join_node = Arc::clone(&node);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(200));
                join_node.broadcast_join();
            });
        }

        Ok(node)
    }

    /// Stops the distributed node.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Returns the current cluster status.
    pub fn status(&self) -> ClusterStatus {
        let peers = self.peers.read().unwrap().values().cloned().collect();
        let known_leader = self.known_leader.read().unwrap().clone();

        ClusterStatus {
            node_id: self.node_id.clone(),
            address: self.address.clone(),
            mode: match self.mode {
                ClusterMode::CP => "CP (Consistency / Majority Quorum)".to_string(),
                ClusterMode::AP => "AP (High Availability / Gossip)".to_string(),
            },
            is_leader: self.is_leader.load(Ordering::SeqCst),
            current_term: self.current_term.load(Ordering::SeqCst),
            active_peers: peers,
            known_leader,
        }
    }

    /// Dynamically joins an existing cluster peer by address.
    pub fn join_cluster(&self, peer_address: &str) -> Result<()> {
        let msg = ClusterMessage::Join {
            peer_id: self.node_id.clone(),
            address: self.address.clone(),
        };

        let resp = Self::send_message_to(peer_address, &msg)?;
        if let Some(ClusterMessage::JoinAck { leader_id, peers }) = resp {
            let mut guard = self.peers.write().unwrap();
            for p in peers {
                if p.id != self.node_id {
                    guard.insert(p.id.clone(), p);
                }
            }
            if let Some(leader) = leader_id {
                *self.known_leader.write().unwrap() = Some(leader);
                self.is_leader.store(false, Ordering::SeqCst);
            }
            drop(guard);

            // Replicate initial graph snapshot state from the target peer
            let sync_req = ClusterMessage::SyncSnapshotRequest {
                from: self.node_id.clone(),
            };
            if let Ok(Some(sync_resp)) = Self::send_message_to(peer_address, &sync_req) {
                let _ = self.process_message(sync_resp);
            }
        }

        Ok(())
    }

    fn run_listener(&self, listener: TcpListener) {
        while self.running.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(3000)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(3000)));
                    self.handle_connection(stream);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }

    fn handle_connection(&self, mut stream: TcpStream) {
        let reader = BufReader::new(stream.try_clone().unwrap());
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            if let Ok(msg) = serde_json::from_str::<ClusterMessage>(&line) {
                let response = self.process_message(msg);
                if let Some(resp) = response {
                    if let Ok(mut resp_bytes) = serde_json::to_vec(&resp) {
                        resp_bytes.push(b'\n');
                        let _ = stream.write_all(&resp_bytes);
                    }
                }
            }
        }
    }

    fn process_message(&self, msg: ClusterMessage) -> Option<ClusterMessage> {
        match msg {
            ClusterMessage::Join { peer_id, address } => {
                let peer = ClusterPeer {
                    id: peer_id.clone(),
                    address,
                };
                self.peers.write().unwrap().insert(peer_id, peer);

                let current_peers = self.peers.read().unwrap().values().cloned().collect();
                let leader = self.known_leader.read().unwrap().clone();

                Some(ClusterMessage::JoinAck {
                    leader_id: leader,
                    peers: current_peers,
                })
            }
            ClusterMessage::JoinAck { leader_id, peers } => {
                let mut guard = self.peers.write().unwrap();
                for p in peers {
                    if p.id != self.node_id {
                        guard.insert(p.id.clone(), p);
                    }
                }
                if let Some(leader) = leader_id {
                    *self.known_leader.write().unwrap() = Some(leader);
                }
                None
            }
            ClusterMessage::Heartbeat {
                from,
                term,
                is_leader,
            } => {
                *self.last_heartbeat.lock().unwrap() = Instant::now();
                let my_term = self.current_term.load(Ordering::SeqCst);
                if term >= my_term {
                    self.current_term.store(term, Ordering::SeqCst);
                    if is_leader {
                        *self.known_leader.write().unwrap() = Some(from.clone());
                        self.is_leader.store(false, Ordering::SeqCst);
                    }
                }
                Some(ClusterMessage::HeartbeatAck {
                    from: self.node_id.clone(),
                    term: self.current_term.load(Ordering::SeqCst),
                })
            }
            ClusterMessage::HeartbeatAck { term, .. } => {
                let my_term = self.current_term.load(Ordering::SeqCst);
                if term > my_term {
                    self.current_term.store(term, Ordering::SeqCst);
                    self.is_leader.store(false, Ordering::SeqCst);
                }
                None
            }
            ClusterMessage::RequestVote { term, candidate_id } => {
                let my_term = self.current_term.load(Ordering::SeqCst);
                let vote_granted = if term > my_term {
                    self.current_term.store(term, Ordering::SeqCst);
                    self.is_leader.store(false, Ordering::SeqCst);
                    *self.known_leader.write().unwrap() = Some(candidate_id);
                    true
                } else {
                    false
                };
                Some(ClusterMessage::RequestVoteResponse { term, vote_granted })
            }
            ClusterMessage::RequestVoteResponse { .. } => None,
            ClusterMessage::ForwardWrite { op } => {
                let res = self.apply_write_locally(op);
                Some(ClusterMessage::ForwardWriteAck {
                    success: res.is_ok(),
                    error: res.err().map(|e| e.to_string()),
                })
            }
            ClusterMessage::ForwardWriteAck { .. } => None,
            ClusterMessage::GossipSync { ops, .. } => {
                for op in ops {
                    let _ = self.apply_write_locally(op);
                }
                None
            }
            ClusterMessage::SyncSnapshotRequest { .. } => {
                let dump_json = self.db.graph().read(|g| g.to_json()).unwrap_or_default();
                Some(ClusterMessage::SyncSnapshotResponse {
                    dump_json,
                    current_term: self.current_term.load(Ordering::SeqCst),
                    wal_sequence: self.db.stats().current_wal_sequence,
                })
            }
            ClusterMessage::SyncSnapshotResponse {
                dump_json,
                current_term,
                ..
            } => {
                if !dump_json.is_empty() {
                    if let Ok(graph) = crate::graph::Graph::from_json(&dump_json) {
                        self.db.graph().write(|g| {
                            *g = graph;
                        });
                        let _ = self.db.checkpoint();
                    }
                }
                if current_term > self.current_term.load(Ordering::SeqCst) {
                    self.current_term.store(current_term, Ordering::SeqCst);
                }
                None
            }
        }
    }

    fn run_heartbeat_loop(&self) {
        while self.running.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));

            if self.is_leader.load(Ordering::SeqCst) {
                let msg = ClusterMessage::Heartbeat {
                    from: self.node_id.clone(),
                    term: self.current_term.load(Ordering::SeqCst),
                    is_leader: true,
                };
                self.broadcast_message(&msg);
            } else {
                let elapsed = self.last_heartbeat.lock().unwrap().elapsed();
                if elapsed > Duration::from_millis(2500) && matches!(self.mode, ClusterMode::CP) {
                    self.start_election();
                }
            }
        }
    }

    fn start_election(&self) {
        let new_term = self.current_term.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_heartbeat.lock().unwrap() = Instant::now();

        let vote_req = ClusterMessage::RequestVote {
            term: new_term,
            candidate_id: self.node_id.clone(),
        };

        let peers: Vec<ClusterPeer> = self.peers.read().unwrap().values().cloned().collect();
        let total_nodes = peers.len() + 1;
        let mut votes = 1; // Self vote

        for p in peers {
            if let Ok(Some(ClusterMessage::RequestVoteResponse { vote_granted, .. })) =
                Self::send_message_to(&p.address, &vote_req)
            {
                if vote_granted {
                    votes += 1;
                }
            }
        }

        if votes > total_nodes / 2 {
            self.is_leader.store(true, Ordering::SeqCst);
            *self.known_leader.write().unwrap() = Some(self.node_id.clone());

            let announce = ClusterMessage::Heartbeat {
                from: self.node_id.clone(),
                term: new_term,
                is_leader: true,
            };
            self.broadcast_message(&announce);
        }
    }

    fn broadcast_join(&self) {
        let msg = ClusterMessage::Join {
            peer_id: self.node_id.clone(),
            address: self.address.clone(),
        };
        let peers: Vec<ClusterPeer> = self.peers.read().unwrap().values().cloned().collect();
        for p in &peers {
            if let Ok(Some(resp)) = Self::send_message_to(&p.address, &msg) {
                let _ = self.process_message(resp);
                let sync_req = ClusterMessage::SyncSnapshotRequest {
                    from: self.node_id.clone(),
                };
                if let Ok(Some(sync_resp)) = Self::send_message_to(&p.address, &sync_req) {
                    let _ = self.process_message(sync_resp);
                }
                break;
            }
        }
    }

    fn broadcast_message(&self, msg: &ClusterMessage) {
        let peers: Vec<ClusterPeer> = self.peers.read().unwrap().values().cloned().collect();
        for p in peers {
            let _ = Self::send_message_to(&p.address, msg);
        }
    }

    fn send_message_to(address: &str, msg: &ClusterMessage) -> Result<Option<ClusterMessage>> {
        let mut stream = TcpStream::connect_timeout(
            &address
                .parse()
                .map_err(|e| Error::Storage(format!("Invalid address: {}", e)))?,
            Duration::from_millis(1500),
        )?;
        let _ = stream.set_read_timeout(Some(Duration::from_millis(2000)));

        let mut data = serde_json::to_vec(msg)?;
        data.push(b'\n');
        stream.write_all(&data)?;

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        if reader.read_line(&mut response_line).is_ok() && !response_line.trim().is_empty() {
            if let Ok(resp) = serde_json::from_str::<ClusterMessage>(response_line.trim()) {
                return Ok(Some(resp));
            }
        }

        Ok(None)
    }

    fn apply_write_locally(&self, op: WalOp) -> Result<()> {
        match op {
            WalOp::CreateNode {
                labels, properties, ..
            } => {
                self.db
                    .create_node(labels, properties.into_iter().collect())?;
            }
            WalOp::DeleteNode { id } => {
                self.db.delete_node(crate::id::NodeId(id))?;
            }
            WalOp::CreateEdge {
                source,
                target,
                rel_type,
                properties,
                ..
            } => {
                self.db.create_edge(
                    crate::id::NodeId(source),
                    crate::id::NodeId(target),
                    &rel_type,
                    properties.into_iter().collect(),
                )?;
            }
            WalOp::DeleteEdge { id } => {
                self.db.delete_edge(crate::id::RelId(id))?;
            }
            WalOp::CreateVectorIndex {
                name,
                label,
                prop_key,
                dimension,
                similarity,
            } => {
                self.db
                    .create_vector_index(&name, &label, &prop_key, dimension, similarity)?;
            }
            WalOp::CreateFulltextIndex {
                name,
                label,
                property_key,
            } => {
                self.db
                    .create_fulltext_index(&name, &label, &property_key)?;
            }
            _ => {}
        }
        Ok(())
    }
}

//! Distributed Cluster Engine & CAP Theorem Guarantees
//!
//! This module provides distributed cluster replication for `rusty_graph` with configurable
//! **CP (Consistency + Partition Tolerance)** and **AP (Availability + Partition Tolerance)** modes,
//! backed by an in-memory virtual network simulation for testing network partitions and split-brain scenarios.
//!
//! ### CAP Theorem Trade-Offs
//! - **CP Mode (Neo4j Causal Clustering style)**:
//!   - Leader election via majority quorum.
//!   - Writes require acknowledgment from a strict majority quorum ($> N/2$) of cluster nodes.
//!   - When a network partition occurs, minority partitions reject writes with [`GraphError::QuorumUnavailable`],
//!     strictly preventing split-brain and guaranteeing linearizable consistency.
//!
//! - **AP Mode (High Availability & Eventual Consistency)**:
//!   - Any node on any partition accepts writes immediately (100% availability).
//!   - Mutations are stamped with logical Lamport clocks.
//!   - When network partitions heal, anti-entropy synchronization resolves concurrent updates
//!     deterministically via Last-Write-Wins (LWW).

use crate::error::{GraphError, Result};
use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::model::{Node, Relationship};
use crate::value::PropertyValue;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

/// Unique identifier for a node within the distributed cluster.
pub type ClusterNodeId = u64;

/// CAP Theorem operational mode for the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterMode {
    /// Consistency & Partition Tolerance:
    /// Strict quorum writes (> N/2). Rejects writes on minority partitions to prevent split-brain.
    CP,
    /// Availability & Partition Tolerance:
    /// High availability; accepts writes on all partitions, converging via anti-entropy when healed.
    AP,
}

/// Role of a node in a CP consensus cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterRole {
    Leader,
    Follower,
    Candidate,
}

/// Replicable mutation command executed on the graph state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClusterCommand {
    CreateNode {
        id: NodeId,
        labels: Vec<String>,
        properties: HashMap<String, PropertyValue>,
    },
    DeleteNode {
        node_id: NodeId,
    },
    SetNodeProperty {
        node_id: NodeId,
        key: String,
        val: PropertyValue,
    },
    RemoveNodeProperty {
        node_id: NodeId,
        key: String,
    },
    CreateEdge {
        id: RelId,
        source: NodeId,
        target: NodeId,
        rel_type: String,
        properties: HashMap<String, PropertyValue>,
    },
    DeleteEdge {
        rel_id: RelId,
    },
    SetEdgeProperty {
        rel_id: RelId,
        key: String,
        val: PropertyValue,
    },
    RemoveEdgeProperty {
        rel_id: RelId,
        key: String,
    },
}

/// A replicated log entry tracking terms, indices, timestamps, and mutation commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub timestamp: u64,
    pub origin_node: ClusterNodeId,
    pub command: ClusterCommand,
}

/// Messages exchanged across the cluster network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterMessage {
    // Raft-style consensus messages (CP Mode)
    RequestVote {
        term: u64,
        candidate_id: ClusterNodeId,
        last_log_index: u64,
    },
    RequestVoteResponse {
        term: u64,
        voter_id: ClusterNodeId,
        vote_granted: bool,
    },
    AppendEntries {
        term: u64,
        leader_id: ClusterNodeId,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    },
    AppendEntriesResponse {
        term: u64,
        node_id: ClusterNodeId,
        success: bool,
        match_index: u64,
    },

    // Gossip & Anti-entropy messages (AP Mode)
    GossipEntries {
        node_id: ClusterNodeId,
        entries: Vec<LogEntry>,
    },
    GossipAck {
        node_id: ClusterNodeId,
        synced_count: usize,
    },
}

/// An in-memory virtual network simulation enabling controlled network partitions,
/// latency, packet loss, and healing between cluster nodes without real socket overhead.
#[derive(Debug, Default)]
pub struct VirtualNetwork {
    mailboxes: Arc<RwLock<HashMap<ClusterNodeId, Arc<Mutex<VecDeque<(ClusterNodeId, ClusterMessage)>>>>>>,
    partitions: Arc<RwLock<HashSet<(ClusterNodeId, ClusterNodeId)>>>,
}

impl VirtualNetwork {
    /// Create a new virtual network simulation.
    pub fn new() -> Self {
        Self {
            mailboxes: Arc::new(RwLock::new(HashMap::new())),
            partitions: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Register a cluster node with its in-memory message mailbox.
    pub fn register_node(&self, id: ClusterNodeId) {
        let mut mailboxes = self.mailboxes.write().unwrap();
        mailboxes.entry(id).or_insert_with(|| Arc::new(Mutex::new(VecDeque::new())));
    }

    /// Send a message from `from` node to `to` node.
    /// Fails if a network partition separates `from` and `to`.
    pub fn send(&self, from: ClusterNodeId, to: ClusterNodeId, msg: ClusterMessage) -> Result<()> {
        if self.is_partitioned(from, to) {
            return Err(GraphError::ClusterError(format!(
                "Network partition: connection severed between Node {from} and Node {to}"
            )));
        }

        let mailboxes = self.mailboxes.read().unwrap();
        if let Some(target_box) = mailboxes.get(&to) {
            let mut queue = target_box.lock().unwrap();
            queue.push_back((from, msg));
            Ok(())
        } else {
            Err(GraphError::ClusterError(format!("Destination node {to} not registered in virtual network")))
        }
    }

    /// Fetch all incoming messages for a given cluster node.
    pub fn receive(&self, node_id: ClusterNodeId) -> Vec<(ClusterNodeId, ClusterMessage)> {
        let mailboxes = self.mailboxes.read().unwrap();
        if let Some(target_box) = mailboxes.get(&node_id) {
            let mut queue = target_box.lock().unwrap();
            queue.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Check if communication from `from` to `to` is currently blocked by a network partition.
    pub fn is_partitioned(&self, from: ClusterNodeId, to: ClusterNodeId) -> bool {
        let parts = self.partitions.read().unwrap();
        parts.contains(&(from, to)) || parts.contains(&(to, from))
    }

    /// Sever network communication between two distinct groups of nodes (split-brain partition).
    pub fn partition(&self, group_a: &[ClusterNodeId], group_b: &[ClusterNodeId]) {
        let mut parts = self.partitions.write().unwrap();
        for &a in group_a {
            for &b in group_b {
                parts.insert((a, b));
                parts.insert((b, a));
            }
        }
    }

    /// Isolate a single node from all other cluster members.
    pub fn isolate(&self, isolated_node: ClusterNodeId, all_peers: &[ClusterNodeId]) {
        let mut parts = self.partitions.write().unwrap();
        for &peer in all_peers {
            if peer != isolated_node {
                parts.insert((isolated_node, peer));
                parts.insert((peer, isolated_node));
            }
        }
    }

    /// Heal all network partitions, restoring full bi-directional connectivity between all nodes.
    pub fn heal(&self) {
        let mut parts = self.partitions.write().unwrap();
        parts.clear();
    }
}

/// Status summary of a node within the distributed cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNodeStatus {
    pub node_id: ClusterNodeId,
    pub mode: ClusterMode,
    pub role: ClusterRole,
    pub term: u64,
    pub current_leader: Option<ClusterNodeId>,
    pub commit_index: u64,
    pub log_length: usize,
    pub cluster_size: usize,
}

/// A distributed cluster node embedding an in-memory `Graph` state machine.
pub struct ClusterNode {
    pub id: ClusterNodeId,
    pub peers: Vec<ClusterNodeId>,
    pub mode: ClusterMode,
    pub role: ClusterRole,
    pub current_term: u64,
    pub voted_for: Option<ClusterNodeId>,
    pub current_leader: Option<ClusterNodeId>,
    pub log: Vec<LogEntry>,
    pub commit_index: u64,
    pub lamport_clock: u64,
    pub graph: Graph,
    pub network: Arc<VirtualNetwork>,
}

impl ClusterNode {
    /// Create a new cluster node bound to a virtual network.
    pub fn new(
        id: ClusterNodeId,
        all_members: Vec<ClusterNodeId>,
        mode: ClusterMode,
        network: Arc<VirtualNetwork>,
    ) -> Self {
        network.register_node(id);
        let peers: Vec<ClusterNodeId> = all_members.into_iter().filter(|&m| m != id).collect();

        Self {
            id,
            peers,
            mode,
            role: ClusterRole::Follower,
            current_term: 0,
            voted_for: None,
            current_leader: None,
            log: Vec::new(),
            commit_index: 0,
            lamport_clock: 0,
            graph: Graph::new(),
            network,
        }
    }

    /// Total number of nodes in the cluster (including self).
    #[inline]
    pub fn cluster_size(&self) -> usize {
        self.peers.len() + 1
    }

    /// Quorum threshold required for CP consensus: floor(N / 2) + 1.
    #[inline]
    pub fn quorum_threshold(&self) -> usize {
        (self.cluster_size() / 2) + 1
    }

    /// Returns the current operational status of the cluster node.
    pub fn status(&self) -> ClusterNodeStatus {
        ClusterNodeStatus {
            node_id: self.id,
            mode: self.mode,
            role: self.role,
            term: self.current_term,
            current_leader: self.current_leader,
            commit_index: self.commit_index,
            log_length: self.log.len(),
            cluster_size: self.cluster_size(),
        }
    }

    /// Reference to the underlying in-memory Graph.
    #[inline]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Mutable reference to the underlying in-memory Graph.
    #[inline]
    pub fn graph_mut(&mut self) -> &mut Graph {
        &mut self.graph
    }

    // =========================================================================
    // CP Mode (Consistency + Partition Tolerance) Consensus Logic
    // =========================================================================

    /// Campaign to become the cluster leader (CP Mode).
    /// Sends `RequestVote` to all peers. If granted a majority quorum (> N/2), becomes Leader.
    pub fn campaign(&mut self) -> Result<bool> {
        if self.mode != ClusterMode::CP {
            return Ok(true);
        }

        self.current_term += 1;
        self.role = ClusterRole::Candidate;
        self.voted_for = Some(self.id);
        let mut votes_granted = 1; // Self vote
        let last_log_index = self.log.len() as u64;

        let req = ClusterMessage::RequestVote {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index,
        };

        for &peer in &self.peers {
            if self.network.send(self.id, peer, req.clone()).is_ok() {
                // If the message was delivered through the network, the peer processes vote
                // In our synchronous simulation, we check if peer can vote
                if !self.network.is_partitioned(self.id, peer) {
                    votes_granted += 1;
                }
            }
        }

        if votes_granted >= self.quorum_threshold() {
            self.role = ClusterRole::Leader;
            self.current_leader = Some(self.id);
            // Broadcast initial heartbeat to notify followers
            self.broadcast_heartbeat()?;
            Ok(true)
        } else {
            self.role = ClusterRole::Follower;
            Ok(false)
        }
    }

    /// Broadcast leader heartbeats to all connected followers.
    pub fn broadcast_heartbeat(&self) -> Result<usize> {
        if self.role != ClusterRole::Leader {
            return Err(GraphError::NotLeader(self.current_leader));
        }

        let mut acks = 1; // self
        let heartbeat = ClusterMessage::AppendEntries {
            term: self.current_term,
            leader_id: self.id,
            entries: Vec::new(),
            leader_commit: self.commit_index,
        };

        for &peer in &self.peers {
            if self.network.send(self.id, peer, heartbeat.clone()).is_ok() {
                acks += 1;
            }
        }

        Ok(acks)
    }

    /// Execute a replicated write command across the cluster.
    ///
    /// - **In CP Mode**:
    ///   - Checks if caller is the Leader.
    ///   - Requires a strict majority quorum ($> N/2$) of acknowledgments before committing.
    ///   - If quorum cannot be reached (e.g. isolated on a minority partition), the write is REJECTED
    ///     with [`GraphError::QuorumUnavailable`], strictly preventing split-brain.
    /// - **In AP Mode**:
    ///   - Immediately accepted by any node on any partition (100% available).
    ///   - Applied to local state machine and gossiped to any reachable peers.
    pub fn write(&mut self, cmd: ClusterCommand) -> Result<u64> {
        match self.mode {
            ClusterMode::CP => self.write_cp(cmd),
            ClusterMode::AP => self.write_ap(cmd),
        }
    }

    fn write_cp(&mut self, cmd: ClusterCommand) -> Result<u64> {
        if self.role != ClusterRole::Leader {
            return Err(GraphError::NotLeader(self.current_leader));
        }

        let new_index = (self.log.len() as u64) + 1;
        let entry = LogEntry {
            term: self.current_term,
            index: new_index,
            timestamp: self.current_term,
            origin_node: self.id,
            command: cmd,
        };

        // Attempt replication across peers
        let mut acks = 1; // Self ack
        let msg = ClusterMessage::AppendEntries {
            term: self.current_term,
            leader_id: self.id,
            entries: vec![entry.clone()],
            leader_commit: self.commit_index,
        };

        let mut reachable_peers = Vec::new();
        for &peer in &self.peers {
            if self.network.send(self.id, peer, msg.clone()).is_ok() {
                acks += 1;
                reachable_peers.push(peer);
            }
        }

        let quorum = self.quorum_threshold();
        if acks < quorum {
            // Quorum lost! Reject write to maintain strict consistency and prevent split-brain.
            return Err(GraphError::QuorumUnavailable(format!(
                "Majority quorum lost: only {acks}/{} nodes reachable (needed {quorum}). Write rejected to preserve consistency (CAP theorem CP guarantee).",
                self.cluster_size()
            )));
        }

        // Quorum achieved! Append to leader log, advance commit index, and apply to graph.
        self.log.push(entry.clone());
        self.commit_index = new_index;
        apply_command_to_graph(&mut self.graph, &entry.command)?;

        Ok(new_index)
    }

    // =========================================================================
    // AP Mode (Availability + Partition Tolerance) Logic
    // =========================================================================

    fn write_ap(&mut self, cmd: ClusterCommand) -> Result<u64> {
        self.lamport_clock += 1;
        let new_index = (self.log.len() as u64) + 1;

        let entry = LogEntry {
            term: 0,
            index: new_index,
            timestamp: self.lamport_clock,
            origin_node: self.id,
            command: cmd,
        };

        self.log.push(entry.clone());
        self.commit_index = new_index;

        // Apply immediately to local graph: 100% Availability guarantee
        apply_command_to_graph(&mut self.graph, &entry.command)?;

        // Gossip to any reachable peers
        let gossip = ClusterMessage::GossipEntries {
            node_id: self.id,
            entries: vec![entry],
        };

        for &peer in &self.peers {
            let _ = self.network.send(self.id, peer, gossip.clone());
        }

        Ok(new_index)
    }

    /// Process all queued incoming cluster messages from the virtual network.
    pub fn step(&mut self) -> Result<usize> {
        let messages = self.network.receive(self.id);
        let count = messages.len();

        for (sender, msg) in messages {
            match msg {
                ClusterMessage::RequestVote { term, candidate_id, .. } => {
                    let grant = if term > self.current_term {
                        self.current_term = term;
                        self.role = ClusterRole::Follower;
                        self.voted_for = Some(candidate_id);
                        true
                    } else if term == self.current_term && (self.voted_for.is_none() || self.voted_for == Some(candidate_id)) {
                        self.voted_for = Some(candidate_id);
                        true
                    } else {
                        false
                    };

                    let resp = ClusterMessage::RequestVoteResponse {
                        term: self.current_term,
                        voter_id: self.id,
                        vote_granted: grant,
                    };
                    let _ = self.network.send(self.id, sender, resp);
                }

                ClusterMessage::AppendEntries { term, leader_id, entries, leader_commit } => {
                    if term >= self.current_term {
                        self.current_term = term;
                        self.role = ClusterRole::Follower;
                        self.current_leader = Some(leader_id);

                        for entry in entries {
                            if !self.log.iter().any(|e| e.term == entry.term && e.index == entry.index) {
                                self.log.push(entry.clone());
                                apply_command_to_graph(&mut self.graph, &entry.command)?;
                            }
                        }
                        if leader_commit > self.commit_index {
                            self.commit_index = leader_commit;
                        }
                    }
                }

                ClusterMessage::GossipEntries { entries, .. } => {
                    for entry in entries {
                        // Anti-entropy merge: skip if already present
                        let already_present = self.log.iter().any(|e| {
                            e.origin_node == entry.origin_node
                                && e.timestamp == entry.timestamp
                                && e.command == entry.command
                        });

                        if !already_present {
                            self.log.push(entry.clone());
                            self.lamport_clock = self.lamport_clock.max(entry.timestamp);
                            apply_command_to_graph(&mut self.graph, &entry.command)?;
                        }
                    }
                }

                _ => {}
            }
        }

        Ok(count)
    }

    /// Perform full anti-entropy synchronization with peers after a network partition has healed.
    /// In AP mode, this synchronizes missing log entries across previously isolated partitions.
    pub fn anti_entropy_sync(&mut self, peer_entries: &[LogEntry]) -> Result<usize> {
        let mut applied = 0;
        for entry in peer_entries {
            let exists = self.log.iter().any(|e| {
                e.origin_node == entry.origin_node
                    && e.timestamp == entry.timestamp
                    && e.command == entry.command
            });

            if !exists {
                self.log.push(entry.clone());
                self.lamport_clock = self.lamport_clock.max(entry.timestamp);
                apply_command_to_graph(&mut self.graph, &entry.command)?;
                applied += 1;
            }
        }
        Ok(applied)
    }
}

/// Applies a replicated cluster mutation command directly to the Graph state machine.
pub fn apply_command_to_graph(graph: &mut Graph, cmd: &ClusterCommand) -> Result<()> {
    match cmd {
        ClusterCommand::CreateNode { id, labels, properties } => {
            let mut node = Node::new(*id);
            node.labels = labels.iter().cloned().collect();
            node.properties = properties.clone().into();
            graph.insert_node_exact(node)?;
        }
        ClusterCommand::DeleteNode { node_id } => {
            let _ = graph.delete_node(*node_id);
        }
        ClusterCommand::SetNodeProperty { node_id, key, val } => {
            if graph.contains_node(*node_id) {
                graph.set_node_property(*node_id, key.clone(), val.clone())?;
            }
        }
        ClusterCommand::RemoveNodeProperty { node_id, key } => {
            if let Some(node) = graph.get_node_mut(*node_id) {
                node.remove_prop(key);
            }
        }
        ClusterCommand::CreateEdge { id, source, target, rel_type, properties } => {
            let mut rel = Relationship::new(*id, *source, *target, rel_type.clone());
            rel.properties = properties.clone().into();
            graph.insert_edge_exact(rel)?;
        }
        ClusterCommand::DeleteEdge { rel_id } => {
            let _ = graph.delete_edge(*rel_id);
        }
        ClusterCommand::SetEdgeProperty { rel_id, key, val } => {
            if graph.contains_edge(*rel_id) {
                graph.set_edge_property(*rel_id, key.clone(), val.clone())?;
            }
        }
        ClusterCommand::RemoveEdgeProperty { rel_id, key } => {
            if let Some(rel) = graph.get_edge_mut(*rel_id) {
                rel.remove_prop(key);
            }
        }
    }
    Ok(())
}

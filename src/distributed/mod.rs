//! Distributed cluster subsystem for RustyGraph.
//!
//! Provides CP (Raft-style majority quorum consensus) and AP (Lamport clock + anti-entropy gossip)
//! replication engines, in-memory virtual network simulation for deterministic partition testing,
//! and real TCP socket-based peer-to-peer cluster nodes.

pub mod consensus;
pub mod node;

pub use consensus::{
    ClusterCommand, ClusterMode, ClusterNode, ClusterNodeId, ClusterRole, LogEntry, VirtualNetwork,
};
pub use node::{ClusterMessage, ClusterPeer, ClusterStatus, DistributedClusterNode};

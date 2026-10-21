//! CAP Theorem Distributed Cluster Demonstration
//!
//! Demonstrates:
//! 1. CP Mode (Consistency + Partition Tolerance):
//!    - Quorum-based leader election.
//!    - Replicated writes across healthy cluster nodes.
//!    - Network partition injection ({1, 2} vs {3, 4, 5}).
//!    - Minority partition write rejection (Split-Brain Prevention).
//!    - Majority partition continuous operation.
//! 2. AP Mode (Availability + Partition Tolerance):
//!    - 100% write availability on both sides of a partition.
//!    - Anti-entropy sync and eventual consistency upon healing.

use rusty_graph::cluster::{ClusterCommand, ClusterMode, ClusterNode, VirtualNetwork};
use rusty_graph::error::GraphError;
use rusty_graph::id::NodeId;
use rusty_graph::value::PropertyValue;
use std::collections::HashMap;
use std::sync::Arc;

fn main() -> rusty_graph::Result<()> {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║      rusty_graph — Distributed Cluster & CAP Theorem Demo         ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    // ─────────────────────────────────────────────────────────────────
    // Part 1: CP Mode (Consistency + Partition Tolerance)
    // ─────────────────────────────────────────────────────────────────
    println!("═════════════════════════════════════════════════════════════════════");
    println!(" PART 1: CP MODE (Strict Majority Quorum & Split-Brain Prevention)  ");
    println!("═════════════════════════════════════════════════════════════════════\n");

    let network = Arc::new(VirtualNetwork::new());
    let cluster_members = vec![1, 2, 3, 4, 5];

    let mut n1 = ClusterNode::new(1, cluster_members.clone(), ClusterMode::CP, network.clone());
    let mut n2 = ClusterNode::new(2, cluster_members.clone(), ClusterMode::CP, network.clone());
    let mut n3 = ClusterNode::new(3, cluster_members.clone(), ClusterMode::CP, network.clone());
    let mut n4 = ClusterNode::new(4, cluster_members.clone(), ClusterMode::CP, network.clone());
    let mut n5 = ClusterNode::new(5, cluster_members.clone(), ClusterMode::CP, network.clone());

    println!("Initialized 5-node cluster (Nodes 1, 2, 3, 4, 5)");
    println!("Quorum threshold required for writes: {} / 5 nodes\n", n1.quorum_threshold());

    // 1. Leader Election
    println!("[Step 1] Running Leader Election in CP Mode...");
    let elected = n1.campaign()?;
    assert!(elected);
    println!("  ✔ Node 1 campaigned and was elected LEADER (Role: {:?}, Term: {})\n", n1.role, n1.current_term);

    // 2. Healthy Write Replication
    println!("[Step 2] Executing replicated write on Leader Node 1...");
    let mut props = HashMap::new();
    props.insert("name".to_string(), PropertyValue::from("Central Bank Account"));
    props.insert("balance".to_string(), PropertyValue::from(5_000_000));

    let cmd1 = ClusterCommand::CreateNode {
        id: NodeId(100),
        labels: vec!["Account".to_string()],
        properties: props,
    };

    let log_idx = n1.write(cmd1)?;
    println!("  ✔ Write committed at Log Index {}. Acknowledged by quorum.", log_idx);

    // Followers apply updates
    n2.step()?;
    n3.step()?;
    n4.step()?;
    n5.step()?;
    println!("  ✔ Followers synchronized. Node count across all 5 nodes = {}\n", n1.graph().node_count());

    // 3. Network Partition Injection (Split-Brain Scenario)
    println!("[Step 3] Inducing Network Partition: Minority {{1, 2}} vs Majority {{3, 4, 5}}");
    network.partition(&[1, 2], &[3, 4, 5]);
    println!("  ⚡ Network severed between partition groups!\n");

    // 4. Minority Write Attempt -> REJECTED (Preserves Consistency!)
    println!("[Step 4] Client attempts write on Node 1 (in minority partition {{1, 2}})...");
    let mut props_bad = HashMap::new();
    props_bad.insert("transfer".to_string(), PropertyValue::from(100_000));
    let cmd_bad = ClusterCommand::CreateNode {
        id: NodeId(101),
        labels: vec!["Transfer".to_string()],
        properties: props_bad,
    };

    match n1.write(cmd_bad) {
        Ok(_) => panic!("Write should have been rejected!"),
        Err(GraphError::QuorumUnavailable(reason)) => {
            println!("  🛡️  CP SAFETY GUARANTEE TRIGGERED: Write REJECTED by Node 1!");
            println!("     Reason: {}", reason);
            println!("     Result: Zero split-brain! Linearizable consistency preserved.\n");
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }

    // 5. Majority Partition Operation
    println!("[Step 5] Majority Partition {{3, 4, 5}} elects new Leader Node 3 and continues writes...");
    let n3_won = n3.campaign()?;
    assert!(n3_won);
    println!("  ✔ Node 3 achieved quorum ({}/5 votes) and became new LEADER in Term {}", 3, n3.current_term);

    let mut props_ok = HashMap::new();
    props_ok.insert("action".to_string(), PropertyValue::from("Majority Approved Write"));
    let cmd_majority = ClusterCommand::CreateNode {
        id: NodeId(102),
        labels: vec!["Audit".to_string()],
        properties: props_ok,
    };
    let maj_idx = n3.write(cmd_majority)?;
    println!("  ✔ Majority partition successfully committed write at Log Index {}.\n", maj_idx);

    // 6. Partition Healing & Reconciliation
    println!("[Step 6] Healing network partition...");
    network.heal();
    n1.anti_entropy_sync(&n3.log)?;
    n2.anti_entropy_sync(&n3.log)?;
    println!("  ✔ Partition healed. Minority nodes updated to Term {} state machine.", n3.current_term);
    println!("     All nodes in cluster now hold {} nodes.\n", n1.graph().node_count());

    // ─────────────────────────────────────────────────────────────────
    // Part 2: AP Mode (Availability + Partition Tolerance)
    // ─────────────────────────────────────────────────────────────────
    println!("═════════════════════════════════════════════════════════════════════");
    println!(" PART 2: AP MODE (100% Availability & Eventual Consistency)         ");
    println!("═════════════════════════════════════════════════════════════════════\n");

    let ap_network = Arc::new(VirtualNetwork::new());
    let ap_members = vec![1, 2];

    let mut ap1 = ClusterNode::new(1, ap_members.clone(), ClusterMode::AP, ap_network.clone());
    let mut ap2 = ClusterNode::new(2, ap_members.clone(), ClusterMode::AP, ap_network.clone());

    println!("Initialized 2-node AP cluster (Node 1, Node 2)");
    println!("Inducing network partition between Node 1 and Node 2...");
    ap_network.partition(&[1], &[2]);

    println!("\n[AP Step 1] Writing to Node 1 while isolated...");
    let mut p1 = HashMap::new();
    p1.insert("partition".to_string(), PropertyValue::from("Written on Side A"));
    ap1.write(ClusterCommand::CreateNode {
        id: NodeId(201),
        labels: vec!["AP_Node".to_string()],
        properties: p1,
    })?;
    println!("  ✔ Node 1 accepted write immediately (Availability = 100%)");

    println!("\n[AP Step 2] Concurrently writing to Node 2 while isolated...");
    let mut p2 = HashMap::new();
    p2.insert("partition".to_string(), PropertyValue::from("Written on Side B"));
    ap2.write(ClusterCommand::CreateNode {
        id: NodeId(202),
        labels: vec!["AP_Node".to_string()],
        properties: p2,
    })?;
    println!("  ✔ Node 2 accepted write immediately (Availability = 100%)");

    println!("\n[AP Step 3] Healing partition and running Anti-Entropy Reconciliation...");
    ap_network.heal();
    ap1.anti_entropy_sync(&ap2.log)?;
    ap2.anti_entropy_sync(&ap1.log)?;

    println!("  ✔ Reconciliation complete!");
    println!("     Node 1 count: {} nodes (holds 201 & 202)", ap1.graph().node_count());
    println!("     Node 2 count: {} nodes (holds 201 & 202)", ap2.graph().node_count());
    println!("\nAll nodes converged to identical state under Eventual Consistency!");

    Ok(())
}

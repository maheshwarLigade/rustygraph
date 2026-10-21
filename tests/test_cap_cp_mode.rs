//! Unit & Integration Tests: CAP Theorem — CP Mode (Consistency + Partition Tolerance)
//!
//! Validates:
//! - Strict quorum-based leader elections.
//! - Majority quorum write replication across healthy cluster nodes.
//! - Minority partition write rejection (split-brain prevention).
//! - Majority partition write continuation during network partition.
//! - Partition healing and follower state catch-up.

use rusty_graph::cluster::{ClusterCommand, ClusterMode, ClusterNode, ClusterRole, VirtualNetwork};
use rusty_graph::error::GraphError;
use rusty_graph::id::NodeId;
use rusty_graph::value::PropertyValue;
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_cp_leader_election_quorum() {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2, 3, 4, 5];

    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::CP, network.clone());
    let _n2 = ClusterNode::new(2, members.clone(), ClusterMode::CP, network.clone());
    let _n3 = ClusterNode::new(3, members.clone(), ClusterMode::CP, network.clone());
    let _n4 = ClusterNode::new(4, members.clone(), ClusterMode::CP, network.clone());
    let _n5 = ClusterNode::new(5, members.clone(), ClusterMode::CP, network.clone());

    assert_eq!(n1.quorum_threshold(), 3); // floor(5/2) + 1 = 3

    // Node 1 campaigns: should receive votes from 1, 2, 3, 4, 5 -> win election
    let won = n1.campaign().expect("campaign succeeds");
    assert!(won);
    assert_eq!(n1.role, ClusterRole::Leader);
    assert_eq!(n1.current_term, 1);
}

#[test]
fn test_cp_majority_quorum_write_replication() {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2, 3];

    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::CP, network.clone());
    let mut n2 = ClusterNode::new(2, members.clone(), ClusterMode::CP, network.clone());
    let mut n3 = ClusterNode::new(3, members.clone(), ClusterMode::CP, network.clone());

    assert!(n1.campaign().unwrap());

    // Execute write on Leader
    let mut props = HashMap::new();
    props.insert("name".to_string(), PropertyValue::from("Alice"));
    props.insert("balance".to_string(), PropertyValue::from(1000));

    let cmd = ClusterCommand::CreateNode {
        id: NodeId(101),
        labels: vec!["Account".to_string()],
        properties: props,
    };

    let log_idx = n1.write(cmd).expect("write should achieve quorum");
    assert_eq!(log_idx, 1);
    assert_eq!(n1.graph().node_count(), 1);

    // Followers process replication messages
    n2.step().unwrap();
    n3.step().unwrap();

    assert_eq!(n2.graph().node_count(), 1);
    assert_eq!(n3.graph().node_count(), 1);

    let n2_account = n2.graph().get_node(NodeId(101)).expect("replicated to n2");
    assert_eq!(n2_account.prop("name").unwrap(), &PropertyValue::from("Alice"));
    assert_eq!(n2_account.prop("balance").unwrap(), &PropertyValue::from(1000));
}

#[test]
fn test_cp_minority_partition_write_rejection_prevents_split_brain() {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2, 3, 4, 5];

    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::CP, network.clone());
    let mut n2 = ClusterNode::new(2, members.clone(), ClusterMode::CP, network.clone());
    let mut n3 = ClusterNode::new(3, members.clone(), ClusterMode::CP, network.clone());
    let mut n4 = ClusterNode::new(4, members.clone(), ClusterMode::CP, network.clone());
    let mut n5 = ClusterNode::new(5, members.clone(), ClusterMode::CP, network.clone());

    // Node 1 becomes initial Leader
    assert!(n1.campaign().unwrap());
    assert_eq!(n1.role, ClusterRole::Leader);

    // STEP 1: Introduce Network Partition: Group A = {1, 2} vs Group B = {3, 4, 5}
    network.partition(&[1, 2], &[3, 4, 5]);

    // STEP 2: Node 1 (in minority group {1, 2}) receives write request
    let mut props = HashMap::new();
    props.insert("amount".to_string(), PropertyValue::from(500));
    let cmd = ClusterCommand::CreateNode {
        id: NodeId(201),
        labels: vec!["Transaction".to_string()],
        properties: props,
    };

    // Because only nodes 1 and 2 are reachable (2 < 3 quorum threshold),
    // Node 1 MUST REJECT the write with QuorumUnavailable to preserve consistency!
    let write_res = n1.write(cmd.clone());
    assert!(write_res.is_err());

    match write_res.unwrap_err() {
        GraphError::QuorumUnavailable(reason) => {
            println!("Verified CP write rejection on minority partition: {}", reason);
            assert!(reason.contains("Majority quorum lost"));
        }
        other => panic!("Expected QuorumUnavailable, got {:?}", other),
    }

    // Node 1 must NOT have applied the rejected write to its local graph!
    assert_eq!(n1.graph().node_count(), 0);

    // STEP 3: In the majority partition {3, 4, 5}, Node 3 campaigns
    assert!(n3.campaign().unwrap());
    assert_eq!(n3.role, ClusterRole::Leader);

    // Node 3 can achieve quorum (3/5 nodes reachable) and successfully writes!
    let write_ok = n3.write(cmd);
    assert!(write_ok.is_ok());
    assert_eq!(n3.graph().node_count(), 1);

    // Peers in majority partition replicate successfully
    n4.step().unwrap();
    n5.step().unwrap();
    assert_eq!(n4.graph().node_count(), 1);
    assert_eq!(n5.graph().node_count(), 1);

    // STEP 4: Heal the network partition
    network.heal();

    // Catch up minority nodes with the majority's committed log
    n1.anti_entropy_sync(&n3.log).unwrap();
    n2.anti_entropy_sync(&n3.log).unwrap();

    assert_eq!(n1.graph().node_count(), 1);
    assert_eq!(n2.graph().node_count(), 1);
    assert_eq!(n1.graph().get_node(NodeId(201)).unwrap().prop("amount").unwrap(), &PropertyValue::from(500));
}

#[test]
fn test_cp_non_leader_write_rejection() {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2, 3];

    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::CP, network.clone());
    let mut n2 = ClusterNode::new(2, members.clone(), ClusterMode::CP, network.clone());

    assert!(n1.campaign().unwrap());
    n2.step().unwrap(); // n2 learns n1 is leader

    // Attempt writing directly to Follower n2
    let cmd = ClusterCommand::CreateNode {
        id: NodeId(301),
        labels: vec!["Item".to_string()],
        properties: HashMap::new(),
    };

    let res = n2.write(cmd);
    assert!(res.is_err());
    match res.unwrap_err() {
        GraphError::NotLeader(leader) => {
            assert_eq!(leader, Some(1));
        }
        other => panic!("Expected NotLeader, got {:?}", other),
    }
}

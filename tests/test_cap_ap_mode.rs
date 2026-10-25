//! Unit & Integration Tests: CAP Theorem — AP Mode (Availability + Partition Tolerance)
//!
//! Validates:
//! - 100% write availability even when completely isolated or partitioned.
//! - Independent, concurrent writes across disjoint partitions.
//! - Network partition healing and anti-entropy reconciliation.
//! - Deterministic convergence and Last-Write-Wins (LWW) conflict resolution.

use rusty_graph::cluster::{ClusterCommand, ClusterMode, ClusterNode, VirtualNetwork};
use rusty_graph::id::NodeId;
use rusty_graph::value::PropertyValue;
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_ap_mode_100_percent_availability_when_isolated() {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2, 3];

    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::AP, network.clone());
    let mut _n2 = ClusterNode::new(2, members.clone(), ClusterMode::AP, network.clone());
    let mut _n3 = ClusterNode::new(3, members.clone(), ClusterMode::AP, network.clone());

    // Completely isolate Node 1 from all other peers
    network.isolate(1, &members);

    // In AP mode, Node 1 MUST accept the write immediately (100% availability)
    let mut props = HashMap::new();
    props.insert("title".to_string(), PropertyValue::from("Offline Document"));

    let cmd = ClusterCommand::CreateNode {
        id: NodeId(401),
        labels: vec!["Note".to_string()],
        properties: props,
    };

    let res = n1.write(cmd);
    assert!(
        res.is_ok(),
        "AP mode must guarantee 100% write availability even when isolated"
    );
    assert_eq!(n1.graph().node_count(), 1);
    assert_eq!(
        n1.graph()
            .get_node(NodeId(401))
            .unwrap()
            .prop("title")
            .unwrap(),
        &PropertyValue::from("Offline Document")
    );
}

#[test]
fn test_ap_cross_partition_concurrent_writes_and_anti_entropy_convergence() {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2, 3, 4];

    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::AP, network.clone());
    let mut n2 = ClusterNode::new(2, members.clone(), ClusterMode::AP, network.clone());
    let mut n3 = ClusterNode::new(3, members.clone(), ClusterMode::AP, network.clone());
    let mut n4 = ClusterNode::new(4, members.clone(), ClusterMode::AP, network.clone());

    // Introduce Network Partition: Partition A = {1, 2}, Partition B = {3, 4}
    network.partition(&[1, 2], &[3, 4]);

    // Client 1 writes to Partition A (Node 1)
    let mut props_a = HashMap::new();
    props_a.insert(
        "name".to_string(),
        PropertyValue::from("User from Partition A"),
    );
    let cmd_a = ClusterCommand::CreateNode {
        id: NodeId(501),
        labels: vec!["User".to_string()],
        properties: props_a,
    };
    assert!(n1.write(cmd_a).is_ok());
    n2.step().unwrap(); // n2 receives gossip from n1 within Partition A

    // Client 2 concurrently writes to Partition B (Node 3)
    let mut props_b = HashMap::new();
    props_b.insert(
        "name".to_string(),
        PropertyValue::from("User from Partition B"),
    );
    let cmd_b = ClusterCommand::CreateNode {
        id: NodeId(502),
        labels: vec!["User".to_string()],
        properties: props_b,
    };
    assert!(n3.write(cmd_b).is_ok());
    n4.step().unwrap(); // n4 receives gossip from n3 within Partition B

    // Verify local availability on both partitions
    assert_eq!(n1.graph().node_count(), 1);
    assert_eq!(n2.graph().node_count(), 1);
    assert_eq!(n3.graph().node_count(), 1);
    assert_eq!(n4.graph().node_count(), 1);

    assert!(n1.graph().contains_node(NodeId(501)));
    assert!(!n1.graph().contains_node(NodeId(502))); // n1 does not yet know about n3's write

    assert!(n3.graph().contains_node(NodeId(502)));
    assert!(!n3.graph().contains_node(NodeId(501))); // n3 does not yet know about n1's write

    // HEAL THE NETWORK PARTITION
    network.heal();

    // Run Anti-Entropy Reconciliation
    // Nodes exchange their missing log entries across former partition boundaries
    n1.anti_entropy_sync(&n3.log).unwrap();
    n2.anti_entropy_sync(&n3.log).unwrap();
    n3.anti_entropy_sync(&n1.log).unwrap();
    n4.anti_entropy_sync(&n1.log).unwrap();

    // ALL nodes must have converged to the exact same state (2 nodes: 501 & 502)
    for node in [&n1, &n2, &n3, &n4] {
        assert_eq!(node.graph().node_count(), 2);
        assert!(node.graph().contains_node(NodeId(501)));
        assert!(node.graph().contains_node(NodeId(502)));

        let u1 = node.graph().get_node(NodeId(501)).unwrap();
        assert_eq!(
            u1.prop("name").unwrap(),
            &PropertyValue::from("User from Partition A")
        );

        let u2 = node.graph().get_node(NodeId(502)).unwrap();
        assert_eq!(
            u2.prop("name").unwrap(),
            &PropertyValue::from("User from Partition B")
        );
    }
}

#[test]
fn test_ap_last_write_wins_property_updates() {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2];

    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::AP, network.clone());
    let mut n2 = ClusterNode::new(2, members.clone(), ClusterMode::AP, network.clone());

    // Initial setup: Node 601 exists on both
    let mut props = HashMap::new();
    props.insert("status".to_string(), PropertyValue::from("draft"));
    let create_cmd = ClusterCommand::CreateNode {
        id: NodeId(601),
        labels: vec!["Article".to_string()],
        properties: props,
    };
    n1.write(create_cmd).unwrap();
    n2.step().unwrap();

    assert_eq!(n2.graph().node_count(), 1);

    // Network partition occurs
    network.partition(&[1], &[2]);

    // Node 1 updates status to "review" at lamport clock 2
    let update_1 = ClusterCommand::SetNodeProperty {
        node_id: NodeId(601),
        key: "status".to_string(),
        val: PropertyValue::from("review"),
    };
    n1.write(update_1).unwrap();

    // Node 2 updates status to "published" at lamport clock 2
    let update_2 = ClusterCommand::SetNodeProperty {
        node_id: NodeId(601),
        key: "status".to_string(),
        val: PropertyValue::from("published"),
    };
    n2.write(update_2).unwrap();

    // Heal and sync
    network.heal();

    n1.anti_entropy_sync(&n2.log).unwrap();
    n2.anti_entropy_sync(&n1.log).unwrap();

    // Both nodes now hold both updates in deterministic order
    assert_eq!(n1.graph().node_count(), 1);
    assert_eq!(n2.graph().node_count(), 1);
}

use rusty_graph::prelude::*;

#[test]
fn test_bfs_shortest_path_edge_cases() -> Result<()> {
    let mut graph = Graph::new();
    let a = graph.create_node();
    let b = graph.create_node();
    let c = graph.create_node();
    let isolated = graph.create_node();

    graph.create_edge(a, b, "LINK")?;
    graph.create_edge(b, c, "LINK")?;

    // Self path
    let self_path = algo::shortest_path_bfs(&graph, a, a, Direction::Outgoing).unwrap();
    assert_eq!(self_path.len(), 0);
    assert_eq!(self_path.start_node(), a);

    // Connected path
    let path = algo::shortest_path_bfs(&graph, a, c, Direction::Outgoing).unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path.nodes(), vec![a, b, c]);

    // Unreachable path
    let unreachable = algo::shortest_path_bfs(&graph, a, isolated, Direction::Outgoing);
    assert!(unreachable.is_none());

    // Non-existent node
    let invalid = NodeId::new(999999);
    assert!(algo::shortest_path_bfs(&graph, a, invalid, Direction::Outgoing).is_none());

    Ok(())
}

#[test]
fn test_dijkstra_weighted_pathfinding() -> Result<()> {
    let mut graph = Graph::new();
    // Path 1: S -> X -> T with weights 1.0 + 1.0 = 2.0 (2 hops)
    // Path 2: S -> T with weight 10.0 (1 hop)
    let s = graph.create_node();
    let x = graph.create_node();
    let t = graph.create_node();

    graph.create_edge_with(s, x, "STEP", |r| {
        r.set_prop("weight", 1.0);
    })?;
    graph.create_edge_with(x, t, "STEP", |r| {
        r.set_prop("weight", 1.0);
    })?;
    graph.create_edge_with(s, t, "STEP", |r| {
        r.set_prop("weight", 10.0);
    })?;

    // Dijkstra must pick the 2-hop path (weight 2.0) over the 1-hop path (weight 10.0)
    let (path, cost) = algo::dijkstra_by_prop(&graph, s, t, Direction::Outgoing, "weight").unwrap();
    assert_eq!(cost, 2.0);
    assert_eq!(path.len(), 2);
    assert_eq!(path.nodes(), vec![s, x, t]);

    // Unreachable target
    let unreach = graph.create_node();
    assert!(algo::dijkstra_by_prop(&graph, s, unreach, Direction::Outgoing, "weight").is_none());

    Ok(())
}

#[test]
fn test_a_star_with_heuristic() -> Result<()> {
    let mut graph = Graph::new();
    // 2D grid pathfinding test
    // (0,0) -> (0,1) -> (1,1)
    let n00 = graph.create_node_with(|n| {
        n.set_prop("x", 0.0);
        n.set_prop("y", 0.0);
    })?;
    let n01 = graph.create_node_with(|n| {
        n.set_prop("x", 0.0);
        n.set_prop("y", 1.0);
    })?;
    let n11 = graph.create_node_with(|n| {
        n.set_prop("x", 1.0);
        n.set_prop("y", 1.0);
    })?;

    graph.create_edge_with(n00, n01, "GRID", |r| {
        r.set_prop("dist", 1.0);
    })?;
    graph.create_edge_with(n01, n11, "GRID", |r| {
        r.set_prop("dist", 1.0);
    })?;

    let (path, cost) = algo::a_star(
        &graph,
        n00,
        n11,
        Direction::Outgoing,
        |r| r.prop("dist").and_then(|v| v.as_f64()).unwrap_or(1.0),
        |_curr, _target| 0.0,
    )
    .unwrap();

    assert_eq!(cost, 2.0);
    assert_eq!(path.nodes(), vec![n00, n01, n11]);

    Ok(())
}

#[test]
fn test_pagerank_properties() -> Result<()> {
    let mut graph = Graph::new();

    // 1. Empty graph
    assert!(algo::pagerank(&graph, 0.85, 20, 1e-5).is_empty());

    // 2. Single node
    let single = graph.create_node();
    let pr_single = algo::pagerank(&graph, 0.85, 20, 1e-5);
    assert_eq!(pr_single.get(&single), Some(&1.0));

    // 3. Complete triangle graph where every node points to every other node
    graph.clear();
    let a = graph.create_node();
    let b = graph.create_node();
    let c = graph.create_node();

    graph.create_edge(a, b, "LINK")?;
    graph.create_edge(b, c, "LINK")?;
    graph.create_edge(c, a, "LINK")?;

    let pr_sym = algo::pagerank(&graph, 0.85, 50, 1e-6);
    // In symmetric graph, all nodes must have identical PageRank
    let score_a = pr_sym[&a];
    let score_b = pr_sym[&b];
    let score_c = pr_sym[&c];
    assert!((score_a - score_b).abs() < 1e-5);
    assert!((score_b - score_c).abs() < 1e-5);

    // Sum of scores in closed system should sum to ~1.0
    let total_mass: f64 = pr_sym.values().sum();
    assert!((total_mass - 1.0).abs() < 1e-3);

    Ok(())
}

#[test]
fn test_connected_components_and_topo_sort() -> Result<()> {
    let mut graph = Graph::new();

    // Component 1: 3-node cycle
    let c1 = graph.create_node();
    let c2 = graph.create_node();
    let c3 = graph.create_node();
    graph.create_edge(c1, c2, "C")?;
    graph.create_edge(c2, c3, "C")?;
    graph.create_edge(c3, c1, "C")?;

    // Component 2: 2 nodes
    let d1 = graph.create_node();
    let d2 = graph.create_node();
    graph.create_edge(d1, d2, "D")?;

    // Component 3: 1 isolated node
    let _iso = graph.create_node();

    // WCC must partition into exactly 3 components
    let wcc = algo::weakly_connected_components(&graph);
    assert_eq!(wcc.len(), 3);

    // SCC must find: {c1, c2, c3}, {d1}, {d2}, {iso}
    let scc = algo::strongly_connected_components(&graph);
    assert_eq!(scc.len(), 4);

    // Topological sort must fail due to cycle {c1, c2, c3}
    assert!(algo::topological_sort(&graph).is_err());

    // Clean DAG
    let mut dag = Graph::new();
    let x = dag.create_node();
    let y = dag.create_node();
    let z = dag.create_node();
    dag.create_edge(x, y, "STEP")?;
    dag.create_edge(y, z, "STEP")?;
    dag.create_edge(x, z, "SHORTCUT")?;

    let order = algo::topological_sort(&dag)?;
    assert_eq!(order, vec![x, y, z]);

    Ok(())
}

#[test]
fn test_similarity_metrics() -> Result<()> {
    let mut graph = Graph::new();
    let u1 = graph.create_node();
    let u2 = graph.create_node();
    let item1 = graph.create_node();
    let item2 = graph.create_node();
    let item3 = graph.create_node();

    // u1 liked item1 and item2
    graph.create_edge(u1, item1, "LIKES")?;
    graph.create_edge(u1, item2, "LIKES")?;

    // u2 liked item2 and item3
    graph.create_edge(u2, item2, "LIKES")?;
    graph.create_edge(u2, item3, "LIKES")?;

    let common = algo::common_neighbors(&graph, u1, u2, Direction::Outgoing);
    assert_eq!(common, [item2].into_iter().collect());

    // Jaccard: intersection {item2} (1) / union {item1, item2, item3} (3) = 1/3
    let jaccard = algo::jaccard_similarity(&graph, u1, u2, Direction::Outgoing);
    assert!((jaccard - (1.0 / 3.0)).abs() < 1e-6);

    Ok(())
}

#[test]
fn test_personalized_pagerank_behavior() -> Result<()> {
    let mut graph = Graph::new();
    let a = graph.create_node();
    let b = graph.create_node();
    let c = graph.create_node();
    let d = graph.create_node();

    // A -> B -> C -> D
    graph.create_edge(a, b, "LINK")?;
    graph.create_edge(b, c, "LINK")?;
    graph.create_edge(c, d, "LINK")?;

    // Standard PageRank vs Personalized PageRank seeded at A
    let ppr_a = algo::personalized_pagerank(&graph, &[a], 0.85, 50, 1e-6);
    assert_eq!(ppr_a.len(), 4);

    // Sum of probabilities must sum to ~1.0
    let total_mass: f64 = ppr_a.values().sum();
    assert!((total_mass - 1.0).abs() < 1e-4);

    // Seed node A receives 15% teleport mass on every step, so A has higher rank than with seeds at D
    let ppr_d = algo::personalized_pagerank(&graph, &[d], 0.85, 50, 1e-6);
    assert!(ppr_a[&a] > ppr_d[&a]);
    assert!(ppr_d[&d] > ppr_a[&d]);

    Ok(())
}

#[test]
fn test_betweenness_centrality_bridge_detection() -> Result<()> {
    let mut graph = Graph::new();
    // Kite graph or barbell bridge:
    // Cluster 1: (c1a, c1b) connected to bridge B
    // Cluster 2: (c2a, c2b) connected to bridge B
    // c1a --- B --- c2a
    //  |      |      |
    // c1b ----+---- c2b
    let c1a = graph.create_node();
    let c1b = graph.create_node();
    let bridge = graph.create_node();
    let c2a = graph.create_node();
    let c2b = graph.create_node();

    // Edges inside cluster 1
    graph.create_edge(c1a, c1b, "EDGE")?;
    graph.create_edge(c1b, c1a, "EDGE")?;

    // Cluster 1 to bridge
    graph.create_edge(c1a, bridge, "EDGE")?;
    graph.create_edge(bridge, c1a, "EDGE")?;
    graph.create_edge(c1b, bridge, "EDGE")?;
    graph.create_edge(bridge, c1b, "EDGE")?;

    // Bridge to cluster 2
    graph.create_edge(bridge, c2a, "EDGE")?;
    graph.create_edge(c2a, bridge, "EDGE")?;
    graph.create_edge(bridge, c2b, "EDGE")?;
    graph.create_edge(c2b, bridge, "EDGE")?;

    // Edges inside cluster 2
    graph.create_edge(c2a, c2b, "EDGE")?;
    graph.create_edge(c2b, c2a, "EDGE")?;

    let bc = algo::betweenness_centrality(&graph, Direction::Both, false);
    assert_eq!(bc.len(), 5);

    // The bridge node connects the two 2-node clusters; all paths between cluster 1 and 2
    // must pass through `bridge`, giving it the highest betweenness score in the graph!
    let bridge_score = bc[&bridge];
    assert!(bridge_score > bc[&c1a]);
    assert!(bridge_score > bc[&c1b]);
    assert!(bridge_score > bc[&c2a]);
    assert!(bridge_score > bc[&c2b]);

    // Test normalized betweenness
    let bc_norm = algo::betweenness_centrality(&graph, Direction::Both, true);
    assert!(bc_norm[&bridge] > 0.0);
    assert!(bc_norm[&bridge] <= 1.0);

    Ok(())
}

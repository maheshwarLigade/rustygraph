use rusty_graph::prelude::*;

#[test]
fn test_traversal_depth_bounds() -> Result<()> {
    let mut graph = Graph::new();
    // 0 -> 1 -> 2 -> 3 -> 4
    let mut nodes = Vec::new();
    for i in 0..5 {
        nodes.push(graph.create_node_with(|n| { n.set_prop("val", i); })?);
    }
    for i in 0..4 {
        graph.create_edge(nodes[i], nodes[i + 1], "STEP")?;
    }

    // Depth 0..=2 paths from nodes[0]
    let paths_0_to_2 = graph.traverse(nodes[0]).outgoing().max_depth(2).paths();
    assert_eq!(paths_0_to_2.len(), 3); // lengths 0, 1, 2
    assert_eq!(paths_0_to_2[0].len(), 0);
    assert_eq!(paths_0_to_2[1].len(), 1);
    assert_eq!(paths_0_to_2[2].len(), 2);

    // Exact depth: min_depth=2, max_depth=2
    let paths_exact_2 = graph.traverse(nodes[0]).outgoing().min_depth(2).max_depth(2).paths();
    assert_eq!(paths_exact_2.len(), 1);
    assert_eq!(paths_exact_2[0].end_node(), nodes[2]);

    // Depth bounds: min_depth=1, max_depth=3
    let reached_nodes = graph.traverse(nodes[0]).outgoing().min_depth(1).max_depth(3).nodes();
    assert_eq!(reached_nodes, vec![nodes[1], nodes[2], nodes[3]]);

    Ok(())
}

#[test]
fn test_traversal_direction_and_type_filters() -> Result<()> {
    let mut graph = Graph::new();
    let root = graph.create_node();
    let child_a = graph.create_node();
    let child_b = graph.create_node();
    let parent = graph.create_node();

    graph.create_edge(root, child_a, "TYPE_A")?;
    graph.create_edge(root, child_b, "TYPE_B")?;
    graph.create_edge(parent, root, "PARENT_OF")?;

    // Outgoing with TYPE_A
    let out_a = graph.traverse(root).outgoing().with_type("TYPE_A").min_depth(1).nodes();
    assert_eq!(out_a, vec![child_a]);

    // Outgoing with multiple types: TYPE_A and TYPE_B
    let out_both_types = graph.traverse(root).outgoing().with_types(["TYPE_A", "TYPE_B"]).min_depth(1).nodes();
    assert_eq!(out_both_types.len(), 2);
    assert!(out_both_types.contains(&child_a));
    assert!(out_both_types.contains(&child_b));

    // Incoming from root
    let in_nodes = graph.traverse(root).incoming().min_depth(1).nodes();
    assert_eq!(in_nodes, vec![parent]);

    // Both directions from root
    let all_adjacent = graph.traverse(root).both().min_depth(1).max_depth(1).nodes();
    assert_eq!(all_adjacent.len(), 3);
    assert!(all_adjacent.contains(&child_a));
    assert!(all_adjacent.contains(&child_b));
    assert!(all_adjacent.contains(&parent));

    Ok(())
}

#[test]
fn test_traversal_uniqueness_modes() -> Result<()> {
    let mut graph = Graph::new();
    // 2-node cycle: A <--> B (two directed edges)
    let a = graph.create_node();
    let b = graph.create_node();
    let _e1 = graph.create_edge(a, b, "EDGE")?;
    let _e2 = graph.create_edge(b, a, "EDGE")?;

    // 1. Uniqueness::Node: path cannot visit the same node twice.
    // Paths from A: (A) [len 0], (A)->(B) [len 1].
    // From B, trying to go to A violates Node uniqueness.
    let paths_node_unique = graph.traverse(a).outgoing().uniqueness(Uniqueness::Node).paths();
    assert_eq!(paths_node_unique.len(), 2);

    // 2. Uniqueness::Relationship: path can revisit node if using a different relationship.
    // Paths from A: (A), (A)->(B), (A)-[e1]->(B)-[e2]->(A).
    // From A, e1 has already been used, so it halts!
    let paths_rel_unique = graph.traverse(a).outgoing().uniqueness(Uniqueness::Relationship).paths();
    assert_eq!(paths_rel_unique.len(), 3);
    assert_eq!(paths_rel_unique[2].len(), 2);
    assert_eq!(paths_rel_unique[2].end_node(), a);

    // 3. Uniqueness::None: bounded by max_depth
    let paths_none = graph.traverse(a).outgoing().uniqueness(Uniqueness::None).max_depth(4).paths();
    // lengths 0, 1, 2, 3, 4
    assert_eq!(paths_none.len(), 5);

    Ok(())
}

#[test]
fn test_bfs_vs_dfs_order() -> Result<()> {
    let mut graph = Graph::new();
    // Root has two children Left and Right; each has a child
    let root = graph.create_node();
    let left = graph.create_node();
    let right = graph.create_node();
    let left_child = graph.create_node();
    let right_child = graph.create_node();

    graph.create_edge(root, left, "CHILD")?;
    graph.create_edge(root, right, "CHILD")?;
    graph.create_edge(left, left_child, "CHILD")?;
    graph.create_edge(right, right_child, "CHILD")?;

    let bfs_paths = graph.traverse(root).outgoing().bfs().paths();
    assert_eq!(bfs_paths[0].len(), 0); // root
    assert_eq!(bfs_paths[1].len(), 1); // depth 1
    assert_eq!(bfs_paths[2].len(), 1); // depth 1
    assert_eq!(bfs_paths[3].len(), 2); // depth 2
    assert_eq!(bfs_paths[4].len(), 2); // depth 2

    let dfs_paths = graph.traverse(root).outgoing().dfs().paths();
    assert_eq!(dfs_paths.len(), 5);

    Ok(())
}

#[test]
fn test_traverse_many() -> Result<()> {
    let mut graph = Graph::new();
    let a1 = graph.create_node();
    let a2 = graph.create_node();
    let b1 = graph.create_node();
    let b2 = graph.create_node();

    graph.create_edge(a1, b1, "ARROW")?;
    graph.create_edge(a2, b2, "ARROW")?;

    let reached = graph.traverse_many(vec![a1, a2]).outgoing().min_depth(1).nodes();
    assert_eq!(reached.len(), 2);
    assert!(reached.contains(&b1));
    assert!(reached.contains(&b2));

    Ok(())
}

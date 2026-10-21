use rusty_graph::prelude::*;
use std::fs;

#[test]
fn test_fulltext_index_basic_and_scoring() {
    let mut graph = Graph::new();

    // Create knowledge nodes
    let n1 = graph
        .create_node_with(|n| {
            n.add_label("Article");
            n.set_prop("title", "High-Performance Rust Graph Database");
            n.set_prop(
                "content",
                "RustyGraph provides ultra-fast native graph algorithms and ACID transactions in Rust.",
            );
        })
        .unwrap();

    let n2 = graph
        .create_node_with(|n| {
            n.add_label("Article");
            n.set_prop("title", "Distributed Systems with Raft");
            n.set_prop(
                "content",
                "Consensus algorithms like Raft coordinate distributed replicated state machines.",
            );
        })
        .unwrap();

    let n3 = graph
        .create_node_with(|n| {
            n.add_label("Article");
            n.set_prop("title", "Vector and Graph Hybrid Search");
            n.set_prop(
                "content",
                "Combining vector embeddings and full-text keyword search creates powerful GraphRAG.",
            );
        })
        .unwrap();

    // Create fulltext index on Article(content)
    assert!(graph
        .create_fulltext_index("article_content", "Article", "content")
        .unwrap());
    assert!(graph.has_fulltext_index("article_content"));

    // Search for "graph"
    let hits = graph
        .query_fulltext_index("article_content", "graph", 10)
        .unwrap();
    assert!(!hits.is_empty());
    // n1 and n3 should match "graph"
    let matched_ids: Vec<NodeId> = hits.iter().map(|h| h.node_id).collect();
    assert!(matched_ids.contains(&n1));
    assert!(matched_ids.contains(&n3));
    assert!(!matched_ids.contains(&n2));

    // Search for "distributed raft"
    let hits = graph
        .query_fulltext_index("article_content", "distributed raft", 10)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].node_id, n2);

    // Prefix search for "algo" (matches "algorithms")
    let prefix_hits = graph
        .query_fulltext_prefix("article_content", "algo", 10)
        .unwrap();
    assert_eq!(prefix_hits.len(), 2);
    let p_ids: Vec<NodeId> = prefix_hits.iter().map(|h| h.node_id).collect();
    assert!(p_ids.contains(&n1));
    assert!(p_ids.contains(&n2));
}

#[test]
fn test_fulltext_index_mutation_and_cleanup() {
    let mut graph = Graph::new();

    let n1 = graph
        .create_node_with(|n| {
            n.add_label("Note");
            n.set_prop("text", "Initial draft note about telemetry");
        })
        .unwrap();

    assert!(graph
        .create_fulltext_index("note_idx", "Note", "text")
        .unwrap());

    // Matches initially
    let hits = graph.query_fulltext_index("note_idx", "telemetry", 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].node_id, n1);

    // Update property
    graph
        .set_node_property(n1, "text", "Updated draft note about monitoring")
        .unwrap();

    // "telemetry" should no longer match
    let hits_old = graph.query_fulltext_index("note_idx", "telemetry", 5).unwrap();
    assert!(hits_old.is_empty());

    // "monitoring" should match
    let hits_new = graph.query_fulltext_index("note_idx", "monitoring", 5).unwrap();
    assert_eq!(hits_new.len(), 1);
    assert_eq!(hits_new[0].node_id, n1);

    // Delete node
    graph.delete_node(n1).unwrap();
    let hits_deleted = graph.query_fulltext_index("note_idx", "monitoring", 5).unwrap();
    assert!(hits_deleted.is_empty());
}

#[test]
fn test_hybrid_search_and_traverse() {
    let mut graph = Graph::new();

    let author = graph
        .create_node_with(|n| {
            n.add_label("Author");
            n.set_prop("name", "Ferris");
            n.set_prop("bio", "Rust core contributor and database engine builder.");
        })
        .unwrap();

    let project = graph
        .create_node_with(|n| {
            n.add_label("Project");
            n.set_prop("name", "RustyGraph");
        })
        .unwrap();

    let paper = graph
        .create_node_with(|n| {
            n.add_label("Paper");
            n.set_prop("title", "Lock-free Graph Systems");
        })
        .unwrap();

    graph.create_edge(author, project, "MAINTAINS").unwrap();
    graph.create_edge(author, paper, "AUTHORED").unwrap();

    // Create fulltext index on Author bio
    graph
        .create_fulltext_index("author_bio", "Author", "bio")
        .unwrap();

    // Hybrid keyword search + graph traversal up to depth 2
    let hybrid_results = graph
        .hybrid_search_and_traverse("author_bio", "database engine", 5, 2)
        .unwrap();

    assert_eq!(hybrid_results.len(), 1);
    let (hit, paths) = &hybrid_results[0];
    assert_eq!(hit.node_id, author);
    // Traversing from author reaches both project and paper
    assert!(!paths.is_empty());
    let reached_targets: Vec<NodeId> = paths.iter().map(|p| p.end_node()).collect();
    assert!(reached_targets.contains(&project));
    assert!(reached_targets.contains(&paper));
}

#[test]
fn test_standalone_database_fulltext_wal_persistence() {
    let test_dir = std::env::temp_dir().join(format!("rustygraph_ft_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&test_dir);

    let config = DatabaseConfig {
        data_dir: test_dir.clone(),
        fsync_policy: "always".to_string(),
        keep_snapshots: 3,
        auto_checkpoint_ops: 100,
    };

    {
        let db = StandaloneDatabase::open(config.clone()).unwrap();

        let n1 = db
            .create_node(
                vec!["Doc".to_string()],
                [("summary".to_string(), PropertyValue::from("Distributed graph storage with write-ahead log."))]
                    .into_iter()
                    .collect(),
            )
            .unwrap();

        db.create_fulltext_index("doc_ft", "Doc", "summary").unwrap();

        let hits = db.query_fulltext_index("doc_ft", "write-ahead", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].node_id, n1);

        // Checkpoint to flush snapshots
        db.checkpoint().unwrap();
    }

    // Reopen database and verify WAL recovery restored full-text indices
    {
        let db = StandaloneDatabase::open(config).unwrap();
        let hits = db.query_fulltext_index("doc_ft", "storage", 5).unwrap();
        assert_eq!(hits.len(), 1);
    }

    let _ = fs::remove_dir_all(&test_dir);
}

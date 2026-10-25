use rusty_graph::prelude::*;
use rusty_graph::vector::{cosine_similarity, dot_product, euclidean_similarity};

#[test]
fn test_vector_similarity_metrics() {
    let v1 = vec![1.0f32, 0.0, 0.0];
    let v2 = vec![1.0f32, 0.0, 0.0];
    let v3 = vec![0.0f32, 1.0, 0.0];
    let v4 = vec![-1.0f32, 0.0, 0.0];

    // Cosine similarity
    assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-6); // Identical
    assert!((cosine_similarity(&v1, &v3) - 0.0).abs() < 1e-6); // Orthogonal
    assert!((cosine_similarity(&v1, &v4) - (-1.0)).abs() < 1e-6); // Opposite

    // Euclidean similarity
    assert!((euclidean_similarity(&v1, &v2) - 1.0).abs() < 1e-6); // Dist 0 -> 1.0
    assert!(euclidean_similarity(&v1, &v3) < 1.0);

    // Dot product
    assert_eq!(dot_product(&v1, &v2), 1.0);
    assert_eq!(dot_product(&v1, &v3), 0.0);
}

#[test]
fn test_vector_index_creation_and_search() -> Result<()> {
    let mut graph = Graph::new();

    // Create 3-dimensional vector index on (:Document {embedding}) with Cosine metric
    graph.create_vector_index(
        "doc_embeddings",
        "Document",
        "embedding",
        3,
        VectorSimilarity::Cosine,
    )?;
    assert!(graph.has_vector_index("doc_embeddings"));

    // Insert documents with 3D embeddings
    // Doc 1: AI / Machine Learning (oriented along X axis)
    let d1 = graph.create_node_with(|n| {
        n.add_label("Document");
        n.set_prop("title", "Deep Learning Architectures");
        n.set_prop("embedding", vec![0.9f32, 0.1, 0.0]);
    })?;

    // Doc 2: Graph Databases (oriented along Y axis)
    let _d2 = graph.create_node_with(|n| {
        n.add_label("Document");
        n.set_prop("title", "Property Graph Theory");
        n.set_prop("embedding", vec![0.1f32, 0.9, 0.1]);
    })?;

    // Doc 3: Biology / Genetics (oriented along Z axis)
    let d3 = graph.create_node_with(|n| {
        n.add_label("Document");
        n.set_prop("title", "Genome Sequencing");
        n.set_prop("embedding", vec![0.0f32, 0.1, 0.95]);
    })?;

    // Search with query vector close to Doc 1: [1.0, 0.0, 0.0]
    let query_ai = vec![1.0f32, 0.0, 0.0];
    let results = graph.query_vector_index("doc_embeddings", 2, &query_ai)?;

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].node_id, d1); // Doc 1 should be closest match
    assert!(results[0].score > 0.9);
    assert!(results[0].score > results[1].score); // Descending score order

    // Search with query vector close to Doc 3: [0.0, 0.0, 1.0]
    let query_bio = vec![0.0f32, 0.0, 1.0];
    let bio_results = graph.query_vector_index("doc_embeddings", 1, &query_bio)?;
    assert_eq!(bio_results.len(), 1);
    assert_eq!(bio_results[0].node_id, d3);

    Ok(())
}

#[test]
fn test_vector_dimension_mismatch_error() -> Result<()> {
    let mut graph = Graph::new();
    graph.create_vector_index("idx", "Item", "vec", 4, VectorSimilarity::Cosine)?;

    // Attempt to insert 3D vector into 4D index
    let _err_insert = graph.create_node_with(|n| {
        n.add_label("Item");
        n.set_prop("vec", vec![1.0f32, 2.0, 3.0]); // 3 dims instead of 4
    });
    // Vector index handles dimension mismatch gracefully (or reports error)
    // Querying with wrong dimension:
    let err_query = graph.query_vector_index("idx", 5, &[1.0f32, 2.0]);
    assert!(err_query.is_err());
    assert!(matches!(err_query, Err(GraphError::InvalidOperation(_))));

    Ok(())
}

#[test]
fn test_vector_index_mutation_and_deletion() -> Result<()> {
    let mut graph = Graph::new();
    graph.create_vector_index("v_idx", "Node", "emb", 2, VectorSimilarity::Euclidean)?;

    let n = graph.create_node_with(|node| {
        node.add_label("Node");
        node.set_prop("emb", vec![1.0f32, 1.0]);
    })?;

    let res = graph.query_vector_index("v_idx", 10, &[1.0f32, 1.0])?;
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].node_id, n);

    // Update embedding to [10.0, 10.0]
    graph.set_node_property(n, "emb", vec![10.0f32, 10.0])?;
    let updated_res = graph.query_vector_index("v_idx", 10, &[1.0f32, 1.0])?;
    assert_eq!(updated_res.len(), 1);
    // Score should now be much lower
    assert!(updated_res[0].score < res[0].score);

    // Delete node
    graph.delete_node(n)?;
    let empty_res = graph.query_vector_index("v_idx", 10, &[1.0f32, 1.0])?;
    assert!(empty_res.is_empty());

    // Drop index
    assert!(graph.drop_vector_index("v_idx"));
    assert!(!graph.has_vector_index("v_idx"));

    Ok(())
}

#[test]
fn test_graph_rag_hybrid_vector_and_graph_traversal() -> Result<()> {
    let mut graph = Graph::new();
    graph.create_vector_index("concept_idx", "Concept", "vec", 2, VectorSimilarity::Cosine)?;

    // Concept: Machine Learning
    let ml = graph.create_node_with(|n| {
        n.add_label("Concept");
        n.set_prop("name", "Machine Learning");
        n.set_prop("vec", vec![1.0f32, 0.0]);
    })?;

    // Related Concepts connected by graph relationships
    let nn = graph.create_node_with(|n| {
        n.add_label("Concept");
        n.set_prop("name", "Neural Networks");
    })?;

    let transformers = graph.create_node_with(|n| {
        n.add_label("Concept");
        n.set_prop("name", "Transformers");
    })?;

    graph.create_edge(ml, nn, "SUBFIELD_OF")?;
    graph.create_edge(nn, transformers, "LEADS_TO")?;

    // GraphRAG search: find entry node with vector [0.95, 0.05], then traverse 2 hops!
    let rag_results = graph.vector_search_and_traverse("concept_idx", &[0.95f32, 0.05], 1, 2)?;

    assert_eq!(rag_results.len(), 1);
    let (matched_item, paths) = &rag_results[0];
    assert_eq!(matched_item.node_id, ml);
    assert!(matched_item.score > 0.95);

    // Paths should trace (ML) -> (NN) -> (Transformers)
    let reached_end_nodes: Vec<_> = paths.iter().map(|p| p.end_node()).collect();
    assert!(reached_end_nodes.contains(&ml));
    assert!(reached_end_nodes.contains(&nn));
    assert!(reached_end_nodes.contains(&transformers));

    Ok(())
}

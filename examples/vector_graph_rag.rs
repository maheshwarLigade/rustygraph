//! GraphRAG: Hybrid Vector Search & Knowledge Graph Traversal
//!
//! This example demonstrates building a GraphRAG (Retrieval-Augmented Generation)
//! pipeline using `rusty_graph`. We index research papers using semantic vector
//! embeddings, query for relevant context via Cosine similarity, and traverse the
//! graph topology to retrieve co-authors, citations, and related topics.

use rusty_graph::vector::VectorSimilarity;
use rusty_graph::{Graph, Result};

fn main() -> Result<()> {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║       rusty_graph — GraphRAG & Vector Database Demonstration      ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    let mut graph = Graph::new();

    // 1. Create Vector Index on Paper embeddings
    // Vector dimensions: [ML, NLP, GraphTheory, Systems]
    let embedding_dim = 4;
    graph.create_vector_index(
        "paper_embeddings",
        "Paper",
        "embedding",
        embedding_dim,
        VectorSimilarity::Cosine,
    )?;
    println!("Created Vector Index 'paper_embeddings' (dim: 4, metric: Cosine)\n");

    // 2. Create Topic Nodes
    let t_gnn = graph.create_node_with(|n| {
        n.add_label("Topic");
        n.set_prop("name", "Graph Neural Networks");
    })?;

    let t_rag = graph.create_node_with(|n| {
        n.add_label("Topic");
        n.set_prop("name", "Retrieval Augmented Generation");
    })?;

    let t_llm = graph.create_node_with(|n| {
        n.add_label("Topic");
        n.set_prop("name", "Large Language Models");
    })?;

    // 3. Create Authors
    let a_kipf = graph.create_node_with(|n| {
        n.add_label("Author");
        n.set_prop("name", "Thomas Kipf");
    })?;

    let a_velickovic = graph.create_node_with(|n| {
        n.add_label("Author");
        n.set_prop("name", "Petar Veličković");
    })?;

    let a_lewis = graph.create_node_with(|n| {
        n.add_label("Author");
        n.set_prop("name", "Patrick Lewis");
    })?;

    // 4. Create Paper Nodes with Embeddings
    // Vector format: [ML, NLP, GraphTheory, Systems]
    let p_gcn = graph.create_node_with(|n| {
        n.add_label("Paper");
        n.set_prop(
            "title",
            "Semi-Supervised Classification with Graph Convolutional Networks",
        );
        n.set_prop("year", 2017);
        n.set_prop("embedding", vec![0.85f32, 0.10, 0.95, 0.20]);
    })?;

    let p_gat = graph.create_node_with(|n| {
        n.add_label("Paper");
        n.set_prop("title", "Graph Attention Networks");
        n.set_prop("year", 2018);
        n.set_prop("embedding", vec![0.88f32, 0.15, 0.92, 0.25]);
    })?;

    let p_rag_paper = graph.create_node_with(|n| {
        n.add_label("Paper");
        n.set_prop(
            "title",
            "Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks",
        );
        n.set_prop("year", 2020);
        n.set_prop("embedding", vec![0.80f32, 0.95, 0.30, 0.40]);
    })?;

    let p_graph_rag = graph.create_node_with(|n| {
        n.add_label("Paper");
        n.set_prop(
            "title",
            "GraphRAG: Unlocking LLM Discovery on Complex Knowledge Networks",
        );
        n.set_prop("year", 2024);
        n.set_prop("embedding", vec![0.90f32, 0.85, 0.88, 0.50]);
    })?;

    // 5. Connect Relationships
    // Authorship
    graph.create_edge(a_kipf, p_gcn, "AUTHORED")?;
    graph.create_edge(a_velickovic, p_gat, "AUTHORED")?;
    graph.create_edge(a_lewis, p_rag_paper, "AUTHORED")?;

    // Topic tagging
    graph.create_edge(p_gcn, t_gnn, "COVERS")?;
    graph.create_edge(p_gat, t_gnn, "COVERS")?;
    graph.create_edge(p_rag_paper, t_rag, "COVERS")?;
    graph.create_edge(p_rag_paper, t_llm, "COVERS")?;
    graph.create_edge(p_graph_rag, t_gnn, "COVERS")?;
    graph.create_edge(p_graph_rag, t_rag, "COVERS")?;
    graph.create_edge(p_graph_rag, t_llm, "COVERS")?;

    // Citation network
    graph.create_edge(p_gat, p_gcn, "CITES")?;
    graph.create_edge(p_graph_rag, p_gat, "CITES")?;
    graph.create_edge(p_graph_rag, p_rag_paper, "CITES")?;

    println!(
        "Populated Knowledge Graph with {} nodes and {} edges.\n",
        graph.node_count(),
        graph.edge_count()
    );

    // ─────────────────────────────────────────────────────────────────
    // Step 1: Pure Vector Similarity Search
    // ─────────────────────────────────────────────────────────────────
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Step 1: Semantic Vector Search                              │");
    println!("└─────────────────────────────────────────────────────────────┘");
    // Incoming Query: "How can graph structures improve LLM retrieval?"
    // User query embedding: high in NLP (0.80) and GraphTheory (0.85)
    let query_vector = vec![0.75f32, 0.80, 0.85, 0.30];
    println!("Query Embedding: {:?}\n", query_vector);

    let hits = graph.query_vector_index("paper_embeddings", 2, &query_vector)?;
    for (i, hit) in hits.iter().enumerate() {
        let node = graph.get_node(hit.node_id).unwrap();
        let title = node.prop("title").unwrap();
        let year = node.prop("year").unwrap();
        println!(
            "  Hit #{}: Score: {:.4} | Node: {} | {} ({})",
            i + 1,
            hit.score,
            hit.node_id,
            title,
            year
        );
    }
    println!();

    // ─────────────────────────────────────────────────────────────────
    // Step 2: GraphRAG Hybrid Retrieval
    // Vector Search -> Seed Nodes -> Graph Traversal for Rich Context
    // ─────────────────────────────────────────────────────────────────
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Step 2: GraphRAG Hybrid Context Expansion                   │");
    println!("│ (Vector Seed -> 2-Hop Relationship Expansion)               │");
    println!("└─────────────────────────────────────────────────────────────┘");

    let rag_results = graph.vector_search_and_traverse(
        "paper_embeddings",
        &query_vector,
        1, // Top-1 most relevant seed paper
        2, // Expand up to 2 hops away
    )?;

    for (hit, paths) in rag_results {
        let seed_node = graph.get_node(hit.node_id).unwrap();
        let seed_title = seed_node.prop("title").unwrap();
        println!("Primary Seed Node: {} (\"{}\")", hit.node_id, seed_title);
        println!(
            "Expanded Graph Context ({} relevant paths discovered):",
            paths.len()
        );

        for path in &paths {
            if path.is_empty() {
                continue;
            }
            let target_node = graph.get_node(path.end_node()).unwrap();
            let labels: Vec<String> = target_node.labels.iter().cloned().collect();
            let label_str = labels.join(", ");
            let desc = target_node
                .prop("name")
                .or_else(|| target_node.prop("title"))
                .map(|p| p.to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            println!(
                "  ↳ Depth {}: Path {} (Target [{}]: \"{}\")",
                path.len(),
                path,
                label_str,
                desc
            );
        }
    }
    println!();

    // ─────────────────────────────────────────────────────────────────
    // Step 3: Graph-Augmented LLM Prompt Context Construction
    // ─────────────────────────────────────────────────────────────────
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Step 3: Synthesized GraphRAG Context for LLM Prompt         │");
    println!("└─────────────────────────────────────────────────────────────┘");

    let top_hit = &hits[0];
    let top_paper = graph.get_node(top_hit.node_id).unwrap();

    // Find all papers cited by top hit
    let citations: Vec<String> = graph
        .traverse(top_hit.node_id)
        .outgoing()
        .with_type("CITES")
        .min_depth(1)
        .max_depth(1)
        .paths()
        .iter()
        .filter_map(|p| graph.get_node(p.end_node()))
        .filter_map(|n| n.prop("title").map(|t| t.to_string()))
        .collect();

    // Find topics covered
    let topics: Vec<String> = graph
        .traverse(top_hit.node_id)
        .outgoing()
        .with_type("COVERS")
        .min_depth(1)
        .max_depth(1)
        .paths()
        .iter()
        .filter_map(|p| graph.get_node(p.end_node()))
        .filter_map(|n| n.prop("name").map(|t| t.to_string()))
        .collect();

    // Find authors of the cited foundational works
    let mut authors = Vec::new();
    for cite_path in graph
        .traverse(top_hit.node_id)
        .outgoing()
        .with_type("CITES")
        .min_depth(1)
        .max_depth(1)
        .paths()
    {
        for author_path in graph
            .traverse(cite_path.end_node())
            .incoming()
            .with_type("AUTHORED")
            .min_depth(1)
            .max_depth(1)
            .paths()
        {
            if let Some(author_node) = graph.get_node(author_path.end_node()) {
                if let Some(name) = author_node.prop("name") {
                    authors.push(name.to_string());
                }
            }
        }
    }

    println!("Context Block injected into Prompt:");
    println!("───────────────────────────────────────────────────────────────");
    println!("Primary Reference: {}", top_paper.prop("title").unwrap());
    println!("Topics:            {}", topics.join(", "));
    println!("Foundational Work: {}", citations.join(" | "));
    println!("Key Researchers:   {}", authors.join(", "));
    println!("───────────────────────────────────────────────────────────────");
    println!("\nGraphRAG hybrid workflow successfully executed!");

    Ok(())
}

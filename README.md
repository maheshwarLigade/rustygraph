# RustyGraph 🦀🕸️

An ultra-fast, in-memory labeled property graph database library written in Rust, inspired by Neo4j.

`rusty_graph` provides high-performance in-memory graph storage, property indexing, path traversal, declarative pattern matching, and graph data science (GDS) algorithms with zero external database dependencies.

---

## ✨ Features

- **Neo4j Property Graph Model**:
  - **Nodes**: Unique generational 64-bit IDs, multiple labels (`:Person:Engineer`), rich key-value properties.
  - **Relationships**: Directed edges with exactly one relationship type (`:KNOWS`), rich properties, and incoming/outgoing/undirected traversal.
  - **Algebraic Property Values**: Rich dynamic types (`Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `DateTime`, `List`, `Map`).
- **High-Performance Generational Storage**:
  - Contiguous generational arena with $O(1)$ random lookups, insertions, and safe slot reuse preventing use-after-free or stale handle bugs.
  - Inline `SmallVec` adjacency vectors ensuring nodes with $\le 8$ degrees allocate zero heap memory on traversals.
- **Secondary Indexing & Constraints**:
  - `BTreeMap`-backed property indexing for $O(1)$ exact lookups and $O(\log N)$ range queries.
  - Unique constraints (e.g. `CREATE CONSTRAINT ON (u:User) ASSERT u.username IS UNIQUE`).
- **Fluent Traversal Engine**:
  - Breadth-First (BFS) and Depth-First (DFS) traversals with depth bounds (`min_depth..max_depth`), relationship filtering, and cycle handling (`NodeUnique`, `RelationshipUnique`, `AllPaths`).
- **Graph Data Science (GDS) Algorithms**:
  - **Pathfinding**: BFS Unweighted Shortest Path, Dijkstra Weighted Shortest Path, and A* algorithm.
  - **Centrality**: Degree Centrality and iterative power-method PageRank.
  - **Community Detection**: Weakly Connected Components (WCC via Disjoint-Set / Union-Find) and Strongly Connected Components (SCC via Tarjan's algorithm).
  - **Topological Analysis**: DAG validation & Kahn's Topological Sort.
  - **Similarity**: Jaccard similarity coefficient and common neighbor identification.
- **Declarative Pattern Matching (Cypher-style DSL)**:
  - Strongly typed query builder supporting multi-hop graph patterns, `WHERE` predicates, `ORDER BY`, pagination (`SKIP`/`LIMIT`), and arbitrary projections.
- **Concurrency & Persistence**:
  - `ConcurrentGraph` with `Arc<RwLock>` for multi-threaded reads/writes.
  - Full JSON export and import (`to_json` / `from_json`).

---

## 🚀 Quick Start

Add `rusty_graph` to your `Cargo.toml`:

```toml
[dependencies]
rusty_graph = { path = "." }
```

### Basic Example

```rust
use rusty_graph::prelude::*;

fn main() -> Result<()> {
    let mut graph = Graph::new();

    // 1. Create nodes with labels and properties
    let alice = graph.create_node_with(|n| {
        n.add_label("Person");
        n.set_prop("name", "Alice");
        n.set_prop("age", 30);
    })?;

    let bob = graph.create_node_with(|n| {
        n.add_label("Person");
        n.set_prop("name", "Bob");
        n.set_prop("age", 28);
    })?;

    // 2. Create a directed relationship with properties
    graph.create_edge_with(alice, bob, "KNOWS", |r| {
        r.set_prop("since", 2021);
        r.set_prop("closeness", 0.95);
    })?;

    // 3. Declarative Cypher-style query
    // MATCH (p:Person)-[:KNOWS]->(friend:Person) WHERE friend.age < 30 RETURN p.name, friend.name
    let matches: Vec<(String, String)> = graph.query()
        .match_node("p").with_label("Person")
        .outgoing("KNOWS", "friend").with_label("Person")
        .filter(|ctx| ctx.prop("friend", "age").and_then(|v| v.as_i64()).unwrap_or(0) < 30)
        .project(|ctx| {
            (
                ctx.prop("p", "name").unwrap().as_str().unwrap().to_string(),
                ctx.prop("friend", "name").unwrap().as_str().unwrap().to_string(),
            )
        });

    println!("Matched: {:?}", matches);
    // Output: Matched: [("Alice", "Bob")]

    Ok(())
}
```

---

## 📊 Graph Algorithms (GDS) Example

```rust
use rusty_graph::prelude::*;

fn main() -> Result<()> {
    let mut graph = Graph::new();

    let a = graph.create_node();
    let b = graph.create_node();
    let c = graph.create_node();

    graph.create_edge_with(a, b, "ROAD", |r| { r.set_prop("distance", 10.0); })?;
    graph.create_edge_with(b, c, "ROAD", |r| { r.set_prop("distance", 5.0); })?;

    // Dijkstra weighted shortest path
    let (path, cost) = algo::dijkstra_by_prop(&graph, a, c, Direction::Outgoing, "distance")
        .expect("path exists");
    println!("Shortest path cost: {}, hops: {}", cost, path.len());

    // PageRank centrality
    let ranks = algo::pagerank(&graph, 0.85, 50, 1e-6);
    println!("PageRank scores: {:?}", ranks);

    Ok(())
}
```

---

## 🛠️ Running Tests and Examples

Run all unit and doc tests:
```bash
cargo test
```

Run the complete social network demonstration:
```bash
cargo run --example neo4j_social_network
```

---

## 🗺️ Roadmap & Phase Status

| Phase | Description | Status |
| :--- | :--- | :--- |
| **Phase 1** | Core Property Graph Model (Nodes, Edges, Properties, Arena Storage, Adjacency) | ✅ Complete |
| **Phase 2** | Secondary Property Indexing (`BTreeMap`), Unique Constraints, Graph Stats | ✅ Complete |
| **Phase 3** | Traversal Engine (`Traverser`, `Path`, BFS/DFS, Cycle Management) | ✅ Complete |
| **Phase 4** | Graph Data Science (Dijkstra, A*, BFS Shortest Path, PageRank, WCC, SCC, TopoSort, Jaccard) | ✅ Complete |
| **Phase 5** | Declarative Pattern Matching Query DSL (`QueryBuilder`, Filters, Order, Project) | ✅ Complete |
| **Phase 6** | Persistence (JSON Export/Import) & Concurrency (`ConcurrentGraph`) | ✅ Complete |

---

## 📄 License

Licensed under MIT OR Apache-2.0.

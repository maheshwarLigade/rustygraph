# RustyGraph 🦀🕸️

[![Latest Release](https://img.shields.io/github/v/release/maheshwarLigade/rustygraph?style=flat-square&color=2ea44f&label=latest%20release)](https://github.com/maheshwarLigade/rustygraph/releases/latest)
[![GitHub Downloads](https://img.shields.io/github/downloads/maheshwarLigade/rustygraph/total?style=flat-square&color=blue)](https://github.com/maheshwarLigade/rustygraph/releases)
[![CI Build](https://img.shields.io/github/actions/workflow/status/maheshwarLigade/rustygraph/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/maheshwarLigade/rustygraph/actions/workflows/ci.yml)
[![Docker Image](https://img.shields.io/badge/docker-ghcr.io-blue?style=flat-square&logo=docker)](https://github.com/maheshwarLigade/rustygraph/pkgs/container/rustygraph)
[![Documentation](https://img.shields.io/badge/docs-GitHub%20Pages-informational?style=flat-square)](https://maheshwarligade.github.io/rustygraph/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=flat-square)](LICENSE)

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
  - **Centrality**: Degree Centrality, iterative power-method PageRank, **Personalized PageRank (Random Walk with Restart)**, and **Betweenness Centrality (Brandes' Algorithm $O(V \cdot E)$)**.
  - **Community Detection**: Weakly Connected Components (WCC via Disjoint-Set / Union-Find) and Strongly Connected Components (SCC via Tarjan's algorithm).
  - **Topological Analysis**: DAG validation & Kahn's Topological Sort.
  - **Similarity**: Jaccard similarity coefficient and common neighbor identification.
- **Declarative Pattern Matching & Cypher Query Engine**:
  - Native Cypher query string parser, lexer, AST evaluation (`graph.cypher(...)`), and tabular ASCII result printer.
  - **Atomic Upsert `MERGE` Clause**: Idempotently matches existing entities or creates new ones, executing conditional `ON CREATE SET` or `ON MATCH SET` blocks.
  - **Variable-Length Path Expansion**: Multi-hop pattern matching syntax `*`, `*3`, `*1..3`, and `*..5` executed via high-performance breadth-first search.
  - **List Aggregations & DISTINCT Projections**: Flexible columnar deduplication (`RETURN DISTINCT ...`), `collect(n.prop)`, and `count(DISTINCT n.prop)`.
  - **Query Plan Caching**: Thread-safe LRU AST precompilation cache (`QueryPlanCache`) avoiding repeated lexing and parsing on hot queries.
  - **Parameterized Cypher Execution**: Safe and reusable prepared queries using `$param` syntax (`MATCH (n:Person) WHERE n.role = $role RETURN n`).
  - Strongly typed query builder supporting multi-hop graph patterns, `WHERE` predicates, `ORDER BY`, pagination (`SKIP`/`LIMIT`), and arbitrary projections.
- **Inverted Full-Text Search & Hybrid GraphRAG**:
  - BM25 and TF-IDF relevance scoring with tokenization, stop-word elimination, and prefix search.
  - Inverted postings index automatically kept in sync across node mutations, property updates, and deletions.
  - **Hybrid Search**: Seamlessly find entry nodes by keyword relevance, then execute topological graph traversals (`hybrid_search_and_traverse`).
- **Vector Database & Semantic Search**:
  - In-memory vector indexing with multiple similarity metrics: **Cosine Similarity**, **Euclidean Distance**, and **Dot Product**.
  - **SIMD Auto-Vectorization Loop Unrolling**: 8-lane parallel accumulator unrolling for high-throughput distance calculations without CPU pipeline stalls.
  - Dynamic dimension validation, top-$k$ nearest neighbor search, and automatic index synchronization upon node insertions, property updates, and deletions.
  - Native **GraphRAG** hybrid search (`vector_search_and_traverse`) combining semantic vector retrieval with relational knowledge graph expansion.
- **Embedded Interactive Web Studio**:
  - Zero-dependency modern dark-mode visual console served directly from `GET /` and `GET /ui` on `rustygraph-server`.
  - Force-directed interactive canvas graph visualizer, parameter-enabled Cypher editor, live database telemetry, and full-text keyword explorer.
- **Distributed Clustering & CAP Theorem Guarantees**:
  - **Automatic State Snapshot Synchronization**: Cold or rejoining cluster nodes automatically fetch snapshot state on cluster join (`SyncSnapshotRequest`/`SyncSnapshotResponse`) for instantaneous catch-up.
  - **Configurable CP Mode (Neo4j Causal Clustering)**: Strict majority quorum writes ($> N/2$), leader election, and automatic write rejection on minority partitions to strictly prevent split-brain.
  - **Configurable AP Mode (High Availability)**: 100% write availability across all partitions with Lamport logical clocks and anti-entropy Last-Write-Wins (LWW) reconciliation upon healing.
  - **Zero-Dependency Virtual Network**: In-memory network partition simulation (`network.partition(...)`, `network.isolate(...)`, `network.heal()`) for reproducible distributed testing.
- **Supernode & Dense Hub Skew Mitigation**:
  - Solves the classic power-law degree skew problem where high-degree hubs cause out-of-memory errors and traversal latency spikes.
  - Dynamic `is_supernode(node, threshold)` detection and zero-copy cursor/paginated edge scanning (`incident_edges_paged(node, dir, rel_type, offset, limit)`).
  - Traversal edge-capping (`Traverser::with_supernode_limit(limit)`) preventing combinatorial path explosions on hubs.
- **Graph as NoSQL Multi-Model Fusion**:
  - **Nested Document Paths**: Seamless dot-notation traversal and mutation inside complex JSON documents in openCypher and property maps (`u.profile.address.city`).
  - **High-Throughput Bulk Ingestion**: Low-latency batch ingestion endpoints (`POST /bulk/nodes`, `POST /bulk/edges`) with atomic WAL logging.
  - **Temporal TTL Expiration**: Automatic Time-To-Live expiration and cascading background pruning on ephemeral nodes (`POST /nodes/:id/ttl`, `POST /ttl/prune`).
- **Production Dockerization**:
  - Multi-stage minimal production `Dockerfile` with unprivileged user security and built-in healthchecks.
  - Zero-configuration `docker-compose.yml` supporting standalone and 3-node distributed clusters.
- **Model Context Protocol (MCP) Server**:
  - Native JSON-RPC 2.0 stdio server (`rustygraph-mcp`) connecting RustyGraph directly to Anthropic Claude Desktop, Cursor, Antigravity, and autonomous AI agents.
  - Standard MCP tools: `cypher_query` (with $params, MERGE, variable-length paths), `vector_search`, `graph_rag`, `fulltext_search`, `get_schema`, and `run_algorithm` (PageRank, Personalized PageRank, Betweenness Centrality, Shortest Path, WCC, SCC, Degree Centrality).
  - Standard MCP resources: `rustygraph://schema` and `rustygraph://stats`.
  - Supports embedded in-memory graphs, persistent local WAL databases (`--data <DIR>`), and remote `rustygraph-server` clusters (`--url <URL>`).
- **Concurrency & Persistence**:
  - `ConcurrentGraph` with `Arc<RwLock>` for multi-threaded reads/writes.
  - Full JSON export and import (`to_json` / `from_json`).

---

## 📦 Pre-Built Binaries & Release Downloads

Pre-compiled, standalone binaries (`rustygraph-server`, `rustygraph-cli`, `rustygraph-mcp`) are published for all major operating systems on every release. You do not need Rust or any C/C++ toolchain to run RustyGraph!

### 1. Download from GitHub Releases

The latest release packages and SHA-256 checksums are always published on **[GitHub Releases (Latest)](https://github.com/maheshwarLigade/rustygraph/releases/latest)**.

| Operating System / Architecture | Target Tuple | Format | Release Asset Link |
| :--- | :--- | :--- | :--- |
| **Linux x86_64 (glibc)** | `x86_64-unknown-linux-gnu` | `.tar.gz` | [Download Linux x86_64](https://github.com/maheshwarLigade/rustygraph/releases/latest) |
| **Linux x86_64 (musl static)** | `x86_64-unknown-linux-musl` | `.tar.gz` | [Download Linux musl Static](https://github.com/maheshwarLigade/rustygraph/releases/latest) |
| **Linux ARM64 / aarch64** | `aarch64-unknown-linux-gnu` | `.tar.gz` | [Download Linux ARM64](https://github.com/maheshwarLigade/rustygraph/releases/latest) |
| **macOS Apple Silicon (M1/M2/M3/M4)** | `aarch64-apple-darwin` | `.tar.gz` | [Download macOS ARM64](https://github.com/maheshwarLigade/rustygraph/releases/latest) |
| **macOS Intel** | `x86_64-apple-darwin` | `.tar.gz` | [Download macOS Intel](https://github.com/maheshwarLigade/rustygraph/releases/latest) |
| **Windows x86_64** | `x86_64-pc-windows-msvc` | `.zip` | [Download Windows x64](https://github.com/maheshwarLigade/rustygraph/releases/latest) |

Every release bundle contains:
- `bin/rustygraph-server` - Standalone high-throughput HTTP REST engine + web studio
- `bin/rustygraph-cli` - Interactive Cypher REPL and administration client
- `bin/rustygraph-mcp` - Standard Model Context Protocol server for AI agents
- `lib/` - Dynamic C-FFI shared libraries for Go and Java bindings
- `SHA256SUMS.txt` & `.sha256` checksum files for integrity verification

### 2. Automated Download & Install (Linux / macOS curl)

Fetch and unpack the latest release archive automatically via the GitHub API:

```bash
# Choose target (e.g. x86_64-unknown-linux-musl, aarch64-unknown-linux-gnu, aarch64-apple-darwin)
TARGET="x86_64-unknown-linux-musl"

# Discover latest tag and download archive
TAG=$(curl -sL https://api.github.com/repos/maheshwarLigade/rustygraph/releases/latest | grep '"tag_name":' | cut -d'"' -f4)
curl -sLO "https://github.com/maheshwarLigade/rustygraph/releases/download/${TAG}/rustygraph-${TAG}-${TARGET}.tar.gz"

# Verify SHA256 checksum
curl -sLO "https://github.com/maheshwarLigade/rustygraph/releases/download/${TAG}/rustygraph-${TAG}-${TARGET}.tar.gz.sha256"
sha256sum -c "rustygraph-${TAG}-${TARGET}.tar.gz.sha256"

# Unpack and start
tar -xzf "rustygraph-${TAG}-${TARGET}.tar.gz"
cd "rustygraph-${TAG}-${TARGET}/bin"
./rustygraph-server --help
```

### 3. Windows PowerShell One-Liner

```powershell
$TAG = (Invoke-RestMethod "https://api.github.com/repos/maheshwarLigade/rustygraph/releases/latest").tag_name
Invoke-WebRequest "https://github.com/maheshwarLigade/rustygraph/releases/download/$TAG/rustygraph-$TAG-x86_64-pc-windows-msvc.zip" -OutFile "rustygraph.zip"
Expand-Archive -Path "rustygraph.zip" -DestinationPath "rustygraph"
.\rustygraph\bin\rustygraph-server.exe --help
```

### 4. Production Docker Container (GHCR)

You can also run pre-built multi-architecture containers (`linux/amd64`, `linux/arm64`) directly from GitHub Container Registry:

```bash
# Pull and run the latest image
docker pull ghcr.io/maheshwarligade/rustygraph:latest
docker run -d -p 8000:8000 -v ./rustygraph_data:/data ghcr.io/maheshwarligade/rustygraph:latest
```

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

## 🔍 Native Cypher Query Support

`rusty_graph` includes a built-in Cypher query parser and execution engine:

```rust
use rusty_graph::prelude::*;

fn main() -> Result<()> {
    let mut graph = Graph::new();
    // ... populate graph ...

    // 1. Variable-length path query with DISTINCT projection
    let result = graph.cypher(
        "MATCH (p:Person {name: 'Alice'})-[*1..3]->(target) \
         RETURN DISTINCT target.name, target.role"
    )?;
    println!("{}", result.to_ascii_table());

    // 2. Aggregation with collect(...) and count(...)
    let agg = graph.cypher(
        "MATCH (p:Person)-[:CONTRIBUTES_TO]->(d:Database) \
         RETURN d.name, count(p) as total, collect(p.name) as contributors"
    )?;
    println!("{}", agg.to_ascii_table());

    Ok(())
}
```

---

## 🧠 Vector Database & GraphRAG Hybrid Search

`rusty_graph` provides native vector indexing for embedding-based nearest neighbor search and relational GraphRAG:

```rust
use rusty_graph::prelude::*;
use rusty_graph::vector::VectorSimilarity;

fn main() -> Result<()> {
    let mut graph = Graph::new();

    // 1. Create a Vector Index on 'Paper' embeddings using Cosine similarity
    graph.create_vector_index(
        "paper_embeddings",
        "Paper",
        "embedding",
        4, // 4-dimensional vectors
        VectorSimilarity::Cosine,
    )?;

    // 2. Hybrid GraphRAG: Find top 2 nearest papers and expand 2 hops into citations/topics
    let query_vector = vec![0.75, 0.80, 0.85, 0.30];
    let rag_results = graph.vector_search_and_traverse(
        "paper_embeddings",
        &query_vector,
        2, // top 2 seed documents
        2, // expand 2 hops
    )?;

    for (hit, paths) in rag_results {
        println!("Seed Node {} with similarity score: {:.4}", hit.node_id, hit.score);
        for path in paths {
            println!("  Found context path: {}", path);
        }
    }

    Ok(())
}
```

---

## 🌐 Distributed Cluster & CAP Theorem Guarantees

`rusty_graph` provides built-in distributed clustering with configurable **CP** and **AP** modes backed by an in-memory virtual network simulation:

```rust
use rusty_graph::cluster::{ClusterCommand, ClusterMode, ClusterNode, VirtualNetwork};
use rusty_graph::id::NodeId;
use std::sync::Arc;
use std::collections::HashMap;

fn main() -> rusty_graph::Result<()> {
    let network = Arc::new(VirtualNetwork::new());
    let members = vec![1, 2, 3, 4, 5];

    // 1. Create a 5-node CP cluster (Strict Quorum)
    let mut n1 = ClusterNode::new(1, members.clone(), ClusterMode::CP, network.clone());
    n1.campaign()?; // Node 1 elected Leader via quorum

    // 2. Simulate network partition: minority {1, 2} vs majority {3, 4, 5}
    network.partition(&[1, 2], &[3, 4, 5]);

    // 3. Minority write attempt: REJECTED to strictly prevent split-brain!
    let cmd = ClusterCommand::CreateNode {
        id: NodeId(101),
        labels: vec!["Account".to_string()],
        properties: HashMap::new(),
    };
    assert!(n1.write(cmd).is_err()); // QuorumUnavailable!

    // 4. Heal partition and reconcile
    network.heal();
    Ok(())
}
```

---

## 🐹 Go Language SDK

`rusty_graph` provides an idiomatic, high-performance Go SDK in `bindings/go/rustygraph`:

```go
package main

import (
	"fmt"
	"github.com/mahesh/rusty-graph/bindings/go/rustygraph"
)

func main() {
	graph, err := rustygraph.New()
	if err != nil {
		panic(err)
	}
	defer graph.Close()

	// Create nodes with labels & properties
	aliceID, _ := graph.CreateNode([]string{"Person"}, map[string]interface{}{"name": "Alice", "age": 30})
	bobID, _ := graph.CreateNode([]string{"Person"}, map[string]interface{}{"name": "Bob", "age": 28})
	graph.CreateEdge(aliceID, bobID, "KNOWS", map[string]interface{}{"since": 2021})

	// Run Cypher query directly in Go
	res, err := graph.Cypher("MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p.name, f.name")
	if err != nil {
		panic(err)
	}

	fmt.Printf("Columns: %v\n", res.Columns)
	fmt.Printf("Rows: %v\n", res.Rows) // Output: [[Alice Bob]]
}
```

Run Go unit tests:
```bash
cd bindings/go/rustygraph && go test -v .
```

---

## ☕ Java Language SDK (Java 21 FFM)

`rusty_graph` provides an official Java 21 SDK in `bindings/java` powered by Java's Foreign Function & Memory (FFM) API:

```java
import com.rustygraph.RustyGraph;
import com.rustygraph.CypherResult;
import java.util.List;
import java.util.Map;

public class Main {
    public static void main(String[] args) {
        try (RustyGraph graph = RustyGraph.create()) {
            // Create nodes and relationships
            long alice = graph.createNode(List.of("Person"), Map.of("name", "Alice", "age", 30));
            long bob = graph.createNode(List.of("Person"), Map.of("name", "Bob", "age", 28));
            graph.createEdge(alice, bob, "KNOWS", Map.of("since", 2021));

            // Execute Cypher queries natively from Java
            CypherResult res = graph.cypher("MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p.name, f.name");
            System.out.println("Columns: " + res.columns());
            System.out.println("Rows: " + res.rows());
        }
    }
}
```

Run Java SDK test suite:
```bash
javac --enable-preview --release 21 -d target/classes bindings/java/src/main/java/com/rustygraph/*.java bindings/java/src/test/java/com/rustygraph/*.java
java --enable-preview -cp target/classes com.rustygraph.RustyGraphTest
```

---

## 🛠️ Running Tests and Examples

Run all unit, integration, and doc tests:
```bash
cargo test
```

Run the complete social network demonstration:
```bash
cargo run --example neo4j_social_network
```

Run the GraphRAG hybrid vector search demonstration:
```bash
cargo run --example vector_graph_rag
```

Run the CAP theorem distributed cluster demonstration:
```bash
cargo run --example cap_distributed_cluster
```

---

## 🌐 Standalone NoSQL Server & Interactive CLI

RustyGraph can run as a **standalone NoSQL distributed database server** with zero driver dependencies, accessible via standard HTTP REST, WebSockets, or the native interactive Cypher REPL CLI.

### Starting the Server (`rustygraph-server`)

RustyGraph adopts a **Layered Configuration Architecture** (Configuration over Convention with sensible defaults):

$$\text{CLI Flags} > \text{Environment Variables} > \text{Config File (\texttt{rustygraph.toml})} > \text{Default Conventions}$$

#### 1. Zero-Config Convention Mode
Run with sensible built-in defaults out of the box:
```bash
cargo run --bin rustygraph-server
```

#### 2. Declarative Configuration File Mode (`rustygraph.toml`)
Generate and use a dedicated configuration file for full declarative control:
```bash
# Generate sample configuration file in current directory
cargo run --bin rustygraph-server -- --init-config

# Launch using custom configuration file
cargo run --bin rustygraph-server -- --config ./rustygraph.toml
```

Sample `rustygraph.toml`:
```toml
[server]
http_addr = "0.0.0.0:8000"
auth_enabled = true
default_admin_password = "admin"

[cluster]
node_id = "node-1"
cluster_addr = "127.0.0.1:9001"
cluster_mode = "cp"
peer_addrs = []

[storage]
data_dir = "./data"
fsync_policy = "always"
keep_snapshots = 3
auto_checkpoint_ops = 5000
```

#### 3. Environment Variable Mode (12-Factor App / Docker)
Every setting can be configured via environment variables:
```bash
export RUSTYGRAPH_HTTP_ADDR="0.0.0.0:8000"
export RUSTYGRAPH_DATA_DIR="/var/lib/rustygraph"
export RUSTYGRAPH_CLUSTER_MODE="cp"
export RUSTYGRAPH_AUTH_ENABLED="true"
cargo run --bin rustygraph-server
```

#### 4. Command-Line Flag Overrides
CLI flags take highest precedence and override any file or environment setting:
```bash
cargo run --bin rustygraph-server -- --http 127.0.0.1:8000 --cluster 127.0.0.1:9001 --node-id node-1 --data-dir ./data
```

### Interactive Cypher CLI (`rustygraph-cli`)

Launch the terminal REPL client to query and manage the database:

```bash
# Launch interactive REPL
cargo run --bin rustygraph-cli -- --server http://127.0.0.1:8000

# One-shot batch execution
cargo run --bin rustygraph-cli -- --server http://127.0.0.1:8000 --exec "MATCH (p:Person) RETURN p.name, p.age"
```

In the interactive REPL:
```text
rustygraph> MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p.name, f.name;
┌────────┬────────┐
│ p.name │ f.name │
├────────┼────────┤
│ Alice  │ Bob    │
└────────┴────────┘
(1 row in 1.42ms)

rustygraph> :explain MATCH (a:Person)-[:KNOWS]->(b) WHERE a.rank > 1 RETURN a.name, b.name
+--------------------+---------------------------------------------------------+
| OPERATOR           | DETAILS                                                 |
+--------------------+---------------------------------------------------------+
| ProduceResults     | Columns: [a.name, b.name]                               |
|   Filter           | Predicate: (a.rank > 1)                                 |
|     ExpandAll      | (a)-[:KNOWS]->(b)                                       |
|       NodeScan     | Scan Label: Person                                      |
+--------------------+---------------------------------------------------------+

rustygraph> :schema
--- Graph Schema Catalog (HTTP 200) ---
{
  "nodes_total": 4,
  "edges_total": 4,
  "label_counts": { "Person": 4 },
  "relationship_type_counts": { "KNOWS": 4 },
  "property_indices": [
    { "label": "Person", "property": "name", "unique": false }
  ]
}

rustygraph> :algo pagerank {"damping": 0.85, "max_iterations": 20}
--- Algorithm Result (HTTP 200) ---
{
  "algorithm": "pagerank",
  "top_nodes": [
    { "node_id": 4294967299, "score": 0.416, "properties": { "name": "Diana" } },
    { "node_id": 4294967298, "score": 0.278, "properties": { "name": "Charlie" } }
  ]
}

rustygraph> :stats
--- Database Statistics (HTTP 200) ---
{
  "node_count": 4,
  "edge_count": 4,
  "estimated_memory_bytes": 65536,
  "active_transactions": 0,
  "current_wal_sequence": 8,
  "last_checkpoint_sequence": 5,
  "uncheckpointed_ops": 3
}

rustygraph> :cluster
--- Cluster Status (HTTP 200) ---
{
  "node_id": "node-1",
  "address": "127.0.0.1:9001",
  "mode": "CP (Consistency / Majority Quorum)",
  "is_leader": true,
  "current_term": 1,
  "active_peers": [],
  "known_leader": "node-1"
}
```

### 🎛️ Developer Studio & Web Cockpit
Navigate to `http://localhost:8000/` or `http://localhost:8000/ui` in any modern web browser to open the zero-dependency embedded **RustyGraph Studio Cockpit**:
- **Cypher Studio & Force-Directed Canvas**: Live visual graph exploration, node/edge inspection, inline editing/deletions, query history, and openCypher `EXPLAIN` query execution visualizer.
- **Graph Algorithm Studio**: Interactive runner for PageRank, Shortest Path (BFS & Dijkstra), Centrality (Degree & Brandes Betweenness), Connected Components (WCC & SCC), and Jaccard Node Similarity.
- **Schema & Index Management**: Graph structural catalog with real-time property, vector, and full-text index creation and deletion modals.
- **Hybrid Search**: Side-by-side BM25 full-text keyword ranking and SIMD-accelerated vector semantic search testing.
- **Live Telemetry & Transaction Monitor**: Real-time system gauges (memory, cache, WAL latency), active ACID snapshot isolation transaction inspection with live Commit/Rollback, on-demand checkpoints, and full JSON database export/import.
- **Interactive REST API Sandbox**: In-browser API explorer with sample JSON payloads and live execution.

### HTTP REST API Endpoints

| Method | Endpoint | Description | Sample Payload |
| :--- | :--- | :--- | :--- |
| `GET` | `/` or `/ui` | Embedded Interactive Web Studio Cockpit | `none` |
| `GET` | `/metrics` | Prometheus telemetry metrics export | `none` |
| `POST` | `/cypher` | Execute openCypher query (supports `EXPLAIN ` prefix) | `{"query": "MATCH (n) RETURN n"}` |
| `POST` | `/cypher/explain` | Generate AST execution plan and ASCII operator tree | `{"query": "MATCH (a)-[:KNOWS]->(b) RETURN a, b"}` |
| `GET` | `/schema` | Graph topology catalog, degrees, and active indexes | `none` |
| `GET` | `/indexes` | Full index catalog (property, vector, fulltext) | `none` |
| `POST` | `/indexes/property` | Create dynamic secondary B-tree property index | `{"label": "Person", "property": "name"}` |
| `DELETE` | `/indexes/property/:label/:prop` | Drop secondary property index | `none` |
| `DELETE` | `/vector/indexes/:name` | Drop vector similarity index | `none` |
| `DELETE` | `/fulltext/indexes/:name` | Drop BM25 full-text index | `none` |
| `POST` | `/algo/pagerank` | Compute PageRank or Personalized PageRank | `{"damping": 0.85, "max_iterations": 20}` |
| `POST` | `/algo/shortest-path` | Unweighted (BFS) or weighted (Dijkstra) pathfinding | `{"source": 1, "target": 4, "algorithm": "dijkstra", "weight_property": "weight"}` |
| `POST` | `/algo/centrality` | Compute Betweenness or Degree Centrality | `{"metric": "betweenness", "direction": "both", "normalized": true}` |
| `POST` | `/algo/community` | Community detection (WCC or Tarjan SCC) | `{"algorithm": "wcc"}` |
| `POST` | `/algo/similarity` | Jaccard node neighborhood similarity | `{"source": 1, "target": 2, "metric": "jaccard"}` |
| `POST` | `/tx/begin` | Begin a multi-statement ACID transaction | `{"timeout_ms": 30000}` |
| `POST` | `/tx/:id/cypher` | Execute a Cypher query inside an active transaction | `{"query": "CREATE (u:User {name: 'Alice'})"}` |
| `POST` | `/tx/:id/commit` | Commit transaction to WAL and live database | `none` |
| `POST` | `/tx/:id/rollback` | Rollback transaction, discarding staged changes | `none` |
| `GET` | `/tx/list` | List active transactions and remaining timeouts | `none` |
| `GET` | `/health` | Server health check | `none` |
| `GET` | `/stats` | Database nodes, edges, memory, and WAL metrics | `none` |
| `POST` | `/nodes` | Create labeled node with properties | `{"labels": ["User"], "properties": {"name": "Alice"}}` |
| `GET` | `/nodes/:id` | Fetch node properties & adjacent edges | `none` |
| `DELETE` | `/nodes/:id` | Cascading delete node & relationships | `none` |
| `POST` | `/nodes/:id/ttl` | Set automatic Time-To-Live (TTL) expiration on node | `{"ttl_seconds": 3600}` |
| `POST` | `/ttl/prune` | Trigger immediate cascade pruning of all expired nodes | `none` |
| `POST` | `/bulk/nodes` | High-throughput batch ingestion of nodes | `{"nodes": [{"labels": ["Dev"], "properties": {"k": "v"}}]}` |
| `POST` | `/bulk/edges` | High-throughput batch ingestion of edges | `{"edges": [{"source": 1, "target": 2, "rel_type": "KNOWS"}]}` |
| `POST` | `/edges` | Create relationship between nodes | `{"source": 1, "target": 2, "type": "KNOWS"}` |
| `GET` | `/edges/:id` | Fetch relationship details | `none` |
| `DELETE` | `/edges/:id` | Delete relationship | `none` |
| `POST` | `/vector/indexes` | Create vector similarity index | `{"name": "v", "label": "Doc", "prop_key": "vec", "dimension": 3}` |
| `POST` | `/vector/search` | Top-k vector nearest-neighbor search | `{"index": "v", "k": 5, "vector": [1.0, 0.0, 0.0]}` |
| `POST` | `/fulltext/indexes` | Create inverted fulltext index | `{"name": "ft", "label": "Article", "prop_key": "body"}` |
| `POST` | `/fulltext/search` | BM25 relevance keyword search | `{"index": "ft", "query": "graph database", "limit": 10}` |
| `POST` | `/checkpoint` | Force atomic snapshot & truncate WAL | `none` |
| `GET` | `/export` | Full graph topology JSON dump | `none` |
| `POST` | `/import` | Restore full database from JSON | Raw JSON dump string |
| `GET` | `/cluster/status`| Cluster role, term, active peers | `none` |
| `POST` | `/cluster/join` | Join an existing cluster peer | `{"peer_address": "127.0.0.1:9002"}` |

---

## 🐳 Docker Deployment

RustyGraph provides a multi-stage production Docker build and out-of-the-box Compose setup for both standalone and distributed deployments.

### 1. Standalone Deployment (Recommended for Single Nodes)

Run an ultra-fast standalone node with persistent storage and healthchecks:

```bash
# Using Docker Compose
docker compose up -d rustygraph

# Or using plain Docker CLI
docker build -t rustygraph:latest .
docker run -d \
  --name rustygraph \
  -p 8080:8080 \
  -v rustygraph_data:/data \
  rustygraph:latest
```

Verify status:
```bash
curl http://localhost:8080/health
# {"status":"healthy","engine":"rusty_graph"}
```

### 2. High-Availability 3-Node Distributed Cluster (CP Quorum)

Spin up an enterprise 3-node cluster with automated leader election, majority quorum replication, and minority write rejection:

```bash
docker compose --profile cluster up -d
```

Cluster topology:
- **Node 1 (CP Leader / Seed)**: HTTP `http://localhost:8081`, Cluster TCP `7001`
- **Node 2 (Follower)**: HTTP `http://localhost:8082`, Cluster TCP `7002`
- **Node 3 (Follower)**: HTTP `http://localhost:8083`, Cluster TCP `7003`

Check cluster status from any node:
```bash
curl http://localhost:8081/cluster/status
```

---

## 🤖 Model Context Protocol (MCP) Server

RustyGraph includes a native, production-grade [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server (`rustygraph-mcp`) compliant with the standard JSON-RPC 2.0 stdio transport. It enables AI assistants (Claude Desktop, Cursor, Antigravity, and autonomous LLM agents) to directly read, query, traverse, and execute graph algorithms over your graph database.

### 1. Launch Modes

```bash
# Mode A: Embedded in-memory graph
rustygraph-mcp

# Mode B: Persistent database backed by WAL log and snapshots
rustygraph-mcp --data ./rustygraph_data

# Mode C: Remote connection to a running rustygraph-server daemon
rustygraph-mcp --url http://127.0.0.1:8000
# Or shorthand:
rustygraph-mcp --remote
```

### 2. Claude Desktop Integration

Add `rustygraph` to your `claude_desktop_config.json` (`%APPDATA%\Claude\claude_desktop_config.json` on Windows or `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "rustygraph": {
      "command": "rustygraph-mcp",
      "args": ["--data", "C:/path/to/rustygraph_data"]
    }
  }
}
```

### 3. Exposed MCP Tools

| Tool | Parameters | Description |
| :--- | :--- | :--- |
| `cypher_query` | `query: string`, `params?: object` | Execute read/write openCypher queries with parameters, variable-length paths (`*1..3`), `MERGE`, and list aggregations. Returns ASCII tabular results. |
| `vector_search` | `index: string`, `vector: number[]`, `k?: number` | Perform top-$k$ nearest neighbor search using Cosine, Euclidean, or Dot Product similarity. |
| `graph_rag` | `index: string`, `vector: number[]`, `k?: number`, `hops?: number`, `direction?: string` | Hybrid GraphRAG combining semantic vector retrieval with topological relational neighborhood expansion. |
| `fulltext_search` | `index: string`, `query: string`, `top_k?: number`, `prefix?: boolean` | Query inverted BM25 / TF-IDF index for keywords, document matches, and relevance scores. |
| `get_schema` | *(none)* | Inspect complete graph schema: node labels, relationship types, edge counts, vector/fulltext indices, density, and average degree. |
| `run_algorithm` | `algorithm: string`, `params?: object` | Run graph algorithms: `pagerank`, `personalized_pagerank`, `betweenness_centrality`, `shortest_path`, `wcc`, `scc`, `degree_centrality`. |

### 4. Exposed MCP Resources

| URI | MIME Type | Description |
| :--- | :--- | :--- |
| `rustygraph://schema` | `application/json` | Graph schema metadata (label distributions, relationship types, indexed fields). |
| `rustygraph://stats` | `application/json` | Live database telemetry (node count, edge count, density, average degree, isolated nodes). |

---

## 🗺️ Roadmap & Phase Status

| Phase | Description | Status |
| :--- | :--- | :--- |
| **Phase 1** | Core Property Graph Model (Nodes, Edges, Properties, Arena Storage, Adjacency) | ✅ Complete |
| **Phase 2** | Secondary Property Indexing (`BTreeMap`), Unique Constraints, Graph Stats | ✅ Complete |
| **Phase 3** | Traversal Engine (`Traverser`, `Path`, BFS/DFS, Cycle Management) | ✅ Complete |
| **Phase 4** | Graph Data Science (Dijkstra, A*, BFS Shortest Path, PageRank, WCC, SCC, TopoSort, Jaccard) | ✅ Complete |
| **Phase 5** | Declarative Pattern Matching Query DSL (`QueryBuilder`, Filters, Order, Project) | ✅ Complete |
| **Phase 5+** | Native Cypher Query Engine (Lexer, Parser, AST Evaluator, Tabular ASCII Formatter) | ✅ Complete |
| **Phase 6** | Persistence (JSON Export/Import) & Concurrency (`ConcurrentGraph`) | ✅ Complete |
| **Phase 7** | Vector Database & GraphRAG (Vector Indexing, Cosine/Euclidean/Dot, Hybrid Search) | ✅ Complete |
| **Phase 8** | Distributed Cluster & CAP Theorem (Configurable CP/AP Modes, Split-Brain Prevention, Virtual Network) | ✅ Complete |
| **Phase 9** | Multi-Language SDKs & C FFI (Native Go SDK via dynamic linking & Java 21 SDK via FFM) | ✅ Complete |
| **Phase 10** | Standalone NoSQL Distributed Server & CLI (WAL Durability, TCP Clustering, REST API, REPL) | ✅ Complete |
| **Phase 11** | Full Cypher Mutations & ACID Transactions (CREATE/SET/DELETE, Aggregations, Isolation, Timeout Reaping) | ✅ Complete |
| **Phase 12** | Supernode Skew Mitigation, Graph-as-NoSQL Multi-Model Fusion & Production Dockerization | ✅ Complete |
| **Phase 13** | Model Context Protocol (MCP) Server for AI Agents (JSON-RPC stdio, Cypher, GDS, Vector GraphRAG) | ✅ Complete |

---

## 🤝 Contributing

Contributions, bug reports, and feature suggestions are warmly welcome! Please see our [Contributing Guide](CONTRIBUTING.md) for setup instructions, coding guidelines, testing workflows, and PR submission details.

---

## 📄 License
 
This project is licensed under the **Apache License, Version 2.0**.
See the [LICENSE](LICENSE) file for the full license text.

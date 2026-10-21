# Contributing to RustyGraph 🦀🕸️

Thank you for your interest in contributing to **RustyGraph**! We are building an ultra-fast, in-memory labeled property graph database in Rust with openCypher query evaluation, vector database nearest-neighbor search, distributed clustering, and native multi-language SDKs (Go, Java).

Whether you are fixing a bug, adding new Cypher query clauses, optimizing graph algorithms, extending language SDKs, or improving documentation, your contributions are welcome!

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How Can I Contribute?](#how-can-i-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Submitting Pull Requests](#submitting-pull-requests)
- [Development Setup](#development-setup)
  - [Prerequisites](#prerequisites)
  - [Cloning the Repository](#cloning-the-repository)
  - [Building the Project](#building-the-project)
- [Project Architecture](#project-architecture)
- [Running Tests & Verifications](#running-tests--verifications)
  - [Rust Core Tests](#rust-core-tests)
  - [Go SDK Tests](#go-sdk-tests)
  - [Java SDK Tests](#java-sdk-tests)
  - [Running Example Applications](#running-example-applications)
- [Coding Standards & Style Guide](#coding-standards--style-guide)
  - [Rust Code Standards](#rust-code-standards)
  - [Commit Message Conventions](#commit-message-conventions)
- [License](#license)

---

## Code of Conduct

We are dedicated to providing a welcoming, inclusive, and harassment-free experience for everyone. Please be respectful, constructive, and collaborative in all communications, issues, and pull request reviews.

---

## How Can I Contribute?

### Reporting Bugs

Before creating a bug report, please check existing issues to avoid duplicates. When filing an issue, please include:
1. **Description**: Clear summary of the issue.
2. **Steps to Reproduce**: Minimal code snippet or steps reproducing the bug.
3. **Expected vs. Actual Behavior**: What you expected to happen vs what actually happened.
4. **Environment**: Operating System, Rust version (`rustc --version`), Go version (`go version`), and JDK version (`java -version`).

### Suggesting Enhancements

Feature requests and performance optimizations are encouraged. Please open an issue outlining:
- The problem or use-case you are trying to solve.
- Proposed API changes or architecture modifications.
- Compatibility considerations (e.g. FFI ABI stability, thread-safety, CAP invariants).

### Submitting Pull Requests

1. **Fork** the repository and create a branch from `main`:
   ```bash
   git checkout -b feature/my-new-feature
   ```
2. **Implement** your changes with comprehensive tests.
3. **Verify** that all tests pass across Rust, Go, and Java:
   ```bash
   cargo test
   go test -count=1 -v ./bindings/go/rustygraph/...
   ```
4. **Format & Lint**:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   go vet ./bindings/go/rustygraph/...
   ```
5. **Commit** using conventional commit messages (e.g. `feat: add MATCH clause variable-length path expansion`).
6. **Push** your branch to your fork and open a Pull Request against `main`.

---

## Development Setup

### Prerequisites

- **Rust**: Rust 1.85+ (Edition 2024 compatible)
  ```bash
  rustup update stable
  ```
- **Go**: Go 1.21 or newer (for Go SDK bindings)
  ```bash
  go version
  ```
- **Java**: OpenJDK 21 or newer with preview features enabled (for Java Foreign Function & Memory SDK)
  ```bash
  java -version
  ```

### Cloning the Repository

```bash
git clone https://github.com/maheshwarLigade/rustygraph.git
cd rustygraph
```

### Building the Project

Build both the Rust library (`rlib`) and C-compatible dynamic library (`cdylib`):
```bash
cargo build
```
This produces `target/debug/rusty_graph.dll` (Windows), `librusty_graph.so` (Linux), or `librusty_graph.dylib` (macOS).

---

## Project Architecture

```
rustygraph/
├── src/
│   ├── lib.rs              # Crate root and prelude exports
│   ├── storage.rs          # Generational slot arena for O(1) node/edge lookups
│   ├── model.rs            # Node, Relationship, and Direction data structures
│   ├── value.rs            # Dynamic algebraic property value types (PropertyValue)
│   ├── id.rs               # Generational 64-bit NodeId and RelId types
│   ├── index.rs            # BTreeMap property indices & unique constraints
│   ├── graph.rs            # In-memory property graph engine implementation
│   ├── traversal.rs        # Fluent BFS/DFS path traversal engine
│   ├── algo/               # Graph Data Science (GDS) algorithms
│   │   ├── shortest_path.rs # Dijkstra, A*, BFS shortest path
│   │   ├── centrality.rs    # PageRank & Degree Centrality
│   │   ├── community.rs     # Tarjan's SCC & Disjoint-Set WCC
│   │   └── similarity.rs    # Jaccard similarity & common neighbors
│   ├── query.rs            # Declarative pattern-matching query builder
│   ├── cypher.rs           # Native openCypher query parser, lexer & AST evaluator
│   ├── vector.rs           # Vector indexing (Cosine, Euclidean, Dot Product) & GraphRAG
│   ├── cluster.rs          # Distributed clustering with CAP CP and AP guarantees
│   ├── concurrent.rs       # Multi-threaded ConcurrentGraph with Arc<RwLock>
│   ├── serde_export.rs     # JSON export and import serialization
│   └── ffi.rs              # Zero-overhead C ABI export layer for Go and Java
├── bindings/
│   ├── c/                  # C header definition (rusty_graph.h)
│   ├── go/rustygraph/      # Official Go SDK (zero-dependency in-process bindings)
│   └── java/               # Official Java 21 SDK (modern FFM API)
├── examples/               # Standalone runnable demonstrations
└── tests/                  # Exhaustive integration test suites
```

---

## Running Tests & Verifications

### Rust Core Tests

Run all unit, integration, and documentation tests (75+ tests):
```bash
cargo test
```

Run tests with backtrace for debugging:
```bash
RUST_BACKTRACE=1 cargo test
```

### Go SDK Tests

Run the Go test suite:
```bash
cd bindings/go/rustygraph
go test -count=1 -v .
go vet ./...
```

Run the runnable Go SDK example:
```bash
go run ./example/main.go
```

### Java SDK Tests

Compile and run the Java 21 test demonstration:
```bash
javac --enable-preview --release 21 -d target/classes bindings/java/src/main/java/com/rustygraph/*.java bindings/java/src/test/java/com/rustygraph/*.java
java --enable-preview -cp target/classes com.rustygraph.RustyGraphTest
```

### Running Example Applications

- **Neo4j Social Network Demo**:
  ```bash
  cargo run --example neo4j_social_network
  ```
- **GraphRAG Vector & Graph Hybrid Search**:
  ```bash
  cargo run --example vector_graph_rag
  ```
- **Distributed CAP Theorem Clustering**:
  ```bash
  cargo run --example cap_distributed_cluster
  ```

---

## Coding Standards & Style Guide

### Rust Code Standards

- **Safety**: Keep `unsafe` blocks isolated strictly to `src/ffi.rs` or low-level primitives. All public Rust APIs must be safe, sound, and idiomatically documented.
- **Error Handling**: Use the [`Error`](src/error.rs) enum with `thiserror` variants; avoid `panic!` or `unwrap()` in library code.
- **Performance**:
  - Prefer small vector optimizations (`SmallVec<[RelId; 8]>`) for graph adjacency.
  - Avoid unnecessary memory allocations in hot traversal loops.
  - Keep lock contention minimal in `ConcurrentGraph`.

### Commit Message Conventions

We adhere to the [Conventional Commits](https://www.conventionalcommits.org/) specification:
- `feat:` A new feature or capability.
- `fix:` A bug fix.
- `docs:` Documentation improvements or additions.
- `perf:` Performance optimizations without behavioral changes.
- `test:` Adding or updating test cases.
- `refactor:` Code reorganization without adding features or fixing bugs.
- `chore:` Build scripts, `.gitignore`, dependency updates.

**Example**:
```text
feat(cypher): add support for variable length pattern MATCH (a)-[:KNOWS*1..3]->(b)
```

---

## License

By contributing to **RustyGraph**, you agree that your contributions will be licensed under the [Apache License, Version 2.0](LICENSE).

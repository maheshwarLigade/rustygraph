---
name: "🐛 Bug Report"
about: "Report unexpected behavior, crash, or bug in RustyGraph"
title: "[BUG]: "
labels: ["bug"]
assignees: ""
---

## 🐛 Bug Description
A clear and concise description of what the bug is.

## 📦 Component Affected
Please mark the component(s) where the issue occurs:
- [ ] Core Graph Model / Generational Arena Storage (`src/graph.rs`, `src/storage.rs`)
- [ ] openCypher Query Engine / Mutation Parser (`src/cypher.rs`, `src/query.rs`)
- [ ] ACID Transactions / Isolation (`src/transaction.rs`)
- [ ] Write-Ahead Log (WAL) & Crash Recovery (`src/wal.rs`, `src/snapshot.rs`)
- [ ] Graph Data Science (GDS) Algorithms (`src/algo/`)
- [ ] Vector Database & GraphRAG (`src/vector.rs`)
- [ ] Distributed Clustering / CAP Consensus (`src/distributed.rs`, `src/cluster.rs`)
- [ ] Standalone REST Server Daemon (`src/server.rs`, `src/bin/server.rs`)
- [ ] Interactive CLI Client (`src/bin/cli.rs`)
- [ ] Go SDK Bindings (`bindings/go/`)
- [ ] Java 21 SDK Bindings (`bindings/java/`)
- [ ] Other (please specify below)

## 🔁 Steps to Reproduce
1. Execute query or code: '...'
2. Run database / server with args: '...'
3. See error

## 💻 Minimal Code / Cypher Reproduction
```rust
// Or Cypher query string
```

## ✅ Expected Behavior
A clear and concise description of what you expected to happen.

## ❌ Actual Behavior / Stacktrace
Include actual output, error message, or backtrace:
```text
(Paste error or RUST_BACKTRACE=1 output here)
```

## 🖥️ Environment & Versions
- **OS**: [e.g. Ubuntu 22.04, Windows 11, macOS Sequoia]
- **Rust Version**: `rustc --version` output [e.g. 1.82.0]
- **RustyGraph Version / Commit**: [e.g. 0.1.0 or git commit hash]
- **Go / Java Version (if applicable)**: [e.g. Go 1.22, OpenJDK 21]

## 📝 Additional Context
Add any other context about the problem here (e.g., concurrency load, cluster size, fsync policy).

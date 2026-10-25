# ==============================================================================
# RustyGraph Multi-Stage Production Dockerfile
# Native Property Graph & Multi-Model NoSQL Database Engine in Pure Rust
# ==============================================================================

# ------------------------------------------------------------------------------
# Build Stage
# ------------------------------------------------------------------------------
FROM rust:latest AS builder

WORKDIR /usr/src/rustygraph

# Copy Cargo manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build release binaries with optimizations
RUN cargo build --release --bins && \
    strip /usr/src/rustygraph/target/release/rustygraph-server && \
    strip /usr/src/rustygraph/target/release/rustygraph-cli && \
    strip /usr/src/rustygraph/target/release/rustygraph-mcp

# ------------------------------------------------------------------------------
# Runtime Stage
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies (ca-certificates, curl for healthchecks)
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

# Create unprivileged system user and group
RUN groupadd -g 10001 rustygraph && \
    useradd -u 10001 -g rustygraph -s /bin/false -M rustygraph

# Create persistent storage data directory
RUN mkdir -p /data && \
    chown -R rustygraph:rustygraph /data

# Copy compiled binaries from builder stage
COPY --from=builder /usr/src/rustygraph/target/release/rustygraph-server /usr/local/bin/rustygraph-server
COPY --from=builder /usr/src/rustygraph/target/release/rustygraph-cli /usr/local/bin/rustygraph-cli
COPY --from=builder /usr/src/rustygraph/target/release/rustygraph-mcp /usr/local/bin/rustygraph-mcp

# Expose HTTP REST / Cypher API (8080) and Distributed Cluster TCP (7000)
EXPOSE 8080 7000

# Persistent volume for snapshots and write-ahead log (WAL)
VOLUME ["/data"]

# Switch to unprivileged user
USER rustygraph:rustygraph
WORKDIR /data

# Healthcheck monitoring
HEALTHCHECK --interval=10s --timeout=3s --retries=3 --start-period=5s \
    CMD curl -f http://localhost:8080/health || exit 1

# Default entrypoint and arguments
ENTRYPOINT ["rustygraph-server"]
CMD ["--listen-http", "0.0.0.0:8080", "--data-dir", "/data"]

package com.rustygraph;

/**
 * A nearest-neighbor search hit containing the matched node ID and similarity score.
 */
public record VectorHit(long nodeId, float score) {}

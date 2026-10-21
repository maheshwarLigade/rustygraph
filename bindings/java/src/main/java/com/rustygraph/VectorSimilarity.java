package com.rustygraph;

/**
 * Similarity metric used for vector database indexing and search.
 */
public enum VectorSimilarity {
    COSINE(0),
    EUCLIDEAN(1),
    DOT_PRODUCT(2);

    private final int value;

    VectorSimilarity(int value) {
        this.value = value;
    }

    public int getValue() {
        return value;
    }
}

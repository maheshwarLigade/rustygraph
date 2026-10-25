use crate::error::{GraphError, Result};
use crate::id::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported similarity metrics for vector indexing and semantic nearest neighbor search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VectorSimilarity {
    /// Cosine similarity: `dot(A, B) / (norm(A) * norm(B))`. Range [-1.0, 1.0].
    Cosine,
    /// Inverted Euclidean distance similarity: `1.0 / (1.0 + L2_dist(A, B))`. Range (0.0, 1.0].
    Euclidean,
    /// Raw dot product: `dot(A, B)`.
    DotProduct,
}

impl VectorSimilarity {
    /// Compute the similarity score between two vectors according to this metric.
    pub fn compute(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            Self::Cosine => cosine_similarity(a, b),
            Self::Euclidean => euclidean_similarity(a, b),
            Self::DotProduct => dot_product(a, b),
        }
    }
}

/// Compute cosine similarity between two equal-length vectors with auto-vectorization friendly unrolling.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let (a_slice, b_slice) = (&a[..len], &b[..len]);

    let mut dot0 = 0.0f32;
    let mut dot1 = 0.0f32;
    let mut norm_a0 = 0.0f32;
    let mut norm_a1 = 0.0f32;
    let mut norm_b0 = 0.0f32;
    let mut norm_b1 = 0.0f32;

    let mut a_chunks = a_slice.chunks_exact(8);
    let mut b_chunks = b_slice.chunks_exact(8);

    for (ac, bc) in (&mut a_chunks).zip(&mut b_chunks) {
        dot0 += ac[0] * bc[0] + ac[1] * bc[1] + ac[2] * bc[2] + ac[3] * bc[3];
        dot1 += ac[4] * bc[4] + ac[5] * bc[5] + ac[6] * bc[6] + ac[7] * bc[7];

        norm_a0 += ac[0] * ac[0] + ac[1] * ac[1] + ac[2] * ac[2] + ac[3] * ac[3];
        norm_a1 += ac[4] * ac[4] + ac[5] * ac[5] + ac[6] * ac[6] + ac[7] * ac[7];

        norm_b0 += bc[0] * bc[0] + bc[1] * bc[1] + bc[2] * bc[2] + bc[3] * bc[3];
        norm_b1 += bc[4] * bc[4] + bc[5] * bc[5] + bc[6] * bc[6] + bc[7] * bc[7];
    }

    let mut dot = dot0 + dot1;
    let mut norm_a = norm_a0 + norm_a1;
    let mut norm_b = norm_b0 + norm_b1;

    for (&x, &y) in a_chunks.remainder().iter().zip(b_chunks.remainder()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// Compute Euclidean similarity `1 / (1 + L2_distance)`.
pub fn euclidean_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let (a_slice, b_slice) = (&a[..len], &b[..len]);

    let mut sum_sq0 = 0.0f32;
    let mut sum_sq1 = 0.0f32;

    let mut a_chunks = a_slice.chunks_exact(8);
    let mut b_chunks = b_slice.chunks_exact(8);

    for (ac, bc) in (&mut a_chunks).zip(&mut b_chunks) {
        let d0 = ac[0] - bc[0];
        let d1 = ac[1] - bc[1];
        let d2 = ac[2] - bc[2];
        let d3 = ac[3] - bc[3];
        let d4 = ac[4] - bc[4];
        let d5 = ac[5] - bc[5];
        let d6 = ac[6] - bc[6];
        let d7 = ac[7] - bc[7];

        sum_sq0 += d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3;
        sum_sq1 += d4 * d4 + d5 * d5 + d6 * d6 + d7 * d7;
    }

    let mut sum_sq = sum_sq0 + sum_sq1;
    for (&x, &y) in a_chunks.remainder().iter().zip(b_chunks.remainder()) {
        let diff = x - y;
        sum_sq += diff * diff;
    }

    let dist = sum_sq.sqrt();
    1.0 / (1.0 + dist)
}

/// Compute standard dot product between two equal-length vectors.
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let (a_slice, b_slice) = (&a[..len], &b[..len]);

    let mut dot0 = 0.0f32;
    let mut dot1 = 0.0f32;

    let mut a_chunks = a_slice.chunks_exact(8);
    let mut b_chunks = b_slice.chunks_exact(8);

    for (ac, bc) in (&mut a_chunks).zip(&mut b_chunks) {
        dot0 += ac[0] * bc[0] + ac[1] * bc[1] + ac[2] * bc[2] + ac[3] * bc[3];
        dot1 += ac[4] * bc[4] + ac[5] * bc[5] + ac[6] * bc[6] + ac[7] * bc[7];
    }

    let mut dot = dot0 + dot1;
    for (&x, &y) in a_chunks.remainder().iter().zip(b_chunks.remainder()) {
        dot += x * y;
    }
    dot
}

/// Represents a nearest-neighbor match returned by a vector search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// Matched node identifier.
    pub node_id: NodeId,
    /// Similarity score according to the index metric (higher is more similar).
    pub score: f32,
}

/// An in-memory vector index mapping nodes with a given label to dense embedding vectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndex {
    pub name: String,
    pub label: String,
    pub property_key: String,
    pub dimensions: usize,
    pub metric: VectorSimilarity,
    vectors: HashMap<NodeId, Vec<f32>>,
}

impl VectorIndex {
    /// Create a new VectorIndex.
    pub fn new<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
        name: S1,
        label: S2,
        property_key: S3,
        dimensions: usize,
        metric: VectorSimilarity,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            property_key: property_key.into(),
            dimensions,
            metric,
            vectors: HashMap::new(),
        }
    }

    /// Insert or update a vector for a node, validating expected dimensionality.
    pub fn insert(&mut self, node: NodeId, vector: Vec<f32>) -> Result<()> {
        if vector.len() != self.dimensions {
            return Err(GraphError::InvalidOperation(format!(
                "Vector index '{}' expects dimension {}, but got {}",
                self.name,
                self.dimensions,
                vector.len()
            )));
        }
        self.vectors.insert(node, vector);
        Ok(())
    }

    /// Remove a node from the vector index.
    pub fn remove(&mut self, node: NodeId) -> bool {
        self.vectors.remove(&node).is_some()
    }

    /// Retrieve the indexed vector for a node if present.
    pub fn get(&self, node: NodeId) -> Option<&[f32]> {
        self.vectors.get(&node).map(|v| v.as_slice())
    }

    /// Search for the top `k` most similar nodes to `query_vector`.
    pub fn search(&self, query_vector: &[f32], k: usize) -> Result<Vec<VectorSearchResult>> {
        if query_vector.len() != self.dimensions {
            return Err(GraphError::InvalidOperation(format!(
                "Query vector dimension mismatch for index '{}': expected {}, but got {}",
                self.name,
                self.dimensions,
                query_vector.len()
            )));
        }

        if k == 0 || self.vectors.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::with_capacity(self.vectors.len());
        for (&node_id, vec) in &self.vectors {
            let score = self.metric.compute(query_vector, vec);
            results.push(VectorSearchResult { node_id, score });
        }

        // Sort descending by score
        results.sort_by(|a, b| b.score.total_cmp(&a.score));

        if results.len() > k {
            results.truncate(k);
        }

        Ok(results)
    }

    /// Number of indexed vectors.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Returns true if no vectors are indexed.
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Clear all vectors from this index.
    pub fn clear(&mut self) {
        self.vectors.clear();
    }
}

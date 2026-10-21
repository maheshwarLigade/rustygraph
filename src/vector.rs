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

/// Compute cosine similarity between two equal-length vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

/// Compute Euclidean similarity `1 / (1 + L2_distance)`.
pub fn euclidean_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut sum_sq = 0.0f32;
    for (&x, &y) in a.iter().zip(b.iter()) {
        let diff = x - y;
        sum_sq += diff * diff;
    }
    let dist = sum_sq.sqrt();
    1.0 / (1.0 + dist)
}

/// Compute standard dot product between two equal-length vectors.
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
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

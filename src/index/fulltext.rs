use crate::id::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Stop words commonly filtered to enhance inverted search relevance.
const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is",
    "it", "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there",
    "these", "they", "this", "to", "was", "will", "with",
];

/// Represents a scored document hit returned by a full-text search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FulltextSearchResult {
    pub node_id: NodeId,
    pub score: f32,
    pub matched_terms: Vec<String>,
}

/// Tokenizer converting text into normalized keyword tokens.
pub fn tokenize(text: &str) -> Vec<String> {
    let stop_set: HashSet<&str> = STOP_WORDS.iter().copied().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            if current.len() > 1 && !stop_set.contains(current.as_str()) {
                tokens.push(current.clone());
            }
            current.clear();
        }
    }

    if !current.is_empty() && current.len() > 1 && !stop_set.contains(current.as_str()) {
        tokens.push(current);
    }

    tokens
}

/// An in-memory inverted full-text search index with BM25/TF-IDF relevance ranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulltextIndex {
    pub name: String,
    pub label: String,
    pub property_key: String,
    /// Inverted index mapping term -> (NodeId -> term frequency).
    postings: HashMap<String, HashMap<NodeId, u32>>,
    /// Document length (total token count) per node.
    doc_lengths: HashMap<NodeId, usize>,
    /// Stored raw text per node for updates/deletions.
    indexed_texts: HashMap<NodeId, String>,
}

impl FulltextIndex {
    /// Creates a new, empty `FulltextIndex`.
    pub fn new<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
        name: S1,
        label: S2,
        property_key: S3,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            property_key: property_key.into(),
            postings: HashMap::new(),
            doc_lengths: HashMap::new(),
            indexed_texts: HashMap::new(),
        }
    }

    /// Indexes or updates the text content for a given node.
    pub fn insert(&mut self, node: NodeId, text: &str) {
        if self.indexed_texts.contains_key(&node) {
            self.remove(node);
        }

        let tokens = tokenize(text);
        if tokens.is_empty() {
            return;
        }

        self.doc_lengths.insert(node, tokens.len());
        self.indexed_texts.insert(node, text.to_string());

        for token in tokens {
            self.postings
                .entry(token)
                .or_default()
                .entry(node)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    /// Removes a node from the full-text inverted index.
    pub fn remove(&mut self, node: NodeId) -> bool {
        if let Some(text) = self.indexed_texts.remove(&node) {
            self.doc_lengths.remove(&node);
            let tokens = tokenize(&text);
            for token in tokens {
                if let Some(node_map) = self.postings.get_mut(&token) {
                    node_map.remove(&node);
                    if node_map.is_empty() {
                        self.postings.remove(&token);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Searches the full-text index using TF-IDF ranking with prefix matching support.
    pub fn search(&self, query_text: &str, limit: usize) -> Vec<FulltextSearchResult> {
        let query_tokens = tokenize(query_text);
        if query_tokens.is_empty() || self.doc_lengths.is_empty() {
            return Vec::new();
        }

        let total_docs = self.doc_lengths.len() as f32;
        let mut node_scores: HashMap<NodeId, (f32, Vec<String>)> = HashMap::new();

        for q_term in &query_tokens {
            // Find exact matches or terms starting with q_term
            for (term, postings) in &self.postings {
                let matches_term = term == q_term || (term.starts_with(q_term) && q_term.len() >= 3);
                if !matches_term {
                    continue;
                }

                let doc_freq = postings.len() as f32;
                // Standard smoothed IDF: ln(1 + (N - df + 0.5) / (df + 0.5))
                let idf = ((total_docs - doc_freq + 0.5) / (doc_freq + 0.5) + 1.0).ln().max(0.1);

                for (&node_id, &tf) in postings {
                    let doc_len = *self.doc_lengths.get(&node_id).unwrap_or(&1) as f32;
                    let norm_tf = (tf as f32) / doc_len;
                    let term_score = norm_tf * idf;

                    let entry = node_scores.entry(node_id).or_insert_with(|| (0.0, Vec::new()));
                    entry.0 += term_score;
                    if !entry.1.contains(term) {
                        entry.1.push(term.clone());
                    }
                }
            }
        }

        let mut results: Vec<FulltextSearchResult> = node_scores
            .into_iter()
            .map(|(node_id, (score, matched_terms))| FulltextSearchResult {
                node_id,
                score,
                matched_terms,
            })
            .collect();

        // Sort descending by score
        results.sort_by(|a, b| b.score.total_cmp(&a.score));

        if results.len() > limit && limit > 0 {
            results.truncate(limit);
        }

        results
    }

    /// Searches the full-text index explicitly for terms starting with `prefix`.
    pub fn search_prefix(&self, prefix: &str, limit: usize) -> Vec<FulltextSearchResult> {
        let p_lower = prefix.to_lowercase();
        if p_lower.is_empty() || self.doc_lengths.is_empty() {
            return Vec::new();
        }

        let total_docs = self.doc_lengths.len() as f32;
        let mut node_scores: HashMap<NodeId, (f32, Vec<String>)> = HashMap::new();

        for (term, postings) in &self.postings {
            if !term.starts_with(&p_lower) {
                continue;
            }

            let doc_freq = postings.len() as f32;
            let idf = ((total_docs - doc_freq + 0.5) / (doc_freq + 0.5) + 1.0).ln().max(0.1);

            for (&node_id, &tf) in postings {
                let doc_len = *self.doc_lengths.get(&node_id).unwrap_or(&1) as f32;
                let tf_norm = (tf as f32) / doc_len;
                let score = tf_norm * idf;

                let entry = node_scores.entry(node_id).or_insert((0.0, Vec::new()));
                entry.0 += score;
                if !entry.1.contains(term) {
                    entry.1.push(term.clone());
                }
            }
        }

        let mut results: Vec<FulltextSearchResult> = node_scores
            .into_iter()
            .map(|(node_id, (score, matched_terms))| FulltextSearchResult {
                node_id,
                score,
                matched_terms,
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    /// Number of indexed documents in this index.
    pub fn len(&self) -> usize {
        self.doc_lengths.len()
    }

    /// Returns true if no documents are indexed.
    pub fn is_empty(&self) -> bool {
        self.doc_lengths.is_empty()
    }

    /// Clear all index postings and document mappings.
    pub fn clear(&mut self) {
        self.postings.clear();
        self.doc_lengths.clear();
        self.indexed_texts.clear();
    }
}

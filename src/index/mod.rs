//! Secondary Property Indexing and Inverted Full-Text Search Subsystem for RustyGraph.
//!
//! Provides B-Tree secondary property indexes for exact and range lookups, unique constraints,
//! and BM25/TF-IDF inverted full-text keyword search indexes.

pub mod fulltext;
pub mod property;

pub use fulltext::{FulltextIndex, FulltextSearchResult, tokenize};
pub use property::{IndexKey, IndexManager, PropertyIndex};

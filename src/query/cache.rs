use super::cypher::ast::AstQuery;
use super::cypher::lexer::Lexer;
use super::cypher::parser::Parser;
use crate::error::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Thread-safe LRU query plan cache storing parsed openCypher ASTs to eliminate parsing overhead on hot queries.
pub struct QueryPlanCache {
    cache: Mutex<HashMap<String, Arc<AstQuery>>>,
    order: Mutex<Vec<String>>,
    capacity: usize,
}

impl Default for QueryPlanCache {
    fn default() -> Self {
        Self::new(512)
    }
}

impl QueryPlanCache {
    /// Creates a new `QueryPlanCache` with a bounded maximum number of compiled ASTs.
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(HashMap::with_capacity(capacity)),
            order: Mutex::new(Vec::with_capacity(capacity)),
            capacity: capacity.max(16),
        }
    }

    /// Retrieves a cached query plan AST or parses and compiles it.
    pub fn get_or_parse(&self, query_str: &str) -> Result<Arc<AstQuery>> {
        let key = query_str.trim();

        // 1. Fast read from cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(ast) = cache.get(key) {
                let ast_clone = Arc::clone(ast);
                // Move key to back of LRU order list
                let mut order = self.order.lock().unwrap();
                if let Some(pos) = order.iter().position(|k| k == key) {
                    let k = order.remove(pos);
                    order.push(k);
                }
                return Ok(ast_clone);
            }
        }

        // 2. Parse query string into AST
        let mut lexer = Lexer::new(key);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let ast = Arc::new(parser.parse_query()?);

        // 3. Insert into LRU cache with eviction
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.order.lock().unwrap();

        if cache.len() >= self.capacity {
            if !order.is_empty() {
                let oldest = order.remove(0);
                cache.remove(&oldest);
            }
        }

        order.push(key.to_string());
        cache.insert(key.to_string(), Arc::clone(&ast));

        Ok(ast)
    }

    /// Clear all cached query plans.
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
        self.order.lock().unwrap().clear();
    }

    /// Number of compiled queries in cache.
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// Returns true if the plan cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub mod ast;
pub mod executor;
pub mod explain;
pub mod lexer;
pub mod parser;
pub mod result;
pub mod token;

pub use ast::*;
pub use executor::{execute_ast, execute_ast_with_params};
pub use explain::{QueryPlan, QueryPlanStep, explain_ast};
pub use lexer::Lexer;
pub use parser::Parser;
pub use result::CypherResult;
pub use token::Token;

use crate::error::Result;
use crate::graph::Graph;
use crate::value::PropertyValue;
use std::collections::HashMap;
use std::sync::LazyLock;

static GLOBAL_PLAN_CACHE: LazyLock<super::cache::QueryPlanCache> =
    LazyLock::new(|| super::cache::QueryPlanCache::new(512));

impl Graph {
    /// Execute an openCypher query string against the graph with plan caching.
    /// Supports `MATCH`, `CREATE`, `SET`, `DELETE`, `DETACH DELETE`, `WHERE`,
    /// aggregations (`count`, `sum`, `avg`, `min`, `max`), and `ORDER BY`.
    pub fn cypher(&mut self, query_str: &str) -> Result<CypherResult> {
        self.cypher_with_params(query_str, &HashMap::new())
    }

    /// Execute a parameterized openCypher query string ($param) against the graph using cached AST query plans.
    pub fn cypher_with_params(
        &mut self,
        query_str: &str,
        params: &HashMap<String, PropertyValue>,
    ) -> Result<CypherResult> {
        let ast = GLOBAL_PLAN_CACHE.get_or_parse(query_str)?;
        execute_ast_with_params(self, (*ast).clone(), params)
    }

    /// Returns the parsed execution plan and operator tree for an openCypher query without executing it.
    pub fn explain_cypher(&self, query_str: &str) -> Result<QueryPlan> {
        let ast = GLOBAL_PLAN_CACHE.get_or_parse(query_str)?;
        Ok(explain_ast(&ast, query_str))
    }
}

/// Returns true if the Cypher query string contains any mutating operations (`CREATE`, `SET`, `DELETE`, `DETACH`).
pub fn is_cypher_mutation(query_str: &str) -> bool {
    if let Ok(ast) = GLOBAL_PLAN_CACHE.get_or_parse(query_str) {
        ast.is_mutation()
    } else {
        false
    }
}

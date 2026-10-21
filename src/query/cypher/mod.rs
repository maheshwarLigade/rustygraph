pub mod ast;
pub mod executor;
pub mod lexer;
pub mod parser;
pub mod result;
pub mod token;

pub use ast::*;
pub use executor::execute_ast;
pub use lexer::Lexer;
pub use parser::Parser;
pub use result::CypherResult;
pub use token::Token;

use crate::error::Result;
use crate::graph::Graph;

impl Graph {
    /// Execute an openCypher query string against the graph.
    /// Supports `MATCH`, `CREATE`, `SET`, `DELETE`, `DETACH DELETE`, `WHERE`,
    /// aggregations (`count`, `sum`, `avg`, `min`, `max`), and `ORDER BY`.
    pub fn cypher(&mut self, query_str: &str) -> Result<CypherResult> {
        let mut lexer = Lexer::new(query_str);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_query()?;
        execute_ast(self, ast)
    }
}

/// Returns true if the Cypher query string contains any mutating operations (`CREATE`, `SET`, `DELETE`, `DETACH`).
pub fn is_cypher_mutation(query_str: &str) -> bool {
    let mut lexer = Lexer::new(query_str);
    if let Ok(tokens) = lexer.tokenize() {
        tokens.iter().any(|t| {
            matches!(
                t,
                Token::Create | Token::Set | Token::Delete | Token::Detach
            )
        })
    } else {
        false
    }
}

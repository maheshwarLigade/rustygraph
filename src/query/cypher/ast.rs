use crate::error::{GraphError, Result};
use crate::model::Direction;
use crate::value::PropertyValue;
use std::collections::HashMap;

/// An evaluatable value in an AST, either a direct literal or a runtime query parameter ($param).
#[derive(Debug, Clone, PartialEq)]
pub enum QueryValue {
    Literal(PropertyValue),
    Parameter(String),
}

impl QueryValue {
    pub fn resolve(&self, params: &HashMap<String, PropertyValue>) -> Result<PropertyValue> {
        match self {
            QueryValue::Literal(v) => Ok(v.clone()),
            QueryValue::Parameter(name) => params
                .get(name)
                .cloned()
                .ok_or_else(|| GraphError::InvalidOperation(format!("Missing parameter '${name}' for Cypher query"))),
        }
    }
}

/// A parsed node in a MATCH or CREATE pattern.
#[derive(Debug, Clone)]
pub struct ParsedNode {
    pub alias: String,
    pub label: Option<String>,
    pub props: Vec<(String, QueryValue)>,
}

/// A parsed relationship pattern in a MATCH or CREATE pattern.
#[derive(Debug, Clone)]
pub struct ParsedRel {
    pub alias: Option<String>,
    pub rel_type: Option<String>,
    pub direction: Direction,
    pub hops: Option<(usize, usize)>,
    pub props: Vec<(String, QueryValue)>,
}

/// Comparison operators in WHERE expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum CompOp {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}

/// A WHERE filter condition.
#[derive(Debug, Clone)]
pub struct WhereClause {
    pub alias: String,
    pub prop: String,
    pub op: CompOp,
    pub val: QueryValue,
}

/// Aggregation or projection kinds in RETURN statements.
#[derive(Debug, Clone)]
pub enum ReturnKind {
    Property(String, String),             // alias.prop
    Node(String),                         // alias
    CountStar,                            // count(*)
    Count(String, Option<String>, bool),  // count(p) or count(DISTINCT p.age)
    Sum(String, String),                  // sum(p.age)
    Avg(String, String),                  // avg(p.age)
    Min(String, String),                  // min(p.age)
    Max(String, String),                  // max(p.age)
    Collect(String, Option<String>, bool),// collect(p) or collect(DISTINCT p.name)
}

/// A projected item in a RETURN clause.
#[derive(Debug, Clone)]
pub struct ReturnItem {
    pub kind: ReturnKind,
    pub output_name: String,
}

/// A SET mutation clause.
#[derive(Debug, Clone)]
pub struct SetClause {
    pub alias: String,
    pub prop: String,
    pub val: QueryValue,
}

/// A DELETE or DETACH DELETE clause.
#[derive(Debug, Clone)]
pub struct DeleteClause {
    pub alias: String,
    pub detach: bool,
}

/// An ORDER BY item.
#[derive(Debug, Clone)]
pub struct OrderByClause {
    pub alias: String,
    pub prop: Option<String>,
    pub descending: bool,
}

/// A MERGE clause pattern with optional ON CREATE SET and ON MATCH SET actions.
#[derive(Debug, Clone)]
pub struct ParsedMerge {
    pub pattern: (Vec<ParsedNode>, Vec<ParsedRel>),
    pub on_create_set: Vec<SetClause>,
    pub on_match_set: Vec<SetClause>,
}

/// The complete Abstract Syntax Tree of a parsed Cypher query.
#[derive(Debug, Clone)]
pub struct AstQuery {
    pub matches: Vec<(Vec<ParsedNode>, Vec<ParsedRel>)>,
    pub creates: Vec<(Vec<ParsedNode>, Vec<ParsedRel>)>,
    pub merges: Vec<ParsedMerge>,
    pub where_clauses: Vec<WhereClause>,
    pub set_clauses: Vec<SetClause>,
    pub delete_clauses: Vec<DeleteClause>,
    pub returns: Vec<ReturnItem>,
    pub distinct: bool,
    pub order_by: Vec<OrderByClause>,
    pub limit: Option<usize>,
}

impl AstQuery {
    /// Returns true if this AST contains any mutation clauses (CREATE, MERGE, SET, DELETE, DETACH).
    pub fn is_mutation(&self) -> bool {
        !self.creates.is_empty() || !self.merges.is_empty() || !self.set_clauses.is_empty() || !self.delete_clauses.is_empty()
    }
}

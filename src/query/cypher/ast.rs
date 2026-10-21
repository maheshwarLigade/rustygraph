use crate::model::Direction;
use crate::value::PropertyValue;

/// A parsed node in a MATCH or CREATE pattern.
#[derive(Debug, Clone)]
pub struct ParsedNode {
    pub alias: String,
    pub label: Option<String>,
    pub props: Vec<(String, PropertyValue)>,
}

/// A parsed relationship pattern in a MATCH or CREATE pattern.
#[derive(Debug, Clone)]
pub struct ParsedRel {
    pub alias: Option<String>,
    pub rel_type: Option<String>,
    pub direction: Direction,
    pub props: Vec<(String, PropertyValue)>,
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
    pub val: PropertyValue,
}

/// Aggregation or projection kinds in RETURN statements.
#[derive(Debug, Clone)]
pub enum ReturnKind {
    Property(String, String),       // alias.prop
    Node(String),                   // alias
    CountStar,                      // count(*)
    Count(String, Option<String>),  // count(p) or count(p.age)
    Sum(String, String),            // sum(p.age)
    Avg(String, String),            // avg(p.age)
    Min(String, String),            // min(p.age)
    Max(String, String),            // max(p.age)
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
    pub val: PropertyValue,
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

/// The complete Abstract Syntax Tree of a parsed Cypher query.
#[derive(Debug, Clone)]
pub struct AstQuery {
    pub matches: Vec<(Vec<ParsedNode>, Vec<ParsedRel>)>,
    pub creates: Vec<(Vec<ParsedNode>, Vec<ParsedRel>)>,
    pub where_clauses: Vec<WhereClause>,
    pub set_clauses: Vec<SetClause>,
    pub delete_clauses: Vec<DeleteClause>,
    pub returns: Vec<ReturnItem>,
    pub order_by: Vec<OrderByClause>,
    pub limit: Option<usize>,
}

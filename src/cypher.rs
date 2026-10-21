use crate::error::{GraphError, Result};
use crate::graph::Graph;
use crate::model::Direction;
use crate::value::PropertyValue;
use std::fmt;

/// Tabular result returned by executing a Cypher query string.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CypherResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<PropertyValue>>,
}

impl CypherResult {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl fmt::Display for CypherResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.columns.is_empty() {
            return write!(f, "(Empty result)");
        }

        // Compute column widths
        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.len()).collect();
        for row in &self.rows {
            for (i, val) in row.iter().enumerate() {
                if i < widths.len() {
                    let s = format!("{val}");
                    widths[i] = widths[i].max(s.len());
                }
            }
        }

        // Header
        write!(f, "|")?;
        for (i, col) in self.columns.iter().enumerate() {
            write!(f, " {:<width$} |", col, width = widths[i])?;
        }
        writeln!(f)?;

        // Separator
        write!(f, "+")?;
        for w in &widths {
            write!(f, "{}+", "-".repeat(w + 2))?;
        }
        writeln!(f)?;

        // Rows
        for row in &self.rows {
            write!(f, "|")?;
            for (i, val) in row.iter().enumerate() {
                let s = format!("{val}");
                write!(f, " {:<width$} |", s, width = widths[i])?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

// =========================================================================
// Lexer & Tokens
// =========================================================================

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Match,
    Where,
    Return,
    Order,
    By,
    Limit,
    Skip,
    And,
    Or,
    Asc,
    Desc,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Dot,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    ArrowRight, // ->
    ArrowLeft,  // <-
    Dash,       // -
    Ident(String),
    StringLit(String),
    IntLit(i64),
    FloatLit(f64),
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();

        while let Some(&ch) = self.chars.peek() {
            match ch {
                ' ' | '\t' | '\r' | '\n' => {
                    self.chars.next();
                }
                '(' => {
                    self.chars.next();
                    tokens.push(Token::LParen);
                }
                ')' => {
                    self.chars.next();
                    tokens.push(Token::RParen);
                }
                '[' => {
                    self.chars.next();
                    tokens.push(Token::LBracket);
                }
                ']' => {
                    self.chars.next();
                    tokens.push(Token::RBracket);
                }
                '{' => {
                    self.chars.next();
                    tokens.push(Token::LBrace);
                }
                '}' => {
                    self.chars.next();
                    tokens.push(Token::RBrace);
                }
                ':' => {
                    self.chars.next();
                    tokens.push(Token::Colon);
                }
                ',' => {
                    self.chars.next();
                    tokens.push(Token::Comma);
                }
                '.' => {
                    self.chars.next();
                    tokens.push(Token::Dot);
                }
                '=' => {
                    self.chars.next();
                    tokens.push(Token::Eq);
                }
                '<' => {
                    self.chars.next();
                    if self.chars.peek() == Some(&'-') {
                        self.chars.next();
                        tokens.push(Token::ArrowLeft);
                    } else if self.chars.peek() == Some(&'=') {
                        self.chars.next();
                        tokens.push(Token::Lte);
                    } else {
                        tokens.push(Token::Lt);
                    }
                }
                '>' => {
                    self.chars.next();
                    if self.chars.peek() == Some(&'=') {
                        self.chars.next();
                        tokens.push(Token::Gte);
                    } else {
                        tokens.push(Token::Gt);
                    }
                }
                '!' => {
                    self.chars.next();
                    if self.chars.peek() == Some(&'=') {
                        self.chars.next();
                        tokens.push(Token::Neq);
                    } else {
                        return Err(GraphError::InvalidOperation("Unexpected character '!'".into()));
                    }
                }
                '-' => {
                    self.chars.next();
                    if self.chars.peek() == Some(&'>') {
                        self.chars.next();
                        tokens.push(Token::ArrowRight);
                    } else {
                        tokens.push(Token::Dash);
                    }
                }
                '"' | '\'' => {
                    let quote = ch;
                    self.chars.next();
                    let mut s = String::new();
                    while let Some(&c) = self.chars.peek() {
                        if c == quote {
                            self.chars.next();
                            break;
                        }
                        s.push(c);
                        self.chars.next();
                    }
                    tokens.push(Token::StringLit(s));
                }
                '0'..='9' => {
                    let mut num_str = String::new();
                    let mut is_float = false;
                    while let Some(&c) = self.chars.peek() {
                        if c.is_ascii_digit() {
                            num_str.push(c);
                            self.chars.next();
                        } else if c == '.' && !is_float {
                            is_float = true;
                            num_str.push(c);
                            self.chars.next();
                        } else {
                            break;
                        }
                    }
                    if is_float {
                        let f: f64 = num_str.parse().map_err(|e: std::num::ParseFloatError| {
                            GraphError::InvalidOperation(e.to_string())
                        })?;
                        tokens.push(Token::FloatLit(f));
                    } else {
                        let i: i64 = num_str.parse().map_err(|e: std::num::ParseIntError| {
                            GraphError::InvalidOperation(e.to_string())
                        })?;
                        tokens.push(Token::IntLit(i));
                    }
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    let mut ident = String::new();
                    while let Some(&c) = self.chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            ident.push(c);
                            self.chars.next();
                        } else {
                            break;
                        }
                    }
                    match ident.to_uppercase().as_str() {
                        "MATCH" => tokens.push(Token::Match),
                        "WHERE" => tokens.push(Token::Where),
                        "RETURN" => tokens.push(Token::Return),
                        "ORDER" => tokens.push(Token::Order),
                        "BY" => tokens.push(Token::By),
                        "LIMIT" => tokens.push(Token::Limit),
                        "SKIP" => tokens.push(Token::Skip),
                        "AND" => tokens.push(Token::And),
                        "OR" => tokens.push(Token::Or),
                        "ASC" => tokens.push(Token::Asc),
                        "DESC" => tokens.push(Token::Desc),
                        _ => tokens.push(Token::Ident(ident)),
                    }
                }
                _ => {
                    return Err(GraphError::InvalidOperation(format!(
                        "Unexpected character: '{ch}'"
                    )));
                }
            }
        }

        Ok(tokens)
    }
}

// =========================================================================
// Parser & AST
// =========================================================================

#[derive(Debug, Clone)]
struct ParsedNode {
    alias: String,
    label: Option<String>,
    props: Vec<(String, PropertyValue)>,
}

#[derive(Debug, Clone)]
struct ParsedRel {
    alias: Option<String>,
    rel_type: Option<String>,
    direction: Direction,
}

#[derive(Debug, Clone)]
enum CompOp {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone)]
struct WhereClause {
    alias: String,
    prop: String,
    op: CompOp,
    val: PropertyValue,
}

#[derive(Debug, Clone)]
struct ReturnItem {
    alias: String,
    prop: Option<String>,
}

#[derive(Debug, Clone)]
struct AstQuery {
    nodes: Vec<ParsedNode>,
    rels: Vec<ParsedRel>,
    where_clauses: Vec<WhereClause>,
    returns: Vec<ReturnItem>,
    limit: Option<usize>,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: Token) -> Result<()> {
        let t = self.next();
        if t.as_ref() == Some(&expected) {
            Ok(())
        } else {
            Err(GraphError::InvalidOperation(format!(
                "Expected token {expected:?}, found {t:?}"
            )))
        }
    }

    fn parse_node(&mut self) -> Result<ParsedNode> {
        self.expect(Token::LParen)?;

        let mut alias = String::new();
        if let Some(Token::Ident(name)) = self.peek() {
            alias = name.clone();
            self.next();
        }

        let mut label = None;
        if self.peek() == Some(&Token::Colon) {
            self.next();
            if let Some(Token::Ident(lbl)) = self.next() {
                label = Some(lbl);
            } else {
                return Err(GraphError::InvalidOperation("Expected label identifier".into()));
            }
        }

        let mut props = Vec::new();
        if self.peek() == Some(&Token::LBrace) {
            self.next();
            while self.peek() != Some(&Token::RBrace) {
                let key = if let Some(Token::Ident(k)) = self.next() {
                    k
                } else {
                    return Err(GraphError::InvalidOperation("Expected property key".into()));
                };
                self.expect(Token::Colon)?;
                let val = match self.next() {
                    Some(Token::StringLit(s)) => PropertyValue::String(s),
                    Some(Token::IntLit(i)) => PropertyValue::Int(i),
                    Some(Token::FloatLit(f)) => PropertyValue::Float(f),
                    other => {
                        return Err(GraphError::InvalidOperation(format!(
                            "Expected literal value, found {other:?}"
                        )));
                    }
                };
                props.push((key, val));
                if self.peek() == Some(&Token::Comma) {
                    self.next();
                } else {
                    break;
                }
            }
            self.expect(Token::RBrace)?;
        }

        self.expect(Token::RParen)?;
        Ok(ParsedNode { alias, label, props })
    }

    fn parse_rel(&mut self) -> Result<ParsedRel> {
        // e.g. -[:KNOWS]-> or -[r:KNOWS]-> or <-[:KNOWS]-
        let is_incoming = if self.peek() == Some(&Token::ArrowLeft) {
            self.next();
            true
        } else {
            self.expect(Token::Dash)?;
            false
        };

        self.expect(Token::LBracket)?;
        let mut alias = None;
        if let Some(Token::Ident(name)) = self.peek() {
            alias = Some(name.clone());
            self.next();
        }

        let mut rel_type = None;
        if self.peek() == Some(&Token::Colon) {
            self.next();
            if let Some(Token::Ident(t)) = self.next() {
                rel_type = Some(t);
            }
        }
        self.expect(Token::RBracket)?;

        let dir = if is_incoming {
            self.expect(Token::Dash)?;
            Direction::Incoming
        } else if self.peek() == Some(&Token::ArrowRight) {
            self.next();
            Direction::Outgoing
        } else {
            self.expect(Token::Dash)?;
            Direction::Both
        };

        Ok(ParsedRel {
            alias,
            rel_type,
            direction: dir,
        })
    }

    fn parse_query(&mut self) -> Result<AstQuery> {
        self.expect(Token::Match)?;

        let mut nodes = Vec::new();
        let mut rels = Vec::new();

        nodes.push(self.parse_node()?);

        while self.peek() == Some(&Token::Dash) || self.peek() == Some(&Token::ArrowLeft) {
            rels.push(self.parse_rel()?);
            nodes.push(self.parse_node()?);
        }

        let mut where_clauses = Vec::new();
        if self.peek() == Some(&Token::Where) {
            self.next();
            loop {
                let alias = match self.next() {
                    Some(Token::Ident(id)) => id,
                    other => return Err(GraphError::InvalidOperation(format!("Expected alias, found {other:?}"))),
                };
                self.expect(Token::Dot)?;
                let prop = match self.next() {
                    Some(Token::Ident(p)) => p,
                    other => return Err(GraphError::InvalidOperation(format!("Expected property, found {other:?}"))),
                };

                let op = match self.next() {
                    Some(Token::Eq) => CompOp::Eq,
                    Some(Token::Neq) => CompOp::Neq,
                    Some(Token::Lt) => CompOp::Lt,
                    Some(Token::Lte) => CompOp::Lte,
                    Some(Token::Gt) => CompOp::Gt,
                    Some(Token::Gte) => CompOp::Gte,
                    other => return Err(GraphError::InvalidOperation(format!("Expected comparison operator, found {other:?}"))),
                };

                let val = match self.next() {
                    Some(Token::StringLit(s)) => PropertyValue::String(s),
                    Some(Token::IntLit(i)) => PropertyValue::Int(i),
                    Some(Token::FloatLit(f)) => PropertyValue::Float(f),
                    other => return Err(GraphError::InvalidOperation(format!("Expected value, found {other:?}"))),
                };

                where_clauses.push(WhereClause { alias, prop, op, val });

                if self.peek() == Some(&Token::And) {
                    self.next();
                } else {
                    break;
                }
            }
        }

        self.expect(Token::Return)?;
        let mut returns = Vec::new();
        loop {
            let alias = match self.next() {
                Some(Token::Ident(id)) => id,
                other => return Err(GraphError::InvalidOperation(format!("Expected return alias, found {other:?}"))),
            };

            let prop = if self.peek() == Some(&Token::Dot) {
                self.next();
                match self.next() {
                    Some(Token::Ident(p)) => Some(p),
                    other => return Err(GraphError::InvalidOperation(format!("Expected property name, found {other:?}"))),
                }
            } else {
                None
            };

            returns.push(ReturnItem { alias, prop });

            if self.peek() == Some(&Token::Comma) {
                self.next();
            } else {
                break;
            }
        }

        let mut limit = None;
        if self.peek() == Some(&Token::Limit) {
            self.next();
            if let Some(Token::IntLit(lim)) = self.next() {
                limit = Some(lim as usize);
            }
        }

        Ok(AstQuery {
            nodes,
            rels,
            where_clauses,
            returns,
            limit,
        })
    }
}

// =========================================================================
// Cypher Execution
// =========================================================================

impl Graph {
    /// Execute a Cypher query string against this in-memory graph.
    ///
    /// Supported syntax subset:
    /// `MATCH (a:Label {prop: val})-[r:TYPE]->(b:Label) WHERE b.age > 25 RETURN a.name, b.name LIMIT 10`
    pub fn cypher(&self, query_str: &str) -> Result<CypherResult> {
        let mut lexer = Lexer::new(query_str);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_query()?;

        let mut builder = self.query();

        // 1. Build pattern
        if let Some(first_node) = ast.nodes.first() {
            builder = builder.match_node(&first_node.alias);
            if let Some(lbl) = &first_node.label {
                builder = builder.with_label(lbl);
            }
            for (k, v) in &first_node.props {
                builder = builder.with_prop(k, v.clone());
            }
        }

        for (i, rel) in ast.rels.iter().enumerate() {
            let target_node = &ast.nodes[i + 1];
            builder = builder.rel(
                rel.alias.clone(),
                rel.rel_type.clone(),
                rel.direction,
                target_node.alias.clone(),
            );
            if let Some(lbl) = &target_node.label {
                builder = builder.with_label(lbl);
            }
            for (k, v) in &target_node.props {
                builder = builder.with_prop(k, v.clone());
            }
        }

        // 2. WHERE clauses
        for w in ast.where_clauses {
            let alias = w.alias.clone();
            let prop = w.prop.clone();
            let op = w.op.clone();
            let expected_val = w.val.clone();

            builder = builder.filter(move |ctx| {
                if let Some(actual) = ctx.prop(&alias, &prop) {
                    match op {
                        CompOp::Eq => actual == &expected_val,
                        CompOp::Neq => actual != &expected_val,
                        CompOp::Lt => actual < &expected_val,
                        CompOp::Lte => actual <= &expected_val,
                        CompOp::Gt => actual > &expected_val,
                        CompOp::Gte => actual >= &expected_val,
                    }
                } else {
                    false
                }
            });
        }

        // 3. Limit
        if let Some(lim) = ast.limit {
            builder = builder.limit(lim);
        }

        // 4. Projection
        let return_items = ast.returns.clone();
        let columns: Vec<String> = return_items
            .iter()
            .map(|item| {
                if let Some(p) = &item.prop {
                    format!("{}.{}", item.alias, p)
                } else {
                    item.alias.clone()
                }
            })
            .collect();

        let rows: Vec<Vec<PropertyValue>> = builder.project(move |ctx| {
            let mut row = Vec::with_capacity(return_items.len());
            for item in &return_items {
                if let Some(p) = &item.prop {
                    let val = ctx.prop(&item.alias, p).cloned().unwrap_or(PropertyValue::Null);
                    row.push(val);
                } else if let Some(n) = ctx.node(&item.alias) {
                    let mut map = std::collections::BTreeMap::new();
                    map.insert("id".to_string(), PropertyValue::Int(n.id.raw() as i64));
                    for (k, v) in n.properties.iter() {
                        map.insert(k.clone(), v.clone());
                    }
                    row.push(PropertyValue::Map(map));
                } else {
                    row.push(PropertyValue::Null);
                }
            }
            row
        });

        Ok(CypherResult { columns, rows })
    }
}

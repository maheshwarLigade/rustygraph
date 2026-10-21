use crate::error::{GraphError, Result};
use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::Direction;
use crate::value::PropertyValue;
use std::collections::{BTreeMap, HashMap};
use std::fmt;

/// Tabular result returned by executing a Cypher query string.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CypherResult {
    #[serde(default = "default_true")]
    pub success: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<PropertyValue>>,
    #[serde(default)]
    pub error: Option<String>,
}

fn default_true() -> bool {
    true
}

impl CypherResult {
    pub fn new(columns: Vec<String>, rows: Vec<Vec<PropertyValue>>) -> Self {
        Self {
            success: true,
            columns,
            rows,
            error: None,
        }
    }

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
    Create,
    Set,
    Delete,
    Detach,
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
    Count,
    Sum,
    Avg,
    Min,
    Max,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Dot,
    Star,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    ArrowRight,
    ArrowLeft,
    Dash,
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
                '*' => {
                    self.chars.next();
                    tokens.push(Token::Star);
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
                        "CREATE" => tokens.push(Token::Create),
                        "SET" => tokens.push(Token::Set),
                        "DELETE" => tokens.push(Token::Delete),
                        "DETACH" => tokens.push(Token::Detach),
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
                        "COUNT" => tokens.push(Token::Count),
                        "SUM" => tokens.push(Token::Sum),
                        "AVG" => tokens.push(Token::Avg),
                        "MIN" => tokens.push(Token::Min),
                        "MAX" => tokens.push(Token::Max),
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
    props: Vec<(String, PropertyValue)>,
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
enum ReturnKind {
    Property(String, String), // alias.prop
    Node(String),             // alias
    CountStar,                // count(*)
    Count(String, Option<String>), // count(p) or count(p.age)
    Sum(String, String),      // sum(p.age)
    Avg(String, String),      // avg(p.age)
    Min(String, String),      // min(p.age)
    Max(String, String),      // max(p.age)
}

#[derive(Debug, Clone)]
struct ReturnItem {
    kind: ReturnKind,
    output_name: String,
}

#[derive(Debug, Clone)]
struct SetClause {
    alias: String,
    prop: String,
    val: PropertyValue,
}

#[derive(Debug, Clone)]
struct DeleteClause {
    alias: String,
    detach: bool,
}

#[derive(Debug, Clone)]
struct OrderByClause {
    alias: String,
    prop: Option<String>,
    descending: bool,
}

#[derive(Debug, Clone)]
struct AstQuery {
    matches: Vec<(Vec<ParsedNode>, Vec<ParsedRel>)>,
    creates: Vec<(Vec<ParsedNode>, Vec<ParsedRel>)>,
    where_clauses: Vec<WhereClause>,
    set_clauses: Vec<SetClause>,
    delete_clauses: Vec<DeleteClause>,
    returns: Vec<ReturnItem>,
    order_by: Vec<OrderByClause>,
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

        let mut props = Vec::new();
        if self.peek() == Some(&Token::LBrace) {
            self.next();
            while self.peek() != Some(&Token::RBrace) {
                let key = if let Some(Token::Ident(k)) = self.next() {
                    k
                } else {
                    return Err(GraphError::InvalidOperation("Expected property key in relationship".into()));
                };
                self.expect(Token::Colon)?;
                let val = match self.next() {
                    Some(Token::StringLit(s)) => PropertyValue::String(s),
                    Some(Token::IntLit(i)) => PropertyValue::Int(i),
                    Some(Token::FloatLit(f)) => PropertyValue::Float(f),
                    other => {
                        return Err(GraphError::InvalidOperation(format!("Expected literal value in relationship props, found {:?}", other)));
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

        self.expect(Token::RBracket)?;

        let direction = if is_incoming {
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
            direction,
            props,
        })
    }

    fn parse_pattern(&mut self) -> Result<(Vec<ParsedNode>, Vec<ParsedRel>)> {
        let mut nodes = Vec::new();
        let mut rels = Vec::new();

        nodes.push(self.parse_node()?);

        while self.peek() == Some(&Token::Dash) || self.peek() == Some(&Token::ArrowLeft) {
            rels.push(self.parse_rel()?);
            nodes.push(self.parse_node()?);
        }

        Ok((nodes, rels))
    }

    fn parse_query(&mut self) -> Result<AstQuery> {
        let mut matches = Vec::new();
        let mut creates = Vec::new();
        let mut where_clauses = Vec::new();
        let mut set_clauses = Vec::new();
        let mut delete_clauses = Vec::new();
        let mut returns = Vec::new();
        let mut order_by = Vec::new();
        let mut limit = None;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Match => {
                    self.next();
                    matches.push(self.parse_pattern()?);
                }
                Token::Create => {
                    self.next();
                    creates.push(self.parse_pattern()?);
                }
                Token::Where => {
                    self.next();
                    loop {
                        let alias = if let Some(Token::Ident(a)) = self.next() {
                            a
                        } else {
                            return Err(GraphError::InvalidOperation("Expected alias in WHERE".into()));
                        };

                        self.expect(Token::Dot)?;
                        let mut prop = if let Some(Token::Ident(p)) = self.next() {
                            p
                        } else {
                            return Err(GraphError::InvalidOperation("Expected property in WHERE".into()));
                        };
                        while self.peek() == Some(&Token::Dot) {
                            self.next();
                            if let Some(Token::Ident(sub)) = self.next() {
                                prop.push('.');
                                prop.push_str(&sub);
                            } else {
                                return Err(GraphError::InvalidOperation("Expected property segment after dot in WHERE".into()));
                            }
                        }

                        let op = match self.next() {
                            Some(Token::Eq) => CompOp::Eq,
                            Some(Token::Neq) => CompOp::Neq,
                            Some(Token::Lt) => CompOp::Lt,
                            Some(Token::Lte) => CompOp::Lte,
                            Some(Token::Gt) => CompOp::Gt,
                            Some(Token::Gte) => CompOp::Gte,
                            other => {
                                return Err(GraphError::InvalidOperation(format!(
                                    "Expected comparison operator in WHERE, found {other:?}"
                                )));
                            }
                        };

                        let val = match self.next() {
                            Some(Token::StringLit(s)) => PropertyValue::String(s),
                            Some(Token::IntLit(i)) => PropertyValue::Int(i),
                            Some(Token::FloatLit(f)) => PropertyValue::Float(f),
                            other => {
                                return Err(GraphError::InvalidOperation(format!(
                                    "Expected literal value in WHERE, found {other:?}"
                                )));
                            }
                        };

                        where_clauses.push(WhereClause { alias, prop, op, val });

                        if self.peek() == Some(&Token::And) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                Token::Set => {
                    self.next();
                    loop {
                        let alias = if let Some(Token::Ident(a)) = self.next() {
                            a
                        } else {
                            return Err(GraphError::InvalidOperation("Expected alias in SET clause".into()));
                        };
                        self.expect(Token::Dot)?;
                        let mut prop = if let Some(Token::Ident(p)) = self.next() {
                            p
                        } else {
                            return Err(GraphError::InvalidOperation("Expected property in SET clause".into()));
                        };
                        while self.peek() == Some(&Token::Dot) {
                            self.next();
                            if let Some(Token::Ident(sub)) = self.next() {
                                prop.push('.');
                                prop.push_str(&sub);
                            } else {
                                return Err(GraphError::InvalidOperation("Expected property segment after dot in SET".into()));
                            }
                        }
                        self.expect(Token::Eq)?;
                        let val = match self.next() {
                            Some(Token::StringLit(s)) => PropertyValue::String(s),
                            Some(Token::IntLit(i)) => PropertyValue::Int(i),
                            Some(Token::FloatLit(f)) => PropertyValue::Float(f),
                            other => {
                                return Err(GraphError::InvalidOperation(format!(
                                    "Expected literal value in SET, found {other:?}"
                                )));
                            }
                        };
                        set_clauses.push(SetClause { alias, prop, val });
                        if self.peek() == Some(&Token::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                Token::Delete => {
                    self.next();
                    loop {
                        if let Some(Token::Ident(alias)) = self.next() {
                            delete_clauses.push(DeleteClause { alias, detach: false });
                        }
                        if self.peek() == Some(&Token::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                Token::Detach => {
                    self.next();
                    self.expect(Token::Delete)?;
                    loop {
                        if let Some(Token::Ident(alias)) = self.next() {
                            delete_clauses.push(DeleteClause { alias, detach: true });
                        }
                        if self.peek() == Some(&Token::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                Token::Return => {
                    self.next();
                    loop {
                        let return_item = self.parse_return_item()?;
                        returns.push(return_item);
                        if self.peek() == Some(&Token::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                Token::Order => {
                    self.next();
                    self.expect(Token::By)?;
                    loop {
                        let alias = if let Some(Token::Ident(a)) = self.next() {
                            a
                        } else {
                            return Err(GraphError::InvalidOperation("Expected identifier in ORDER BY".into()));
                        };
                        let mut prop = None;
                        if self.peek() == Some(&Token::Dot) {
                            self.next();
                            if let Some(Token::Ident(mut p)) = self.next() {
                                while self.peek() == Some(&Token::Dot) {
                                    self.next();
                                    if let Some(Token::Ident(sub)) = self.next() {
                                        p.push('.');
                                        p.push_str(&sub);
                                    }
                                }
                                prop = Some(p);
                            }
                        }
                        let mut descending = false;
                        if self.peek() == Some(&Token::Desc) {
                            self.next();
                            descending = true;
                        } else if self.peek() == Some(&Token::Asc) {
                            self.next();
                        }
                        order_by.push(OrderByClause { alias, prop, descending });
                        if self.peek() == Some(&Token::Comma) {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                Token::Limit => {
                    self.next();
                    if let Some(Token::IntLit(n)) = self.next() {
                        limit = Some(n as usize);
                    } else {
                        return Err(GraphError::InvalidOperation("Expected integer in LIMIT".into()));
                    }
                }
                other => {
                    return Err(GraphError::InvalidOperation(format!(
                        "Unexpected token '{:?}' in Cypher query",
                        other
                    )));
                }
            }
        }

        if matches.is_empty() && creates.is_empty() && returns.is_empty() {
            return Err(GraphError::InvalidOperation(
                "Cypher query must contain at least a MATCH, CREATE, or RETURN clause".into(),
            ));
        }

        Ok(AstQuery {
            matches,
            creates,
            where_clauses,
            set_clauses,
            delete_clauses,
            returns,
            order_by,
            limit,
        })
    }

    fn parse_return_item(&mut self) -> Result<ReturnItem> {
        match self.peek() {
            Some(Token::Count) => {
                self.next();
                self.expect(Token::LParen)?;
                if self.peek() == Some(&Token::Star) {
                    self.next();
                    self.expect(Token::RParen)?;
                    Ok(ReturnItem {
                        kind: ReturnKind::CountStar,
                        output_name: "count(*)".to_string(),
                    })
                } else if let Some(Token::Ident(alias)) = self.next() {
                    let mut prop = None;
                    if self.peek() == Some(&Token::Dot) {
                        self.next();
                        if let Some(Token::Ident(mut p)) = self.next() {
                            while self.peek() == Some(&Token::Dot) {
                                self.next();
                                if let Some(Token::Ident(sub)) = self.next() {
                                    p.push('.');
                                    p.push_str(&sub);
                                }
                            }
                            prop = Some(p);
                        }
                    }
                    self.expect(Token::RParen)?;
                    let name = match &prop {
                        Some(p) => format!("count({}.{})", alias, p),
                        None => format!("count({})", alias),
                    };
                    Ok(ReturnItem {
                        kind: ReturnKind::Count(alias, prop),
                        output_name: name,
                    })
                } else {
                    Err(GraphError::InvalidOperation("Expected expression in count(...)".into()))
                }
            }
            Some(Token::Sum) => self.parse_agg_func(|a, p| ReturnKind::Sum(a, p), "sum"),
            Some(Token::Avg) => self.parse_agg_func(|a, p| ReturnKind::Avg(a, p), "avg"),
            Some(Token::Min) => self.parse_agg_func(|a, p| ReturnKind::Min(a, p), "min"),
            Some(Token::Max) => self.parse_agg_func(|a, p| ReturnKind::Max(a, p), "max"),
            Some(Token::Ident(name)) => {
                let alias = name.clone();
                self.next();
                if self.peek() == Some(&Token::Dot) {
                    self.next();
                    if let Some(Token::Ident(mut prop)) = self.next() {
                        while self.peek() == Some(&Token::Dot) {
                            self.next();
                            if let Some(Token::Ident(sub)) = self.next() {
                                prop.push('.');
                                prop.push_str(&sub);
                            }
                        }
                        let name = format!("{}.{}", alias, prop);
                        Ok(ReturnItem {
                            kind: ReturnKind::Property(alias, prop),
                            output_name: name,
                        })
                    } else {
                        Err(GraphError::InvalidOperation("Expected property after dot".into()))
                    }
                } else {
                    Ok(ReturnItem {
                        kind: ReturnKind::Node(alias.clone()),
                        output_name: alias,
                    })
                }
            }
            other => Err(GraphError::InvalidOperation(format!(
                "Unexpected return item: {:?}",
                other
            ))),
        }
    }

    fn parse_agg_func<F>(&mut self, constructor: F, func_name: &str) -> Result<ReturnItem>
    where
        F: FnOnce(String, String) -> ReturnKind,
    {
        self.next();
        self.expect(Token::LParen)?;
        let alias = if let Some(Token::Ident(a)) = self.next() {
            a
        } else {
            return Err(GraphError::InvalidOperation(format!("Expected identifier in {}()", func_name)));
        };
        self.expect(Token::Dot)?;
        let mut prop = if let Some(Token::Ident(p)) = self.next() {
            p
        } else {
            return Err(GraphError::InvalidOperation(format!("Expected property in {}()", func_name)));
        };
        while self.peek() == Some(&Token::Dot) {
            self.next();
            if let Some(Token::Ident(sub)) = self.next() {
                prop.push('.');
                prop.push_str(&sub);
            }
        }
        self.expect(Token::RParen)?;
        let name = format!("{}({}.{})", func_name, alias, prop);
        Ok(ReturnItem {
            kind: constructor(alias, prop),
            output_name: name,
        })
    }
}

// =========================================================================
// Graph Cypher Execution Engine
// =========================================================================

impl Graph {
    /// Execute an openCypher query string against the graph.
    /// Supports `MATCH`, `CREATE`, `SET`, `DELETE`, `DETACH DELETE`, `WHERE`,
    /// aggregations (`count`, `sum`, `avg`, `min`, `max`), and `ORDER BY`.
    pub fn cypher(&mut self, query_str: &str) -> Result<CypherResult> {
        let mut lexer = Lexer::new(query_str);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_query()?;

        // Keep track of aliases created or matched to node IDs
        let mut alias_node_map: HashMap<String, NodeId> = HashMap::new();

        // 1. Handle standalone or pre-match CREATE patterns
        for (nodes, rels) in &ast.creates {
            let mut pattern_node_ids = Vec::new();
            for n in nodes {
                let node_id = self.create_node_with(|node| {
                    if let Some(lbl) = &n.label {
                        node.add_label(lbl);
                    }
                    for (k, v) in &n.props {
                        node.set_prop(k, v.clone());
                    }
                })?;
                if !n.alias.is_empty() {
                    alias_node_map.insert(n.alias.clone(), node_id);
                }
                pattern_node_ids.push(node_id);
            }

            for (i, rel) in rels.iter().enumerate() {
                let src = pattern_node_ids[i];
                let dst = pattern_node_ids[i + 1];
                let rel_type = rel.rel_type.as_deref().unwrap_or("RELATED");
                self.create_edge_with(src, dst, rel_type, |edge| {
                    for (k, v) in &rel.props {
                        edge.set_prop(k, v.clone());
                    }
                })?;
            }
        }

        // If purely a CREATE query with no MATCH and no RETURN
        if ast.matches.is_empty() && ast.returns.is_empty() {
            let columns = vec!["result".to_string()];
            let rows = vec![vec![PropertyValue::String(format!(
                "Created {} nodes",
                alias_node_map.len()
            ))]];
            return Ok(CypherResult::new(columns, rows));
        }

        // If CREATE with RETURN
        if ast.matches.is_empty() && !ast.returns.is_empty() {
            let columns: Vec<String> = ast.returns.iter().map(|r| r.output_name.clone()).collect();
            let mut row = Vec::new();
            for item in &ast.returns {
                match &item.kind {
                    ReturnKind::Property(alias, prop) => {
                        let val = alias_node_map
                            .get(alias)
                            .and_then(|id| self.get_node_property(*id, prop))
                            .cloned()
                            .unwrap_or(PropertyValue::Null);
                        row.push(val);
                    }
                    ReturnKind::Node(alias) => {
                        if let Some(id) = alias_node_map.get(alias) {
                            if let Some(node) = self.get_node(*id) {
                                let mut map = BTreeMap::new();
                                map.insert("id".to_string(), PropertyValue::Int(node.id.raw() as i64));
                                for (k, v) in node.properties.iter() {
                                    map.insert(k.clone(), v.clone());
                                }
                                row.push(PropertyValue::Map(map));
                            } else {
                                row.push(PropertyValue::Null);
                            }
                        } else {
                            row.push(PropertyValue::Null);
                        }
                    }
                    ReturnKind::CountStar | ReturnKind::Count(_, _) => {
                        row.push(PropertyValue::Int(alias_node_map.len() as i64));
                    }
                    _ => row.push(PropertyValue::Null),
                }
            }
            return Ok(CypherResult::new(columns, vec![row]));
        }

        // 2. Process MATCH queries
        let (pattern_nodes, pattern_rels) = &ast.matches[0];
        let mut builder = self.query();

        if let Some(first_node) = pattern_nodes.first() {
            builder = builder.match_node(&first_node.alias);
            if let Some(lbl) = &first_node.label {
                builder = builder.with_label(lbl);
            }
            for (k, v) in &first_node.props {
                builder = builder.with_prop(k, v.clone());
            }
        }

        for (i, rel) in pattern_rels.iter().enumerate() {
            let target_node = &pattern_nodes[i + 1];
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

        // Apply WHERE clauses
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

        // Execute matching to capture context bindings
        struct MatchBinding {
            nodes: HashMap<String, NodeId>,
            edges: HashMap<String, crate::id::RelId>,
        }

        let bindings: Vec<MatchBinding> = builder
            .execute()
            .into_iter()
            .map(|ctx| MatchBinding {
                nodes: ctx.node_bindings().clone(),
                edges: ctx.edge_bindings().clone(),
            })
            .collect();

        // 3. Process SET clauses on matched entities
        if !ast.set_clauses.is_empty() {
            for b in &bindings {
                for s in &ast.set_clauses {
                    if let Some(&node_id) = b.nodes.get(&s.alias) {
                        let _ = self.set_node_property(node_id, &s.prop, s.val.clone());
                    }
                }
            }
        }

        // 4. Process DELETE & DETACH DELETE clauses
        if !ast.delete_clauses.is_empty() {
            let mut nodes_to_delete = Vec::new();
            for b in &bindings {
                for d in &ast.delete_clauses {
                    if let Some(&node_id) = b.nodes.get(&d.alias) {
                        nodes_to_delete.push((node_id, d.detach));
                    }
                }
            }

            for (node_id, detach) in nodes_to_delete {
                if detach {
                    let _ = self.delete_node(node_id);
                } else {
                    let deg = self.degree(node_id, Direction::Both)?;
                    if deg > 0 {
                        return Err(GraphError::InvalidOperation(format!(
                            "Cannot delete node {} because it still has {} relationships; use DETACH DELETE",
                            node_id.0, deg
                        )));
                    }
                    self.delete_node(node_id)?;
                }
            }
        }

        // If query was purely a mutation (SET/DELETE) without RETURN
        if ast.returns.is_empty() {
            let columns = vec!["result".to_string()];
            let rows = vec![vec![PropertyValue::String(format!(
                "Updated {} matched entities",
                bindings.len()
            ))]];
            return Ok(CypherResult::new(columns, rows));
        }

        // Helper closures for reading properties / nodes post-mutation
        let get_prop = |b: &MatchBinding, alias: &str, prop: &str| -> Option<PropertyValue> {
            if let Some(&nid) = b.nodes.get(alias) {
                self.get_node(nid).and_then(|n| n.prop_path(prop)).cloned()
            } else if let Some(&eid) = b.edges.get(alias) {
                self.get_edge(eid).and_then(|e| e.prop_path(prop)).cloned()
            } else {
                None
            }
        };

        let get_node_val = |b: &MatchBinding, alias: &str| -> PropertyValue {
            if let Some(&nid) = b.nodes.get(alias) {
                if let Some(n) = self.get_node(nid) {
                    let mut map = BTreeMap::new();
                    map.insert("id".to_string(), PropertyValue::Int(n.id.raw() as i64));
                    for (k, v) in n.properties.iter() {
                        map.insert(k.clone(), v.clone());
                    }
                    PropertyValue::Map(map)
                } else {
                    PropertyValue::Null
                }
            } else {
                PropertyValue::Null
            }
        };

        // 5. Check if query contains aggregation functions
        let has_aggregations = ast.returns.iter().any(|r| {
            matches!(
                r.kind,
                ReturnKind::CountStar
                    | ReturnKind::Count(_, _)
                    | ReturnKind::Sum(_, _)
                    | ReturnKind::Avg(_, _)
                    | ReturnKind::Min(_, _)
                    | ReturnKind::Max(_, _)
            )
        });

        let columns: Vec<String> = ast.returns.iter().map(|r| r.output_name.clone()).collect();

        if has_aggregations {
            let mut row = Vec::new();
            for item in &ast.returns {
                match &item.kind {
                    ReturnKind::CountStar => {
                        row.push(PropertyValue::Int(bindings.len() as i64));
                    }
                    ReturnKind::Count(alias, prop) => {
                        let mut cnt = 0;
                        for b in &bindings {
                            if let Some(p) = prop {
                                if get_prop(b, alias, p).is_some() {
                                    cnt += 1;
                                }
                            } else if b.nodes.contains_key(alias) {
                                cnt += 1;
                            }
                        }
                        row.push(PropertyValue::Int(cnt));
                    }
                    ReturnKind::Sum(alias, prop) => {
                        let mut sum = 0.0f64;
                        for b in &bindings {
                            if let Some(v) = get_prop(b, alias, prop) {
                                if let Some(n) = v.as_f64() {
                                    sum += n;
                                }
                            }
                        }
                        row.push(PropertyValue::Float(sum));
                    }
                    ReturnKind::Avg(alias, prop) => {
                        let mut sum = 0.0f64;
                        let mut cnt = 0;
                        for b in &bindings {
                            if let Some(v) = get_prop(b, alias, prop) {
                                if let Some(n) = v.as_f64() {
                                    sum += n;
                                    cnt += 1;
                                }
                            }
                        }
                        let avg = if cnt > 0 { sum / cnt as f64 } else { 0.0 };
                        row.push(PropertyValue::Float(avg));
                    }
                    ReturnKind::Min(alias, prop) => {
                        let mut min_val: Option<PropertyValue> = None;
                        for b in &bindings {
                            if let Some(v) = get_prop(b, alias, prop) {
                                match &min_val {
                                    None => min_val = Some(v),
                                    Some(curr) => {
                                        if &v < curr {
                                            min_val = Some(v);
                                        }
                                    }
                                }
                            }
                        }
                        row.push(min_val.unwrap_or(PropertyValue::Null));
                    }
                    ReturnKind::Max(alias, prop) => {
                        let mut max_val: Option<PropertyValue> = None;
                        for b in &bindings {
                            if let Some(v) = get_prop(b, alias, prop) {
                                match &max_val {
                                    None => max_val = Some(v),
                                    Some(curr) => {
                                        if &v > curr {
                                            max_val = Some(v);
                                        }
                                    }
                                }
                            }
                        }
                        row.push(max_val.unwrap_or(PropertyValue::Null));
                    }
                    ReturnKind::Property(alias, prop) => {
                        let val = bindings
                            .first()
                            .and_then(|b| get_prop(b, alias, prop))
                            .unwrap_or(PropertyValue::Null);
                        row.push(val);
                    }
                    ReturnKind::Node(alias) => {
                        let node_val = bindings
                            .first()
                            .map(|b| get_node_val(b, alias))
                            .unwrap_or(PropertyValue::Null);
                        row.push(node_val);
                    }
                }
            }
            return Ok(CypherResult::new(columns, vec![row]));
        }

        // Standard Projections (non-aggregated)
        let mut rows: Vec<Vec<PropertyValue>> = Vec::with_capacity(bindings.len());

        for b in &bindings {
            let mut row = Vec::with_capacity(ast.returns.len());
            for item in &ast.returns {
                match &item.kind {
                    ReturnKind::Property(alias, prop) => {
                        let val = get_prop(b, alias, prop).unwrap_or(PropertyValue::Null);
                        row.push(val);
                    }
                    ReturnKind::Node(alias) => {
                        row.push(get_node_val(b, alias));
                    }
                    _ => row.push(PropertyValue::Null),
                }
            }
            rows.push(row);
        }

        // 6. Handle ORDER BY
        if !ast.order_by.is_empty() {
            let order_spec = ast.order_by.clone();
            let return_spec = ast.returns.clone();

            rows.sort_by(|row_a, row_b| {
                for order in &order_spec {
                    // Locate column index
                    let col_idx = return_spec.iter().position(|r| match &r.kind {
                        ReturnKind::Property(a, p) => {
                            a == &order.alias && order.prop.as_deref() == Some(p)
                        }
                        ReturnKind::Node(a) => a == &order.alias && order.prop.is_none(),
                        _ => false,
                    });

                    if let Some(idx) = col_idx {
                        let val_a = &row_a[idx];
                        let val_b = &row_b[idx];
                        let cmp = val_a.cmp(val_b);
                        if cmp != std::cmp::Ordering::Equal {
                            return if order.descending {
                                cmp.reverse()
                            } else {
                                cmp
                            };
                        }
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        // 7. Handle LIMIT
        if let Some(lim) = ast.limit {
            if rows.len() > lim {
                rows.truncate(lim);
            }
        }

        Ok(CypherResult::new(columns, rows))
    }
}

/// Returns true if the Cypher query string contains any mutating operations (`CREATE`, `SET`, `DELETE`, `DETACH`).
pub fn is_cypher_mutation(query_str: &str) -> bool {
    let mut lexer = Lexer::new(query_str);
    if let Ok(tokens) = lexer.tokenize() {
        tokens.iter().any(|t| matches!(t, Token::Create | Token::Set | Token::Delete | Token::Detach))
    } else {
        false
    }
}

use super::ast::{
    AstQuery, CompOp, DeleteClause, OrderByClause, ParsedNode, ParsedRel, ReturnItem, ReturnKind,
    SetClause, WhereClause,
};
use super::token::Token;
use crate::error::{GraphError, Result};
use crate::model::Direction;
use crate::value::PropertyValue;

/// Recursive-descent parser for openCypher queries.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
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

    pub fn parse_query(&mut self) -> Result<AstQuery> {
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

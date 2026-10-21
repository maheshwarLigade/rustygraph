use super::token::Token;
use crate::error::{GraphError, Result};

/// Lexical scanner for openCypher query strings.
pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
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
                '$' => {
                    self.chars.next();
                    let mut param = String::new();
                    while let Some(&c) = self.chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            param.push(c);
                            self.chars.next();
                        } else {
                            break;
                        }
                    }
                    if param.is_empty() {
                        return Err(GraphError::InvalidOperation("Expected parameter name after '$'".into()));
                    }
                    tokens.push(Token::Parameter(param));
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

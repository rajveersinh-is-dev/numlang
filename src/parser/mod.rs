pub mod expr;
pub mod stmt;

use crate::ast::{Expr, Program};
use crate::span::Span;
use crate::token::{SpannedToken, Token};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseError {
    #[error("Unexpected end of input, expected {expected} at {span:?}")]
    UnexpectedEof { expected: String, span: Span },

    #[error("Unexpected token {found:?}, expected {expected} at {span:?}")]
    UnexpectedToken {
        found: Token,
        expected: String,
        span: Span,
    },

    #[error("Invalid prefix operator {found:?} at {span:?}")]
    InvalidPrefix { found: Token, span: Span },
}

pub struct Parser<'a> {
    tokens: &'a [SpannedToken],
    cursor: usize,
    pub errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [SpannedToken]) -> Self {
        Self {
            tokens,
            cursor: 0,
            errors: Vec::new(),
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor).map(|t| &t.token)
    }

    pub fn peek_token(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.cursor)
    }

    pub fn advance(&mut self) -> Option<&SpannedToken> {
        if !self.is_at_end() {
            let tok = &self.tokens[self.cursor];
            self.cursor += 1;
            Some(tok)
        } else {
            None
        }
    }

    pub fn previous_span(&self) -> Span {
        if self.cursor > 0 && self.cursor - 1 < self.tokens.len() {
            self.tokens[self.cursor - 1].span
        } else {
            Span::default()
        }
    }

    pub fn current_span(&self) -> Span {
        if let Some(t) = self.tokens.get(self.cursor) {
            t.span
        } else {
            self.previous_span()
        }
    }

    pub fn check(&self, expected: &Token) -> bool {
        match self.peek() {
            Some(tok) => tok == expected,
            None => false,
        }
    }

    pub fn match_token(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn consume(&mut self, expected: &Token, desc: &str) -> Result<Span, ParseError> {
        if self.check(expected) {
            let sp = self.advance().unwrap().span;
            Ok(sp)
        } else if let Some(tok) = self.peek_token() {
            Err(ParseError::UnexpectedToken {
                found: tok.token.clone(),
                expected: desc.to_string(),
                span: tok.span,
            })
        } else {
            Err(ParseError::UnexpectedEof {
                expected: desc.to_string(),
                span: self.previous_span(),
            })
        }
    }

    pub fn expect_ident(&mut self, desc: &str) -> Result<String, ParseError> {
        match self.peek_token().cloned() {
            Some(tok) => match tok.token {
                Token::Ident(name) => {
                    self.advance();
                    Ok(name)
                }
                other => Err(ParseError::UnexpectedToken {
                    found: other,
                    expected: desc.to_string(),
                    span: tok.span,
                }),
            },
            None => Err(ParseError::UnexpectedEof {
                expected: desc.to_string(),
                span: self.previous_span(),
            }),
        }
    }

    pub fn synchronize(&mut self) {
        let start_cursor = self.cursor;
        while !self.is_at_end() {
            if let Some(tok) = self.peek() {
                match tok {
                    Token::Semi => {
                        self.advance();
                        return;
                    }
                    Token::RBrace | Token::Fn | Token::Struct | Token::Enum => {
                        if self.cursor == start_cursor {
                            self.advance();
                        }
                        return;
                    }
                    _ => {
                        self.advance();
                    }
                }
            } else {
                return;
            }
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut enums = Vec::new();
        let mut items = Vec::new();
        let mut pending_doc_comments: Vec<String> = Vec::new();

        while !self.is_at_end() {
            if let Some(Token::DocComment(doc)) = self.peek().cloned() {
                self.advance();
                pending_doc_comments.push(doc);
                continue;
            }

            if self.check(&Token::Struct) {
                let doc = if pending_doc_comments.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut pending_doc_comments).join("\n"))
                };
                match self.parse_struct_def(doc) {
                    Ok(s) => {
                        structs.push(s.clone());
                        items.push(crate::ast::Item::Struct(s));
                    }
                    Err(e) => {
                        self.errors.push(e);
                        self.synchronize();
                    }
                }
            } else if self.check(&Token::Enum) {
                let doc = if pending_doc_comments.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut pending_doc_comments).join("\n"))
                };
                match self.parse_enum_def(doc) {
                    Ok(e) => {
                        enums.push(e.clone());
                        items.push(crate::ast::Item::Enum(e));
                    }
                    Err(e) => {
                        self.errors.push(e);
                        self.synchronize();
                    }
                }
            } else if self.check(&Token::Fn) {
                let doc = if pending_doc_comments.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut pending_doc_comments).join("\n"))
                };
                match self.parse_function(doc) {
                    Ok(f) => {
                        functions.push(f.clone());
                        items.push(crate::ast::Item::Function(f));
                    }
                    Err(e) => {
                        self.errors.push(e);
                        self.synchronize();
                    }
                }
            } else if let Some(tok) = self.peek_token().cloned() {
                pending_doc_comments.clear();
                self.errors.push(ParseError::UnexpectedToken {
                    found: tok.token,
                    expected: "function, struct, or enum definition".to_string(),
                    span: tok.span,
                });
                self.synchronize();
            } else {
                break;
            }
        }

        if let Some(first_err) = self.errors.first() {
            return Err(first_err.clone());
        }

        Ok(Program {
            items,
            functions,
            structs,
            enums,
        })
    }
}

pub fn parse(tokens: &[SpannedToken]) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

pub fn parse_expr_str(tokens: &[SpannedToken]) -> Result<Expr, ParseError> {
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr(0)?;
    Ok(expr)
}

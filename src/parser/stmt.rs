use crate::ast::{Block, Function, Param, Stmt};
use crate::parser::{ParseError, Parser};
use crate::token::Token;

impl<'a> Parser<'a> {
    pub fn parse_function(&mut self) -> Result<Function, ParseError> {
        let fn_span = self.consume(&Token::Fn, "'fn' keyword")?;

        let name = match self.peek_token().cloned() {
            Some(t) => match t.token {
                Token::Ident(id) => {
                    self.advance();
                    id
                }
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        found: t.token,
                        expected: "function name".to_string(),
                        span: t.span,
                    });
                }
            },
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "function name".to_string(),
                    span: fn_span,
                });
            }
        };

        self.consume(&Token::LParen, "'(' after function name")?;
        let mut params = Vec::new();
        if !self.check(&Token::RParen) {
            loop {
                params.push(self.parse_param()?);
                if self.match_token(&Token::Comma) {
                    continue;
                } else {
                    break;
                }
            }
        }
        self.consume(&Token::RParen, "')' after parameters")?;

        let mut return_ty = None;
        if self.match_token(&Token::Arrow) {
            return_ty = Some(self.parse_type()?);
        }

        let body = self.parse_block()?;
        let span = fn_span.merge(&body.span);

        Ok(Function {
            name,
            params,
            return_ty,
            body,
            span,
        })
    }

    pub fn parse_param(&mut self) -> Result<Param, ParseError> {
        let (name, name_span) = match self.peek_token().cloned() {
            Some(t) => match t.token {
                Token::Ident(id) => {
                    self.advance();
                    (id, t.span)
                }
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        found: t.token,
                        expected: "parameter name".to_string(),
                        span: t.span,
                    });
                }
            },
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "parameter name".to_string(),
                    span: self.previous_span(),
                });
            }
        };

        self.consume(&Token::Colon, "':' after parameter name")?;
        let ty = self.parse_type()?;

        Ok(Param {
            name,
            ty,
            span: name_span,
        })
    }

    pub fn parse_block(&mut self) -> Result<Block, ParseError> {
        let open_span = self.consume(&Token::LBrace, "'{' to begin block")?;
        let mut stmts = Vec::new();

        while !self.check(&Token::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }

        let close_span = self.consume(&Token::RBrace, "'}' to close block")?;
        Ok(Block {
            stmts,
            span: open_span.merge(&close_span),
        })
    }

    pub fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        if self.check(&Token::Let) {
            let let_span = self.advance().unwrap().span;
            let is_mutable = if self.check(&Token::Mut) {
                self.advance();
                true
            } else {
                false
            };
            let (name, _) = match self.peek_token().cloned() {
                Some(t) => match t.token {
                    Token::Ident(id) => {
                        self.advance();
                        (id, t.span)
                    }
                    _ => {
                        return Err(ParseError::UnexpectedToken {
                            found: t.token,
                            expected: "variable name".to_string(),
                            span: t.span,
                        });
                    }
                },
                None => {
                    return Err(ParseError::UnexpectedEof {
                        expected: "variable name".to_string(),
                        span: let_span,
                    });
                }
            };

            let mut ty = None;
            if self.match_token(&Token::Colon) {
                ty = Some(self.parse_type()?);
            }

            self.consume(&Token::Assign, "'=' in variable binding")?;
            let value = self.parse_expr(0)?;
            let semi_span = self.consume(&Token::Semi, "';' after variable binding")?;
            let span = let_span.merge(&semi_span);

            Ok(Stmt::Let {
                name,
                is_mutable,
                ty,
                value,
                span,
            })
        } else if self.check(&Token::Return) {
            let ret_span = self.advance().unwrap().span;
            let mut value = None;
            if !self.check(&Token::Semi) {
                value = Some(self.parse_expr(0)?);
            }
            let semi_span = self.consume(&Token::Semi, "';' after return statement")?;
            let span = ret_span.merge(&semi_span);
            Ok(Stmt::Return(value, span))
        } else if self.check(&Token::If) {
            let if_span = self.advance().unwrap().span;
            let condition = self.parse_expr(0)?;
            let then_branch = self.parse_block()?;

            let mut else_branch = None;
            let mut end_span = then_branch.span;
            if self.match_token(&Token::Else) {
                let else_block = self.parse_block()?;
                end_span = else_block.span;
                else_branch = Some(else_block);
            }

            let span = if_span.merge(&end_span);
            Ok(Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            })
        } else if self.check(&Token::While) {
            let while_span = self.advance().unwrap().span;
            let condition = self.parse_expr(0)?;
            let body = self.parse_block()?;
            let span = while_span.merge(&body.span);
            Ok(Stmt::While {
                condition,
                body,
                span,
            })
        } else if self.cursor + 1 < self.tokens.len()
            && matches!(self.tokens[self.cursor].token, Token::Ident(_))
            && self.tokens[self.cursor + 1].token == Token::Assign
        {
            let id_token = self.advance().unwrap();
            let name = match &id_token.token {
                Token::Ident(id) => id.clone(),
                _ => unreachable!(),
            };
            let start_span = id_token.span;
            self.consume(&Token::Assign, "'=' in assignment")?;
            let value = self.parse_expr(0)?;
            let semi_span = self.consume(&Token::Semi, "';' after assignment")?;
            let span = start_span.merge(&semi_span);
            Ok(Stmt::Assign { name, value, span })
        } else if self.is_index_assignment() {
            let id_token = self.advance().unwrap();
            let name = match &id_token.token {
                Token::Ident(id) => id.clone(),
                _ => unreachable!(),
            };
            let start_span = id_token.span;
            self.consume(&Token::LBracket, "'[' in array index assignment")?;
            let index = self.parse_expr(0)?;
            self.consume(&Token::RBracket, "']' in array index assignment")?;
            self.consume(&Token::Assign, "'=' in array element assignment")?;
            let value = self.parse_expr(0)?;
            let semi_span = self.consume(&Token::Semi, "';' after assignment")?;
            let span = start_span.merge(&semi_span);
            Ok(Stmt::IndexAssign {
                target: name,
                index,
                value,
                span,
            })
        } else {
            let expr = self.parse_expr(0)?;
            let _semi_span = self.consume(&Token::Semi, "';' after expression statement")?;
            Ok(Stmt::Expr(expr))
        }
    }

    pub fn parse_type(&mut self) -> Result<String, ParseError> {
        match self.peek_token().cloned() {
            Some(t) => match t.token {
                Token::Ident(id) => {
                    self.advance();
                    Ok(id)
                }
                Token::LBracket => {
                    self.advance();
                    let elem_ty = self.parse_type()?;
                    self.consume(&Token::Semi, "';' in array type [T; N]")?;
                    let len = match self.peek_token().cloned() {
                        Some(t) => match t.token {
                            Token::IntLiteral(n) => {
                                self.advance();
                                n
                            }
                            _ => {
                                return Err(ParseError::UnexpectedToken {
                                    found: t.token,
                                    expected: "array length integer".to_string(),
                                    span: t.span,
                                });
                            }
                        },
                        None => {
                            return Err(ParseError::UnexpectedEof {
                                expected: "array length integer".to_string(),
                                span: self.previous_span(),
                            });
                        }
                    };
                    self.consume(&Token::RBracket, "']' after array type")?;
                    Ok(format!("[{}; {}]", elem_ty, len))
                }
                _ => Err(ParseError::UnexpectedToken {
                    found: t.token,
                    expected: "type identifier or array type".to_string(),
                    span: t.span,
                }),
            },
            None => Err(ParseError::UnexpectedEof {
                expected: "type identifier or array type".to_string(),
                span: self.previous_span(),
            }),
        }
    }

    fn is_index_assignment(&self) -> bool {
        if self.cursor >= self.tokens.len() {
            return false;
        }
        if !matches!(self.tokens[self.cursor].token, Token::Ident(_)) {
            return false;
        }
        if self.cursor + 1 >= self.tokens.len() || self.tokens[self.cursor + 1].token != Token::LBracket {
            return false;
        }
        let mut depth = 0;
        let mut i = self.cursor + 1;
        while i < self.tokens.len() {
            match self.tokens[i].token {
                Token::LBracket => depth += 1,
                Token::RBracket => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1 < self.tokens.len() && self.tokens[i + 1].token == Token::Assign;
                    }
                }
                Token::Semi => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }
}

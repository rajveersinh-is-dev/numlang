use crate::ast::{BinaryOp, Expr, Literal, UnaryOp};
use crate::parser::{ParseError, Parser};
use crate::token::Token;

impl<'a> Parser<'a> {
    pub fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // Check for postfix indexing `[`
            if self.check(&Token::LBracket) {
                if min_bp > 25 {
                    break;
                }
                self.advance(); // consume '['
                let index = self.parse_expr(0)?;
                let end_span = self.consume(&Token::RBracket, "']' after array index")?;
                let span = lhs.span().merge(&end_span);
                lhs = Expr::Index {
                    target: Box::new(lhs),
                    index: Box::new(index),
                    span,
                };
                continue;
            }

            // Check for postfix field access `.`
            if self.check(&Token::Dot) {
                if min_bp > 25 {
                    break;
                }
                self.advance(); // consume '.'
                let dot_span = self.previous_span();
                let field_tok = self.advance().ok_or_else(|| ParseError::UnexpectedEof {
                    expected: "field name after '.'".to_string(),
                    span: dot_span,
                })?;
                let field = match &field_tok.token {
                    Token::Ident(id) => id.clone(),
                    other => {
                        return Err(ParseError::UnexpectedToken {
                            found: other.clone(),
                            expected: "field name after '.'".to_string(),
                            span: field_tok.span,
                        })
                    }
                };
                let span = lhs.span().merge(&field_tok.span);
                lhs = Expr::FieldAccess {
                    target: Box::new(lhs),
                    field,
                    span,
                };
                continue;
            }

            let op = match self.peek() {
                Some(op) => op.clone(),
                None => break,
            };

            if let Some((l_bp, r_bp, bin_op)) = infix_binding_power(&op) {
                if l_bp < min_bp {
                    break;
                }

                self.advance(); // consume operator
                let rhs = self.parse_expr(r_bp)?;
                let span = lhs.span().merge(&rhs.span());
                lhs = Expr::Binary {
                    op: bin_op,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                    span,
                };
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        let token_spanned = match self.peek_token() {
            Some(t) => t.clone(),
            None => {
                return Err(ParseError::UnexpectedEof {
                    expected: "expression".to_string(),
                    span: self.previous_span(),
                });
            }
        };

        match token_spanned.token {
            Token::IntLiteral(n) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int(n), token_spanned.span))
            }
            Token::TypedIntLiteral((n, ref s)) => {
                let s_clone = s.clone();
                self.advance();
                Ok(Expr::Literal(Literal::TypedInt(n, s_clone), token_spanned.span))
            }
            Token::FloatLiteral(f) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(f), token_spanned.span))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true), token_spanned.span))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false), token_spanned.span))
            }
            Token::StringLiteral(s) => {
                self.advance();
                Ok(Expr::Literal(Literal::Str(s), token_spanned.span))
            }
            Token::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                if !self.check(&Token::RBracket) {
                    loop {
                        elements.push(self.parse_expr(0)?);
                        if self.match_token(&Token::Comma) {
                            if self.check(&Token::RBracket) {
                                break;
                            }
                            continue;
                        } else {
                            break;
                        }
                    }
                }
                let end_span = self.consume(&Token::RBracket, "']' after array elements")?;
                let span = token_spanned.span.merge(&end_span);
                Ok(Expr::ArrayLiteral { elements, span })
            }
            Token::Ident(name) => {
                self.advance();
                // Check if followed by function call '('
                if self.check(&Token::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(&Token::RParen) {
                        loop {
                            args.push(self.parse_expr(0)?);
                            if self.match_token(&Token::Comma) {
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                    let end_span = self.consume(&Token::RParen, "')' after function arguments")?;
                    let span = token_spanned.span.merge(&end_span);
                    Ok(Expr::Call {
                        callee: name,
                        args,
                        span,
                    })
                } else if self.is_struct_literal_start() {
                    self.advance(); // consume '{'
                    let mut fields = Vec::new();
                    if !self.check(&Token::RBrace) {
                        loop {
                            let prev_span = self.previous_span();
                            let field_tok = self.advance().ok_or_else(|| ParseError::UnexpectedEof {
                                expected: "field name in struct literal".to_string(),
                                span: prev_span,
                            })?;
                            let field_name = match &field_tok.token {
                                Token::Ident(id) => id.clone(),
                                other => {
                                    return Err(ParseError::UnexpectedToken {
                                        found: other.clone(),
                                        expected: "field name in struct literal".to_string(),
                                        span: field_tok.span,
                                    })
                                }
                            };
                            self.consume(&Token::Colon, "':' after field name in struct literal")?;
                            let val = self.parse_expr(0)?;
                            fields.push((field_name, val));
                            if self.match_token(&Token::Comma) {
                                if self.check(&Token::RBrace) {
                                    break;
                                }
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                    let end_span = self.consume(&Token::RBrace, "'}' after struct literal fields")?;
                    let span = token_spanned.span.merge(&end_span);
                    Ok(Expr::StructLiteral {
                        name,
                        fields,
                        span,
                    })
                } else {
                    Ok(Expr::Ident(name, token_spanned.span))
                }
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr(0)?;
                let end_span = self.consume(&Token::RParen, "')' after grouped expression")?;
                let span = token_spanned.span.merge(&end_span);
                Ok(Expr::Group(Box::new(expr), span))
            }
            Token::Minus => {
                self.advance();
                let expr = self.parse_expr(20)?;
                let span = token_spanned.span.merge(&expr.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    span,
                })
            }
            Token::Not => {
                self.advance();
                let expr = self.parse_expr(20)?;
                let span = token_spanned.span.merge(&expr.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    span,
                })
            }
            other => Err(ParseError::InvalidPrefix {
                found: other,
                span: token_spanned.span,
            }),
        }
    }

    fn is_struct_literal_start(&self) -> bool {
        if !self.check(&Token::LBrace) {
            return false;
        }
        if self.cursor + 1 >= self.tokens.len() {
            return false;
        }
        let next_tok = &self.tokens[self.cursor + 1].token;
        if matches!(next_tok, Token::RBrace) {
            return true;
        }
        if matches!(next_tok, Token::Ident(_))
            && self.cursor + 2 < self.tokens.len()
            && self.tokens[self.cursor + 2].token == Token::Colon
        {
            return true;
        }
        false
    }
}

fn infix_binding_power(op: &Token) -> Option<(u8, u8, BinaryOp)> {
    match op {
        Token::Pipe => Some((1, 2, BinaryOp::BitOr)),
        Token::Caret => Some((3, 4, BinaryOp::BitXor)),
        Token::Ampersand => Some((5, 6, BinaryOp::BitAnd)),
        Token::Eq => Some((7, 8, BinaryOp::Eq)),
        Token::Ne => Some((7, 8, BinaryOp::Ne)),
        Token::Lt => Some((9, 10, BinaryOp::Lt)),
        Token::Le => Some((9, 10, BinaryOp::Le)),
        Token::Gt => Some((9, 10, BinaryOp::Gt)),
        Token::Ge => Some((9, 10, BinaryOp::Ge)),
        Token::Shl => Some((11, 12, BinaryOp::Shl)),
        Token::Shr => Some((11, 12, BinaryOp::Shr)),
        Token::Plus => Some((13, 14, BinaryOp::Add)),
        Token::Minus => Some((13, 14, BinaryOp::Sub)),
        Token::Star => Some((15, 16, BinaryOp::Mul)),
        Token::Slash => Some((15, 16, BinaryOp::Div)),
        Token::Percent => Some((15, 16, BinaryOp::Mod)),
        Token::StarStar => Some((18, 17, BinaryOp::Pow)), // Right-associative (left power > right power)
        _ => None,
    }
}

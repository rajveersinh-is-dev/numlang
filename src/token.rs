use logos::Logos;
use crate::span::Span;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f\r]+")]
#[logos(skip r"//[^\n]*")]
pub enum Token {
    // Keywords
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("break")]
    Break,
    #[token("for")]
    For,
    #[token("true")]
    True,
    #[token("false")]
    False,

    // Mathematical & Logical Operators
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("**")]
    StarStar,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("^")]
    Caret,
    #[token("&")]
    Ampersand,
    #[token("|")]
    Pipe,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,
    #[token("=")]
    Assign,
    #[token("==")]
    Eq,
    #[token("!=")]
    Ne,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("!")]
    Not,

    // Delimiters & Punctuation
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semi,
    #[token("->")]
    Arrow,

    // Literals
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLiteral(f64),

    #[regex(r"[0-9]+(u8|u16|u32|u64|usize|i32|i64)", |lex| {
        let s = lex.slice();
        let idx = s.find(|c: char| c.is_alphabetic())?;
        let (num_str, suffix) = s.split_at(idx);
        let n = num_str.parse::<u64>().ok()? as i64;
        Some((n, suffix.to_string()))
    })]
    TypedIntLiteral((i64, String)),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLiteral(i64),

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LexError {
    #[error("Invalid or unrecognized token at {0:?}")]
    InvalidToken(Span),
}

pub fn tokenize(source: &str) -> Result<Vec<SpannedToken>, LexError> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();

    while let Some(token_res) = lexer.next() {
        let span = Span::from(lexer.span());
        match token_res {
            Ok(token) => tokens.push(SpannedToken { token, span }),
            Err(_) => return Err(LexError::InvalidToken(span)),
        }
    }

    Ok(tokens)
}

use logos::Logos;
use crate::span::Span;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f\r]+")]
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
    #[token("continue")]
    Continue,
    #[token("loop")]
    Loop,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("match")]
    Match,
    #[token("box")]
    Box_,
    #[token("deref")]
    Deref_,

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
    #[token("::")]
    ColonColon,
    #[token(":")]
    Colon,
    #[token(";")]
    Semi,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("_", priority = 3)]
    Underscore,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,

    // Literals
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLiteral(f64),

    #[regex(r"[0-9]+(u8|u16|u32|u64|usize|i8|i16|i32|i64)", |lex| {
        let s = lex.slice();
        let idx = s.find(|c: char| c.is_alphabetic())?;
        let (num_str, suffix) = s.split_at(idx);
        let n = num_str.parse::<u64>().ok()? as i64;
        Some((n, suffix.to_string()))
    })]
    TypedIntLiteral((i64, String)),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLiteral(i64),

    #[regex(r#""([^"\\]|\\.)*""#, parse_string_literal)]
    StringLiteral(String),

    // Doc comments (///)
    #[regex(r"///[^\r\n]*", parse_doc_comment)]
    DocComment(String),

    // Regular comments (//)
    #[regex(r"//[^\r\n]*", logos::skip)]
    Comment,

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}

fn parse_doc_comment(lex: &mut logos::Lexer<Token>) -> Option<String> {
    let s = lex.slice().trim_end_matches('\r');
    if let Some(stripped) = s.strip_prefix("/// ") {
        Some(stripped.to_string())
    } else if let Some(stripped) = s.strip_prefix("///") {
        Some(stripped.to_string())
    } else {
        Some(s.to_string())
    }
}

fn parse_string_literal(lex: &mut logos::Lexer<Token>) -> Option<String> {
    let s = lex.slice();
    if s.len() < 2 {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    let mut res = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => res.push('\n'),
                Some('r') => res.push('\r'),
                Some('t') => res.push('\t'),
                Some('\\') => res.push('\\'),
                Some('"') => res.push('"'),
                Some('0') => res.push('\0'),
                Some(other) => {
                    res.push('\\');
                    res.push(other);
                }
                None => res.push('\\'),
            }
        } else {
            res.push(c);
        }
    }
    Some(res)
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

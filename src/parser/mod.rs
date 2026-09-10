pub mod expr;
pub mod stmt;

use crate::ast::Program;
use crate::token::SpannedToken;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseError {
    #[error("Parse error: {0}")]
    General(String),
}

pub fn parse(_tokens: &[SpannedToken]) -> Result<Program, ParseError> {
    Ok(Program::default())
}

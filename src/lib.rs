#![deny(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod ast;
pub mod codegen;
pub mod compiler;
pub mod diagnostic;
pub mod doc;
pub mod explain;
pub mod fmt;
pub mod ir;
pub mod mir;
pub mod opt;
pub mod parser;
pub mod runtime;
pub mod span;
pub mod testing;
pub mod token;
pub mod typecheck;

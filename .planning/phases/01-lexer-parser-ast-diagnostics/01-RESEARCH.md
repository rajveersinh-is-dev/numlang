# Phase 1: Lexer, Parser & AST Diagnostics - Research

**Researched:** 2026-09-10
**Domain:** Rust Compiler Frontend (Lexing, AST, Pratt Parsing, Diagnostic Reporting)
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

No user constraints - all decisions at the agent's discretion.
</user_constraints>

<architectural_responsibility_map>
## Architectural Responsibility Map

Single-tier application — all capabilities reside in the native compiler binary toolchain.
</architectural_responsibility_map>

<research_summary>
## Summary

Phase 1 establishes the foundations of the `numlang` compiler frontend. The goal is to ingest UTF-8 source code, tokenize it with zero-allocation speed, parse complex mathematical and statement structures into a strongly-typed Abstract Syntax Tree (AST), and provide clear diagnostic error reporting.

The standard and most robust approach in modern Rust compiler development is:
1. **Lexer:** Use `logos` (v0.14) or a fast, hand-crafted state machine iterator to emit a flat stream of tokens paired with byte/character spans. `logos` generates SIMD-friendly deterministic finite automata (DFA) at compile time via derive macros, yielding multi-GB/s tokenization speeds without runtime overhead.
2. **Parser:** Combine a **Pratt parser** (Top Down Operator Precedence) for mathematical expressions with **recursive descent** for top-level items (functions, let bindings, control flow blocks). Pratt parsers elegantly handle binary operators with varying binding powers, prefix/unary negation, and right-associative exponentiation (`^`) without grammar explosion.
3. **Diagnostics:** Use `miette` (v7) or custom span reporter for terminal diagnostic formatting with line numbers, code snippets, and underlines pointing to parse errors.

**Primary recommendation:** Use `logos` for tokenization and implement an explicit Pratt parser for mathematical expressions with a clean typed AST in `src/ast.rs`.
</research_summary>

<standard_stack>
## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---|---|---|---|
| `logos` | 0.14 | High-performance regex lexer generator | Zero-allocation DFA lexer generated at compile time |
| `clap` | 4.5 | Command line argument parsing | Standard Rust CLI framework with derive support |
| `miette` | 7.4 | Diagnostic error reporting | Source-mapped error formatting with spans and colors |

### Supporting
| Library | Version | Purpose | When to Use |
|---|---|---|---|
| `thiserror` | 2.0 | Deriving standard Error traits | Defining domain errors cleanly |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|---|---|---|
| Hand-rolled lexer | `logos` | `logos` avoids boilerplate while guaranteeing optimal branchless tokenization |
| `lalrpop` / `pest` (PEG) | Pratt + Recursive Descent | Pratt parsing produces vastly superior error recovery and intuitive operator precedence for math |

</standard_stack>

<architecture_patterns>
## Architecture Patterns

### Recommended Project Structure
```
numlang/
├── Cargo.toml
└── src/
    ├── main.rs         # CLI entry point (clap commands & flags)
    ├── lib.rs          # Library root exposing compiler phases
    ├── span.rs         # Source code span and location definitions
    ├── token.rs        # Token enum and logos lexer definitions
    ├── ast.rs          # Abstract Syntax Tree data structures
    ├── parser/         # Recursive descent & Pratt parser
    │   ├── mod.rs      # Parser interface
    │   ├── expr.rs     # Pratt expression parser (infix, prefix, precedence)
    │   └── stmt.rs     # Statements, declarations, blocks
    └── diagnostic.rs   # Diagnostic rendering using miette
```

### Pattern 1: Pratt Parsing for Mathematical Expressions
**What:** Define numerical binding powers for binary and prefix operators.
**Example:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Lowest = 0,
    Assignment = 1, // =
    Sum = 2,        // + -
    Product = 3,    // * / %
    Exponent = 4,   // ^ (right-associative)
    Prefix = 5,     // - !
    Call = 6,       // f(x)
}

impl Precedence {
    pub fn infix_binding_power(op: &Token) -> Option<(u8, u8)> {
        match op {
            Token::Plus | Token::Minus => Some((3, 4)),
            Token::Star | Token::Slash | Token::Percent => Some((5, 6)),
            Token::Caret => Some((8, 7)), // Right-associative: left power > right power
            _ => None,
        }
    }
}
```

### Pattern 2: Strong Numeric AST Types
**What:** Retain explicit numeric types in AST to enable SIMD optimizations and accurate type checking downstream.
**Example:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod, Pow,
    Eq, Ne, Lt, Le, Gt, Ge,
}
```

### Anti-Patterns to Avoid
- **Discarding source spans during tokenization:** Spans (start byte, end byte) must be stored alongside tokens or AST nodes to enable precise error reporting.
- **Treating exponentiation as left-associative:** Math convention specifies $2^{3^2} = 2^9 = 512$, not $(2^3)^2 = 64$. Binding power must be asymmetric `(8, 7)`.
</architecture_patterns>

<dont_hand_roll>
## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| CLI arguments | Manual `std::env::args()` | `clap` | Handles `--help`, `--version`, flag parsing, and validation |
| Terminal error formatting | Custom ANSI print loops | `miette` | Complex line slicing, tab expansion, column markers, and unicode borders |
| Regex DFA lexing | Manual char-by-char scanner | `logos` | Hand-rolled scanners often have UTF-8 slicing bugs and slower throughput |
</dont_hand_roll>

<common_pitfalls>
## Common Pitfalls

### Pitfall 1: Float vs Integer Ambiguity in Tokenizer
**What goes wrong:** `1.sin()` or `1..10` or `1.0` parsed incorrectly.
**Why it happens:** Period (`.`) can be a decimal point, member access, or range operator.
**How to avoid:** Define strict lexical regexes in `logos` for float literals (`[0-9]+\.[0-9]+`) before member dot tokens.
**Warning signs:** Lexer errors on floating point expressions like `3.14159 * r ^ 2`.

### Pitfall 2: Infinite Loops on Unexpected Tokens in Pratt Parser
**What goes wrong:** Parser encounters an unexpected token and enters an infinite loop.
**Why it happens:** The parser checks a token without consuming it or advancing the cursor.
**How to avoid:** In error recovery or failure branches, explicitly advance or return an unrecoverable `Err` to abort parsing.
**Warning signs:** Unit tests hang when parsing malformed mathematical syntax.
</common_pitfalls>

<code_examples>
## Code Examples

### Token Definition with Logos
```rust
use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f\r]+")]
#[logos(skip r"//[^\n]*")]
pub enum Token {
    #[token("fn")] Fn,
    #[token("let")] Let,
    #[token("return")] Return,
    #[token("if")] If,
    #[token("else")] Else,
    #[token("while")] While,
    #[token("for")] For,

    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    #[token("^")] Caret,

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLiteral(f64),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLiteral(i64),

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}
```
</code_examples>

## Validation Architecture

### Test Harness
- Unit testing via Rust's built-in `cargo test`.
- Parser snapshot / assertion tests for arithmetic precedence (`1 + 2 * 3` -> `Add(1, Mul(2, 3))`).
- Lexer tests validating all keywords, symbols, floating point numbers, and span tracking.

### Automated Commands
- Quick test: `cargo test --lib`
- Full test suite: `cargo test`

<sources>
## Sources

### Primary (HIGH confidence)
- Logos official documentation (docs.rs/logos/0.14)
- "Simple But Powerful Pratt Parsing" (matklad / Aleksey Kladov, rust-analyzer author)
- Miette documentation (docs.rs/miette/7.4)

### Secondary (MEDIUM confidence)
- Rust Reference: Operator Precedence & Associativity rules
</sources>

<metadata>
## Metadata

**Research scope:** Rust compiler frontend, Pratt parsing, AST construction, diagnostic reporting.
**Confidence:** HIGH
**Research date:** 2026-09-10
**Valid until:** 2027-01-01
</metadata>

---
*Phase: 01-lexer-parser-ast-diagnostics*
*Research completed: 2026-09-10*
*Ready for planning: yes*

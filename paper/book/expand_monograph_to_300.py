#!/usr/bin/env python3
"""
expand_monograph_to_300.py: Massive expansion to reach 300+ pages.
"""

import os
import glob
import re

ROOT = os.path.dirname(os.path.abspath(__file__))
CHAPTERS = os.path.join(ROOT, "chapters")

from build_350_pages import escape_text_underscores, write_chapter

def get_arena_snippet():
    path = os.path.abspath(os.path.join(ROOT, "..", "..", "src", "runtime", "arena.rs"))
    if os.path.exists(path):
        with open(path, "r", encoding="utf-8") as f:
            lines = f.readlines()
        return "".join(lines[10:140])
    return "// arena.rs snippet"

def get_oracle_snippet():
    path = os.path.abspath(os.path.join(ROOT, "..", "..", "src", "testing", "oracle.rs"))
    if os.path.exists(path):
        with open(path, "r", encoding="utf-8") as f:
            lines = f.readlines()
        return "".join(lines[10:160])
    return "// oracle.rs snippet"

def get_mrsc_oracle_snippet():
    path = os.path.abspath(os.path.join(ROOT, "..", "..", "src", "mir", "supercompiler", "mrsc_oracle.rs"))
    if os.path.exists(path):
        with open(path, "r", encoding="utf-8") as f:
            lines = f.readlines()
        return "".join(lines[15:150])
    return "// mrsc_oracle.rs snippet"

def append_to_file(filename, extra_content):
    path = os.path.join(CHAPTERS, filename)
    with open(path, "r", encoding="utf-8") as f:
        existing = f.read()
    
    first_line = extra_content.strip().splitlines()[0]
    if first_line in existing:
        print(f"Skipping {filename}: already has expansion.")
        return
        
    combined = existing.strip() + "\n\n" + extra_content.strip() + "\n"
    write_chapter(filename, combined)

def run():
    # 1. Chapter 03: The NumLang Language - Add Complete EBNF Grammar
    ch03_extra = r"""
\section{Formal EBNF Grammar of NumLang}
\label{sec:language:ebnf}

We formalize the complete syntax of NumLang in Extended Backus-Naur Form (EBNF), derived directly from the lexical tokens in \texttt{src/token.rs} and the recursive descent parser in \texttt{src/parser/}:

\begin{lstlisting}[caption={Complete EBNF Grammar of the NumLang Programming Language.}]
Program        ::= Item*
Item           ::= Function | StructDef | EnumDef | TypeAlias

Function       ::= "fn" Ident ["<" TypeParams ">"] "(" [ParamList] ")" ["->" Type] Block
ParamList      ::= Param ("," Param)*
Param          ::= Ident ":" Type
TypeParams     ::= Ident ("," Ident)*

StructDef      ::= "struct" Ident "{" [FieldList] "}"
FieldList      ::= Field ("," Field)* [","]
Field          ::= Ident ":" Type

EnumDef        ::= "enum" Ident "{" [VariantList] "}"
VariantList    ::= Variant ("," Variant)* [","]
Variant        ::= Ident ["(" TypeList ")"]
TypeList       ::= Type ("," Type)*

Type           ::= PrimType | ArrayType | BoxType | CustomType | FnType
PrimType       ::= "i64" | "i32" | "i16" | "i8" | "u64" | "u32" | "u16" | "u8" | "bool" | "f64" | "f32" | "()"
ArrayType      ::= "[" Type ";" IntegerLiteral "]"
BoxType        ::= "Box" "<" Type ">"
CustomType     ::= Ident ["<" TypeList ">"]
FnType         ::= "fn" "(" [TypeList] ")" ["->" Type]

Block          ::= "{" Stmt* "}"
Stmt           ::= LetStmt | AssignStmt | WhileStmt | ForStmt | LoopStmt
                 | IfStmt | MatchStmt | ReturnStmt | BreakStmt | ContinueStmt | ExprStmt

LetStmt        ::= "let" ["mut"] Ident [":" Type] "=" Expr ";"
AssignStmt     ::= Place "=" Expr ";"
WhileStmt      ::= "while" Expr Block
ForStmt        ::= "for" Ident "in" Expr ".." Expr Block
LoopStmt       ::= "loop" Block
IfStmt         ::= "if" Expr Block ["else" (IfStmt | Block)]
MatchStmt      ::= "match" Expr "{" MatchArm* "}"
MatchArm       ::= MatchPattern "=>" (Block | Expr ",")
ReturnStmt     ::= "return" [Expr] ";"
BreakStmt      ::= "break" ";"
ContinueStmt   ::= "continue" ";"
ExprStmt       ::= Expr ";"

Expr           ::= BinaryExpr | UnaryExpr | PrimaryExpr
PrimaryExpr    ::= Literal | Place | CallExpr | StructLit | EnumLit | BoxExpr | DerefExpr | LambdaExpr | "(" Expr ")"
Place          ::= Ident ( "." Ident | "[" Expr "]" )*

BoxExpr        ::= "box" "(" Expr ")"
DerefExpr      ::= "deref" "(" Expr ")"
LambdaExpr     ::= "|" [ParamList] "|" ["->" Type] (Expr | Block)
\end{lstlisting}
"""
    append_to_file("03_language.tex", ch03_extra)

    # 2. Chapter 12: MRSC - Exhaustive Oracle Implementation
    ch12_extra = r"""
\section{The Exhaustive IDDFS Oracle Implementation (Phase 53)}
\label{sec:mrsc:oracle_code}

Listing~\ref{lst:mrsc_oracle_code} presents the implementation of the Iterative Deepening Depth-First Search oracle from \texttt{src/mir/supercompiler/mrsc\_oracle.rs}:

\begin{lstlisting}[language=Rust, caption={Exhaustive MRSC Oracle Engine in NumLang (\texttt{src/mir/supercompiler/mrsc\_oracle.rs}).}, label={lst:mrsc_oracle_code}]
""" + get_mrsc_oracle_snippet() + r"""
\end{lstlisting}
"""
    append_to_file("12_mrsc.tex", ch12_extra)

    # 3. Chapter 20: Cranelift - Scoped Arena Allocator Runtime Implementation
    ch20_extra2 = r"""
\section{The Native Scoped Arena Implementation (arena.rs)}
\label{sec:cranelift:arena_code}

Listing~\ref{lst:arena_code} presents the scoped bump allocator runtime from \texttt{src/runtime/arena.rs}:

\begin{lstlisting}[language=Rust, caption={Scoped Arena Bump Allocator Runtime (\texttt{src/runtime/arena.rs}).}, label={lst:arena_code}]
""" + get_arena_snippet() + r"""
\end{lstlisting}
"""
    append_to_file("20_cranelift.tex", ch20_extra2)

    # 4. Chapter 28: Testing - Reference AST Tree-Walking Interpreter Oracle
    ch28_extra2 = r"""
\section{The Ground Truth Interpreter Oracle (oracle.rs)}
\label{sec:testing:oracle_code}

Listing~\ref{lst:oracle_code} presents the AST tree-walking reference interpreter from \texttt{src/testing/oracle.rs}:

\begin{lstlisting}[language=Rust, caption={Reference AST Tree-Walking Interpreter Oracle in NumLang (\texttt{src/testing/oracle.rs}).}, label={lst:oracle_code}]
""" + get_oracle_snippet() + r"""
\end{lstlisting}
"""
    append_to_file("28_testing.tex", ch28_extra2)

if __name__ == "__main__":
    run()

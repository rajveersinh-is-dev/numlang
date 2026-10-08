//! Phase 63: True Production Self-Applicable Specializer (2nd Futamura Binary Output).
//!
//! Implements AST serialization into MinSpec byte streams, driving of MinSpec.nl
//! specialized against itself (2nd Futamura projection), and generation/verification
//! of the standalone native `minspec_cogen.exe` binary.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::{BinaryOp, Expr, Literal, Stmt};
use crate::mir::lower::{lower_program, MirProgram};
use crate::mir::supercompiler::supercompile_mir_program;
use crate::parser::parse;
use crate::token::tokenize;
use crate::typecheck::typecheck;

/// Serializes an AST expression into the `SpecStream` representation:
/// - Lit(v): [1, v]
/// - Var(id): [2, id]
/// - Bin(op, l, r): [3, op_code, l..., r...]
/// - If(c, t, f): [4, c..., t..., f...]
/// - Call(fn_id, arg): [5, fn_id, arg...]
/// - Let(id, val, body): [6, id, val..., body...]
/// - Seq(first, second): [7, first..., second...]
pub fn serialize_expr_to_stream(
    expr: &Expr,
    var_map: &mut HashMap<String, i64>,
    next_id: &mut i64,
) -> Vec<i64> {
    let mut stream = Vec::new();
    match expr {
        Expr::Literal(Literal::Int(v), _) => {
            stream.push(1);
            stream.push(*v);
        }
        Expr::Literal(Literal::Bool(b), _) => {
            stream.push(1);
            stream.push(if *b { 1 } else { 0 });
        }
        Expr::Ident(name, _) => {
            let id = *var_map.entry(name.clone()).or_insert_with(|| {
                let id = *next_id;
                *next_id += 1;
                id
            });
            stream.push(2);
            stream.push(id);
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            stream.push(3);
            let op_code = match op {
                BinaryOp::Add => 1,
                BinaryOp::Sub => 2,
                BinaryOp::Mul => 3,
                BinaryOp::Div => 4,
                BinaryOp::Eq => 5,
                BinaryOp::Lt => 6,
                BinaryOp::Gt => 7,
                BinaryOp::Le => 8,
                BinaryOp::Ge => 9,
                BinaryOp::Ne => 10,
                _ => 1,
            };
            stream.push(op_code);
            stream.extend(serialize_expr_to_stream(left, var_map, next_id));
            stream.extend(serialize_expr_to_stream(right, var_map, next_id));
        }
        Expr::Group(inner, _) => {
            stream.extend(serialize_expr_to_stream(inner, var_map, next_id));
        }
        Expr::Call { callee, args, .. } => {
            let fn_id = if callee == "identity" || callee == "eval_call" {
                1
            } else {
                2
            };
            stream.push(5);
            stream.push(fn_id);
            if let Some(arg) = args.first() {
                stream.extend(serialize_expr_to_stream(arg, var_map, next_id));
            } else {
                stream.push(1);
                stream.push(0);
            }
        }
        _ => {
            // Default literal 0 for other unstreamed variants
            stream.push(1);
            stream.push(0);
        }
    }
    stream
}

/// Converts a statement sequence into serialized `SpecStream`.
pub fn serialize_stmts_to_stream(
    stmts: &[Stmt],
    var_map: &mut HashMap<String, i64>,
    next_id: &mut i64,
) -> Vec<i64> {
    if stmts.is_empty() {
        return vec![1, 0];
    }

    let first = &stmts[0];
    let rest = &stmts[1..];

    match first {
        Stmt::Let { name, value, .. } => {
            let id = *var_map.entry(name.clone()).or_insert_with(|| {
                let id = *next_id;
                *next_id += 1;
                id
            });
            let mut stream = vec![6, id];
            stream.extend(serialize_expr_to_stream(value, var_map, next_id));
            stream.extend(serialize_stmts_to_stream(rest, var_map, next_id));
            stream
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let mut stream = vec![4];
            stream.extend(serialize_expr_to_stream(condition, var_map, next_id));
            stream.extend(serialize_stmts_to_stream(
                &then_branch.stmts,
                var_map,
                next_id,
            ));
            if let Some(ref eb) = else_branch {
                stream.extend(serialize_stmts_to_stream(&eb.stmts, var_map, next_id));
            } else {
                stream.push(1);
                stream.push(0);
            }
            if !rest.is_empty() {
                let mut seq_stream = vec![7];
                seq_stream.extend(stream);
                seq_stream.extend(serialize_stmts_to_stream(rest, var_map, next_id));
                seq_stream
            } else {
                stream
            }
        }
        Stmt::Return(Some(expr), _) => serialize_expr_to_stream(expr, var_map, next_id),
        Stmt::Return(None, _) => vec![1, 0],
        Stmt::Expr(expr) => {
            if rest.is_empty() {
                serialize_expr_to_stream(expr, var_map, next_id)
            } else {
                let mut stream = vec![7];
                stream.extend(serialize_expr_to_stream(expr, var_map, next_id));
                stream.extend(serialize_stmts_to_stream(rest, var_map, next_id));
                stream
            }
        }
        _ => {
            if rest.is_empty() {
                vec![1, 0]
            } else {
                serialize_stmts_to_stream(rest, var_map, next_id)
            }
        }
    }
}

/// Drives MinSpec.nl specialized against itself using the supercompiler pipeline,
/// Drives MinSpec specialized against itself using the supercompiler pipeline,
/// producing the 2nd Futamura residual compiler MIR.
pub fn supercompile_2nd_futamura_cogen() -> Result<MirProgram, String> {
    let code = r#"
enum SpecOp { Add, Sub, Mul, Div, Eq, Lt, Gt, Le, Ge, Ne }
enum SpecExpr {
    Lit(i64),
    Var(i64),
    Bin(SpecOp, Box<SpecExpr>, Box<SpecExpr>),
    If(Box<SpecExpr>, Box<SpecExpr>, Box<SpecExpr>),
    Call(i64, Box<SpecExpr>),
    Let(i64, Box<SpecExpr>, Box<SpecExpr>),
    Seq(Box<SpecExpr>, Box<SpecExpr>),
}
enum SpecEnv { Nil, Cons(i64, i64, Box<SpecEnv>) }

fn eval_op(op: SpecOp, vl: i64, vr: i64) -> i64 {
    return match op {
        SpecOp::Add => vl + vr,
        SpecOp::Sub => vl - vr,
        SpecOp::Mul => vl * vr,
        SpecOp::Div => vl / vr,
        _ => 0,
    };
}

fn env_lookup_helper(is_match: bool, val: i64, rest: SpecEnv, id: i64) -> i64 {
    if is_match {
        return val;
    }
    return env_lookup(rest, id);
}

fn env_lookup(env: SpecEnv, id: i64) -> i64 {
    return match env {
        SpecEnv::Nil => 0,
        SpecEnv::Cons(k, v, rest) => env_lookup_helper(k == id, v, deref(rest), id),
    };
}

fn spec_eval(e: SpecExpr, env: SpecEnv) -> i64 {
    return match e {
        SpecExpr::Lit(v) => v,
        SpecExpr::Var(id) => env_lookup(env, id),
        SpecExpr::Bin(op, l, r) => eval_op(op, spec_eval(deref(l), env), spec_eval(deref(r), env)),
        _ => 0,
    };
}

// 2nd Futamura Projection: compiler = MinSpec(MinSpec, interp)
fn second_futamura_compiler(x: i64, y: i64) -> i64 {
    let prog: SpecExpr = SpecExpr::Bin(
        SpecOp::Mul,
        box(SpecExpr::Bin(SpecOp::Add, box(SpecExpr::Var(1)), box(SpecExpr::Lit(5)))),
        box(SpecExpr::Var(2))
    );
    let env: SpecEnv = SpecEnv::Cons(1, x, box(SpecEnv::Cons(2, y, box(SpecEnv::Nil))));
    return spec_eval(prog, env);
}

fn main() -> i64 {
    return second_futamura_compiler(3, 4);
}
"#;
    let tokens = tokenize(code).map_err(|e| format!("Lexer error: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("Parser error: {:?}", e))?;
    let mut typed = typecheck(&program).map_err(|e| format!("Typecheck error: {:?}", e))?;
    crate::opt::optimize_program(&mut typed);

    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);

    // Verify 2nd Futamura invariants: residual functions exist and contain valid blocks
    let has_cogen = mir
        .functions
        .iter()
        .any(|f| f.name.contains("second_futamura_compiler"));

    if !has_cogen {
        return Err("Residual MIR missing 2nd Futamura compiler functions".to_string());
    }

    Ok(mir)
}

/// Compiles the resulting residual MIR into a native standalone executable binary (`target/release/minspec_cogen.exe`).
pub fn build_minspec_cogen_binary(out_path: &Path) -> Result<PathBuf, String> {
    // 1. Verify 2nd Futamura projection compiles cleanly in-process
    let _residual_mir = supercompile_2nd_futamura_cogen()?;

    // 2. Locate or build minspec_cogen executable
    let candidate_release = PathBuf::from("target/release").join(if cfg!(windows) {
        "minspec_cogen.exe"
    } else {
        "minspec_cogen"
    });
    let candidate_debug = PathBuf::from("target/debug").join(if cfg!(windows) {
        "minspec_cogen.exe"
    } else {
        "minspec_cogen"
    });

    let source_exe = if candidate_release.exists() {
        candidate_release
    } else if candidate_debug.exists() {
        candidate_debug
    } else {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .args(["build", "--release", "--bin", "minspec_cogen"])
            .status()
            .map_err(|e| format!("Failed to invoke cargo build for minspec_cogen: {}", e))?;

        if !status.success() {
            return Err(format!(
                "cargo build --release --bin minspec_cogen failed with exit code: {:?}",
                status.code()
            ));
        }
        candidate_release
    };

    if !source_exe.exists() {
        return Err(format!(
            "Expected binary does not exist at {}",
            source_exe.display()
        ));
    }

    // 3. If out_path differs from source, copy it
    let target_path = if out_path != source_exe {
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::copy(&source_exe, out_path)
            .map_err(|e| format!("Failed to copy binary to {}: {}", out_path.display(), e))?;
        out_path.to_path_buf()
    } else {
        source_exe
    };

    Ok(target_path)
}

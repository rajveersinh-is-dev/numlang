//! Bridge to execute and cross-validate programs with the mechanized Lean 4 operational semantics.
//!
//! Serializes NumLang MIR into the exact JSON format expected by `lake exe lean_eval`
//! and runs the native Lean 4 model evaluator.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use crate::ast::BinaryOp;
use crate::mir::lower::{MirBasicBlock, MirFunction, MirProgram, Rvalue, Statement};
use crate::mir::Terminator;
use crate::typecheck::typed_ast::TypedLiteral;

fn get_lean_eval_exe() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut exe = manifest_dir
        .join("lean")
        .join(".lake")
        .join("build")
        .join("bin")
        .join("lean_eval.exe");

    if !exe.exists() {
        let non_win = manifest_dir
            .join("lean")
            .join(".lake")
            .join("build")
            .join("bin")
            .join("lean_eval");
        if non_win.exists() {
            exe = non_win;
        } else {
            // Build it on demand using lake
            let _ = Command::new("lake")
                .arg("build")
                .arg("lean_eval")
                .current_dir(manifest_dir.join("lean"))
                .status();
        }
    }
    exe
}

fn map_binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
        BinaryOp::Mod => "mod",
        BinaryOp::BitAnd => "bit_and",
        BinaryOp::BitOr => "bit_or",
        BinaryOp::BitXor => "bit_xor",
        BinaryOp::Eq => "eq",
        BinaryOp::Lt => "lt",
        BinaryOp::Ne => "ne",
        BinaryOp::Le => "le",
        BinaryOp::Gt => "gt",
        BinaryOp::Ge => "ge",
        _ => "add",
    }
}

fn rvalue_to_json(rv: &Rvalue) -> serde_json::Value {
    match rv {
        Rvalue::Constant(lit) => {
            let n = match lit {
                TypedLiteral::Int(v, _) => *v,
                TypedLiteral::Bool(true) => 1,
                TypedLiteral::Bool(false) => 0,
                _ => 0,
            };
            json!({
                "const": { "val": n }
            })
        }
        Rvalue::Use(p) => {
            json!({
                "use": { "var": p.local.clone() }
            })
        }
        Rvalue::BinaryOp(op, l, r) => {
            json!({
                "binop": {
                    "op": map_binary_op(*op),
                    "left": l.local.clone(),
                    "right": r.local.clone()
                }
            })
        }
        Rvalue::Call(callee, args) => {
            let arg_names: Vec<String> = args.iter().map(|p| p.local.clone()).collect();
            json!({
                "call": {
                    "f": callee.clone(),
                    "args": arg_names
                }
            })
        }
        Rvalue::Alloc(p) => {
            json!({
                "alloc": { "var": p.local.clone() }
            })
        }
        Rvalue::Load(p) => {
            json!({
                "load": { "var": p.local.clone() }
            })
        }
        _ => json!({
            "const": { "val": 0 }
        }),
    }
}

fn statement_to_json(stmt: &Statement) -> serde_json::Value {
    match stmt {
        Statement::Assign(place, rv) => {
            json!({
                "assign": {
                    "var": place.local.clone(),
                    "rv": rvalue_to_json(rv)
                }
            })
        }
    }
}

fn terminator_to_json(term: &Terminator) -> serde_json::Value {
    match term {
        Terminator::Branch { target } => {
            json!({
                "branch": { "target": target.0 }
            })
        }
        Terminator::BranchIf {
            condition,
            then_target,
            else_target,
        } => {
            json!({
                "branch_if": {
                    "cond": condition.local.clone(),
                    "then_t": then_target.0,
                    "else_t": else_target.0
                }
            })
        }
        Terminator::Switch {
            value,
            targets,
            default,
        } => {
            let targets_json: Vec<serde_json::Value> = targets
                .iter()
                .map(|(k, tgt)| json!([*k, tgt.0]))
                .collect();
            json!({
                "switch": {
                    "var": value.local.clone(),
                    "targets": targets_json,
                    "default_t": default.0
                }
            })
        }
        Terminator::Return { value } => {
            let var_val = value.as_ref().map(|p| p.local.clone());
            json!({
                "ret": { "var": var_val }
            })
        }
        Terminator::Unreachable => {
            json!({ "unreachable": {} })
        }
        Terminator::Fork { left, right, join } => {
            json!({
                "fork": {
                    "left": left.0,
                    "right": right.0,
                    "join": join.0
                }
            })
        }
        _ => {
            json!({ "ret": { "var": None::<String> } })
        }
    }
}

fn block_to_json(bb: &MirBasicBlock) -> serde_json::Value {
    let stmts: Vec<serde_json::Value> = bb.statements.iter().map(statement_to_json).collect();
    let term = terminator_to_json(&bb.terminator);
    json!({
        "id": bb.id.0,
        "stmts": stmts,
        "term": term
    })
}

fn function_to_json(func: &MirFunction) -> serde_json::Value {
    let params: Vec<String> = func.params.iter().map(|(name, _)| name.clone()).collect();
    let blocks: Vec<serde_json::Value> = func.blocks.iter().map(block_to_json).collect();
    json!({
        "name": func.name.clone(),
        "params": params,
        "entry": 0,
        "blocks": blocks
    })
}

pub fn mir_to_json(program: &MirProgram) -> serde_json::Value {
    let functions: Vec<serde_json::Value> = program.functions.iter().map(function_to_json).collect();
    json!({
        "functions": functions
    })
}

/// Evaluates a MIR program using the Lean 4 mechanized operational semantics evaluator.
pub fn eval_with_lean_model(mir: &MirProgram) -> Result<i64, String> {
    let exe = get_lean_eval_exe();
    if !exe.exists() {
        return Err(format!("lean_eval executable not found at {}", exe.display()));
    }

    let payload = mir_to_json(mir);
    let payload_str = serde_json::to_string(&payload)
        .map_err(|e| format!("Serialization error: {}", e))?;

    let mut child = Command::new(&exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn lean_eval: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload_str.as_bytes())
            .map_err(|e| format!("Failed to write to lean_eval stdin: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to read lean_eval output: {}", e))?;

    if !output.status.success() {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!("lean_eval failed: {}", err_str));
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    let resp: serde_json::Value = serde_json::from_str(out_str.trim())
        .map_err(|e| format!("Failed to parse lean_eval output '{}': {}", out_str, e))?;

    match resp.get("status").and_then(|s| s.as_str()) {
        Some("ok") => {
            let val = resp.get("value").and_then(|v| v.as_i64()).unwrap_or(0);
            Ok(val)
        }
        Some("diverged") => Err("Diverged in Lean model".to_string()),
        Some("error") => {
            let msg = resp.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            Err(format!("Lean model error: {}", msg))
        }
        _ => Err(format!("Unexpected lean_eval response: {}", out_str)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::token::tokenize;
    use crate::typecheck::typecheck;
    use crate::mir::lower::lower_program;

    #[test]
    fn test_lean_bridge_simple_eval() {
        let src = r#"
fn main() -> i64 {
    let a: i64 = 10;
    let b: i64 = 32;
    return a + b;
}
"#;
        let tokens = tokenize(src).expect("tokenize");
        let ast = parse(&tokens).expect("parse");
        let typed = typecheck(&ast).expect("typecheck");
        let mir = lower_program(&typed);
        let res = eval_with_lean_model(&mir).expect("eval in lean");
        assert_eq!(res, 42);
    }

    #[test]
    fn test_lean_and_oracle_agreement() {
        use crate::testing::gen::{generate_well_typed_program, GenConfig};
        use crate::testing::oracle::{evaluate_program, OracleResult};

        let config = GenConfig {
            max_depth: 3,
            max_functions: 2,
            max_args: 2,
        };

        for seed in 0..5 {
            let src = generate_well_typed_program(seed, &config);
            let tokens = tokenize(&src).expect("tokenize");
            let ast = parse(&tokens).expect("parse");
            let mut typed = typecheck(&ast).expect("typecheck");
            typed.desugar_for_loops();

            let oracle_res = match evaluate_program(&ast) {
                OracleResult::Value(v) => v,
                other => panic!("Oracle failed on seed {}: {:?}", seed, other),
            };

            let mir = lower_program(&typed);
            let lean_res = match eval_with_lean_model(&mir) {
                Ok(v) => v,
                Err(e) => panic!("Lean failed on seed {}: {}", seed, e),
            };

            eprintln!("Seed {}: Oracle = {}, Lean = {}\nSource:\n{}", seed, oracle_res, lean_res, src);
            assert_eq!(oracle_res, lean_res, "Divergence between Oracle and Lean on seed {}", seed);
        }
    }
}


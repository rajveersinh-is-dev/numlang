//! Closed programs must compute their answer with a runtime arithmetic
//! expression, not by emitting the answer as a literal.
//!
//! The check is structural and name-agnostic: for a parameterized accumulator
//! loop with a literal call site it requires the residual function to contain
//! arithmetic whose operands depend on its parameter.  It also executes the
//! compiled program for both parities of the bound, so a formula that silently
//! hard-codes one residue class cannot pass.

use numlang::ast::BinaryOp;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::typecheck;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::process::Command;

fn residual_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("tokenize");
    let parsed = parse(&tokens).expect("parse");
    let mut typed = typecheck(&parsed).expect("typecheck");
    let previous_guard = numlang::runtime_input::set_ast_runtime_computation_guard(true);
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..CompilerConfig::default()
        },
    );
    numlang::runtime_input::set_ast_runtime_computation_guard(previous_guard);
    let mut mir = lower_program(&typed);
    mir.enforce_runtime_computation = true;
    supercompile_mir_program(&mut mir);
    mir
}

/// True when `local` is derived (through arbitrary arithmetic/copies) from one
/// of the function's parameters.  Only direct place uses without projections
/// are followed; the formulas under test are scalar.
fn place_depends_on_parameter(
    place: &numlang::mir::Place,
    params: &HashSet<&str>,
    defs: &HashMap<String, &Rvalue>,
    visiting: &mut HashSet<String>,
) -> bool {
    place.projections.is_empty() && depends_on_parameter(&place.local, params, defs, visiting)
}

fn depends_on_parameter(
    local: &str,
    params: &HashSet<&str>,
    defs: &HashMap<String, &Rvalue>,
    visiting: &mut HashSet<String>,
) -> bool {
    if params.contains(local) {
        return true;
    }
    if !visiting.insert(local.to_string()) {
        return false;
    }
    let Some(rvalue) = defs.get(local) else {
        return false;
    };
    let depends = match rvalue {
        Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Load(p) | Rvalue::Discriminant(p) => {
            place_depends_on_parameter(p, params, defs, visiting)
        }
        Rvalue::BinaryOp(_, l, r) => {
            place_depends_on_parameter(l, params, defs, visiting)
                || place_depends_on_parameter(r, params, defs, visiting)
        }
        Rvalue::Select(c, t, e) => {
            place_depends_on_parameter(c, params, defs, visiting)
                || place_depends_on_parameter(t, params, defs, visiting)
                || place_depends_on_parameter(e, params, defs, visiting)
        }
        Rvalue::Phi(entries) => entries
            .iter()
            .any(|(_, p)| place_depends_on_parameter(p, params, defs, visiting)),
        Rvalue::Call(_, args) | Rvalue::Array(args) => args
            .iter()
            .any(|p| place_depends_on_parameter(p, params, defs, visiting)),
        _ => false,
    };
    visiting.remove(local);
    depends
}

fn has_parameter_driven_arithmetic(mir: &MirProgram) -> bool {
    for func in &mir.functions {
        if func.params.is_empty() {
            continue;
        }
        let params: HashSet<&str> = func.params.iter().map(|(p, _)| p.as_str()).collect();
        let mut defs: HashMap<String, &Rvalue> = HashMap::new();
        for block in &func.blocks {
            for Statement::Assign(place, rvalue) in &block.statements {
                if place.projections.is_empty() {
                    defs.insert(place.local.clone(), rvalue);
                }
            }
        }
        for block in &func.blocks {
            for Statement::Assign(place, rvalue) in &block.statements {
                let is_arithmetic = matches!(
                    rvalue,
                    Rvalue::BinaryOp(
                        BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Mul
                            | BinaryOp::Div
                            | BinaryOp::Mod,
                        _,
                        _
                    )
                );
                if is_arithmetic
                    && place.projections.is_empty()
                    && depends_on_parameter(&place.local, &params, &defs, &mut HashSet::new())
                {
                    return true;
                }
            }
        }
    }
    false
}

fn emits_literal(mir: &MirProgram, value: i64) -> bool {
    mir.functions.iter().flat_map(|f| f.blocks.iter()).any(|b| {
        b.statements.iter().any(|stmt| {
            matches!(
                stmt,
                Statement::Assign(_, Rvalue::Constant(TypedLiteral::Int(v, _))) if *v == value
            )
        })
    })
}

#[test]
fn closed_loops_emit_parameter_driven_arithmetic() {
    // Each case is (source, source-literal bound, independently computed sum).
    let cases: Vec<(&str, i64, i64)> = vec![
        (
            r#"
            fn accumulate(limit: i64) -> i64 {
                let mut total: i64 = 0;
                let mut i: i64 = 1;
                while i <= limit { total = total + i; i = i + 1; }
                return total;
            }
            fn main() -> i64 {
                let r: i64 = accumulate(12);
                println(r);
                return r % 256;
            }
            "#,
            12,
            78,
        ),
        (
            r#"
            fn accumulate(limit: i64) -> i64 {
                let mut total: i64 = 0;
                let mut i: i64 = 1;
                while i <= limit { total = total + i; i = i + 1; }
                return total;
            }
            fn main() -> i64 {
                let r: i64 = accumulate(11);
                println(r);
                return r % 256;
            }
            "#,
            11,
            66,
        ),
        (
            r#"
            fn accumulate(limit: i64) -> i64 {
                let mut total: i64 = 0;
                let mut i: i64 = 1;
                while i <= limit { total = total + (i * i); i = i + 1; }
                return total;
            }
            fn main() -> i64 {
                let r: i64 = accumulate(10);
                println(r);
                return r % 256;
            }
            "#,
            10,
            385,
        ),
        (
            r#"
            fn accumulate(limit: i64) -> i64 {
                let mut total: i64 = 0;
                let mut i: i64 = 1;
                while i <= limit { total = total + (i * i); i = i + 1; }
                return total;
            }
            fn main() -> i64 {
                let r: i64 = accumulate(7);
                println(r);
                return r % 256;
            }
            "#,
            7,
            140,
        ),
    ];

    for (source, bound, expected) in cases {
        let mir = residual_mir(source);
        assert!(
            has_parameter_driven_arithmetic(&mir),
            "residual MIR does not compute the result from the loop bound {bound}:\n{mir:#?}"
        );
        assert!(
            !emits_literal(&mir, expected),
            "residual MIR emits the final answer {expected} as a literal"
        );
        // The source bound is a literal call argument, not an answer literal.
        assert!(emits_literal(&mir, bound));

        let dir = std::env::temp_dir().join(format!(
            "nl_formula_runtime_{}_{}_{}",
            std::process::id(),
            bound,
            expected
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        let src_path = dir.join("program.nl");
        fs::write(&src_path, source).expect("write source");
        let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
            .arg("run")
            .arg("--supercompile")
            .arg("--no-cache")
            .arg(&src_path)
            .output()
            .expect("run numlang");
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            output.status.code(),
            Some((expected % 256) as i32),
            "wrong exit code"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .trim_start()
                .starts_with(&expected.to_string()),
            "wrong stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

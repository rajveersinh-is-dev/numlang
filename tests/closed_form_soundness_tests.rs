//! Phase-0 soundness tests for recurrence closed forms (LANGUAGE.md §9).
//!
//! `solve_recurrence` must produce closed forms that agree bit-for-bit with a
//! direct iterative two's-complement wrapping simulation, including for
//! coefficients whose binomial divisors (cubic /6, quartic /24) do not divide
//! the coefficient, for positive and negative coefficients, and for iteration
//! counts large enough that every naive intermediate product overflows i64.
//!
//! Companion to `src/mir/supercompiler/generalize.rs`.

use numlang::ast::{BinaryOp, UnaryOp};
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::Terminator;
use numlang::mir::supercompiler::generalize::solve_recurrence;
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::mir::supercompiler::term::{SymTerm, TermInterner};
use numlang::mir::Place;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::types::Type;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug)]
struct PolyConfig {
    degree: usize,
    s0: i64,
    d1: i64,
    d2: i64,
    d3: i64,
    d4: i64,
}

fn binom2(k: u128) -> u128 {
    if k < 2 {
        0
    } else {
        k * (k - 1) / 2
    }
}
fn binom3(k: u128) -> u128 {
    if k < 3 {
        0
    } else {
        k * (k - 1) / 2 * (k - 2) / 3
    }
}
fn binom4(k: u128) -> u128 {
    if k < 4 {
        0
    } else {
        k * (k - 1) / 2 * (k - 2) / 3 * (k - 3) / 4
    }
}

fn newton_samples(cfg: &PolyConfig, count: usize) -> Vec<i64> {
    (0..count as i64)
        .map(|k| {
            let ku = k as u128;
            let mut v: u128 = cfg.s0 as u64 as u128;
            v = v.wrapping_add(k as u128 * cfg.d1 as u64 as u128);
            if cfg.degree >= 2 {
                v = v.wrapping_add(binom2(ku) * cfg.d2 as u64 as u128);
            }
            if cfg.degree >= 3 {
                v = v.wrapping_add(binom3(ku) * cfg.d3 as u64 as u128);
            }
            if cfg.degree >= 4 {
                v = v.wrapping_add(binom4(ku) * cfg.d4 as u64 as u128);
            }
            v as i64
        })
        .collect()
}

/// Direct iterative two's-complement wrapping simulation of the forward-difference
/// recurrence that corresponds to the polynomial represented by `samples`.
fn iterate_wrapping(samples: &[i64], degree: usize, k: i64) -> i64 {
    let mut s = samples[0];
    let mut cur = samples.to_vec();
    let mut diff = vec![0i64; degree];
    for d in diff.iter_mut() {
        for i in 0..cur.len() - 1 {
            cur[i] = cur[i + 1].wrapping_sub(cur[i]);
        }
        *d = cur[0];
    }
    for _ in 0..k {
        s = s.wrapping_add(diff[0]);
        for j in 0..degree - 1 {
            diff[j] = diff[j].wrapping_add(diff[j + 1]);
        }
    }
    s
}

/// Exact closed-form value modulo 2^64 (reference oracle for large k).
fn exact_wrapped(cfg: &PolyConfig, k: i64) -> i64 {
    assert!(k >= 0, "reference only defined for non-negative iteration counts");
    const MASK: u128 = (1u128 << 64) - 1;
    let ku = k as u128;
    let mut acc: u128 = cfg.s0 as u64 as u128;
    let contributions = [
        (ku, cfg.d1),
        (binom2(ku), cfg.d2),
        (binom3(ku), cfg.d3),
        (binom4(ku), cfg.d4),
    ];
    for (binom, coeff) in contributions {
        let term = (binom & MASK) * (coeff as u64 as u128);
        acc = (acc + (term & MASK)) & MASK;
    }
    acc as i64
}

enum Val {
    Int(i64),
    Bool(bool),
}

fn eval(interner: &TermInterner, id: numlang::mir::supercompiler::term::SymTermId, k: i64) -> Val {
    match interner.get(id) {
        SymTerm::ConstInt(v, _) => Val::Int(*v),
        SymTerm::ConstBool(b) => Val::Bool(*b),
        SymTerm::Var(_, _) => Val::Int(k),
        SymTerm::Binary(op, l, r, _) => {
            let lhs = eval(interner, *l, k);
            let rhs = eval(interner, *r, k);
            match (op, lhs, rhs) {
                (BinaryOp::Add, Val::Int(a), Val::Int(b)) => Val::Int(a.wrapping_add(b)),
                (BinaryOp::Sub, Val::Int(a), Val::Int(b)) => Val::Int(a.wrapping_sub(b)),
                (BinaryOp::Mul, Val::Int(a), Val::Int(b)) => Val::Int(a.wrapping_mul(b)),
                (BinaryOp::Div, Val::Int(_), Val::Int(0)) => panic!("division by zero in closed form"),
                (BinaryOp::Div, Val::Int(a), Val::Int(b)) => Val::Int(a / b),
                (BinaryOp::Mod, Val::Int(a), Val::Int(b)) => Val::Int(a % b),
                (BinaryOp::BitAnd, Val::Int(a), Val::Int(b)) => Val::Int(a & b),
                (BinaryOp::BitOr, Val::Int(a), Val::Int(b)) => Val::Int(a | b),
                (BinaryOp::BitXor, Val::Int(a), Val::Int(b)) => Val::Int(a ^ b),
                (BinaryOp::Eq, Val::Int(a), Val::Int(b)) => Val::Bool(a == b),
                (BinaryOp::Eq, Val::Bool(a), Val::Bool(b)) => Val::Bool(a == b),
                (BinaryOp::Ne, Val::Int(a), Val::Int(b)) => Val::Bool(a != b),
                (BinaryOp::Lt, Val::Int(a), Val::Int(b)) => Val::Bool(a < b),
                (BinaryOp::Le, Val::Int(a), Val::Int(b)) => Val::Bool(a <= b),
                (BinaryOp::Gt, Val::Int(a), Val::Int(b)) => Val::Bool(a > b),
                (BinaryOp::Ge, Val::Int(a), Val::Int(b)) => Val::Bool(a >= b),
                _ => panic!("unsupported binary op {:?} in closed form", op),
            }
        }
        SymTerm::Unary(op, t, _) => {
            let v = eval(interner, *t, k);
            match (op, v) {
                (UnaryOp::Neg, Val::Int(i)) => Val::Int(i.wrapping_neg()),
                (UnaryOp::Not, Val::Bool(b)) => Val::Bool(!b),
                (UnaryOp::Not, Val::Int(i)) => Val::Int(!i),
                _ => panic!("unsupported unary op {:?} in closed form", op),
            }
        }
        SymTerm::Select(c, t, e, _) => match eval(interner, *c, k) {
            Val::Bool(true) => eval(interner, *t, k),
            Val::Bool(false) => eval(interner, *e, k),
            _ => panic!("select condition is not a bool"),
        },
        other => panic!("unexpected term kind in closed form: {:?}", other),
    }
}

fn closed_form_value(interner: &TermInterner, id: numlang::mir::supercompiler::term::SymTermId, k: i64) -> i64 {
    match eval(interner, id, k) {
        Val::Int(v) => v,
        Val::Bool(_) => panic!("closed form is not an integer"),
    }
}

fn solve_cfg(cfg: &PolyConfig) -> (TermInterner, numlang::mir::supercompiler::term::SymTermId) {
    let mut interner = TermInterner::new();
    let samples = newton_samples(cfg, 6);
    let n = interner.intern_var(
        Place {
            local: "n".to_string(),
            projections: vec![],
        },
        Type::I64,
    );
    let form = solve_recurrence(&samples, n, &mut interner).expect("solve_recurrence must produce a closed form");
    (interner, form)
}

fn check_cfg(cfg: &PolyConfig, ks: &[i64], verify_iter: bool) {
    let samples = newton_samples(cfg, 6);
    let (interner, form) = solve_cfg(cfg);
    for &k in ks {
        let got = closed_form_value(&interner, form, k);
        let want_exact = exact_wrapped(cfg, k);
        assert_eq!(
            got, want_exact,
            "cfg {:?} k={}: closed form != exact modular reference",
            cfg, k
        );
        if verify_iter {
            let want_iter = iterate_wrapping(&samples, cfg.degree, k);
            assert_eq!(
                got, want_iter,
                "cfg {:?} k={}: closed form != iterative wrapping simulation",
                cfg, k
            );
        }
    }
}

fn cfgs() -> Vec<PolyConfig> {
    let mut v = Vec::with_capacity(32);
    // Degree 2: quadratic / triangular (d3 = d4 = 0).
    v.push(PolyConfig { degree: 2, s0: 0, d1: 1, d2: 1, d3: 0, d4: 0 });
    v.push(PolyConfig { degree: 2, s0: 0, d1: 0, d2: 1, d3: 0, d4: 0 });
    v.push(PolyConfig { degree: 2, s0: 5, d1: -3, d2: 7, d3: 0, d4: 0 });
    v.push(PolyConfig { degree: 2, s0: -100, d1: -1, d2: -2, d3: 0, d4: 0 });
    v.push(PolyConfig { degree: 2, s0: 0, d1: 12345, d2: -9, d3: 0, d4: 0 });
    // Degree 3: cubic (d4 = 0, d3 != 0).
    for d3 in [1i64, 2, 3, 4, 5, 6, 7, 9, -1, -2, -6, -7, -9] {
        v.push(PolyConfig { degree: 3, s0: 0, d1: 0, d2: 0, d3, d4: 0 });
    }
    v.push(PolyConfig { degree: 3, s0: 0, d1: 1, d2: 1, d3: 1, d4: 0 });
    v.push(PolyConfig { degree: 3, s0: 10, d1: -4, d2: 3, d3: -5, d4: 0 });
    v.push(PolyConfig { degree: 3, s0: -7, d1: 5, d2: 0, d3: -3, d4: 0 });
    v.push(PolyConfig { degree: 3, s0: 0, d1: 0, d2: 5, d3: 4, d4: 0 });
    // Degree 4: quartic (d4 != 0).
    for d4 in [1i64, 2, 3, 4, 5, 6, 7, 8, 9, 12, 24, -1, -5, -24] {
        v.push(PolyConfig { degree: 4, s0: 0, d1: 0, d2: 0, d3: 0, d4 });
    }
    v.push(PolyConfig { degree: 4, s0: 0, d1: 1, d2: 1, d3: 1, d4: 1 });
    v.push(PolyConfig { degree: 4, s0: 7, d1: -2, d2: 5, d3: -3, d4: 7 });
    v.push(PolyConfig { degree: 4, s0: 0, d1: 0, d2: 1, d3: 0, d4: -9 });
    v.push(PolyConfig { degree: 4, s0: 0, d1: 0, d2: 0, d3: 1, d4: 2 });
    v
}

const SMALL_KS: &[i64] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 97, 1000, 65_536, 65_537, 1_000_003];
const LARGE_KS: &[i64] = &[4_194_305, 8_388_617, 16_777_223];
const HUGE_KS: &[i64] = &[4_000_000_000];

#[test]
fn small_and_medium_k_match_iteration_and_reference() {
    for cfg in cfgs() {
        check_cfg(&cfg, SMALL_KS, true);
    }
}

#[test]
fn mid_k_matches_iteration() {
    let ks = [1_000_003i64];
    for cfg in cfgs() {
        check_cfg(&cfg, &ks, true);
    }
}

#[test]
fn large_k_matches_reference() {
    // Iterative simulation is infeasible at this scale; compare against the exact
    // modular reference for every configuration.
    for cfg in cfgs() {
        check_cfg(&cfg, LARGE_KS, false);
    }
}

#[test]
fn huge_k_matches_reference() {
    for cfg in cfgs() {
        check_cfg(&cfg, HUGE_KS, false);
    }
}

// Representative configurations for which a direct iterative wrapping
// simulation is actually run at large N (> 2^22, where naive intermediate
// products overflow i64), satisfying the mission's "large N" requirement.
#[test]
fn large_n_iterative_simulation() {
    let reps = [
        PolyConfig { degree: 2, s0: 0, d1: 1, d2: 1, d3: 0, d4: 0 },
        PolyConfig { degree: 2, s0: 5, d1: -3, d2: 7, d3: 0, d4: 0 },
        PolyConfig { degree: 3, s0: 0, d1: 0, d2: 0, d3: 1, d4: 0 },
        PolyConfig { degree: 3, s0: 0, d1: 0, d2: 0, d3: -1, d4: 0 },
        PolyConfig { degree: 3, s0: 0, d1: 0, d2: 0, d3: 2, d4: 0 },
        PolyConfig { degree: 3, s0: 0, d1: 0, d2: 0, d3: 6, d4: 0 },
        PolyConfig { degree: 4, s0: 0, d1: 0, d2: 0, d3: 0, d4: 1 },
        PolyConfig { degree: 4, s0: 0, d1: 0, d2: 0, d3: 0, d4: -1 },
        PolyConfig { degree: 4, s0: 0, d1: 0, d2: 0, d3: 0, d4: 24 },
        PolyConfig { degree: 4, s0: 0, d1: 0, d2: 0, d3: 0, d4: 6 },
    ];
    for cfg in reps {
        check_cfg(&cfg, LARGE_KS, true);
    }
}

// ---------------------------------------------------------------------------
// Runtime-exponent geometric collapse end-to-end tests
// ---------------------------------------------------------------------------
//
// These build a real native binary from a program whose exponent comes from
// `read_i64()` (i.e. it is a *runtime* value, exactly like the official
// showdown harness's `runtime_arg`) and compare its result against a direct
// wrapping-i64 simulation of the original loop for every n in 1..=1000.

/// Builds a supercompiled native executable for `src` in the temp directory.
fn build_runtime_pow_exe(src: &str, tag: &str) -> (PathBuf, PathBuf) {
    let stem = format!("w3a_{}_{}", tag, std::process::id());
    let src_path = std::env::temp_dir().join(format!("{}.nl", stem));
    let exe_path = std::env::temp_dir().join(format!("{}.exe", stem));
    std::fs::write(&src_path, src).expect("write temp source");
    let out = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&src_path)
        .arg("--bench") // the official harness uses the bench entry (read_i64 argv)
        .arg("--no-cache")
        .arg("-o")
        .arg(&exe_path)
        .arg("--supercompile")
        .arg("--mode")
        .arg("distill")
        .output()
        .expect("spawn numlang");
    assert!(
        out.status.success(),
        "build failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (src_path, exe_path)
}

/// Runs `exe` with `arg` as its single command-line argument and parses the
/// printed integer result.
fn run_runtime_pow_exe(exe: &Path, arg: i64) -> i64 {
    let out = Command::new(exe)
        .arg(arg.to_string())
        .output()
        .expect("run generated executable");
    // The generated program returns the computed value from `main`, so a
    // non-zero process exit code is normal (it is the result on Windows).
    assert!(
        out.status.code().is_some(),
        "no exit code for arg {}",
        arg
    );
    // The bench entry prints `<result> COMPUTE_NS: <ticks>` on one line.
    let text = String::from_utf8_lossy(&out.stdout);
    let token = text
        .split_whitespace()
        .next()
        .expect("no output")
        .trim()
        .parse::<i64>()
        .unwrap_or_else(|_| panic!("cannot parse output {:?}", text));
    token
}

/// Direct two's-complement simulation of `acc = acc * K` repeated `n` times,
/// optionally reduced modulo `M` with signed remainder.
fn simulate_geometric(n: i64, k: i64, m: Option<i64>) -> i64 {
    let mut acc = 1i64;
    for _ in 0..n {
        acc = acc.wrapping_mul(k);
    }
    match m {
        Some(m) if m != 0 => acc.wrapping_rem(m),
        _ => acc,
    }
}

/// Source with a genuinely runtime exponent (no constant guard), deriving
/// both the base and the modulus from ordinary program literals.
fn runtime_pow_source(k: i64, modulus: Option<i64>) -> String {
    let body = match modulus {
        Some(m) => format!("return acc % {};", m),
        None => "return acc;".to_string(),
    };
    format!(
        r#"
        fn geom(n: i64) -> i64 {{
            let mut acc: i64 = 1;
            let mut i: i64 = 0;
            while i < n {{ acc = acc * {}; i = i + 1; }}
            {}
        }}
        fn main() -> i64 {{
            let n: i64 = read_i64();
            let r: i64 = geom(n);
            println(r);
            return r;
        }}
        "#,
        k, body
    )
}

fn mir_val(env: &std::collections::HashMap<String, i64>, p: &numlang::mir::Place) -> i64 {
    assert!(p.projections.is_empty(), "unexpected projected place");
    *env.get(&p.local)
        .unwrap_or_else(|| panic!("undefined MIR local {:?}", p.local))
}

/// Evaluates the straight-line collapsed MIR of `geom` for a given exponent.
/// The exponent local is the function parameter named `n`; all arithmetic uses
/// wrapping i64 semantics, matching the generated native code.
fn eval_collapsed_mir(mir: &MirProgram, n: i64) -> i64 {
    let func = mir.functions.iter().find(|f| f.name == "geom").unwrap();
    let mut env: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    env.insert("n".to_string(), n);
    // The collapse is emitted as one straight-line block; other blocks may be
    // early exits (e.g. the n <= 0 -> 1 path) which must not be simulated.
    let collapse_block = func
        .blocks
        .iter()
        .find(|b| {
            b.statements.iter().any(|Statement::Assign(dest, _)| {
                dest.local.starts_with("__rgp_")
            })
        })
        .expect("no collapse block in MIR");
    for Statement::Assign(dest, rv) in &collapse_block.statements {
        let v = match rv {
            Rvalue::Constant(TypedLiteral::Int(v, _)) => *v,
            Rvalue::Use(p) => mir_val(&env, p),
            Rvalue::BinaryOp(op, a, b) => {
                let (l, r) = (mir_val(&env, a), mir_val(&env, b));
                match op {
                    BinaryOp::Add => l.wrapping_add(r),
                    BinaryOp::Sub => l.wrapping_sub(r),
                    BinaryOp::Mul => l.wrapping_mul(r),
                    BinaryOp::Div if r != 0 => l.wrapping_div(r),
                    BinaryOp::Mod if r != 0 => l.wrapping_rem(r),
                    BinaryOp::BitAnd => l & r,
                    BinaryOp::BitOr => l | r,
                    BinaryOp::BitXor => l ^ r,
                    BinaryOp::Shl => l.wrapping_shl((r as u32) & 63),
                    BinaryOp::Shr => l.wrapping_shr((r as u32) & 63),
                    BinaryOp::Eq => (l == r) as i64,
                    BinaryOp::Ne => (l != r) as i64,
                    BinaryOp::Lt => (l < r) as i64,
                    BinaryOp::Le => (l <= r) as i64,
                    BinaryOp::Gt => (l > r) as i64,
                    BinaryOp::Ge => (l >= r) as i64,
                    _ => continue,
                }
            }
            Rvalue::Select(c, t, f) => {
                if mir_val(&env, c) != 0 {
                    mir_val(&env, t)
                } else {
                    mir_val(&env, f)
                }
            }
            _ => continue,
        };
        env.insert(dest.local.clone(), v);
    }
    let Terminator::Return { value: Some(p) } = &collapse_block.terminator else {
        panic!("no return in collapse block");
    };
    mir_val(&env, p)
}

/// Supercompiles `src` (without writing a binary) and returns its MIR.
fn supercompile_src(src: &str) -> MirProgram {
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);
    mir
}

#[test]
fn runtime_exponent_modular_power_matches_interpreted_n_1_to_1000() {
    let src = runtime_pow_source(2, Some(256));
    let (src_path, exe_path) = build_runtime_pow_exe(&src, "modpow");

    // The residual MIR must be free of the runtime `Pow` intrinsic: the O(1)
    // saturation collapse has replaced it with compares / shift / selects.
    let mir = supercompile_src(&src);
    let dump = format!("{:?}", mir);
    assert!(
        !dump.contains("Pow"),
        "runtime Pow survived the O(1) collapse"
    );
    assert!(
        dump.contains("__rgp_csat"),
        "saturation collapse did not fire"
    );

    // Full 1..=1000 differential sweep at the generated-code (MIR) level.
    for n in 1..=1000i64 {
        let actual = eval_collapsed_mir(&mir, n);
        let expected = simulate_geometric(n, 2, Some(256));
        assert_eq!(
            actual, expected,
            "n={}: collapsed {} != interpreted {}",
            n, actual, expected
        );
    }
    // Boundary cases: non-positive exponents (`x^0 == 1`) and the wrapping
    // overflow region (`2^n` saturates to zero modulo 256 for n >= 8).
    for n in [-1000i64, -5, 0, 63, 64, 1000] {
        let actual = eval_collapsed_mir(&mir, n);
        assert_eq!(actual, simulate_geometric(n, 2, Some(256)), "n={}", n);
    }
    // End-to-end through the actual native binary with the runtime argument.
    for n in [1i64, 7, 8, 9, 100, 1000, -5] {
        let actual = run_runtime_pow_exe(&exe_path, n);
        assert_eq!(
            actual,
            simulate_geometric(n, 2, Some(256)),
            "binary n={}",
            n
        );
    }

    let _ = std::fs::remove_file(&src_path);
    let _ = std::fs::remove_file(&exe_path);
}

/// The collapse derives k / m from the defining MIR statements, so it must
/// work for bases and moduli other than the benchmark's 2 / 256, including
/// the unreduced wrapping-i64 form (no trailing `% M`).
#[test]
fn runtime_exponent_collapse_is_base_and_modulus_general() {
    for (k, modulus, limit) in [(4i64, Some(64i64), 40i64), (8, None, 70)] {
        let src = runtime_pow_source(k, modulus);
        let mir = supercompile_src(&src);
        let (src_path, exe_path) = build_runtime_pow_exe(&src, &format!("k{}", k));
        for n in 1..=limit {
            let actual = eval_collapsed_mir(&mir, n);
            let expected = simulate_geometric(n, k, modulus);
            assert_eq!(
                actual, expected,
                "K={} M={:?} n={}: collapsed {} != interpreted {}",
                k, modulus, n, actual, expected
            );
        }
        for n in [1i64, 3, limit] {
            let actual = run_runtime_pow_exe(&exe_path, n);
            assert_eq!(
                actual,
                simulate_geometric(n, k, modulus),
                "binary K={} M={:?} n={}",
                k,
                modulus,
                n
            );
        }
        let _ = std::fs::remove_file(&src_path);
        let _ = std::fs::remove_file(&exe_path);
    }
}


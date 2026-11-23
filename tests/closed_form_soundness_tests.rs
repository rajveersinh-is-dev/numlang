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
use numlang::mir::supercompiler::generalize::solve_recurrence;
use numlang::mir::supercompiler::term::{SymTerm, TermInterner};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::mir::{Place, Terminator};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::types::Type;
use numlang::typecheck::typecheck;

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

/// Extract the scalar constant returned by `main` in a fully collapsed MIR.
fn returned_constant(mir: &MirProgram) -> Option<i64> {
    let main = mir.functions.iter().find(|f| f.name == "main")?;
    for block in &main.blocks {
        let Terminator::Return { value: Some(p) } = &block.terminator else {
            continue;
        };
        if !p.projections.is_empty() {
            continue;
        }
        let mut cur = p.clone();
        for Statement::Assign(dest, rv) in block.statements.iter().rev() {
            if dest.local != cur.local || !dest.projections.is_empty() {
                continue;
            }
            match rv {
                Rvalue::Constant(TypedLiteral::Int(v, _)) => return Some(*v),
                Rvalue::Use(next) => cur = next.clone(),
                _ => {}
            }
        }
    }
    None
}

/// Randomized differential test for the O(1) modular-geometric collapse:
/// `acc = acc * K; ... acc % M` for K=2, M=256.  For every iteration count in
/// 1..=1000 we compile a statically-instantiated copy through the full
/// supercompiler (the "specialized" result) and compare it bit-for-bit with a
/// direct wrapping i64 simulation of the interpreted loop (the "interpreted"
/// result).  This exercises both the small-n saturation identity (2^n < 256)
/// and the large-n zero region where 2^n overflows i64.
#[test]
fn modular_power_collapse_matches_interpreted_n_1_to_1000() {
    for n in 1..=1000i64 {
        let src = format!(
            r#"
            fn pow2(k: i64) -> i64 {{
                let mut acc: i64 = 1;
                let mut i: i64 = 0;
                while i < k {{ acc = acc * 2; i = i + 1; }}
                return acc % 256;
            }}
            fn main() -> i64 {{
                let r: i64 = pow2({});
                return r;
            }}
            "#,
            n
        );
        let tokens = tokenize(&src).unwrap();
        let ast = parse(&tokens).unwrap();
        let typed = typecheck(&ast).unwrap();
        let mut mir = lower_program(&typed);
        supercompile_mir_program(&mut mir);

        let specialized = returned_constant(&mir)
            .unwrap_or_else(|| panic!("n={}: supercompiled result is not a constant", n));

        let mut acc = 1i64;
        for _ in 0..n {
            acc = acc.wrapping_mul(2);
        }
        let interpreted = acc.wrapping_rem(256);

        assert_eq!(
            specialized, interpreted,
            "n={}: specialized {} != interpreted {}",
            n, specialized, interpreted
        );
    }
}


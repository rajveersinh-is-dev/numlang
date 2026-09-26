use std::fs;
use std::process::Command;

use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::validate::{
    check_satisfiability, verify_formula_validity, BoolFormula, BvExpr, SmtLib2Printer,
    SmtResult, TranslationValidator, ValidationError,
};
use numlang::mir::supercompiler::{
    supercompile_mir_program_with_mode, verify_program_equivalence, SupercompileMode,
};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&program).expect("Typecheck failed");
    typed.desugar_for_loops();
    lower_program(&typed)
}

#[test]
fn test_smt_bitvector_algebra_and_decision_procedure() {
    let x = BvExpr::var("x", 64);
    let y = BvExpr::var("y", 64);
    let z = BvExpr::var("z", 64);

    // 1. Commutativity of bitwise AND: (x & y) == (y & x)
    let and_comm = BoolFormula::Eq(
        Box::new(BvExpr::And(Box::new(x.clone()), Box::new(y.clone()))),
        Box::new(BvExpr::And(Box::new(y.clone()), Box::new(x.clone()))),
    );
    assert_eq!(
        verify_formula_validity(&and_comm),
        Ok(()),
        "Bitwise AND commutativity must hold for all 64-bit inputs"
    );

    // 2. Identity: x ^ x == 0
    let xor_self = BoolFormula::Eq(
        Box::new(BvExpr::Xor(Box::new(x.clone()), Box::new(x.clone()))),
        Box::new(BvExpr::constant(0, 64)),
    );
    assert_eq!(
        verify_formula_validity(&xor_self),
        Ok(()),
        "x ^ x == 0 must hold for all 64-bit inputs"
    );

    // 3. Two's complement identity: x + (-x) == 0 (mod 2^64)
    let twos_complement = BoolFormula::Eq(
        Box::new(BvExpr::Add(
            Box::new(x.clone()),
            Box::new(BvExpr::Neg(Box::new(x.clone()))),
        )),
        Box::new(BvExpr::constant(0, 64)),
    );
    assert_eq!(
        verify_formula_validity(&twos_complement),
        Ok(()),
        "x + (-x) == 0 must hold for all 64-bit inputs"
    );

    // 4. Shift and multiply relation: (x << 1) == (x * 2)
    let shl_mul = BoolFormula::Eq(
        Box::new(BvExpr::Shl(
            Box::new(x.clone()),
            Box::new(BvExpr::constant(1, 64)),
        )),
        Box::new(BvExpr::Mul(
            Box::new(x.clone()),
            Box::new(BvExpr::constant(2, 64)),
        )),
    );
    assert_eq!(
        verify_formula_validity(&shl_mul),
        Ok(()),
        "(x << 1) == (x * 2) must hold for all 64-bit inputs"
    );

    // 5. De Morgan's Law: ~(x & y) == (~x | ~y)
    let demorgan = BoolFormula::Eq(
        Box::new(BvExpr::Not(Box::new(BvExpr::And(
            Box::new(x.clone()),
            Box::new(y.clone()),
        )))),
        Box::new(BvExpr::Or(
            Box::new(BvExpr::Not(Box::new(x.clone()))),
            Box::new(BvExpr::Not(Box::new(y.clone()))),
        )),
    );
    assert_eq!(
        verify_formula_validity(&demorgan),
        Ok(()),
        "De Morgan's law must hold for all 64-bit inputs"
    );

    // 6. Associativity of addition: (x + y) + z == x + (y + z)
    let assoc_add = BoolFormula::Eq(
        Box::new(BvExpr::Add(
            Box::new(BvExpr::Add(Box::new(x.clone()), Box::new(y.clone()))),
            Box::new(z.clone()),
        )),
        Box::new(BvExpr::Add(
            Box::new(x.clone()),
            Box::new(BvExpr::Add(Box::new(y.clone()), Box::new(z.clone()))),
        )),
    );
    assert_eq!(
        verify_formula_validity(&assoc_add),
        Ok(()),
        "Addition associativity must hold for all 64-bit inputs"
    );

    // 7. Non-theorem: x & y == x | y (must FAIL with counterexample)
    let non_theorem = BoolFormula::Eq(
        Box::new(BvExpr::And(Box::new(x.clone()), Box::new(y.clone()))),
        Box::new(BvExpr::Or(Box::new(x.clone()), Box::new(y))),
    );
    let res = verify_formula_validity(&non_theorem);
    assert!(res.is_err(), "x & y == x | y must be rejected as invalid");
    let counterexample = res.unwrap_err();
    assert!(
        counterexample.contains_key("x") || counterexample.contains_key("y"),
        "Counterexample model must assign conflicting values"
    );

    // 8. Direct satisfiability query using check_satisfiability and SmtResult
    let sat_query = BoolFormula::Eq(
        Box::new(BvExpr::Add(Box::new(x.clone()), Box::new(BvExpr::constant(5, 64)))),
        Box::new(BvExpr::constant(15, 64)),
    );
    match check_satisfiability(&sat_query) {
        SmtResult::Sat(model) => {
            assert!(model.is_empty() || model.contains_key("x"));
        }
        SmtResult::Unsat => panic!("x + 5 == 15 must be satisfiable!"),
    }
}

#[test]
fn test_smt_uninterpreted_functions_and_congruence() {
    let x = BvExpr::var("x", 64);
    let y = BvExpr::var("y", 64);

    let fx = BvExpr::Apply("f".to_string(), vec![x.clone()], 64);
    let fy = BvExpr::Apply("f".to_string(), vec![y.clone()], 64);

    // Congruence rule: (x == y) => (f(x) == f(y))
    let premise = BoolFormula::Eq(Box::new(x.clone()), Box::new(y.clone()));
    let conclusion = BoolFormula::Eq(Box::new(fx.clone()), Box::new(fy.clone()));
    let congruence = BoolFormula::Implies(Box::new(premise), Box::new(conclusion));

    assert_eq!(
        verify_formula_validity(&congruence),
        Ok(()),
        "Uninterpreted function congruence must hold"
    );

    // Violated congruence: (x == y) => (f(x) == f(y) + 1)
    let violated_conclusion = BoolFormula::Eq(
        Box::new(fx),
        Box::new(BvExpr::Add(
            Box::new(fy),
            Box::new(BvExpr::constant(1, 64)),
        )),
    );
    let false_prop = BoolFormula::Implies(
        Box::new(BoolFormula::Eq(Box::new(x), Box::new(y))),
        Box::new(violated_conclusion),
    );

    let check_res = verify_formula_validity(&false_prop);
    assert!(check_res.is_err(), "False congruence must be refuted");
}

#[test]
fn test_smt_relational_path_vc_conditional_branches() {
    let code_spec = r#"
    fn abs_val(x: i64) -> i64 {
        if x >= 0 {
            return x;
        } else {
            return 0 - x;
        }
    }
    "#;

    let code_opt = r#"
    fn abs_val(x: i64) -> i64 {
        if x < 0 {
            return -x;
        } else {
            return x;
        }
    }
    "#;

    let orig_mir = get_mir(code_spec);
    let res_mir = get_mir(code_opt);

    let orig_func = orig_mir.functions.iter().find(|f| f.name == "abs_val").unwrap();
    let res_func = res_mir.functions.iter().find(|f| f.name == "abs_val").unwrap();

    let mut validator = TranslationValidator::new(orig_func, res_func);
    let cert = validator.verify().expect("Translation validation must prove branch reorganization");

    assert!(cert.is_certified);
    assert_eq!(cert.function_name, "abs_val");
    assert!(cert.smt_queries_proved >= 2, "Must prove simulation across both branch paths");
}

#[test]
fn test_smt_mutation_detection_operator_swap() {
    let code = r#"
    fn compute(x: i64, y: i64) -> i64 {
        let sum: i64 = x + y;
        return sum * 2;
    }
    "#;

    let orig_mir = get_mir(code);
    let mut mutated_mir = orig_mir.clone();

    // Mutate operator: replace `x + y` with `x - y`
    let func = mutated_mir.functions.iter_mut().find(|f| f.name == "compute").unwrap();
    for b in &mut func.blocks {
        for stmt in &mut b.statements {
            let Statement::Assign(_, rval) = stmt;
            if let Rvalue::BinaryOp(op, p1, p2) = rval {
                if *op == numlang::ast::BinaryOp::Add {
                    *rval = Rvalue::BinaryOp(numlang::ast::BinaryOp::Sub, p1.clone(), p2.clone());
                }
            }
        }
    }

    let orig_func = orig_mir.functions.iter().find(|f| f.name == "compute").unwrap();
    let mutated_func = mutated_mir.functions.iter().find(|f| f.name == "compute").unwrap();

    let mut validator = TranslationValidator::new(orig_func, mutated_func);
    let err = validator.verify().expect_err("SMT validation must reject operator swap");

    assert!(
        matches!(err, ValidationError::OutputMismatch { .. }),
        "Expected OutputMismatch error with counterexample, got: {:?}",
        err
    );
}

#[test]
fn test_smt_mutation_detection_off_by_one_and_branch_boundary() {
    let code = r#"
    fn threshold(x: i64) -> i64 {
        if x < 10 {
            return 100;
        } else {
            return 200;
        }
    }
    "#;

    let orig_mir = get_mir(code);
    let mut mutated_mir = orig_mir.clone();

    // Mutate condition: replace `< 10` with `<= 10`
    let func = mutated_mir.functions.iter_mut().find(|f| f.name == "threshold").unwrap();
    for b in &mut func.blocks {
        for stmt in &mut b.statements {
            let Statement::Assign(_, rval) = stmt;
            if let Rvalue::BinaryOp(op, p1, p2) = rval {
                if *op == numlang::ast::BinaryOp::Lt {
                    *rval = Rvalue::BinaryOp(numlang::ast::BinaryOp::Le, p1.clone(), p2.clone());
                }
            }
        }
    }

    let orig_func = orig_mir.functions.iter().find(|f| f.name == "threshold").unwrap();
    let mutated_func = mutated_mir.functions.iter().find(|f| f.name == "threshold").unwrap();

    let mut validator = TranslationValidator::new(orig_func, mutated_func);
    let err = validator.verify().expect_err("SMT validation must detect boundary condition mutation");

    assert!(
        matches!(err, ValidationError::OutputMismatch { .. }),
        "Expected OutputMismatch error, got: {:?}",
        err
    );
    let err_msg = format!("{}", err);
    // Boundary counterexample at x=10
    assert!(err_msg.contains("10") || err_msg.contains("100") || err_msg.contains("200"),
        "Error message must reference mismatched outputs or counterexample: {}", err_msg);
}

#[test]
fn test_smt_loop_unrolling_and_closed_form_equivalence() {
    let code_unrolled = r#"
    fn scale_triple(n: i64) -> i64 {
        let mut acc: i64 = 0;
        acc = acc + n;
        acc = acc + n;
        acc = acc + n;
        return acc;
    }
    "#;

    let code_closed = r#"
    fn scale_triple(n: i64) -> i64 {
        return n * 3;
    }
    "#;

    let orig_mir = get_mir(code_unrolled);
    let res_mir = get_mir(code_closed);

    let orig_func = orig_mir.functions.iter().find(|f| f.name == "scale_triple").unwrap();
    let res_func = res_mir.functions.iter().find(|f| f.name == "scale_triple").unwrap();

    let mut validator = TranslationValidator::new(orig_func, res_func);
    let cert = validator.verify().expect("Unrolled loop must be bit-for-bit equivalent to closed form");

    assert!(cert.is_certified);
    assert_eq!(cert.function_name, "scale_triple");
    assert!(cert.smt_queries_proved >= 1);
}

#[test]
fn test_smt_smtlib2_export_verification() {
    let x = BvExpr::var("x", 64);
    let y = BvExpr::var("y", 64);

    let formula = BoolFormula::Eq(
        Box::new(BvExpr::Add(Box::new(x.clone()), Box::new(y.clone()))),
        Box::new(BvExpr::Add(Box::new(y), Box::new(x))),
    );

    let smt_text = SmtLib2Printer::to_smtlib2(&formula);
    assert!(smt_text.contains("(set-logic QF_UFBV)"), "Must set QF_UFBV logic");
    assert!(smt_text.contains("(declare-const x (_ BitVec 64))"), "Must declare x");
    assert!(smt_text.contains("(declare-const y (_ BitVec 64))"), "Must declare y");
    assert!(smt_text.contains("(assert (= (bvadd x y) (bvadd y x)))"), "Must assert addition commutativity");
    assert!(smt_text.contains("(check-sat)"), "Must request check-sat");
    assert!(smt_text.contains("(get-model)"), "Must request model");
}

#[test]
fn test_smt_cli_verify_equivalence_with_mutations() {
    let valid_code = r#"
    fn linear(x: i64) -> i64 {
        let a: i64 = x * 4;
        let b: i64 = x * 2;
        return a + b;
    }

    fn main() -> i64 {
        return linear(7);
    }
    "#;

    let test_dir = std::env::temp_dir().join("numlang_test_cli_smt_verify");
    let _ = fs::create_dir_all(&test_dir);
    let src_file = test_dir.join("verify_linear.nl");
    fs::write(&src_file, valid_code).unwrap();

    // 1. Verify valid program succeeds
    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .args(["--verify-equivalence", src_file.to_str().unwrap()])
        .output()
        .expect("CLI execution failed");

    assert!(output.status.success(), "CLI run with --verify-equivalence must succeed on valid program: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("translation validation certified"), "Stdout must confirm certification: {}", stdout);
    assert!(stdout.contains("linear"), "Stdout must mention verified function `linear`");

    // 2. Program-wide translation validation on supercompiled program
    let orig_mir = get_mir(valid_code);
    let mut sc_mir = orig_mir.clone();
    supercompile_mir_program_with_mode(&mut sc_mir, SupercompileMode::Classic, "size");
    let certs = verify_program_equivalence(&orig_mir, &sc_mir)
        .expect("Program-wide translation validation must certify all functions");
    assert_eq!(certs.len(), 2, "Must certify both `linear` and `main`");

    let _ = fs::remove_dir_all(&test_dir);
}

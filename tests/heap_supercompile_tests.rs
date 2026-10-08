use std::process::Command;

use numlang::ast::BinaryOp;
use numlang::mir::supercompiler::generalize::most_specific_generalization;
use numlang::mir::supercompiler::term::{SymTerm, TermInterner};
use numlang::mir::Place;
use numlang::typecheck::types::Type;

#[test]
fn test_textbook_msg_identical_terms() {
    let mut interner = TermInterner::new();
    let mut next_var_id = 0;

    let x_place = Place {
        local: "x".to_string(),
        projections: vec![],
    };
    let x = interner.intern_var(x_place, Type::I64);
    let one = interner.intern_int(1);
    let term1 = interner.intern_binary(BinaryOp::Add, x, one, Type::I64);

    let res = most_specific_generalization(term1, term1, &mut interner, &mut next_var_id);
    assert_eq!(res.common_term, term1);
    assert!(res.var_mappings.is_empty());
}

#[test]
fn test_textbook_anti_unify_interface() {
    use numlang::mir::supercompiler::generalize::anti_unify;
    use std::collections::HashMap;

    let mut interner = TermInterner::new();
    let mut subst1 = HashMap::new();
    let mut subst2 = HashMap::new();

    let x_place = Place {
        local: "x".to_string(),
        projections: vec![],
    };
    let x = interner.intern_var(x_place, Type::I64);
    let one = interner.intern_int(1);
    let two = interner.intern_int(2);

    let t1 = interner.intern_binary(BinaryOp::Add, x, one, Type::I64);
    let t2 = interner.intern_binary(BinaryOp::Add, x, two, Type::I64);

    let common = anti_unify(t1, t2, &mut interner, &mut subst1, &mut subst2);
    assert_eq!(subst1.len(), 1);
    assert_eq!(subst2.len(), 1);
    let gen_var = subst1.keys().next().unwrap();
    assert_eq!(subst1[gen_var], one);
    assert_eq!(subst2[gen_var], two);

    let expected_var_place = Place {
        local: gen_var.clone(),
        projections: vec![],
    };
    let expected_var = interner.intern_var(expected_var_place, Type::I64);
    let expected_common = interner.intern_binary(BinaryOp::Add, x, expected_var, Type::I64);
    assert_eq!(common, expected_common);
}

#[test]
fn test_textbook_msg_differing_subterms() {
    let mut interner = TermInterner::new();
    let mut next_var_id = 0;

    let x_place = Place {
        local: "x".to_string(),
        projections: vec![],
    };
    let x = interner.intern_var(x_place, Type::I64);
    let one = interner.intern_int(1);
    let two = interner.intern_int(2);

    let t1 = interner.intern_binary(BinaryOp::Add, x, one, Type::I64);
    let t2 = interner.intern_binary(BinaryOp::Add, x, two, Type::I64);

    let res = most_specific_generalization(t1, t2, &mut interner, &mut next_var_id);
    assert_eq!(res.var_mappings.len(), 1);
    let (gen_var, v1, v2) = &res.var_mappings[0];
    assert_eq!(*v1, one);
    assert_eq!(*v2, two);

    let expected_gen_term = interner.intern_var(gen_var.clone(), Type::I64);
    let expected_common = interner.intern_binary(BinaryOp::Add, x, expected_gen_term, Type::I64);
    assert_eq!(res.common_term, expected_common);
}

#[test]
fn test_textbook_msg_variable_sharing() {
    let mut interner = TermInterner::new();
    let mut next_var_id = 0;

    let one = interner.intern_int(1);
    let two = interner.intern_int(2);

    // Pair(1, 1) and Pair(2, 2)
    let t1 = interner.intern_constructor("Pair".to_string(), 0, vec![one, one], Type::I64);
    let t2 = interner.intern_constructor("Pair".to_string(), 0, vec![two, two], Type::I64);

    let res = most_specific_generalization(t1, t2, &mut interner, &mut next_var_id);
    // Plotkin (1970) / Sørensen & Glück (1995): Identical differences must reuse the same variable
    assert_eq!(res.var_mappings.len(), 1);

    if let SymTerm::Constructor(name, tag, fields, _) = interner.get(res.common_term) {
        assert_eq!(name, "Pair");
        assert_eq!(*tag, 0);
        assert_eq!(fields.len(), 2);
        assert_eq!(
            fields[0], fields[1],
            "Both fields must share the identical generalized variable"
        );
    } else {
        panic!("Expected Constructor term");
    }
}

#[test]
fn test_textbook_msg_recursive_adts() {
    let mut interner = TermInterner::new();
    let mut next_var_id = 0;

    let nil = interner.intern_constructor("Nil".to_string(), 0, vec![], Type::I64);
    let one = interner.intern_int(1);
    let two = interner.intern_int(2);
    let three = interner.intern_int(3);

    // Cons(1, Cons(2, Nil))
    let l1_tail = interner.intern_constructor("Cons".to_string(), 1, vec![two, nil], Type::I64);
    let l1 = interner.intern_constructor("Cons".to_string(), 1, vec![one, l1_tail], Type::I64);

    // Cons(1, Cons(3, Nil))
    let l2_tail = interner.intern_constructor("Cons".to_string(), 1, vec![three, nil], Type::I64);
    let l2 = interner.intern_constructor("Cons".to_string(), 1, vec![one, l2_tail], Type::I64);

    let res = most_specific_generalization(l1, l2, &mut interner, &mut next_var_id);
    assert_eq!(res.var_mappings.len(), 1);
    let (_, v1, v2) = &res.var_mappings[0];
    assert_eq!(*v1, two);
    assert_eq!(*v2, three);
}

fn run_numlang_mir_supercompiled(path: &str) -> (i32, String) {
    let exe = env!("CARGO_BIN_EXE_numlang");
    let output = Command::new(exe)
        .arg("run")
        .arg(path)
        .arg("--use-mir")
        .arg("--supercompile")
        .output()
        .expect("Failed to execute numlang");

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let code = output.status.code().unwrap_or(-1);
    (code, stdout)
}

#[test]
fn test_supercompiled_nrev() {
    let (code, stdout) = run_numlang_mir_supercompiled("bench/numlang/nrev.nl");
    assert_eq!(stdout, "12000");
    // 12000 % 256 == 224
    assert_eq!(code, 224);
}

#[test]
fn test_supercompiled_append3() {
    let (code, stdout) = run_numlang_mir_supercompiled("bench/numlang/append3.nl");
    assert_eq!(stdout, "46500");
    // 46500 % 256 == 164
    assert_eq!(code, 164);
}

#[test]
fn test_supercompiled_tree_flip() {
    let (code, stdout) = run_numlang_mir_supercompiled("bench/numlang/tree_flip.nl");
    assert_eq!(stdout, "37600");
    // 37600 % 256 == 224
    assert_eq!(code, 224);
}

#[test]
fn test_supercompiled_peano_mul() {
    let (code, stdout) = run_numlang_mir_supercompiled("bench/numlang/peano_mul.nl");
    assert_eq!(stdout, "1200");
    // 1200 % 256 == 176
    assert_eq!(code, 176);
}

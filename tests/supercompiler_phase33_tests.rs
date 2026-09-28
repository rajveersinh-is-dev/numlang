use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::drive::{ProcessEdge, SupercompilerDriver};
use numlang::mir::supercompiler::state::Interval;
use numlang::mir::supercompiler::term::SymTerm;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_refinement_dead_branch_elimination() {
    let src = r#"
fn check(x: i64) -> i64 {
    if x > 100 {
        return 99;
    }
    return x;
}
fn main() -> i64 {
    return check(42);
}
"#;

    let program = get_mir(src);
    let check_func = program.functions.iter().find(|f| f.name == "check").unwrap();

    let tree = SupercompilerDriver::new(check_func)
        .with_param_refinement("x", Interval { lo: Some(0), hi: Some(50) })
        .run();

    // Assert that the root node's edges do NOT branch into 2 edges (edge count is 1, not 2)
    let root = &tree.nodes[tree.root.0];
    assert_eq!(
        root.edges.len(),
        1,
        "Expected exactly 1 transition edge from root (dead branch eliminated), got {}",
        root.edges.len()
    );

    // Verify there are no BranchTrue edges leading to return 99
    for node in &tree.nodes {
        if let Some(ret) = node.return_term {
            if let SymTerm::ConstInt(val, _) = tree.interner.get(ret) {
                assert_ne!(*val, 99, "Found return term 99 from supposedly pruned dead branch");
            }
        }
    }
    assert!(tree.stats.branches_pruned >= 1, "Expected at least 1 branch pruned");
}

#[test]
fn test_refinement_interval_propagation_add() {
    let src = r#"
fn f(x: i64) -> i64 {
    let y: i64 = x + 10;
    return y;
}
fn main() -> i64 {
    return f(5);
}
"#;

    let program = get_mir(src);
    let f_func = program.functions.iter().find(|f| f.name == "f").unwrap();

    let tree = SupercompilerDriver::new(f_func)
        .with_param_refinement("x", Interval { lo: Some(5), hi: Some(20) })
        .run();

    // Assert that the return term has interval [15, 30]
    let ret_node = tree.nodes.iter().find(|n| n.return_term.is_some()).expect("Expected a return node");
    let ret_term = ret_node.return_term.unwrap();
    let ret_iv = ret_node.state.get_refinement(ret_term);

    assert_eq!(
        ret_iv,
        Interval { lo: Some(15), hi: Some(30) },
        "Expected return interval [15, 30], got {:?}",
        ret_iv
    );
}

#[test]
fn test_refinement_bce_loop() {
    let src = r#"
fn fill(n: i64) -> i64 {
    let mut arr: [i64; 4] = [0, 0, 0, 0];
    let mut i: i64 = 0;
    while i < n {
        arr[i] = i;
        i = i + 1;
    }
    return arr[0];
}
fn main() -> i64 {
    return fill(2);
}
"#;

    let program = get_mir(src);
    let fill_func = program.functions.iter().find(|f| f.name == "fill").unwrap();

    let tree = SupercompilerDriver::new(fill_func)
        .with_param_refinement("n", Interval { lo: Some(0), hi: Some(3) })
        .run();

    assert!(
        tree.stats.sc_bce_eliminated >= 1 || tree.stats.branches_pruned >= 1,
        "Expected sc_bce_eliminated >= 1 or branches_pruned >= 1, got sc_bce_eliminated = {}, branches_pruned = {}",
        tree.stats.sc_bce_eliminated,
        tree.stats.branches_pruned
    );
}

#[test]
fn test_refinement_callsite_propagation() {
    let src = r#"
fn inner(k: i64) -> i64 {
    if k > 1000 { return 0; }
    return k * 2;
}
fn outer() -> i64 {
    return inner(500);
}
fn main() -> i64 { return outer(); }
"#;

    let program = get_mir(src);
    let outer_func = program.functions.iter().find(|f| f.name == "outer").unwrap();

    let tree = SupercompilerDriver::new(outer_func)
        .with_program_functions(&program.functions)
        .run();

    // In outer's process tree, inner(500) was inlined with k = 500
    // Assert there is no BranchTrue / BranchFalse edge for k > 1000
    for node in &tree.nodes {
        for edge in &node.edges {
            match edge {
                ProcessEdge::BranchTrue(..) | ProcessEdge::BranchFalse(..) => {
                    panic!("Found branching edge in outer's process tree: dead branch was not pruned!");
                }
                _ => {}
            }
        }
    }

    // Verify return term evaluates to 1000
    let ret_node = tree.nodes.iter().find(|n| n.return_term.is_some()).expect("Expected return node");
    let ret_term = ret_node.return_term.unwrap();
    if let SymTerm::ConstInt(val, _) = tree.interner.get(ret_term) {
        assert_eq!(*val, 1000, "Expected returned constant 1000, got {}", val);
    }
}

#[test]
fn test_refinement_regression_no_spurious_prune() {
    let src = r#"
fn maybe_big(x: i64) -> i64 {
    if x > 50 { return x * 2; }
    return x;
}
fn main() -> i64 { return maybe_big(75); }
"#;

    let program = get_mir(src);
    let maybe_big_func = program.functions.iter().find(|f| f.name == "maybe_big").unwrap();

    let tree = SupercompilerDriver::new(maybe_big_func)
        .with_param_refinement("x", Interval::exact(75))
        .run();

    // Assert the true branch is taken and the false branch is pruned
    assert!(tree.stats.branches_pruned >= 1, "Expected false branch to be pruned");

    // Verify return term is ConstInt(150) or a symbolic multiplication term, NOT 75
    let ret_node = tree.nodes.iter().find(|n| n.return_term.is_some()).expect("Expected return node");
    let ret_term = ret_node.return_term.unwrap();

    match tree.interner.get(ret_term) {
        SymTerm::ConstInt(val, _) => {
            assert_eq!(*val, 150, "Expected returned value 150, got {}", val);
        }
        SymTerm::Binary(op, _, _, _) => {
            assert_eq!(*op, numlang::ast::BinaryOp::Mul);
        }
        other => panic!("Unexpected return term {:?}", other),
    }
}

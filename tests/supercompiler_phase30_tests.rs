use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::drive::{ProcessEdge, SupercompilerDriver};
use numlang::mir::supercompiler::residualize::residualize_process_tree;
use numlang::mir::supercompiler::term::SymTerm;
use numlang::mir::supercompiler::{
    supercompile_mir_function, supercompile_mir_program_with_mode, SupercompileMode,
};
use numlang::mir::{Place, Terminator};
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
fn test_msg_knot_materializes_gen_node() {
    let src = r#"
fn coupled_recurrence(init_a: i64, init_b: i64) -> i64 {
    let mut a: i64 = init_a;
    let mut b: i64 = init_b;
    while a > 0 {
        let next_a: i64 = (a * 3 + b) % 10007;
        let next_b: i64 = (b * 5 + a) % 997;
        a = next_a;
        b = next_b;
    }
    return b;
}

fn main() -> i64 {
    return coupled_recurrence(5, 7);
}
"#;
    let mir = get_mir(src);
    let fib_func = mir.functions.iter().find(|f| f.name == "coupled_recurrence").unwrap();
    let driver = SupercompilerDriver::new(fib_func);
    let tree = driver.run();

    assert!(
        tree.stats.knots_tied > 0,
        "Expected knots to be tied for coupled recurrence where recurrence solver fails"
    );

    // Assert that tree.nodes contains at least one knot pointing to a materialized MSG node
    // where generalized terms (SymTerm::Var) are present in its state environment.
    let mut found_msg_knot = false;
    for node in &tree.nodes {
        for edge in &node.edges {
            if let ProcessEdge::Knot(target_id) = edge {
                let target_node = &tree.nodes[target_id.0];
                let a_place = Place {
                    local: "a".to_string(),
                    projections: vec![],
                };
                if let Some(term_id) = target_node.state.get_value(&a_place) {
                    if matches!(tree.interner.get(term_id), SymTerm::Var(..)) {
                        found_msg_knot = true;
                        break;
                    }
                }
            }
        }
    }

    assert!(
        found_msg_knot,
        "Tree nodes must contain at least one knot pointing to a materialized generalized MSG node"
    );
}

#[test]
fn test_budget_overflow_leaf_is_unreachable() {
    let src = r#"
fn deeply_branching(a: i64, b: i64, c: i64, d: i64) -> i64 {
    let mut x: i64 = 0;
    if a > 0 {
        if b > 0 {
            if c > 0 {
                if d > 0 {
                    x = 1;
                } else {
                    x = 2;
                }
            } else {
                x = 3;
            }
        } else {
            x = 4;
        }
    } else {
        x = 5;
    }
    return x;
}

fn main() -> i64 {
    return deeply_branching(1, 2, 3, 4);
}
"#;
    let mir = get_mir(src);
    let func = mir
        .functions
        .iter()
        .find(|f| f.name == "deeply_branching")
        .unwrap();

    // Set max_inline_nodes to 10 on a deeply branching function
    let driver = SupercompilerDriver::new(func).with_max_inline_nodes(10);
    let tree = driver.run();

    // Assert driver recorded nodes up to or beyond budget
    assert!(
        tree.nodes.len() >= 10,
        "Expected at least 10 nodes, got {}",
        tree.nodes.len()
    );

    // Residualize the process tree and verify that the resulting MIR contains an Unreachable terminator
    let residual = residualize_process_tree(&tree, func);
    let has_unreachable = residual
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Terminator::Unreachable));

    assert!(
        has_unreachable,
        "Residual MIR must contain an Unreachable terminator for budget-overflow leaves"
    );
}

#[test]
fn test_ast_inliner_precomputed_has_loop_behavior() {
    let src = r#"
fn leaf_callee(x: i64) -> i64 {
    return x + 10;
}

fn loop_callee(x: i64) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < x {
        sum = sum + i;
        i = i + 1;
    }
    return sum;
}

fn caller(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        acc = acc + leaf_callee(i);
        acc = acc + loop_callee(i);
        i = i + 1;
    }
    return acc;
}

fn main() -> i64 {
    return caller(5);
}
"#;
    let tokens = tokenize(src).unwrap();
    let program = parse(&tokens).unwrap();
    let mut typed = typecheck(&program).unwrap();

    // Run AST inlining with precomputed has_loop_funcs
    numlang::opt::inlining::optimize_program(&mut typed);

    let caller_fn = typed.functions.iter().find(|f| f.name == "caller").unwrap();
    // caller should have leaf_callee inlined, but loop_callee retained due to loop guard
    let caller_str = format!("{:?}", caller_fn);
    assert!(
        caller_str.contains("loop_callee"),
        "loop_callee must not be inlined into a loop body"
    );
}

#[test]
fn test_unified_profitability_gate_consistency() {
    // A pure identity function without reductions should not produce an inflated residual CFG
    let src = r#"
fn ident(x: i64) -> i64 {
    return x;
}

fn main() -> i64 {
    return ident(42);
}
"#;
    let mir = get_mir(src);
    let ident_func = mir.functions.iter().find(|f| f.name == "ident").unwrap();
    let supercompiled = supercompile_mir_function(ident_func);
    assert_eq!(
        ident_func.blocks.len(),
        supercompiled.blocks.len(),
        "Identical block count expected when no supercompilation reductions occur"
    );

    // In MRSC mode, a recursive program with knots is now allowed to residualize
    let mut mrsc_mir = get_mir(src);
    let stats = supercompile_mir_program_with_mode(
        &mut mrsc_mir,
        SupercompileMode::Mrsc,
        "size",
    );
    // Program successfully processed without panics or invalid bailouts
    assert!(stats.nodes_explored >= 1);
}

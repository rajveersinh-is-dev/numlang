use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::mir::lower::lower_program;
use numlang::mir::{Terminator, BasicBlockId, Projection};
use numlang::mir::dominance::{compute_dominance, detect_loops};
use std::collections::HashMap;

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_mir_cfg_basic_block() {
    let source = "
    fn main() -> i32 {
        let x: i32 = 10;
        let y: i32 = 20;
        return x + y;
    }";
    let mir = get_mir(source);
    assert_eq!(mir.functions.len(), 1);
    let f = &mir.functions[0];
    assert!(!f.blocks.is_empty());

    // There should be a single block that ends with a Return
    let first_block = &f.blocks[0];
    match &first_block.terminator {
        Terminator::Return { .. } => {} // OK
        t => panic!("Expected Return terminator, got {:?}", t),
    }
}

#[test]
fn test_mir_loop_headers_detection() {
    let source = "
    fn main() -> i32 {
        let mut x: i32 = 0;
        while x < 10 {
            x = x + 1;
        }
        return x;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];

    // Compute basic block graph
    let mut preds: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
    let mut succs: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
    let mut all_blocks = Vec::new();

    for b in &f.blocks {
        all_blocks.push(b.id.clone());
        preds.entry(b.id.clone()).or_default();
        succs.entry(b.id.clone()).or_default();
    }

    for b in &f.blocks {
        match &b.terminator {
            Terminator::Branch { target } => {
                succs.get_mut(&b.id).unwrap().push(target.clone());
                preds.entry(target.clone()).or_default().push(b.id.clone());
            }
            Terminator::BranchIf { then_target, else_target, .. } => {
                succs.get_mut(&b.id).unwrap().push(then_target.clone());
                succs.get_mut(&b.id).unwrap().push(else_target.clone());
                preds.entry(then_target.clone()).or_default().push(b.id.clone());
                preds.entry(else_target.clone()).or_default().push(b.id.clone());
            }
            _ => {}
        }
    }

    let entry = f.blocks[0].id.clone();
    let dom = compute_dominance(entry.clone(), &preds, &succs, &all_blocks);
    let loops = detect_loops(entry, &preds, &succs, &all_blocks, &dom);

    // One while loop => exactly one header
    assert_eq!(loops.headers.len(), 1);
}

#[test]
fn test_mir_place_projection_lowering() {
    let source = "
    struct Point { x: i32, y: i32 }
    fn main() -> i32 {
        let p: Point = Point { x: 10, y: 20 };
        return p.x;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];

    // Look for a field projection in the statements
    let mut found_field_proj = false;
    for b in &f.blocks {
        for stmt in &b.statements {
            if let numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Use(p)) = stmt {
                if p.projections.iter().any(|proj| matches!(proj, Projection::Field(_))) {
                    found_field_proj = true;
                }
            }
        }
    }
    assert!(found_field_proj, "Expected to find a Field projection");
}

#[test]
fn test_mir_ssa_correctness() {
    // Basic test ensuring temps are generated.
    let source = "
    fn main() -> i32 {
        return 1 + 2 + 3;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];

    // Check that locals include temp variables `_t0`, `_t1`, etc.
    let temp_count = f.locals.iter().filter(|l| l.name.starts_with("_t")).count();
    assert!(temp_count > 0, "Expected temp variables to be generated for complex expressions");
}

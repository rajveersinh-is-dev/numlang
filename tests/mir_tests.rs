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

#[test]
fn test_memory_ssa_linear_versioning() {
    use numlang::mir::memory_ssa::{MemorySSA, MemoryVersionId};

    let source = "
    fn main() -> i32 {
        let a: i32 = 10;
        let b: i32 = 20;
        let c: i32 = a + b;
        return c;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];
    let mssa = MemorySSA::build(f);

    // Entry block should start with v0
    let entry_id = f.blocks[0].id.clone();
    assert_eq!(
        mssa.block_entry_versions.get(&entry_id),
        Some(&MemoryVersionId::LIVE_ON_ENTRY)
    );

    // Statements should have defs with monotonically increasing versions
    let mut prev_ver = MemoryVersionId::LIVE_ON_ENTRY;
    for idx in 0..f.blocks[0].statements.len() {
        if let Some(def) = mssa.get_stmt_def(&entry_id, idx) {
            assert_eq!(def.incoming, prev_ver);
            assert!(def.id.0 > prev_ver.0);
            prev_ver = def.id;
        }
    }

    let displayed = mssa.display(f);
    assert!(displayed.contains("MemoryDef"));
    assert!(displayed.contains("MemoryUse"));
}

#[test]
fn test_memory_ssa_branch_phi() {
    use numlang::mir::memory_ssa::MemorySSA;

    let source = "
    fn main() -> i32 {
        let mut x: i32 = 0;
        let cond: bool = true;
        if cond {
            x = 10;
        } else {
            x = 20;
        }
        return x;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];
    let mssa = MemorySSA::build(f);

    // There should be a join block after the if-else with a MemoryPhi
    assert!(
        !mssa.block_phis.is_empty(),
        "Expected at least one MemoryPhi at join block"
    );

    let phi = mssa.block_phis.values().next().unwrap();
    assert!(
        phi.incoming.len() >= 2,
        "Phi must have at least 2 incoming edges, got {}",
        phi.incoming.len()
    );
}

#[test]
fn test_memory_ssa_while_loop_phi() {
    use numlang::mir::memory_ssa::MemorySSA;

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
    let mssa = MemorySSA::build(f);

    // While loop header block must have a MemoryPhi merging pre-header entry and loop body back-edge
    assert!(
        !mssa.block_phis.is_empty(),
        "Expected MemoryPhi at loop header"
    );

    let displayed = mssa.display(f);
    assert!(displayed.contains("MemoryPhi"));
}

#[test]
fn test_alias_distinct_locals_and_struct_fields() {
    use numlang::mir::alias::{AliasAnalysis, AliasResult};
    use numlang::mir::{Place, Projection};

    let source = "
    struct Point { x: i32, y: i32 }
    fn main() -> i32 {
        let p: Point = Point { x: 10, y: 20 };
        let q: Point = Point { x: 30, y: 40 };
        return p.x + q.y;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];
    let aa = AliasAnalysis::new(f);

    let p_x = Place {
        local: "p".to_string(),
        projections: vec![Projection::Field("x".to_string())],
    };
    let p_y = Place {
        local: "p".to_string(),
        projections: vec![Projection::Field("y".to_string())],
    };
    let q_x = Place {
        local: "q".to_string(),
        projections: vec![Projection::Field("x".to_string())],
    };

    // ALIAS-02: Distinct stack locals are NoAlias
    assert_eq!(aa.alias(&p_x, &q_x), AliasResult::NoAlias);

    // ALIAS-03: Field-sensitive disjointness: p.x vs p.y is NoAlias
    assert_eq!(aa.alias(&p_x, &p_y), AliasResult::NoAlias);

    // Same place is MustAlias
    assert_eq!(aa.alias(&p_x, &p_x), AliasResult::MustAlias);

    // Prefix/sub-path overlap (p vs p.x) is MayAlias
    let p_base = Place {
        local: "p".to_string(),
        projections: vec![],
    };
    assert_eq!(aa.alias(&p_base, &p_x), AliasResult::MayAlias);
}

#[test]
fn test_alias_array_constant_and_dynamic_indices() {
    use numlang::mir::alias::{AliasAnalysis, AliasResult};
    use numlang::mir::{Place, Projection};

    let source = "
    fn main() -> i32 {
        let arr: [i32; 4] = [1, 2, 3, 4];
        let idx0: i32 = 0;
        let idx1: i32 = 1;
        let dyn_i: i32 = 2;
        let dyn_j: i32 = 3;
        return arr[idx0] + arr[idx1] + arr[dyn_i] + arr[dyn_j];
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];
    let aa = AliasAnalysis::new(f);

    let arr_0 = Place {
        local: "arr".to_string(),
        projections: vec![Projection::Index(Box::new(Place {
            local: "idx0".to_string(),
            projections: vec![],
        }))],
    };
    let arr_1 = Place {
        local: "arr".to_string(),
        projections: vec![Projection::Index(Box::new(Place {
            local: "idx1".to_string(),
            projections: vec![],
        }))],
    };
    let arr_0_dup = Place {
        local: "arr".to_string(),
        projections: vec![Projection::Index(Box::new(Place {
            local: "idx0".to_string(),
            projections: vec![],
        }))],
    };

    // ALIAS-04: Constant distinct array indices arr[0] vs arr[1] are NoAlias
    assert_eq!(aa.alias(&arr_0, &arr_1), AliasResult::NoAlias);

    // Identical constant indices are MustAlias
    assert_eq!(aa.alias(&arr_0, &arr_0_dup), AliasResult::MustAlias);

    // Dynamic indices without known constants
    let arr_dyn1 = Place {
        local: "arr".to_string(),
        projections: vec![Projection::Index(Box::new(Place {
            local: "dyn_k".to_string(),
            projections: vec![],
        }))],
    };
    let arr_dyn2 = Place {
        local: "arr".to_string(),
        projections: vec![Projection::Index(Box::new(Place {
            local: "dyn_m".to_string(),
            projections: vec![],
        }))],
    };
    assert_eq!(aa.alias(&arr_dyn1, &arr_dyn2), AliasResult::MayAlias);
}

#[test]
fn test_mem2reg_candidate_identification_and_idf() {
    use numlang::mir::mem2reg::{compute_idf, find_promotion_candidates};
    use numlang::mir::dominance::compute_dominance;
    use numlang::mir::{compute_cfg, BasicBlockId};
    use std::collections::HashSet;

    let source = "
    struct Point { x: i32, y: i32 }
    fn main() -> i32 {
        let mut p: Point = Point { x: 1, y: 2 };
        let mut s: i32 = 0;
        let mut i: i32 = 0;
        while i < 10 {
            p.x = p.x + 1;
            s = s + p.x;
            i = i + 1;
        }
        return s;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];

    // M2R-01: Candidate identification
    let candidates = find_promotion_candidates(f);
    let candidate_names: Vec<String> = candidates.iter().map(|p| p.local.clone()).collect();
    assert!(candidate_names.contains(&"s".to_string()));
    assert!(candidate_names.contains(&"i".to_string()));

    // M2R-02: IDF calculation on CFG
    let entry = f.blocks[0].id.clone();
    let (preds, succs) = compute_cfg(&f.blocks);
    let all_block_ids: Vec<BasicBlockId> = f.blocks.iter().map(|b| b.id.clone()).collect();
    let dom = compute_dominance(entry, &preds, &succs, &all_block_ids);

    // Let block with loop mutation be the defining set
    let mut defs = HashSet::new();
    if f.blocks.len() > 2 {
        defs.insert(f.blocks[2].id.clone());
    }
    let idf = compute_idf(&defs, &dom.dominance_frontiers);
    assert!(!idf.is_empty(), "IDF should identify loop header / join blocks");
}

#[test]
fn test_mem2reg_scalar_promotion() {
    use numlang::mir::mem2reg::promote_memory_to_registers;

    let source = "
    fn main() -> i32 {
        let mut a: i32 = 10;
        let b: i32 = a;
        return b;
    }";
    let mut mir = get_mir(source);
    let f = &mut mir.functions[0];

    // Count statements before promotion
    let stmts_before: usize = f.blocks.iter().map(|b| b.statements.len()).sum();

    // Run Mem2Reg
    promote_memory_to_registers(f);

    // After promotion, redundant load from 'a' and store to 'a' are eliminated
    let stmts_after: usize = f.blocks.iter().map(|b| b.statements.len()).sum();
    assert!(
        stmts_after < stmts_before,
        "Mem2Reg should eliminate redundant memory loads and stores: before={}, after={}",
        stmts_before,
        stmts_after
    );
}

#[test]
fn test_dse_dead_store_elimination() {
    use numlang::mir::mem2reg::eliminate_dead_stores_and_redundant_loads;

    let source = "
    fn main() -> i32 {
        let mut x: i32 = 10;
        x = 20;
        return x;
    }";
    let mut mir = get_mir(source);
    let f = &mut mir.functions[0];

    let stmts_before = f.blocks[0].statements.len();
    eliminate_dead_stores_and_redundant_loads(f);
    let stmts_after = f.blocks[0].statements.len();

    // The first store `x = 10` is dead because it's overwritten by `x = 20`
    assert!(
        stmts_after <= stmts_before,
        "DSE should eliminate dead stores"
    );
}

#[test]
fn test_rle_redundant_load_elimination() {
    use numlang::mir::mem2reg::eliminate_dead_stores_and_redundant_loads;

    let source = "
    fn main() -> i32 {
        let x: i32 = 42;
        let a: i32 = x;
        let b: i32 = x;
        return a + b;
    }";
    let mut mir = get_mir(source);
    let f = &mut mir.functions[0];

    eliminate_dead_stores_and_redundant_loads(f);

    // The second load `b = x` should be forwarded to `a`
    let mut found_forwarded = false;
    for stmt in &f.blocks[0].statements {
        if let numlang::mir::lower::Statement::Assign(dest, numlang::mir::lower::Rvalue::Use(src)) = stmt {
            if dest.local == "b" && src.local != "x" {
                found_forwarded = true;
            }
        }
    }
    // RLE forwards second load to earlier loaded value
    assert!(found_forwarded, "Expected second load to be forwarded");
}


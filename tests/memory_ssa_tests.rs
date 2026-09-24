use std::fs;
use std::process::Command;

use numlang::mir::alias::{AliasAnalysis, AliasResult};
use numlang::mir::lower::lower_program;
use numlang::mir::mem2reg::{eliminate_dead_stores_and_redundant_loads, promote_memory_to_registers};
use numlang::mir::memory_ssa::MemorySSA;
use numlang::mir::{Place, Projection};
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
fn test_cli_emit_memory_ssa() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join(format!(
        "test_cli_emit_mssa_{}_{}.nl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let code = r#"
    fn compute(n: i32) -> i32 {
        let mut sum: i32 = 0;
        let mut i: i32 = 0;
        while i < n {
            sum = sum + i;
            i = i + 1;
        }
        return sum;
    }
    "#;

    fs::write(&test_file, code).expect("Failed to write test file");

    let bin = env!("CARGO_BIN_EXE_numlang");
    let output = Command::new(bin)
        .arg("--emit-memory-ssa")
        .arg(&test_file)
        .output()
        .expect("Failed to execute numlang with --emit-memory-ssa");

    let _ = fs::remove_file(&test_file);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("MemorySSA for function 'compute'"),
        "Missing MemorySSA header in stdout"
    );
    assert!(
        stdout.contains("MemoryDef"),
        "Missing MemoryDef annotations in stdout"
    );
    assert!(
        stdout.contains("MemoryUse"),
        "Missing MemoryUse annotations in stdout"
    );
    assert!(
        stdout.contains("MemoryPhi"),
        "Missing MemoryPhi in while loop header"
    );
}

#[test]
fn test_integration_multi_branch_memory_ssa() {
    let source = "
    fn test_branch(c1: bool, c2: bool) -> i32 {
        let mut res: i32 = 0;
        if c1 {
            if c2 {
                res = 100;
            } else {
                res = 200;
            }
        } else {
            res = 300;
        }
        return res;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];
    let mssa = MemorySSA::build(f);

    // Should have multiple join blocks with phis
    assert!(
        mssa.block_phis.len() >= 2,
        "Expected nested merge points to have MemoryPhis, found {}",
        mssa.block_phis.len()
    );

    for phi in mssa.block_phis.values() {
        assert!(
            phi.incoming.len() >= 2,
            "Each phi must merge at least 2 incoming edges"
        );
    }
}

#[test]
fn test_integration_nested_loops_memory_ssa() {
    let source = "
    fn nested_loop() -> i32 {
        let mut total: i32 = 0;
        let mut i: i32 = 0;
        while i < 10 {
            let mut j: i32 = 0;
            while j < 5 {
                total = total + (i * j);
                j = j + 1;
            }
            i = i + 1;
        }
        return total;
    }";
    let mir = get_mir(source);
    let f = &mir.functions[0];
    let mssa = MemorySSA::build(f);

    // Both inner and outer loop headers must have MemoryPhi nodes
    assert!(
        mssa.block_phis.len() >= 2,
        "Expected at least 2 MemoryPhis for nested loops"
    );
}

#[test]
fn test_integration_struct_field_alias_precision() {
    let source = "
    struct Particle { x: i32, y: i32, vx: i32, vy: i32 }
    fn update_particle() -> i32 {
        let mut p: Particle = Particle { x: 0, y: 0, vx: 1, vy: 2 };
        p.x = p.x + p.vx;
        p.y = p.y + p.vy;
        return p.x + p.y;
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
    let p_vx = Place {
        local: "p".to_string(),
        projections: vec![Projection::Field("vx".to_string())],
    };
    let p_vy = Place {
        local: "p".to_string(),
        projections: vec![Projection::Field("vy".to_string())],
    };

    // Prove pairwise disjointness between all 4 fields
    assert_eq!(aa.alias(&p_x, &p_y), AliasResult::NoAlias);
    assert_eq!(aa.alias(&p_x, &p_vx), AliasResult::NoAlias);
    assert_eq!(aa.alias(&p_x, &p_vy), AliasResult::NoAlias);
    assert_eq!(aa.alias(&p_y, &p_vx), AliasResult::NoAlias);
    assert_eq!(aa.alias(&p_y, &p_vy), AliasResult::NoAlias);
    assert_eq!(aa.alias(&p_vx, &p_vy), AliasResult::NoAlias);
}

#[test]
fn test_integration_mem2reg_full_pipeline() {
    let source = "
    fn pipeline_test() -> i32 {
        let mut a: i32 = 10;
        let mut b: i32 = 20;
        a = 30; // dead store before overwrite
        a = 40;
        let x: i32 = a;
        let y: i32 = a; // redundant load
        b = b + x + y;
        return b;
    }";
    let mut mir = get_mir(source);
    let f = &mut mir.functions[0];

    let initial_stmts: usize = f.blocks.iter().map(|b| b.statements.len()).sum();

    // 1. Run DSE / RLE
    eliminate_dead_stores_and_redundant_loads(f);

    // 2. Run Mem2Reg SSA promotion
    promote_memory_to_registers(f);

    let final_stmts: usize = f.blocks.iter().map(|b| b.statements.len()).sum();

    // Verification: statements eliminated through DSE, RLE, and Mem2Reg
    assert!(
        final_stmts < initial_stmts,
        "Pipeline should optimize away dead stores and redundant loads: before={}, after={}",
        initial_stmts,
        final_stmts
    );
}

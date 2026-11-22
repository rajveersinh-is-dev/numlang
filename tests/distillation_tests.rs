use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::distill::calculate_function_cost;
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&program).expect("Typecheck failed");
    lower_program(&typed)
}

fn compile_and_run_distill(src: &str, test_name: &str) -> (i32, String) {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes =
        compile_supercompiled_to_obj_with_mode(&typed, SupercompileMode::Distill, "size")
            .expect("Codegen with SupercompileMode::Distill failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_distill_test_{}", test_name));
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Write obj failed");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_dir(&test_dir);

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let code = output.status.code().unwrap_or(-1);
    (code, stdout)
}

fn expected_exit(code: i32) -> i32 {
    #[cfg(target_os = "windows")]
    {
        code
    }
    #[cfg(not(target_os = "windows"))]
    {
        code.rem_euclid(256)
    }
}

#[test]
fn test_append3_single_pass_zero_intermediate_allocations() {
    let code = r#"
    enum List {
        Nil,
        Cons(i64, Box<List>),
    }

    fn append(xs: List, ys: List) -> List {
        return match xs {
            Nil => ys,
            Cons(h, t) => Cons(h, box(append(deref(t), ys))),
        };
    }

    fn append3(xs: List, ys: List, zs: List) -> List {
        return append(append(xs, ys), zs);
    }

    fn sum_list(xs: List) -> i64 {
        return match xs {
            Nil => 0,
            Cons(h, t) => h + sum_list(deref(t)),
        };
    }

    fn main() -> i64 {
        let xs: List = Cons(1, box(Cons(2, box(Nil))));
        let ys: List = Cons(3, box(Nil));
        let zs: List = Cons(4, box(Cons(5, box(Nil))));
        let app: List = append3(xs, ys, zs);
        return sum_list(app);
    }
    "#;

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");

    // 1. Verify global process tree statistics
    assert!(
        stats.knots_tied > 0,
        "Distillation must tie global knots across procedural scopes"
    );
    assert!(
        stats.loops_collapsed > 0,
        "Distillation must deforest intermediate recursive structures"
    );

    // 2. Locate append3 function
    let append3_func = mir
        .functions
        .iter()
        .find(|f| f.name == "append3")
        .expect("append3 must exist in distilled MIR program");

    // 3. Verify signature: 3 parameters (xs, ys, zs)
    assert_eq!(
        append3_func.params.len(),
        3,
        "append3 must take exactly 3 arguments (xs, ys, zs)"
    );
    assert_eq!(append3_func.params[0].0, "xs");
    assert_eq!(append3_func.params[1].0, "ys");
    assert_eq!(append3_func.params[2].0, "zs");

    // 4. Verify single-pass structure with 0 intermediate allocations
    // Find the Cons arm (the recursive arm) in append3
    let mut total_alloc_count = 0;
    let mut recursive_arm_alloc_count = 0;
    let mut found_recursive_call = false;
    let mut found_call_to_append_in_cons = false;

    for block in &append3_func.blocks {
        let has_payload_read = block.statements.iter().any(|s| {
            if let Statement::Assign(_, Rvalue::Use(p)) = s {
                !p.projections.is_empty()
            } else {
                false
            }
        });

        for stmt in &block.statements {
            let Statement::Assign(_, rval) = stmt;
            match rval {
                Rvalue::Alloc(_) => {
                    total_alloc_count += 1;
                    if has_payload_read {
                        recursive_arm_alloc_count += 1;
                    }
                }
                Rvalue::Call(callee, _) => {
                    if callee == "append3" {
                        found_recursive_call = true;
                    }
                    if callee == "append" && has_payload_read {
                        found_call_to_append_in_cons = true;
                    }
                }
                _ => {}
            }
        }
    }

    assert!(
        found_recursive_call,
        "append3 must recurse directly to append3"
    );
    assert!(
        !found_call_to_append_in_cons,
        "Cons branch must NOT call append; intermediate list generation must be deforested"
    );
    assert_eq!(
        recursive_arm_alloc_count, 1,
        "Cons branch must have exactly 1 Alloc (for the output Cons cell), 0 intermediate allocations"
    );
    assert_eq!(
        total_alloc_count, 1,
        "Across entire append3 function, exactly 1 Alloc must exist (0 intermediate allocations)"
    );

    // 5. Verify execution output matches: 1 + 2 + 3 + 4 + 5 = 15
    let (code_res, _) = compile_and_run_distill(code, "append3_single_pass");
    assert_eq!(code_res, 15, "append3 result sum must be 15");
}

#[test]
fn test_distillation_callsite_in_main_rewriting() {
    let code = r#"
    enum List {
        Nil,
        Cons(i64, Box<List>),
    }

    fn append(xs: List, ys: List) -> List {
        return match xs {
            Nil => ys,
            Cons(h, t) => Cons(h, box(append(deref(t), ys))),
        };
    }

    fn sum_list(xs: List) -> i64 {
        return match xs {
            Nil => 0,
            Cons(h, t) => h + sum_list(deref(t)),
        };
    }

    fn main() -> i64 {
        let xs: List = Cons(10, box(Cons(20, box(Nil))));
        let ys: List = Cons(30, box(Nil));
        let zs: List = Cons(40, box(Nil));
        let app: List = append(append(xs, ys), zs);
        return sum_list(app);
    }
    "#;

    let mut mir = get_mir(code);
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");

    let main_func = mir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main must exist");

    // In main's blocks, verify that the nested call was rewritten to append3
    let mut append_call_count = 0;
    let mut append3_call_count = 0;

    for block in &main_func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(_, rval) = stmt;
            if let Rvalue::Call(callee, _) = rval {
                println!("Found callee: {}", callee);
                if callee == "append" {
                    append_call_count += 1;
                }
                if callee != "append" && callee != "sum_list" {
                    append3_call_count += 1;
                }
            }
        }
    }

    assert_eq!(
        append3_call_count, 1,
        "main must invoke the specialized single-pass append3 function directly"
    );
    assert_eq!(
        append_call_count, 0,
        "main must have 0 calls to append after distillation call-site rewriting"
    );

    // Verify execution output: 10 + 20 + 30 + 40 = 100
    let (code_res, _) = compile_and_run_distill(code, "callsite_in_main");
    assert_eq!(code_res, 100, "Sum of appended lists must be 100");
}

#[test]
fn test_distillation_sum_list_append_deforestation() {
    let code = r#"
    enum List {
        Nil,
        Cons(i64, Box<List>),
    }

    fn append(xs: List, ys: List) -> List {
        return match xs {
            Nil => ys,
            Cons(h, t) => Cons(h, box(append(deref(t), ys))),
        };
    }

    fn sum_list(xs: List) -> i64 {
        return match xs {
            Nil => 0,
            Cons(h, t) => h + sum_list(deref(t)),
        };
    }

    fn sum_append(xs: List, ys: List) -> i64 {
        return sum_list(append(xs, ys));
    }

    fn main() -> i64 {
        let xs: List = Cons(100, box(Cons(200, box(Nil))));
        let ys: List = Cons(300, box(Nil));
        return sum_append(xs, ys);
    }
    "#;

    let mut mir = get_mir(code);
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");

    // Locate synthesized or specialized sum_append function
    let sum_append_func = mir
        .functions
        .iter()
        .find(|f| f.name == "sum_append" || f.name == "__distill_sum_list_append")
        .expect("sum_append function must exist in distilled MIR");

    // Verify: Deforestation must eliminate ALL heap allocations (0 Alloc in sum_append)
    let mut alloc_count = 0;
    for block in &sum_append_func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(_, rval) = stmt;
            if matches!(rval, Rvalue::Alloc(_)) {
                alloc_count += 1;
            }
        }
    }

    assert_eq!(
        alloc_count, 0,
        "sum_list(append(xs, ys)) must allocate ZERO heap boxes; intermediate list is 100% deforested"
    );

    // Verify execution output: 100 + 200 + 300 = 600
    let (code_res, _) = compile_and_run_distill(code, "sum_list_append");
    assert_eq!(code_res, expected_exit(600), "Exit code must match 600");
}

#[test]
fn test_distillation_tree_invert_deforestation() {
    let code = r#"
    enum Tree {
        Leaf(i64),
        Node(Box<Tree>, Box<Tree>),
    }

    fn invert(t: Tree) -> Tree {
        return match t {
            Leaf(v) => Leaf(v),
            Node(l, r) => Node(box(invert(deref(r))), box(invert(deref(l)))),
        };
    }

    fn sum_tree(t: Tree) -> i64 {
        return match t {
            Leaf(v) => v,
            Node(l, r) => sum_tree(deref(l)) + sum_tree(deref(r)),
        };
    }

    fn invert2(t: Tree) -> Tree {
        return invert(invert(t));
    }

    fn main() -> i64 {
        let t: Tree = Node(box(Leaf(50)), box(Leaf(70)));
        let inv2: Tree = invert2(t);
        return sum_tree(inv2);
    }
    "#;

    let mut mir = get_mir(code);
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");

    let inv2_func = mir
        .functions
        .iter()
        .find(|f| f.name == "invert2" || f.name == "__distill_invert_invert")
        .expect("invert2 function must exist in distilled MIR");

    // Invert2 should have exactly 2 Alloc statements (for the final left and right children),
    // and ZERO calls to the original invert function in its inductive block.
    let mut alloc_count = 0;
    let mut call_to_invert_count = 0;
    for block in &inv2_func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(_, rval) = stmt;
            if matches!(rval, Rvalue::Alloc(_)) {
                alloc_count += 1;
            }
            if let Rvalue::Call(callee, _) = rval {
                if callee == "invert" {
                    call_to_invert_count += 1;
                }
            }
        }
    }

    assert_eq!(
        call_to_invert_count, 0,
        "invert2 must not call invert; intermediate tree must be completely deforested"
    );
    assert!(
        alloc_count <= 2,
        "invert2 allocates at most 2 boxes (0 if identity-optimized, 2 if reconstructed), got {}",
        alloc_count
    );

    // Verify execution output: 50 + 70 = 120
    let (code_res, _) = compile_and_run_distill(code, "tree_invert2");
    assert_eq!(code_res, 120, "Double invert of [50, 70] produces sum 120");
}

#[test]
fn test_distillation_chained_append() {
    let code = r#"
    enum List {
        Nil,
        Cons(i64, Box<List>),
    }

    fn append(xs: List, ys: List) -> List {
        return match xs {
            Nil => ys,
            Cons(h, t) => Cons(h, box(append(deref(t), ys))),
        };
    }

    fn sum_list(xs: List) -> i64 {
        return match xs {
            Nil => 0,
            Cons(h, t) => h + sum_list(deref(t)),
        };
    }

    fn main() -> i64 {
        let a: List = Cons(1, box(Nil));
        let b: List = Cons(2, box(Nil));
        let c: List = Cons(3, box(Nil));
        let d: List = Cons(4, box(Nil));
        // Chained 4-list append: append(append(append(a, b), c), d)
        let ab: List = append(a, b);
        let abc: List = append(ab, c);
        let abcd: List = append(abc, d);
        return sum_list(abcd);
    }
    "#;

    let (code_res, _) = compile_and_run_distill(code, "chained_append");
    assert_eq!(code_res, 10, "1 + 2 + 3 + 4 = 10");
}

// ---------------------------------------------------------------------------
// General (structural) distillation metamorphic & new-shape coverage
// ---------------------------------------------------------------------------

fn total_cost(mir: &MirProgram, names: &[&str]) -> usize {
    names
        .iter()
        .map(|n| {
            mir.functions
                .iter()
                .find(|f| &f.name == n)
                .map(calculate_function_cost)
                .unwrap_or(0)
        })
        .sum()
}

fn count_calls_to(func: &numlang::mir::lower::MirFunction, callee: &str) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| b.statements.iter())
        .filter_map(|s| match s {
            Statement::Assign(_, Rvalue::Call(c, _)) if c == callee => Some(1),
            _ => None,
        })
        .sum()
}

fn count_calls_in_inductive_branch(func: &numlang::mir::lower::MirFunction, callee: &str) -> usize {
    func.blocks
        .iter()
        .filter(|b| {
            b.statements.iter().any(|s| match s {
                Statement::Assign(_, Rvalue::Use(p)) => !p.projections.is_empty(),
                _ => false,
            })
        })
        .flat_map(|b| b.statements.iter())
        .filter_map(|s| match s {
            Statement::Assign(_, Rvalue::Call(c, _)) if c == callee => Some(1),
            _ => None,
        })
        .sum()
}

fn count_allocation_sites(func: &numlang::mir::lower::MirFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| b.statements.iter())
        .filter(|s| matches!(s, Statement::Assign(_, Rvalue::Alloc(_))))
        .count()
}

#[test]
fn test_distillation_metamorphic_rename_and_field_layout() {
    // Every function/variable is renamed AND the recursive enum's field order
    // is swapped (tail is payload 0, head is payload 1). The general structural
    // pathway must still fuse the producer/consumer composition. A hard-coded
    // "head=payload(0), tail=payload(1)" template would produce wrong code here.
    let code = r#"
    enum Sequence {
        Empty,
        Item(Box<Sequence>, i64),
    }

    fn glue(first: Sequence, second: Sequence) -> Sequence {
        return match first {
            Empty => second,
            Item(rest, value) => Item(box(glue(deref(rest), second)), value),
        };
    }

    fn accumulate(items: Sequence) -> i64 {
        return match items {
            Empty => 0,
            Item(rest, value) => value + accumulate(deref(rest)),
        };
    }

    fn combine(a: Sequence, b: Sequence) -> i64 {
        return accumulate(glue(a, b));
    }

    fn main() -> i64 {
        let p: Sequence = Item(box(Item(box(Empty), 8)), 7);
        let q: Sequence = Item(box(Empty), 9);
        return combine(p, q);
    }
    "#;

    let baseline_cost;
    {
        let baseline = get_mir(code);
        baseline_cost = total_cost(&baseline, &["glue", "accumulate"]);
    }

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(
        stats.knots_tied > 0,
        "General distillation must tie a recursive knot: {:?}",
        stats
    );

    let fused = mir
        .functions
        .iter()
        .find(|f| f.name == "combine")
        .expect("specialized combine must exist");
    assert_eq!(
        count_calls_to(fused, "glue"),
        0,
        "Producer calls must be deforested out of the inductive branch"
    );
    assert_eq!(
        count_allocation_sites(fused),
        0,
        "Intermediate sequence must be fully deforested (0 heap allocations)"
    );
    assert!(
        calculate_function_cost(fused) <= baseline_cost,
        "Residual cost {} must not exceed baseline {}",
        calculate_function_cost(fused),
        baseline_cost
    );

    let (code_res, _) = compile_and_run_distill(code, "metamorphic_rename_layout");
    assert_eq!(code_res, 24, "7 + 8 + 9 must equal 24");
}

#[test]
fn test_distillation_tree_map_fold_composition() {
    // NEW SHAPE (outside the three historical templates): a tree *mapper*
    // composed with a tree *folder*. The general pathway must fuse them into
    // a single pass that allocates zero intermediate trees.
    let code = r#"
    enum Tree {
        Tip(i64),
        Fork(Box<Tree>, Box<Tree>),
    }

    fn mirror(t: Tree) -> Tree {
        return match t {
            Tip(v) => Tip(v),
            Fork(l, r) => Fork(box(mirror(deref(r))), box(mirror(deref(l)))),
        };
    }

    fn fold(t: Tree) -> i64 {
        return match t {
            Tip(v) => v,
            Fork(l, r) => fold(deref(l)) + fold(deref(r)),
        };
    }

    fn mirror_fold(t: Tree) -> i64 {
        return fold(mirror(t));
    }

    fn main() -> i64 {
        let t: Tree = Fork(box(Tip(3)), box(Fork(box(Tip(4)), box(Tip(5)))));
        return mirror_fold(t);
    }
    "#;

    let baseline_cost;
    {
        let baseline = get_mir(code);
        baseline_cost = total_cost(&baseline, &["mirror", "fold"]);
    }

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(
        stats.knots_tied > 0,
        "Tree map/fold fusion must generalise the recurrence: {:?}",
        stats
    );

    let fused = mir
        .functions
        .iter()
        .find(|f| f.name == "mirror_fold")
        .expect("specialized mirror_fold must exist");
    assert_eq!(
        count_calls_to(fused, "mirror"),
        0,
        "No producer calls may remain"
    );
    assert_eq!(
        count_calls_to(fused, "fold"),
        0,
        "No consumer calls may remain"
    );
    assert_eq!(
        count_allocation_sites(fused),
        0,
        "Intermediate mirrored trees must be fully deforested"
    );
    assert!(
        calculate_function_cost(fused) <= baseline_cost,
        "Residual cost {} must not exceed baseline {}",
        calculate_function_cost(fused),
        baseline_cost
    );

    let (code_res, _) = compile_and_run_distill(code, "tree_map_fold");
    assert_eq!(code_res, 12, "3 + 4 + 5 must equal 12");
}

#[test]
fn test_distillation_three_function_chain() {
    // NEW SHAPE: a three-function producer/consumer chain
    // bump -> tack -> tally, exposed through two type-compatible wrappers.
    let code = r#"
    enum Chain {
        Stop,
        More(i64, Box<Chain>),
    }

    fn bump(xs: Chain) -> Chain {
        return match xs {
            Stop => Stop,
            More(h, t) => More(h + 1, box(bump(deref(t)))),
        };
    }

    fn tack(xs: Chain, tail: Chain) -> Chain {
        return match xs {
            Stop => tail,
            More(h, t) => More(h, box(tack(deref(t), tail))),
        };
    }

    fn tally(xs: Chain) -> i64 {
        return match xs {
            Stop => 0,
            More(h, t) => h + tally(deref(t)),
        };
    }

    fn bump_tack(xs: Chain, tail: Chain) -> Chain {
        return tack(bump(xs), tail);
    }

    fn pipeline(xs: Chain, tail: Chain) -> i64 {
        return tally(bump_tack(xs, tail));
    }

    fn main() -> i64 {
        let xs: Chain = More(2, box(More(4, box(Stop))));
        let tail: Chain = More(6, box(Stop));
        return pipeline(xs, tail);
    }
    "#;

    let baseline_cost;
    {
        let baseline = get_mir(code);
        baseline_cost = total_cost(&baseline, &["bump", "tack", "tally"]);
    }

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(
        stats.knots_tied >= 2,
        "Both levels of the 3-function chain must be fused: {:?}",
        stats
    );

    let pipeline = mir
        .functions
        .iter()
        .find(|f| f.name == "pipeline")
        .expect("specialized pipeline must exist");
    // The base case may call the consumer on the remaining tail; the
    // *inductive* branch (with payload reads) must not call either original.
    assert_eq!(count_calls_in_inductive_branch(pipeline, "tally"), 0);
    assert_eq!(count_calls_in_inductive_branch(pipeline, "bump_tack"), 0);
    assert_eq!(
        count_allocation_sites(pipeline),
        0,
        "Fully fused pipeline must not allocate intermediate chains"
    );
    assert!(
        calculate_function_cost(pipeline) <= baseline_cost,
        "Residual cost {} must not exceed baseline {}",
        calculate_function_cost(pipeline),
        baseline_cost
    );

    let (code_res, _) = compile_and_run_distill(code, "three_function_chain");
    assert_eq!(code_res, 14, "(2+1) + (4+1) + 6 = 14");
}


// ---------------------------------------------------------------------------
// Benchmark-shaped pipelines: the standard optimiser inlines direct calls
// before MIR lowering, so the composition must be recovered from the inlined
// switch loops rather than from nested Rvalue::Call statements.
// ---------------------------------------------------------------------------

fn get_pipeline_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&ast).expect("Typecheck failed");
    numlang::compiler::distill_and_optimize(
        &mut typed,
        &numlang::compiler::CompilerConfig {
            supercompile: true,
            ..Default::default()
        },
    );
    lower_program(&typed)
}

fn count_calls_in(func: &numlang::mir::lower::MirFunction, callee: &str) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| b.statements.iter())
        .filter_map(|s| match s {
            Statement::Assign(_, Rvalue::Call(c, _)) if c == callee => Some(1),
            _ => None,
        })
        .sum()
}

#[test]
fn test_distillation_fires_on_renamed_inlined_list_composition() {
    // Generic two-function producer/consumer composition. Nothing is named
    // after a benchmark; the producer/consumer roles are discovered from the
    // inlined loop dataflow (a switch whose arms all write a local consumed by
    // a second switch).
    let code = r#"
    enum Chain {
        End,
        Link(Box<Chain>, i64),
    }

    fn attach(first: Chain, second: Chain) -> Chain {
        return match first {
            End => second,
            Link(rest, value) => Link(box(attach(deref(rest), second)), value),
        };
    }

    fn total(items: Chain) -> i64 {
        return match items {
            End => 0,
            Link(rest, value) => value + total(deref(rest)),
        };
    }

    fn combined(a: Chain, b: Chain) -> i64 {
        return total(attach(a, b));
    }

    fn main() -> i64 {
        let p: Chain = Link(box(Link(box(End), 8)), 7);
        let q: Chain = Link(box(End), 9);
        return combined(p, q);
    }
    "#;

    let baseline_cost;
    {
        let baseline = get_mir(code);
        baseline_cost = total_cost(&baseline, &["attach", "total"]);
    }

    let mut mir = get_pipeline_mir(code);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(
        stats.knots_tied > 0,
        "Inlined list composition must tie a knot: {:?}",
        stats
    );

    let fused = mir
        .functions
        .iter()
        .find(|f| f.is_distilled && f.name.starts_with("__distill_"))
        .expect("a distilled fused function must be synthesised");
    assert_eq!(
        count_calls_to(fused, "attach"),
        0,
        "Producer calls must be eliminated from the fused residual"
    );
    assert_eq!(
        count_allocation_sites(fused),
        0,
        "Intermediate list nodes must be fully deforested"
    );
    assert!(
        calculate_function_cost(fused) <= baseline_cost,
        "Residual cost {} must not exceed baseline {}",
        calculate_function_cost(fused),
        baseline_cost
    );

    let (code_res, _) = compile_and_run_distill(code, "renamed_inlined_composition");
    assert_eq!(code_res, 24, "7 + 8 + 9 must equal 24");
}

#[test]
fn test_distillation_fires_on_nested_inlined_composition() {
    // A nested composition g(g(x)) over a list. The producer's recursive arm
    // ends in a call (not a constructor), so fusion requires the involution
    // pathway: bounded symbolic evaluation proves g(g(x)) == x and residualises
    // a single-pass copy.
    let code = r#"
    enum Bag {
        Nothing,
        Item(i64, Box<Bag>),
    }

    fn join(xs: Bag, ys: Bag) -> Bag {
        return match xs {
            Nothing => ys,
            Item(v, rest) => Item(v, box(join(deref(rest), ys))),
        };
    }

    fn turn(xs: Bag) -> Bag {
        return match xs {
            Nothing => Nothing,
            Item(v, rest) => join(turn(deref(rest)), Item(v, box(Nothing))),
        };
    }

    fn add_up(xs: Bag) -> i64 {
        return match xs {
            Nothing => 0,
            Item(v, rest) => v + add_up(deref(rest)),
        };
    }

    fn twice(xs: Bag) -> Bag {
        return turn(turn(xs));
    }

    fn main() -> i64 {
        let xs: Bag = Item(4, box(Item(5, box(Nothing))));
        return add_up(twice(xs));
    }
    "#;

    let baseline_cost;
    {
        let baseline = get_mir(code);
        baseline_cost = total_cost(&baseline, &["turn", "join"]);
    }

    let mut mir = get_pipeline_mir(code);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(
        stats.knots_tied > 0,
        "Nested inlined composition must tie a knot: {:?}",
        stats
    );

    let fused = mir
        .functions
        .iter()
        .find(|f| f.is_distilled && f.name.starts_with("__distill_"))
        .expect("nested composition must produce a distilled function");
    assert!(
        calculate_function_cost(fused) <= baseline_cost,
        "Residual cost {} must not exceed baseline {}",
        calculate_function_cost(fused),
        baseline_cost
    );
    let caller = mir
        .functions
        .iter()
        .find(|f| f.name == "twice")
        .expect("caller must exist");
    assert_eq!(
        count_calls_in(caller, "turn"),
        0,
        "Inlined producer calls must be replaced by the fused residual"
    );

    let (code_res, _) = compile_and_run_distill(code, "nested_inlined_composition");
    assert_eq!(code_res, 9, "double turn must preserve 4 + 5 = 9");
}

#[test]
fn test_distillation_generic_involution_identity() {
    let code = r#"
    enum Bundle {
        Atom(i64),
        Pair(Box<Bundle>, Box<Bundle>),
    }

    fn twist(t: Bundle) -> Bundle {
        return match t {
            Atom(v) => Atom(v),
            Pair(l, r) => Pair(box(twist(deref(r))), box(twist(deref(l)))),
        };
    }

    fn measure(t: Bundle) -> i64 {
        return match t {
            Atom(v) => v,
            Pair(l, r) => measure(deref(l)) + measure(deref(r)),
        };
    }

    fn twist_twice(t: Bundle) -> Bundle {
        return twist(twist(t));
    }

    fn main() -> i64 {
        let t: Bundle = Pair(box(Atom(3)), box(Pair(box(Atom(4)), box(Atom(5)))));
        let t2: Bundle = twist_twice(t);
        return measure(t2);
    }
    "#;

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(
        stats.knots_tied > 0 || stats.loops_collapsed > 0,
        "Generic involution identity must be detected: {:?}",
        stats
    );

    let fused = mir
        .functions
        .iter()
        .find(|f| f.name == "twist_twice" || f.name == "__distill_twist_twist")
        .expect("twist_twice function must exist in distilled MIR");

    let mut alloc_count = 0;
    let mut call_to_twist = 0;
    for block in &fused.blocks {
        for stmt in &block.statements {
            let Statement::Assign(_, rval) = stmt;
            if matches!(rval, Rvalue::Alloc(_)) {
                alloc_count += 1;
            }
            if let Rvalue::Call(callee, _) = rval {
                if callee == "twist" {
                    call_to_twist += 1;
                }
            }
        }
    }

    assert_eq!(
        call_to_twist, 0,
        "twist_twice must not call twist; involution must be eliminated"
    );
    assert!(
        alloc_count <= 2,
        "twist_twice should be identity (0 allocs) or at most reconstruct (2 allocs), got {}",
        alloc_count
    );

    let (code_res, _) = compile_and_run_distill(code, "generic_involution_identity");
    assert_eq!(code_res, 12, "3 + 4 + 5 must equal 12");
}


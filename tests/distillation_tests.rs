use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
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

    let obj_bytes = compile_supercompiled_to_obj_with_mode(&typed, SupercompileMode::Distill, "size")
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
    assert!(stats.knots_tied > 0, "Distillation must tie global knots across procedural scopes");
    assert!(stats.loops_collapsed > 0, "Distillation must deforest intermediate recursive structures");

    // 2. Locate append3 function
    let append3_func = mir
        .functions
        .iter()
        .find(|f| f.name == "append3")
        .expect("append3 must exist in distilled MIR program");

    // 3. Verify signature: 3 parameters (xs, ys, zs)
    assert_eq!(append3_func.params.len(), 3, "append3 must take exactly 3 arguments (xs, ys, zs)");
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

    assert!(found_recursive_call, "append3 must recurse directly to append3");
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
                if callee == "append" {
                    append_call_count += 1;
                }
                if callee == "append3" {
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
    assert_eq!(code_res, 600, "Exit code must match 600");
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
    assert_eq!(
        alloc_count, 2,
        "invert2 allocates only 2 boxes for the final tree node, 0 intermediate boxes"
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

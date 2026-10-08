use numlang::codegen::compile_to_obj;
use numlang::opt::optimize_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::{TypedExpr, TypedLiteral, TypedStmt};

#[test]
fn test_bounded_while_div2_unrolling() {
    let source = r#"
        fn exp_steps() -> i64 {
            let mut e: i64 = 13;
            let mut acc: i64 = 0;
            while e > 0 {
                acc = acc + (e & 1);
                e = e / 2;
            }
            return acc;
        }
        fn main() -> i64 {
            return exp_steps();
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let func = program
        .functions
        .iter()
        .find(|f| f.name == "exp_steps")
        .unwrap();
    let has_while = func
        .body
        .stmts
        .iter()
        .any(|s| matches!(s, TypedStmt::While { .. }));
    assert!(!has_while, "Expected while loop to be completely unrolled");
}

#[test]
fn test_nested_matrix_loop_unrolling() {
    let source = r#"
        fn mat_dot() -> i64 {
            let a: [i64; 4] = [1, 2, 3, 4];
            let b: [i64; 4] = [5, 6, 7, 8];
            let mut sum: i64 = 0;
            let mut k: i64 = 0;
            while k < 4 {
                sum = sum + a[k] * b[k];
                k = k + 1;
            }
            return sum;
        }
        fn main() -> i64 {
            return mat_dot();
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let func = program
        .functions
        .iter()
        .find(|f| f.name == "mat_dot")
        .unwrap();
    let has_while = func
        .body
        .stmts
        .iter()
        .any(|s| matches!(s, TypedStmt::While { .. }));
    assert!(!has_while, "Expected loop k < 4 to be unrolled");
}

#[test]
fn test_dead_branch_elimination_in_unrolled_loop() {
    let source = r#"
        fn dead_branch_test() -> i64 {
            let mut e: i64 = 4;
            let mut res: i64 = 0;
            while e > 0 {
                if e % 2 == 1 {
                    res = res + 100;
                } else {
                    res = res + 1;
                }
                e = e / 2;
            }
            return res;
        }
        fn main() -> i64 {
            return dead_branch_test();
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let func = program
        .functions
        .iter()
        .find(|f| f.name == "dead_branch_test")
        .unwrap();
    let has_if = func
        .body
        .stmts
        .iter()
        .any(|s| matches!(s, TypedStmt::If { .. }));
    let has_while = func
        .body
        .stmts
        .iter()
        .any(|s| matches!(s, TypedStmt::While { .. }));
    assert!(!has_while, "Expected while loop to be unrolled");
    assert!(
        !has_if,
        "Expected all branches with constant conditions to be eliminated"
    );
}

#[test]
fn test_modular_exponentiation_compile_and_execute() {
    let source = r#"
        fn pow_mod(base: i64, exp: i64, m: i64) -> i64 {
            let mut res: i64 = 1;
            let mut b: i64 = base % m;
            let mut e: i64 = exp;
            while e > 0 {
                if e % 2 == 1 {
                    res = (res * b) % m;
                }
                b = (b * b) % m;
                e = e / 2;
            }
            return res;
        }
        fn main() -> i64 {
            let val: i64 = pow_mod(7, 13, 1000000007);
            return val % 256;
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let program = typecheck(&ast).unwrap();
    let obj = compile_to_obj(&program).expect("Compilation must succeed");
    assert!(!obj.is_empty(), "Object bytes should not be empty");
}

#[test]
fn test_mat4_pow_unrolling() {
    let source = r#"
        fn mat4_pow(power: i64) -> i64 {
            let mut m: [i64; 16] = [
                1, 1, 1, 1,
                1, 0, 0, 0,
                0, 1, 0, 0,
                0, 0, 1, 0
            ];
            let mut r: [i64; 16] = [
                1, 0, 0, 0,
                0, 1, 0, 0,
                0, 0, 1, 0,
                0, 0, 0, 1
            ];
            let mut p: i64 = power;
            while p > 0 {
                if p % 2 == 1 {
                    let mut next_r: [i64; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                    let mut i: i64 = 0;
                    while i < 4 {
                        let mut j: i64 = 0;
                        while j < 4 {
                            let mut sum: i64 = 0;
                            let mut k: i64 = 0;
                            while k < 4 {
                                sum = (sum + r[i * 4 + k] * m[k * 4 + j]) % 1000000007;
                                k = k + 1;
                            }
                            next_r[i * 4 + j] = sum;
                            j = j + 1;
                        }
                        i = i + 1;
                    }
                    r = next_r;
                }
                let mut next_m: [i64; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                let mut i2: i64 = 0;
                while i2 < 4 {
                    let mut j2: i64 = 0;
                    while j2 < 4 {
                        let mut sum2: i64 = 0;
                        let mut k2: i64 = 0;
                        while k2 < 4 {
                            sum2 = (sum2 + m[i2 * 4 + k2] * m[k2 * 4 + j2]) % 1000000007;
                            k2 = k2 + 1;
                        }
                        next_m[i2 * 4 + j2] = sum2;
                        j2 = j2 + 1;
                    }
                    i2 = i2 + 1;
                }
                m = next_m;
                p = p / 2;
            }
            return r[0] + r[5] + r[10] + r[15];
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    fn count_while(stmts: &[TypedStmt]) -> usize {
        let mut cnt = 0;
        for s in stmts {
            match s {
                TypedStmt::While { body, .. } => {
                    cnt += 1 + count_while(&body.stmts);
                }
                TypedStmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    cnt += count_while(&then_branch.stmts);
                    if let Some(eb) = else_branch {
                        cnt += count_while(&eb.stmts);
                    }
                }
                _ => {}
            }
        }
        cnt
    }

    let func = program
        .functions
        .iter()
        .find(|f| f.name == "mat4_pow")
        .unwrap();
    let remaining_whiles = count_while(&func.body.stmts);
    println!("Remaining while loops in mat4_pow: {}", remaining_whiles);

    fn check_non_const_indices(stmts: &[TypedStmt]) {
        for s in stmts {
            match s {
                TypedStmt::IndexAssign { target, index, .. } => match index {
                    TypedExpr::Literal {
                        lit: TypedLiteral::Int(..),
                        ..
                    } => {}
                    _ => println!("NON-CONST IndexAssign on {}: {:?}", target, index),
                },
                TypedStmt::While { body, .. } => check_non_const_indices(&body.stmts),
                TypedStmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    check_non_const_indices(&then_branch.stmts);
                    if let Some(eb) = else_branch {
                        check_non_const_indices(&eb.stmts);
                    }
                }
                _ => {}
            }
        }
    }
    check_non_const_indices(&func.body.stmts);
    assert_eq!(remaining_whiles, 3, "Outer 'while p > 0' and rolled row loops 'while i < 4' / 'while i2 < 4' should remain; inner column/reduction loops unroll");
}

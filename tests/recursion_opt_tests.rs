use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::opt::optimize_program;
use std::fs;
use std::process::Command;

fn expected_fib(n: i64) -> i32 {
    let mut a = 0i64;
    let mut b = 1i64;
    for _ in 0..n {
        let t = a + b;
        a = b;
        b = t;
    }
    (a % 256) as i32
}

#[test]
fn test_recursion_opt_ast_transform() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 {
                return n;
            } else {
                return fib(n - 1) + fib(n - 2);
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let mut typed = typecheck(&ast).unwrap();

    assert_eq!(typed.functions[0].body.stmts.len(), 1);

    optimize_program(&mut typed);

    // After optimization, fib has 20 statements: base checks for <=1..=19 and the unrolled recursive return
    assert_eq!(typed.functions[0].body.stmts.len(), 20);
}

#[test]
fn test_recursion_opt_execution_values() {
    let test_dir = std::env::temp_dir().join("numlang_rec_opt_tests");
    fs::create_dir_all(&test_dir).unwrap();

    for n in [0, 1, 2, 3, 4, 5, 10, 20, 25] {
        let src = format!(
            r#"
            fn fib(n: i64) -> i64 {{
                if n <= 1 {{
                    return n;
                }} else {{
                    return fib(n - 1) + fib(n - 2);
                }}
            }}

            fn main() -> i64 {{
                let res: i64 = fib({});
                return res % 256;
            }}
            "#,
            n
        );

        let src_file = test_dir.join(format!("fib_{}.nl", n));
        let exe_file = test_dir.join(format!("fib_{}.exe", n));
        fs::write(&src_file, &src).unwrap();

        let numlang_bin = env!("CARGO_BIN_EXE_numlang");
        let build_output = Command::new(numlang_bin)
            .arg("build")
            .arg(&src_file)
            .arg("-o")
            .arg(&exe_file)
            .output()
            .expect("Failed to build numlang binary");

        assert!(build_output.status.success());

        let run_output = Command::new(&exe_file)
            .output()
            .expect("Failed to run binary");

        let code = run_output.status.code().unwrap();
        let expected = expected_fib(n);
        assert_eq!(
            code, expected,
            "fib({}) failed: expected {}, got {}",
            n, expected, code
        );
    }
}

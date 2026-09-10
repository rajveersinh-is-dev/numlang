use numlang::codegen::compile_to_obj;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

#[test]
fn test_host_cpu_feature_compilation() {
    let src = r#"
        fn compute(a: f64, b: f64) -> f64 {
            let mut x: f64 = a * b + 1.0;
            return x;
        }

        fn main() -> i64 {
            let res: f64 = compute(2.5, 4.0);
            if res > 10.0 {
                return 42;
            } else {
                return 0;
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let obj_bytes = compile_to_obj(&typed).unwrap();

    assert!(!obj_bytes.is_empty());
    // Verify Windows COFF header
    assert_eq!(obj_bytes[0], 0x64);
    assert_eq!(obj_bytes[1], 0x86);
}

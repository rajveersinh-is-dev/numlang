use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> (Option<i32>, String, String) {
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("numlang_enum_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_unit_variants_matching() {
    let code = r#"
enum Color {
    Red,
    Green,
    Blue,
}

fn to_code(c: Color) -> i64 {
    return match c {
        Color::Red => 10,
        Color::Green => 20,
        Color::Blue => 30,
    };
}

fn main() -> i64 {
    let c: Color = Color::Green;
    return to_code(c);
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(20));
}

#[test]
fn test_unqualified_variant_matching_and_constructors() {
    let code = r#"
enum TrafficLight {
    Stop,
    Caution,
    Go,
}

fn main() -> i64 {
    let light: TrafficLight = Caution;
    let code: i64 = match light {
        Stop => 1,
        Caution => 2,
        Go => 3,
    };
    return code;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(2));
}

#[test]
fn test_single_payload_variant_option() {
    let code = r#"
enum Option {
    Some(i64),
    None,
}

fn unwrap_or_zero(opt: Option) -> i64 {
    return match opt {
        Option::Some(x) => x,
        Option::None => 0,
    };
}

fn main() -> i64 {
    let opt: Option = Option::Some(42);
    return unwrap_or_zero(opt);
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(42));
}

#[test]
fn test_none_variant_matching() {
    let code = r#"
enum Option {
    Some(i64),
    None,
}

fn main() -> i64 {
    let opt: Option = None;
    let res: i64 = match opt {
        Some(x) => x,
        None => 99,
    };
    return res;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(99));
}

#[test]
fn test_multiple_payload_variants() {
    let code = r#"
enum Shape {
    Circle(i64),
    Rect(i64, i64),
}

fn area(s: Shape) -> i64 {
    return match s {
        Circle(r) => r * r * 3,
        Rect(w, h) => w * h,
    };
}

fn main() -> i64 {
    let r: Shape = Rect(7, 6);
    return area(r);
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(42));
}

#[test]
fn test_returning_enum_from_function() {
    let code = r#"
enum Result {
    Ok(i64),
    Err(i64),
}

fn divide(a: i64, b: i64) -> Result {
    if b == 0 {
        return Result::Err(-1);
    }
    return Result::Ok(a / b);
}

fn main() -> i64 {
    let r: Result = divide(100, 4);
    return match r {
        Result::Ok(val) => val,
        Result::Err(err) => err,
    };
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(25));
}

#[test]
fn test_returning_enum_err_from_function() {
    let code = r#"
enum Result {
    Ok(i64),
    Err(i64),
}

fn divide(a: i64, b: i64) -> Result {
    if b == 0 {
        return Result::Err(77);
    }
    return Result::Ok(a / b);
}

fn main() -> i64 {
    let r: Result = divide(100, 0);
    return match r {
        Result::Ok(val) => val,
        Result::Err(err) => err,
    };
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(77));
}

#[test]
fn test_nested_enums() {
    let code = r#"
enum Inner {
    Val(i64),
    Empty,
}

enum Outer {
    Nest(Inner),
    None,
}

fn main() -> i64 {
    let inner: Inner = Inner::Val(123);
    let outer: Outer = Outer::Nest(inner);

    return match outer {
        Nest(i) => match i {
            Val(v) => v,
            Empty => 0,
        },
        None => -1,
    };
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(123));
}

#[test]
fn test_enum_state_machine_loop() {
    let code = r#"
enum State {
    Step(i64),
    Done(i64),
}

fn main() -> i64 {
    let mut state: State = Step(0);
    let mut sum: i64 = 0;

    let mut running: bool = true;
    while running {
        let n: i64 = match state {
            Step(k) => k,
            Done(k) => -1,
        };
        if n >= 10 {
            state = Done(n);
            running = false;
        } else {
            if n >= 0 {
                sum = sum + n;
                state = Step(n + 1);
            } else {
                running = false;
            }
        }
    }

    return sum;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    // 0 + 1 + 2 + ... + 9 = 45
    assert_eq!(exit_code, Some(45));
}

#[test]
fn test_enum_typecheck_errors() {
    let code_bad_arity = r#"
enum Option {
    Some(i64),
    None,
}

fn main() -> i64 {
    let opt: Option = Option::Some(1, 2);
    return 0;
}
"#;
    let (exit_code, _stdout, stderr) = run_numlang_code(code_bad_arity);
    assert_ne!(exit_code, Some(0));
    assert!(stderr.contains("E021") || stderr.contains("Payload") || stderr.contains("arguments"));

    let code_unknown_variant = r#"
enum Option {
    Some(i64),
    None,
}

fn main() -> i64 {
    let opt: Option = Option::Missing;
    return 0;
}
"#;
    let (exit_code, _stdout, stderr) = run_numlang_code(code_unknown_variant);
    assert_ne!(exit_code, Some(0));
    assert!(stderr.contains("E020") || stderr.contains("variant") || stderr.contains("Missing"));
}

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
    let test_dir = std::env::temp_dir().join(format!("numlang_match_{}", id));
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
fn test_match_integer_basic() {
    let code = r#"
fn classify(x: i64) -> i64 {
    return match x {
        0 => 100,
        1 => 200,
        2 => 300,
        _ => -1,
    };
}

fn main() -> i64 {
    let a: i64 = classify(0);
    let b: i64 = classify(1);
    let c: i64 = classify(2);
    let d: i64 = classify(42);
    if a != 100 {
        return 1;
    }
    if b != 200 {
        return 2;
    }
    if c != 300 {
        return 3;
    }
    if d != -1 {
        return 4;
    }
    return 0;
}
"#;
    let (code, _, stderr) = run_numlang_code(code);
    assert_eq!(code, Some(0), "Stderr: {}", stderr);
}

#[test]
fn test_match_or_patterns() {
    let code = r#"
fn categorize(x: i64) -> i64 {
    return match x {
        1 | 2 | 3 => 10,
        4 | 5 => 20,
        6 => 30,
        _ => 0,
    };
}

fn main() -> i64 {
    if categorize(1) != 10 {
        return 1;
    }
    if categorize(2) != 10 {
        return 2;
    }
    if categorize(3) != 10 {
        return 3;
    }
    if categorize(4) != 20 {
        return 4;
    }
    if categorize(5) != 20 {
        return 5;
    }
    if categorize(6) != 30 {
        return 6;
    }
    if categorize(99) != 0 {
        return 7;
    }
    return 0;
}
"#;
    let (code, _, stderr) = run_numlang_code(code);
    assert_eq!(code, Some(0), "Stderr: {}", stderr);
}

#[test]
fn test_match_as_expression_assignment() {
    let code = r#"
fn main() -> i64 {
    let x: i64 = 5;
    let mut result: i64 = match x {
        5 => 50,
        _ => 0,
    };
    if result != 50 {
        return 1;
    }
    let mut y: i64 = 10;
    result = match y {
        10 => 100,
        _ => 0,
    };
    if result == 100 {
        return 0;
    }
    return 2;
}
"#;
    let (code, _, stderr) = run_numlang_code(code);
    assert_eq!(code, Some(0), "Stderr: {}", stderr);
}

#[test]
fn test_match_wildcard_catch_unmatched() {
    let code = r#"
fn main() -> i64 {
    let x: i64 = 777;
    let val: i64 = match x {
        1 => 1,
        2 => 2,
        _ => 42,
    };
    if val == 42 {
        return 0;
    }
    return 1;
}
"#;
    let (code, _, stderr) = run_numlang_code(code);
    assert_eq!(code, Some(0), "Stderr: {}", stderr);
}

#[test]
fn test_match_boolean() {
    let code = r#"
fn bool_to_int(b: bool) -> i64 {
    return match b {
        true => 1,
        false => 0,
    };
}

fn main() -> i64 {
    let t: i64 = bool_to_int(true);
    let f: i64 = bool_to_int(false);
    if t != 1 {
        return 1;
    }
    if f != 0 {
        return 2;
    }
    return 0;
}
"#;
    let (code, _, stderr) = run_numlang_code(code);
    assert_eq!(code, Some(0), "Stderr: {}", stderr);
}

#[test]
fn test_match_negative_patterns() {
    let code = r#"
fn sign_check(x: i64) -> i64 {
    return match x {
        -1 => 11,
        0 => 22,
        1 => 33,
        _ => 99,
    };
}

fn main() -> i64 {
    if sign_check(-1) != 11 {
        return 1;
    }
    if sign_check(0) != 22 {
        return 2;
    }
    if sign_check(1) != 33 {
        return 3;
    }
    if sign_check(-5) != 99 {
        return 4;
    }
    return 0;
}
"#;
    let (code, _, stderr) = run_numlang_code(code);
    assert_eq!(code, Some(0), "Stderr: {}", stderr);
}

#[test]
fn test_match_non_exhaustive_error() {
    let code = r#"
fn main() -> i64 {
    let x: i64 = 10;
    let y: i64 = match x {
        0 => 1,
        1 => 2,
    };
    return y;
}
"#;
    let (status, _, stderr) = run_numlang_code(code);
    assert_ne!(status, Some(0), "Non-exhaustive match should fail");
    assert!(
        stderr.contains("Non-exhaustive") || stderr.contains("wildcard"),
        "Expected non-exhaustive error, got: {}",
        stderr
    );
}

use std::fs;
use std::process::Command;
use std::time::Instant;

fn run_numlang_code(code: &str) -> (Option<i32>, std::time::Duration) {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_sc_nl_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).unwrap();

    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");
    let elapsed = start.elapsed();

    let _ = fs::remove_dir_all(&test_dir);
    (output.status.code(), elapsed)
}

fn run_rust_reference(rust_code: &str) -> Option<i32> {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_sc_rust_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("ref.rs");
    let exe_file = test_dir.join("ref.exe");
    fs::write(&src_file, rust_code).unwrap();

    let compile = Command::new("rustc")
        .arg("-O")
        .arg(&src_file)
        .arg("-o")
        .arg(&exe_file)
        .output()
        .expect("Failed to compile rust reference");

    if !compile.status.success() {
        eprintln!(
            "Rust compile failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let _ = fs::remove_dir_all(&test_dir);
        return None;
    }

    let output = Command::new(&exe_file)
        .output()
        .expect("Failed to run rust reference");

    let _ = fs::remove_dir_all(&test_dir);
    output.status.code()
}

#[test]
fn test_novel_program_1_triangular_sum() {
    let nl_code = r#"
fn tri_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + i;
        i = i + 1;
    }
    return acc % 256;
}
fn main() -> i64 {
    return tri_sum(1000000);
}
"#;

    let rust_code = r#"
fn tri_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + i;
        i = i + 1;
    }
    acc % 256
}
fn main() {
    let exit_code = tri_sum(1000000);
    std::process::exit(exit_code as i32);
}
"#;

    let rust_exit = run_rust_reference(rust_code);
    assert_eq!(rust_exit, Some(32), "Rust reference must yield 32");

    let (nl_exit, elapsed) = run_numlang_code(nl_code);
    assert_eq!(
        nl_exit,
        Some(32),
        "NumLang exit code must match closed-form expectation"
    );
    assert_eq!(
        nl_exit, rust_exit,
        "NumLang exit code must match Rust reference"
    );
    println!("Novel 1 (tri_sum) execution time: {:?}", elapsed);
}

#[test]
fn test_novel_program_2_power_of_two() {
    let nl_code = r#"
fn pow2(n: i64) -> i64 {
    let mut acc: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        acc = acc * 2;
        i = i + 1;
    }
    return acc % 256;
}
fn main() -> i64 {
    return pow2(100);
}
"#;

    let rust_code = r#"
fn pow2(n: i64) -> i64 {
    let mut acc: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        acc = acc.wrapping_mul(2);
        i = i + 1;
    }
    acc % 256
}
fn main() {
    let exit_code = pow2(100);
    std::process::exit(exit_code as i32);
}
"#;

    let rust_exit = run_rust_reference(rust_code);
    assert_eq!(rust_exit, Some(0), "Rust reference must yield 0");

    let (nl_exit, elapsed) = run_numlang_code(nl_code);
    assert_eq!(
        nl_exit,
        Some(0),
        "NumLang exit code must match closed-form expectation"
    );
    assert_eq!(
        nl_exit, rust_exit,
        "NumLang exit code must match Rust reference"
    );
    println!("Novel 2 (pow2) execution time: {:?}", elapsed);
}

#[test]
fn test_novel_program_3_xor_period() {
    let nl_code = r#"
fn xor_period(n: i64) -> i64 {
    let mut x: i64 = 12345;
    let mut i: i64 = 0;
    while i < n {
        x = x ^ (x * 17);
        x = x % 65536;
        i = i + 1;
    }
    return x % 256;
}
fn main() -> i64 {
    return xor_period(500000);
}
"#;

    let rust_code = r#"
fn xor_period(n: i64) -> i64 {
    let mut x: i64 = 12345;
    let mut i: i64 = 0;
    while i < n {
        x = x ^ (x.wrapping_mul(17));
        x = x % 65536;
        i = i + 1;
    }
    x % 256
}
fn main() {
    let exit_code = xor_period(500000);
    std::process::exit(exit_code as i32);
}
"#;

    let rust_exit = run_rust_reference(rust_code);
    assert_eq!(rust_exit, Some(0), "Rust reference must yield 0");

    let (nl_exit, elapsed) = run_numlang_code(nl_code);
    assert_eq!(
        nl_exit,
        Some(0),
        "NumLang exit code must match closed-form expectation"
    );
    assert_eq!(
        nl_exit, rust_exit,
        "NumLang exit code must match Rust reference"
    );
    println!("Novel 3 (xor_period) execution time: {:?}", elapsed);
}

#[test]
fn test_novel_program_4_cubic_sum() {
    let nl_code = r#"
fn cubic_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + (i * i);
        i = i + 1;
    }
    return acc % 256;
}
fn main() -> i64 {
    return cubic_sum(1000);
}
"#;

    let rust_code = r#"
fn cubic_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc.wrapping_add(i * i);
        i = i + 1;
    }
    acc % 256
}
fn main() {
    let exit_code = cubic_sum(1000);
    std::process::exit(exit_code as i32);
}
"#;

    let rust_exit = run_rust_reference(rust_code);
    assert_eq!(rust_exit, Some(28), "Rust reference must yield 28");

    let (nl_exit, elapsed) = run_numlang_code(nl_code);
    assert_eq!(
        nl_exit,
        Some(28),
        "NumLang exit code must match closed-form expectation"
    );
    assert_eq!(
        nl_exit, rust_exit,
        "NumLang exit code must match Rust reference"
    );
    println!("Novel 4 (cubic_sum) execution time: {:?}", elapsed);
}

#[test]
fn test_novel_program_5_fibonacci_coupled() {
    let nl_code = r#"
fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let t: i64 = a + b;
        a = b;
        b = t;
        i = i + 1;
    }
    return a % 256;
}
fn main() -> i64 {
    return fib(50);
}
"#;

    let rust_code = r#"
fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let t: i64 = a.wrapping_add(b);
        a = b;
        b = t;
        i = i + 1;
    }
    a % 256
}
fn main() {
    let exit_code = fib(50);
    std::process::exit(exit_code as i32);
}
"#;

    let rust_exit = run_rust_reference(rust_code);
    let (nl_exit, elapsed) = run_numlang_code(nl_code);
    assert_eq!(
        nl_exit, rust_exit,
        "NumLang exit code must match Rust reference"
    );
    println!("Novel 5 (fibonacci_coupled) execution time: {:?}", elapsed);
}

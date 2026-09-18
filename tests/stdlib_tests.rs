use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> Option<i32> {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_test_stdlib_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.is_empty(), "compiler error occurred:\n{}", stderr);
    output.status.code()
}

#[test]
fn test_stdlib_sqrt() {
    let code = r#"
        fn main() -> i64 {
            let s: f64 = sqrt(4.0);
            if s == 2.0 {
                return 2;
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(2));
}

#[test]
fn test_stdlib_abs() {
    let code = r#"
        fn main() -> i64 {
            let i: i64 = abs(-42);
            let f: f64 = abs(-3.5);
            if i == 42 {
                if f == 3.5 {
                    return 1;
                }
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_stdlib_min_max() {
    let code = r#"
        fn main() -> i64 {
            let m1: i64 = min(3, 5);
            let m2: i64 = max(3, 5);
            let f1: f64 = min(5.5, 2.5);
            let f2: f64 = max(5.5, 2.5);
            if m1 == 3 {
                if m2 == 5 {
                    if f1 == 2.5 {
                        if f2 == 5.5 {
                            return 1;
                        }
                    }
                }
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_stdlib_rounding() {
    let code = r#"
        fn main() -> i64 {
            let f: f64 = floor(3.7);
            let c: f64 = ceil(3.2);
            let r: f64 = round(3.6);
            let t: f64 = trunc(3.9);
            if f == 3.0 {
                if c == 4.0 {
                    if r == 4.0 {
                        if t == 3.0 {
                            return 1;
                        }
                    }
                }
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_stdlib_bswap() {
    let code = r#"
        fn main() -> i64 {
            let x: i64 = 72623859790382856; // 0x0102030405060708
            let swapped: i64 = bswap(x);
            let back: i64 = bswap(swapped);
            if back == x {
                return 1;
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_stdlib_conversions() {
    let code = r#"
        fn main() -> i64 {
            let a: f64 = i64_to_f64(42);
            let b: i64 = f64_to_i64(42.9);
            let c: f32 = i64_to_f32(10);
            let d: f64 = f32_to_f64(c);
            let e: f32 = f64_to_f32(a);
            let f: i32 = i64_to_i32(100);
            let g: i64 = i32_to_i64(f);

            if a == 42.0 {
                if b == 42 {
                    if d == 10.0 {
                        if g == 100 {
                            return 1;
                        }
                    }
                }
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_stdlib_trig() {
    let code = r#"
        fn main() -> i64 {
            let s: f64 = sin(0.0);
            let c: f64 = cos(0.0);
            let t: f64 = tan(0.0);
            if s == 0.0 {
                if c == 1.0 {
                    if t == 0.0 {
                        return 1;
                    }
                }
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_stdlib_exp_log() {
    let code = r#"
        fn main() -> i64 {
            let e: f64 = exp(0.0);
            let l: f64 = ln(1.0);
            let l2: f64 = log2(8.0);
            let l10: f64 = log10(1000.0);
            if e == 1.0 {
                if l == 0.0 {
                    if l2 == 3.0 {
                        if l10 == 3.0 {
                            return 1;
                        }
                    }
                }
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_stdlib_pow() {
    let code = r#"
        fn main() -> i64 {
            let p1: f64 = pow(2.0, 3.0);
            let p2: i64 = 2 ** 10;
            let p3: i64 = (-3) ** 3;
            let p4: i64 = 5 ** 0;
            if p1 == 8.0 {
                if p2 == 1024 {
                    if p3 == -27 {
                        if p4 == 1 {
                            return 1;
                        }
                    }
                }
            }
            return 0;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(1));
}

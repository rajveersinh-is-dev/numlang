use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> Option<i32> {
    let test_dir = std::env::temp_dir().join("numlang_test_bit_intrinsics");
    fs::create_dir_all(&test_dir).unwrap();
    let id = format!("{}_{}", std::process::id(), std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos());
    let src_file = test_dir.join(format!("test_{}.nl", id));
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_file(&src_file);
    output.status.code()
}

#[test]
fn test_intrinsics_direct_calls() {
    let code_ctz = r#"
        fn main() -> i64 {
            let x: i64 = 40; // 40 = 0b101000 -> 3 trailing zeros
            return ctz(x);
        }
    "#;
    assert_eq!(run_numlang_code(code_ctz), Some(3));

    let code_clz = r#"
        fn main() -> i64 {
            let x: i64 = 1; // 63 leading zeros
            return clz(x);
        }
    "#;
    assert_eq!(run_numlang_code(code_clz), Some(63));

    let code_popcnt = r#"
        fn main() -> i64 {
            let x: i64 = 45; // 45 = 0b101101 -> 4 set bits
            return popcnt(x);
        }
    "#;
    assert_eq!(run_numlang_code(code_popcnt), Some(4));

    let code_rotl = r#"
        fn main() -> i64 {
            let x: i64 = 1;
            let r: i64 = rotl(x, 4);
            return r; // 1 << 4 = 16
        }
    "#;
    assert_eq!(run_numlang_code(code_rotl), Some(16));

    let code_rotr = r#"
        fn main() -> i64 {
            let x: i64 = 16;
            let r: i64 = rotr(x, 4);
            return r; // 16 >> 4 = 1
        }
    "#;
    assert_eq!(run_numlang_code(code_rotr), Some(1));
}

#[test]
fn test_single_trailing_zero_loop_recognition() {
    let code = r#"
        fn main() -> i64 {
            let mut u: i64 = 40;
            while (u & 1) == 0 {
                u = u >> 1;
            }
            return u; // 40 >> 3 = 5
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(5));
}

#[test]
fn test_dual_trailing_zero_loop_recognition() {
    let code = r#"
        fn main() -> i64 {
            let mut u: i64 = 12;
            let mut v: i64 = 20;
            let mut shift: i64 = 0;
            while ((u | v) & 1) == 0 {
                u = u >> 1;
                v = v >> 1;
                shift = shift + 1;
            }
            // u=3, v=5, shift=2 -> returns (u + v + shift) = 3 + 5 + 2 = 10
            return u + v + shift;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(10));
}

#[test]
fn test_kernighan_popcount_loop_recognition() {
    let code = r#"
        fn main() -> i64 {
            let mut num: i64 = 255; // 8 bits
            let mut count: i64 = 0;
            while num != 0 {
                num = num & (num - 1);
                count = count + 1;
            }
            return count;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(8));
}

#[test]
fn test_shift_popcount_loop_recognition() {
    let code = r#"
        fn main() -> i64 {
            let mut num: i64 = 127; // 7 bits
            let mut count: i64 = 0;
            while num > 0 {
                count = count + (num & 1);
                num = num >> 1;
            }
            return count;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(7));
}

#[test]
fn test_rotate_idiom_recognition() {
    let code = r#"
        fn main() -> i64 {
            let state: i64 = 1;
            let mask: i64 = 9223372036854775807;
            let left: i64 = (state << 1) | ((state >> 63) & 1);
            let right: i64 = ((state >> 1) & mask) | (state << 63);
            return left;
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(2));
}

#[test]
fn test_stein_gcd_computation() {
    let code = r#"
        fn stein_gcd(u_in: i64, v_in: i64) -> i64 {
            let mut u: i64 = u_in;
            let mut v: i64 = v_in;
            if u == 0 { return v; }
            if v == 0 { return u; }
            let mut shift: i64 = 0;
            while ((u | v) & 1) == 0 {
                u = u >> 1;
                v = v >> 1;
                shift = shift + 1;
            }
            while (u & 1) == 0 {
                u = u >> 1;
            }
            while v != 0 {
                while (v & 1) == 0 {
                    v = v >> 1;
                }
                if u > v {
                    let temp: i64 = u;
                    u = v;
                    v = temp;
                }
                v = v - u;
            }
            return u << shift;
        }

        fn main() -> i64 {
            let g1: i64 = stein_gcd(48, 18);
            let g2: i64 = stein_gcd(101, 103);
            let g3: i64 = stein_gcd(250, 1000);
            return g1 + g2 + (g3 % 100);
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(57));
}

use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> Option<i32> {
    let test_dir = std::env::temp_dir().join("numlang_test_math");
    fs::create_dir_all(&test_dir).unwrap();
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let src_file = test_dir.join(format!("test_{}.nl", id));
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_file(&src_file);
    if !output.status.success() {
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    }
    output.status.code()
}

#[test]
fn test_math_min_max_clamp_int() {
    let code = r#"
fn main() -> i64 {
    let a: i64 = 15;
    let b: i64 = 27;
    let m1: i64 = min(a, b);   // 15
    let m2: i64 = max(a, b);   // 27
    let c1: i64 = clamp(5, 10, 20);  // 10
    let c2: i64 = clamp(25, 10, 20); // 20
    let c3: i64 = clamp(14, 10, 20); // 14
    return m1 + m2 + c1 + c2 + c3;   // 15 + 27 + 10 + 20 + 14 = 86
}
"#;
    assert_eq!(run_numlang_code(code), Some(86));
}

#[test]
fn test_math_min_max_clamp_float() {
    let code = r#"
fn main() -> i64 {
    let a: f64 = 3.5;
    let b: f64 = 7.5;
    let m1: f64 = min(a, b);   // 3.5
    let m2: f64 = max(a, b);   // 7.5
    let c1: f64 = clamp(1.0, 2.0, 5.0); // 2.0
    let c2: f64 = clamp(9.0, 2.0, 5.0); // 5.0
    let sum: f64 = m1 + m2 + c1 + c2;   // 3.5 + 7.5 + 2.0 + 5.0 = 18.0
    if sum == 18.0 {
        return 18;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(18));
}

#[test]
fn test_math_rounding_intrinsics() {
    let code = r#"
fn main() -> i64 {
    let f: f64 = floor(3.7);  // 3.0
    let c: f64 = ceil(3.2);   // 4.0
    let r: f64 = round(3.6);  // 4.0
    let t: f64 = trunc(3.9);  // 3.0
    let sum: f64 = f + c + r + t; // 3 + 4 + 4 + 3 = 14
    if sum == 14.0 {
        return 14;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(14));
}

#[test]
fn test_math_fma_hypot_lerp() {
    let code = r#"
fn main() -> i64 {
    let f: f64 = fma(2.0, 3.0, 4.0); // 2*3 + 4 = 10.0
    let h: f64 = hypot(3.0, 4.0);     // sqrt(9 + 16) = 5.0
    let l: f64 = lerp(10.0, 20.0, 0.5); // 15.0
    let sum: f64 = f + h + l;          // 10 + 5 + 15 = 30
    if sum == 30.0 {
        return 30;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(30));
}

#[test]
fn test_math_signum() {
    let code = r#"
fn main() -> i64 {
    let s_pos: i64 = signum(42);
    let s_neg: i64 = signum(-42);
    let s_zero: i64 = signum(0);

    let sf_pos: f64 = signum(3.14);
    let sf_neg: f64 = signum(-2.71);
    let sf_zero: f64 = signum(0.0);

    let mut score: i64 = 0;
    if s_pos == 1 { score = score + 1; }
    if s_neg == -1 { score = score + 2; }
    if s_zero == 0 { score = score + 4; }
    if sf_pos == 1.0 { score = score + 8; }
    if sf_neg == -1.0 { score = score + 16; }
    if sf_zero == 0.0 { score = score + 32; }

    return score; // 1 + 2 + 4 + 8 + 16 + 32 = 63
}
"#;
    assert_eq!(run_numlang_code(code), Some(63));
}

#[test]
fn test_math_gcd_lcm() {
    let code = r#"
fn main() -> i64 {
    let g1: i64 = gcd(48, 18);   // 6
    let g2: i64 = gcd(-48, 18);  // 6
    let g3: i64 = gcd(0, 15);    // 15
    let l1: i64 = lcm(12, 18);   // 36
    let l2: i64 = lcm(-12, 18);  // 36
    let l3: i64 = lcm(0, 5);     // 0
    return g1 + g2 + g3 + l1 + l2 + l3; // 6 + 6 + 15 + 36 + 36 + 0 = 99
}
"#;
    assert_eq!(run_numlang_code(code), Some(99));
}

#[test]
fn test_vector_algebra() {
    let code = r#"
fn main() -> i64 {
    let a: [f64; 3] = [1.0, 2.0, 3.0];
    let b: [f64; 3] = [4.0, 5.0, 6.0];

    let c: [f64; 3] = vec_add(a, b);   // [5.0, 7.0, 9.0]
    let d: [f64; 3] = vec_sub(b, a);   // [3.0, 3.0, 3.0]
    let e: [f64; 3] = vec_mul(a, b);   // [4.0, 10.0, 18.0]
    let s: [f64; 3] = vec_scale(a, 2.0); // [2.0, 4.0, 6.0]

    let norm_d: f64 = vec_norm(d);     // sqrt(9 + 9 + 9) = sqrt(27) ~ 5.196
    let cross: [f64; 3] = vec_cross3(a, b); // [-3.0, 6.0, -3.0]

    let sum_c: f64 = c[0] + c[1] + c[2]; // 21.0
    let sum_cross: f64 = cross[0] + cross[1] + cross[2]; // 0.0
    let sum_e: f64 = e[0] + e[1] + e[2]; // 32.0
    let sum_s: f64 = s[0] + s[1] + s[2]; // 12.0

    let mut ok: i64 = 0;
    if sum_c == 21.0 { ok = ok + 1; }
    if sum_cross == 0.0 { ok = ok + 2; }
    if sum_e == 32.0 { ok = ok + 4; }
    if sum_s == 12.0 { ok = ok + 8; }
    if norm_d > 5.19 {
        if norm_d < 5.20 {
            ok = ok + 16;
        }
    }
    return ok; // 1 + 2 + 4 + 8 + 16 = 31
}
"#;
    assert_eq!(run_numlang_code(code), Some(31));
}

#[test]
fn test_nested_vector_expressions() {
    let code = r#"
fn main() -> i64 {
    let a: [f64; 3] = [10.0, 20.0, 30.0];
    let b: [f64; 3] = [1.0, 2.0, 3.0];
    let c: [f64; 3] = [2.0, 1.0, 0.0];

    // vec_sub(a, b) = [9.0, 18.0, 27.0]
    // dot([9, 18, 27], [2, 1, 0]) = 18 + 18 + 0 = 36.0
    let res: f64 = dot(vec_sub(a, b), c);
    if res == 36.0 {
        return 36;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(36));
}

#[test]
fn test_matrix_det2() {
    let code = r#"
fn main() -> i64 {
    let m: [f64; 4] = [
        4.0, 7.0,
        2.0, 6.0
    ];
    // det = 4*6 - 7*2 = 24 - 14 = 10
    let d: f64 = mat_det2(m);
    if d == 10.0 {
        return 10;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(10));
}

#[test]
fn test_matrix_det3() {
    let code = r#"
fn main() -> i64 {
    let m: [f64; 9] = [
        1.0, 2.0, 3.0,
        0.0, 1.0, 4.0,
        5.0, 6.0, 0.0
    ];
    // det = 1*(0 - 24) - 2*(0 - 20) + 3*(0 - 5)
    //     = -24 + 40 - 15 = 1
    let d: f64 = mat_det3(m);
    if d == 1.0 {
        return 1;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(1));
}

#[test]
fn test_matrix_operations_4x4() {
    let code = r#"
fn main() -> i64 {
    // Identity 4x4 matrix
    let eye: [f64; 16] = [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0
    ];
    let a: [f64; 16] = [
        2.0, 0.0, 0.0, 0.0,
        0.0, 3.0, 0.0, 0.0,
        0.0, 0.0, 4.0, 0.0,
        0.0, 0.0, 0.0, 5.0
    ];

    // mat_mul4 with identity should equal a
    let prod: [f64; 16] = mat_mul4(a, eye);
    let trace: f64 = mat_trace4(prod); // 2 + 3 + 4 + 5 = 14
    let det: f64 = mat_det4(prod);      // 2 * 3 * 4 * 5 = 120

    let t: [f64; 16] = mat_transpose4(prod);
    let trace_t: f64 = mat_trace4(t);  // 14

    let mut ok: i64 = 0;
    if trace == 14.0 { ok = ok + 1; }
    if det == 120.0 { ok = ok + 2; }
    if trace_t == 14.0 { ok = ok + 4; }
    return ok; // 1 + 2 + 4 = 7
}
"#;
    assert_eq!(run_numlang_code(code), Some(7));
}

#[test]
fn test_nested_array_type_parsing() {
    let code = r#"
fn main() -> i64 {
    let a: [[i64; 2]; 2] = [[10, 20], [30, 40]];
    return a[0][0] + a[0][1] + a[1][0] + a[1][1]; // 100
}
"#;
    assert_eq!(run_numlang_code(code), Some(100));
}

#[test]
fn test_isqrt_tzcnt() {
    let code = r#"
fn main() -> i64 {
    let sq0: i64 = isqrt(0);
    let sq1: i64 = isqrt(1);
    let sq2: i64 = isqrt(2);
    let sq3: i64 = isqrt(3);
    let sq4: i64 = isqrt(4);
    let sq15: i64 = isqrt(15);
    let sq16: i64 = isqrt(16);
    let sq17: i64 = isqrt(17);
    let sq100: i64 = isqrt(100);
    let sq10000: i64 = isqrt(10000);
    let sq_large: i64 = isqrt(1000000000000); // 1,000,000

    let tz1: i64 = tzcnt(1);   // 0
    let tz2: i64 = tzcnt(2);   // 1
    let tz8: i64 = tzcnt(8);   // 3
    let tz16: i64 = tzcnt(16); // 4

    let mut score: i64 = 0;
    if sq0 == 0 { score = score + 1; }
    if sq1 == 1 { score = score + 2; }
    if sq2 == 1 { score = score + 4; }
    if sq3 == 1 { score = score + 8; }
    if sq4 == 2 { score = score + 16; }
    if sq15 == 3 { score = score + 32; }
    if sq16 == 4 { score = score + 64; }
    if sq17 == 4 { score = score + 128; }
    if sq100 == 10 { score = score + 256; }
    if sq10000 == 100 { score = score + 512; }
    if sq_large == 1000000 { score = score + 1024; }
    if tz1 == 0 {
        if tz2 == 1 {
            if tz8 == 3 {
                if tz16 == 4 {
                    score = score + 2048;
                }
            }
        }
    }

    return score; // 1 + 2 + 4 + 8 + 16 + 32 + 64 + 128 + 256 + 512 + 1024 + 2048 = 4095
}
"#;
    assert_eq!(run_numlang_code(code), Some(4095));
}

#[test]
fn test_loop_opt_stein_gcd_and_newton() {
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

fn isqrt_newton(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    let mut x: i64 = n;
    let mut y: i64 = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    return x;
}

fn main() -> i64 {
    let g1: i64 = stein_gcd(12, 18);   // 6
    let g2: i64 = stein_gcd(100, 25);  // 25
    let g3: i64 = stein_gcd(17, 19);   // 1

    let sq1: i64 = isqrt_newton(25);   // 5
    let sq2: i64 = isqrt_newton(100);  // 10
    let sq3: i64 = isqrt_newton(2);    // 1

    return g1 + g2 + g3 + sq1 + sq2 + sq3; // 6 + 25 + 1 + 5 + 10 + 1 = 48
}
"#;
    assert_eq!(run_numlang_code(code), Some(48));
}



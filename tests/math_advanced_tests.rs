use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn run_numlang_code(code: &str) -> Option<i32> {
    let test_dir = std::env::temp_dir().join("numlang_test_math_advanced");
    fs::create_dir_all(&test_dir).unwrap();
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let src_file = test_dir.join(format!("test_{}_{}.nl", pid, id));
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_file(&src_file);
    if !output.stderr.is_empty() {
        eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    }
    output.status.code()
}

// ==========================================
// Category 1: Vector & Matrix Operator Overloading
// ==========================================

#[test]
fn test_vec_operator_overloading() {
    let code = r#"
fn main() -> i64 {
    let v1: [f64; 3] = [1.0, 2.0, 3.0];
    let v2: [f64; 3] = [4.0, 5.0, 6.0];
    
    // Vector + Vector
    let add: [f64; 3] = v1 + v2; // [5.0, 7.0, 9.0]
    
    // Vector - Vector
    let sub: [f64; 3] = v2 - v1; // [3.0, 3.0, 3.0]
    
    // Vector * Scalar
    let mul_s: [f64; 3] = v1 * 2.0; // [2.0, 4.0, 6.0]
    
    // Scalar * Vector
    let s_mul: [f64; 3] = 3.0 * v1; // [3.0, 6.0, 9.0]
    
    // Vector / Scalar
    let div_s: [f64; 3] = v2 / 2.0; // [2.0, 2.5, 3.0]
    
    // Unary Negation
    let neg: [f64; 3] = -v1; // [-1.0, -2.0, -3.0]
    
    // Elementwise Hadamard
    let had: [f64; 3] = v1 * v2; // [4.0, 10.0, 18.0]

    let mut ok: i64 = 1;
    if add[0] != 5.0 { ok = 0; }
    if add[1] != 7.0 { ok = 0; }
    if add[2] != 9.0 { ok = 0; }

    if sub[0] != 3.0 { ok = 0; }
    if sub[1] != 3.0 { ok = 0; }
    if sub[2] != 3.0 { ok = 0; }

    if mul_s[0] != 2.0 { ok = 0; }
    if mul_s[1] != 4.0 { ok = 0; }
    if mul_s[2] != 6.0 { ok = 0; }

    if s_mul[0] != 3.0 { ok = 0; }
    if s_mul[1] != 6.0 { ok = 0; }
    if s_mul[2] != 9.0 { ok = 0; }

    if div_s[0] != 2.0 { ok = 0; }
    if div_s[1] != 2.5 { ok = 0; }
    if div_s[2] != 3.0 { ok = 0; }

    if neg[0] != -1.0 { ok = 0; }
    if neg[1] != -2.0 { ok = 0; }
    if neg[2] != -3.0 { ok = 0; }

    if had[0] != 4.0 { ok = 0; }
    if had[1] != 10.0 { ok = 0; }
    if had[2] != 18.0 { ok = 0; }
    
    if ok == 1 {
        return 42;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(42));
}

// ==========================================
// Category 2: Matrix Inversion & Linear Solvers
// ==========================================

#[test]
fn test_mat_inv2_and_solve2() {
    let code = r#"
fn main() -> i64 {
    // A = [4, 7; 2, 6], det = 24 - 14 = 10
    // A^-1 = [0.6, -0.7; -0.2, 0.4]
    let a: [f64; 4] = [4.0, 7.0, 2.0, 6.0];
    let inv: [f64; 4] = mat_inv2(a);
    
    // A * A^-1 should be identity [1, 0; 0, 1]
    let ident: [f64; 4] = mat_mul2(a, inv);
    
    // Solve A * x = b, where b = [18, 14]
    // Exact solution: x0 = 1, x1 = 2 (4*1 + 7*2 = 18, 2*1 + 6*2 = 14)
    let b: [f64; 2] = [18.0, 14.0];
    let x: [f64; 2] = mat_solve2(a, b);
    
    let mut ok: i64 = 1;
    // Check identity
    if ident[0] < 0.999 { ok = 0; }
    if ident[0] > 1.001 { ok = 0; }
    if ident[1] < -0.001 { ok = 0; }
    if ident[1] > 0.001 { ok = 0; }
    if ident[2] < -0.001 { ok = 0; }
    if ident[2] > 0.001 { ok = 0; }
    if ident[3] < 0.999 { ok = 0; }
    if ident[3] > 1.001 { ok = 0; }
    
    // Check solve
    if x[0] < 0.999 { ok = 0; }
    if x[0] > 1.001 { ok = 0; }
    if x[1] < 1.999 { ok = 0; }
    if x[1] > 2.001 { ok = 0; }
    
    // Check transpose and trace
    let at: [f64; 4] = mat_transpose2(a);
    if at[1] != 2.0 { ok = 0; }
    if at[2] != 7.0 { ok = 0; }
    let tr: f64 = mat_trace2(a);
    if tr != 10.0 { ok = 0; }
    
    if ok == 1 {
        return 100;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(100));
}

#[test]
fn test_mat_inv3_and_solve3() {
    let code = r#"
fn main() -> i64 {
    // A = [1, 2, 3; 0, 1, 4; 5, 6, 0]
    let a: [f64; 9] = [
        1.0, 2.0, 3.0,
        0.0, 1.0, 4.0,
        5.0, 6.0, 0.0
    ];
    let inv: [f64; 9] = mat_inv3(a);
    let ident: [f64; 9] = mat_mul3(a, inv);
    
    // Solve A * x = b with x = [2, -1, 3]
    // b0 = 1*2 + 2*(-1) + 3*3 = 9
    // b1 = 0*2 + 1*(-1) + 4*3 = 11
    // b2 = 5*2 + 6*(-1) + 0*3 = 4
    let b: [f64; 3] = [9.0, 11.0, 4.0];
    let x: [f64; 3] = mat_solve3(a, b);
    
    let mut ok: i64 = 1;
    // Check diagonal elements of A * A^-1
    if ident[0] < 0.999 { ok = 0; }
    if ident[0] > 1.001 { ok = 0; }
    if ident[4] < 0.999 { ok = 0; }
    if ident[4] > 1.001 { ok = 0; }
    if ident[8] < 0.999 { ok = 0; }
    if ident[8] > 1.001 { ok = 0; }
    
    // Check off-diagonal elements
    if ident[1] < -0.001 { ok = 0; }
    if ident[1] > 0.001 { ok = 0; }
    if ident[2] < -0.001 { ok = 0; }
    if ident[2] > 0.001 { ok = 0; }
    if ident[3] < -0.001 { ok = 0; }
    if ident[3] > 0.001 { ok = 0; }
    if ident[5] < -0.001 { ok = 0; }
    if ident[5] > 0.001 { ok = 0; }
    if ident[6] < -0.001 { ok = 0; }
    if ident[6] > 0.001 { ok = 0; }
    if ident[7] < -0.001 { ok = 0; }
    if ident[7] > 0.001 { ok = 0; }
    
    // Check solve: [2, -1, 3]
    if x[0] < 1.999 { ok = 0; }
    if x[0] > 2.001 { ok = 0; }
    if x[1] < -1.001 { ok = 0; }
    if x[1] > -0.999 { ok = 0; }
    if x[2] < 2.999 { ok = 0; }
    if x[2] > 3.001 { ok = 0; }
    
    let at: [f64; 9] = mat_transpose3(a);
    if at[1] != 0.0 { ok = 0; }
    if at[3] != 2.0 { ok = 0; }
    let tr: f64 = mat_trace3(a);
    if tr != 2.0 { ok = 0; }
    
    if ok == 1 {
        return 101;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(101));
}

#[test]
fn test_mat_inv4_and_solve4() {
    let code = r#"
fn main() -> i64 {
    // 4x4 diagonally dominant matrix
    let a: [f64; 16] = [
        5.0, 1.0, 0.0, 2.0,
        1.0, 6.0, 2.0, 0.0,
        0.0, 2.0, 7.0, 1.0,
        2.0, 0.0, 1.0, 8.0
    ];
    let inv: [f64; 16] = mat_inv4(a);
    let ident: [f64; 16] = mat_mul4(a, inv);
    
    // b = A * [1, 2, 3, 4]
    // b0 = 5*1 + 1*2 + 0*3 + 2*4 = 15
    // b1 = 1*1 + 6*2 + 2*3 + 0*4 = 19
    // b2 = 0*1 + 2*2 + 7*3 + 1*4 = 29
    // b3 = 2*1 + 0*2 + 1*3 + 8*4 = 37
    let b: [f64; 4] = [15.0, 19.0, 29.0, 37.0];
    let x: [f64; 4] = mat_solve4(a, b);
    
    let mut ok: i64 = 1;
    // Check diagonal of ident
    if ident[0] < 0.999 { ok = 0; }
    if ident[0] > 1.001 { ok = 0; }
    if ident[5] < 0.999 { ok = 0; }
    if ident[5] > 1.001 { ok = 0; }
    if ident[10] < 0.999 { ok = 0; }
    if ident[10] > 1.001 { ok = 0; }
    if ident[15] < 0.999 { ok = 0; }
    if ident[15] > 1.001 { ok = 0; }
    
    // Check solution: [1, 2, 3, 4]
    if x[0] < 0.999 { ok = 0; }
    if x[0] > 1.001 { ok = 0; }
    if x[1] < 1.999 { ok = 0; }
    if x[1] > 2.001 { ok = 0; }
    if x[2] < 2.999 { ok = 0; }
    if x[2] > 3.001 { ok = 0; }
    if x[3] < 3.999 { ok = 0; }
    if x[3] > 4.001 { ok = 0; }
    
    let tr: f64 = mat_trace4(a);
    if tr != 26.0 { ok = 0; }
    
    if ok == 1 {
        return 102;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(102));
}

// ==========================================
// Category 3: SSA Transcendentals
// ==========================================

#[test]
fn test_transcendentals() {
    let code = r#"
fn main() -> i64 {
    let pi: f64 = 3.141592653589793;
    let s0: f64 = sin(0.0);
    let s_half_pi: f64 = sin(pi * 0.5);
    let c0: f64 = cos(0.0);
    let c_pi: f64 = cos(pi);
    let t_pi_4: f64 = tan(pi * 0.25);
    let e0: f64 = exp(0.0);
    let e1: f64 = exp(1.0);
    let l1: f64 = ln(1.0);
    let l_e: f64 = ln(e1);
    let at: f64 = atan2(1.0, 1.0);
    let p: f64 = powf(2.0, 4.0);

    if s0 < -0.001 { return 1; }
    if s0 > 0.001 { return 1; }
    if s_half_pi < 0.999 { return 2; }
    if s_half_pi > 1.001 { return 2; }
    if c0 < 0.999 { return 3; }
    if c0 > 1.001 { return 3; }
    if c_pi < -1.001 { return 4; }
    if c_pi > -0.999 { return 4; }
    if t_pi_4 < 0.999 { return 5; }
    if t_pi_4 > 1.001 { return 5; }
    if e0 < 0.999 { return 6; }
    if e0 > 1.001 { return 6; }
    if e1 < 2.717 { return 7; }
    if e1 > 2.719 { return 7; }
    if l1 < -0.001 { return 8; }
    if l1 > 0.001 { return 8; }
    if l_e < 0.999 { return 9; }
    if l_e > 1.001 { return 9; }
    if at < 0.784 { return 10; }
    if at > 0.786 { return 10; } // pi/4 = 0.785398
    if p < 15.999 { return 11; }
    if p > 16.001 { return 11; }

    // Check pythagorean identity sin^2(x) + cos^2(x) == 1
    let x: f64 = 1.2345;
    let sx: f64 = sin(x);
    let cx: f64 = cos(x);
    let sum_sq: f64 = sx * sx + cx * cx;
    if sum_sq < 0.999 { return 12; }
    if sum_sq > 1.001 { return 12; }

    return 103;
}
"#;
    assert_eq!(run_numlang_code(code), Some(103));
}

// ==========================================
// Category 4: Native Complex Numbers
// ==========================================

#[test]
fn test_complex_numbers() {
    let code = r#"
fn main() -> i64 {
    let z1: [f64; 2] = c_make(3.0, 4.0);
    let z2: [f64; 2] = c_make(1.0, 2.0);
    
    let re: f64 = c_re(z1); // 3.0
    let im: f64 = c_im(z1); // 4.0
    let abs_val: f64 = c_abs(z1); // 5.0
    
    let sum: [f64; 2] = c_add(z1, z2); // [4.0, 6.0]
    let diff: [f64; 2] = c_sub(z1, z2); // [2.0, 2.0]
    
    // (3 + 4i) * (1 + 2i) = 3 + 6i + 4i - 8 = -5 + 10i
    let prod: [f64; 2] = c_mul(z1, z2);
    
    // (-5 + 10i) / (1 + 2i) = (3 + 4i)
    let quot: [f64; 2] = c_div(prod, z2);
    
    let conj: [f64; 2] = c_conj(z1); // [3.0, -4.0]
    
    // Euler's formula: exp(i * pi) = -1 + 0i
    let pi: f64 = 3.141592653589793;
    let ipi: [f64; 2] = c_make(0.0, pi);
    let e_ipi: [f64; 2] = c_exp(ipi);

    let mut ok: i64 = 1;
    if re != 3.0 { ok = 0; }
    if im != 4.0 { ok = 0; }
    if abs_val < 4.999 { ok = 0; }
    if abs_val > 5.001 { ok = 0; }
    if sum[0] != 4.0 { ok = 0; }
    if sum[1] != 6.0 { ok = 0; }
    if diff[0] != 2.0 { ok = 0; }
    if diff[1] != 2.0 { ok = 0; }
    if prod[0] < -5.001 { ok = 0; }
    if prod[0] > -4.999 { ok = 0; }
    if prod[1] < 9.999 { ok = 0; }
    if prod[1] > 10.001 { ok = 0; }
    if quot[0] < 2.999 { ok = 0; }
    if quot[0] > 3.001 { ok = 0; }
    if quot[1] < 3.999 { ok = 0; }
    if quot[1] > 4.001 { ok = 0; }
    if conj[0] != 3.0 { ok = 0; }
    if conj[1] != -4.0 { ok = 0; }
    
    // e^(i*pi) == -1 + 0i
    if e_ipi[0] < -1.001 { ok = 0; }
    if e_ipi[0] > -0.999 { ok = 0; }
    if e_ipi[1] < -0.001 { ok = 0; }
    if e_ipi[1] > 0.001 { ok = 0; }

    if ok == 1 {
        return 104;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(104));
}

// ==========================================
// Category 5: Radix-2 Cooley-Tukey FFT
// ==========================================

#[test]
fn test_cooley_tukey_fft8_and_fft16() {
    let code = r#"
fn main() -> i64 {
    // 1. FFT8 of impulse signal: [1, 0, 0, 0, 0, 0, 0, 0]
    // The DFT of an impulse is all 1s in real part and 0s in imaginary part.
    let re_imp8: [f64; 8] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let im_zero8: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    
    let fft_re8: [f64; 8] = fft8_re(re_imp8, im_zero8);
    let fft_im8: [f64; 8] = fft8_im(re_imp8, im_zero8);
    
    let mut ok: i64 = 1;
    let mut i: i64 = 0;
    while i < 8 {
        if fft_re8[i] < 0.999 { ok = 0; }
        if fft_re8[i] > 1.001 { ok = 0; }
        if fft_im8[i] < -0.001 { ok = 0; }
        if fft_im8[i] > 0.001 { ok = 0; }
        i = i + 1;
    }
    
    // 2. FFT8 of DC signal: [1, 1, 1, 1, 1, 1, 1, 1]
    // The DFT is [8, 0, 0, 0, 0, 0, 0, 0]
    let re_dc8: [f64; 8] = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
    let dc_re8: [f64; 8] = fft8_re(re_dc8, im_zero8);
    if dc_re8[0] < 7.999 { ok = 0; }
    if dc_re8[0] > 8.001 { ok = 0; }
    i = 1;
    while i < 8 {
        if dc_re8[i] < -0.001 { ok = 0; }
        if dc_re8[i] > 0.001 { ok = 0; }
        i = i + 1;
    }
    
    // 3. FFT16 of impulse signal: [1, 0, 0, ...] -> 16 ones
    let re_imp16: [f64; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
    ];
    let im_zero16: [f64; 16] = [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
    ];
    let fft_re16: [f64; 16] = fft16_re(re_imp16, im_zero16);
    i = 0;
    while i < 16 {
        if fft_re16[i] < 0.999 { ok = 0; }
        if fft_re16[i] > 1.001 { ok = 0; }
        i = i + 1;
    }

    if ok == 1 {
        return 105;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(105));
}

#[test]
fn test_type_conversions() {
    let code = r#"
fn main() -> i64 {
    let f: f64 = 42.99;
    let i: i64 = to_int(f);
    let f2: f64 = to_float(i);
    let i2: i64 = to_int(f2 + 0.5);
    return i + i2;
}
"#;
    assert_eq!(run_numlang_code(code), Some(84));
}

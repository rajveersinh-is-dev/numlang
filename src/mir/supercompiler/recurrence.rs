use std::collections::HashMap;

use super::term::{SymTerm, SymTermId, TermInterner};
use crate::ast::{BinaryOp, UnaryOp};
use crate::mir::lower::{MirBasicBlock, MirFunction, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Terminator};
use crate::typecheck::typed_ast::TypedLiteral;
use crate::typecheck::types::Type;

/// A detected N-way linear recurrence system.
/// Variables x_i(k) = sum_j A[i][j] * x_j(k-1)          (homogeneous)
/// or         x_i(k) = sum_j A[i][j] * x_j(k-1) + c_i  (affine)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NWayLinearSystem {
    pub n: usize,
    pub a: Vec<Vec<i64>>, // n×n companion / transition matrix
    pub c: Vec<i64>,      // n-vector of constants (0 for homogeneous)
    pub init: Vec<i64>,   // initial values x_i(0)
}

/// Unrolled 2x2 matrix multiplication with wrapping integer arithmetic.
pub fn mat_mul_2x2(a: &[[i64; 2]; 2], b: &[[i64; 2]; 2]) -> [[i64; 2]; 2] {
    [
        [
            a[0][0]
                .wrapping_mul(b[0][0])
                .wrapping_add(a[0][1].wrapping_mul(b[1][0])),
            a[0][0]
                .wrapping_mul(b[0][1])
                .wrapping_add(a[0][1].wrapping_mul(b[1][1])),
        ],
        [
            a[1][0]
                .wrapping_mul(b[0][0])
                .wrapping_add(a[1][1].wrapping_mul(b[1][0])),
            a[1][0]
                .wrapping_mul(b[0][1])
                .wrapping_add(a[1][1].wrapping_mul(b[1][1])),
        ],
    ]
}

/// 4x4 matrix multiplication unrolled into 4-wide SIMD vector operations.
/// Operates on 4-element SIMD vector lanes [i64; 4].
pub fn mat_mul_4x4(a: &[[i64; 4]; 4], b: &[[i64; 4]; 4]) -> [[i64; 4]; 4] {
    let mut res = [[0i64; 4]; 4];
    for r in 0..4 {
        let row_a = a[r];
        let mut accum = [0i64; 4];
        for k in 0..4 {
            let a_elem = row_a[k];
            let b_row = b[k];
            for c in 0..4 {
                accum[c] = accum[c].wrapping_add(a_elem.wrapping_mul(b_row[c]));
            }
        }
        res[r] = accum;
    }
    res
}

/// Binary exponentiation for 2x2 integer matrices in pure register state.
pub fn mat_pow_2x2(m: &[[i64; 2]; 2], mut exp: i64) -> [[i64; 2]; 2] {
    let mut res = [[1, 0], [0, 1]];
    if exp <= 0 {
        return res;
    }
    let mut base = *m;
    while exp > 0 {
        if exp & 1 == 1 {
            res = mat_mul_2x2(&res, &base);
        }
        if exp > 1 {
            base = mat_mul_2x2(&base, &base);
        }
        exp >>= 1;
    }
    res
}

/// Binary exponentiation for 4x4 integer matrices in pure 4-wide SIMD vector state.
pub fn mat_pow_4x4(m: &[[i64; 4]; 4], mut exp: i64) -> [[i64; 4]; 4] {
    let mut res = [[0i64; 4]; 4];
    for (i, row) in res.iter_mut().enumerate() {
        row[i] = 1;
    }
    if exp <= 0 {
        return res;
    }
    let mut base = *m;
    while exp > 0 {
        if exp & 1 == 1 {
            res = mat_mul_4x4(&res, &base);
        }
        if exp > 1 {
            base = mat_mul_4x4(&base, &base);
        }
        exp >>= 1;
    }
    res
}

/// Helper: element-wise matrix addition with wrapping arithmetic.
pub fn mat_add_nxn(a: &[Vec<i64>], b: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let rows = a.len();
    let cols = if rows > 0 { a[0].len() } else { 0 };
    let mut res = vec![vec![0i64; cols]; rows];
    for r in 0..rows {
        for c in 0..cols {
            res[r][c] = a[r][c].wrapping_add(b[r][c]);
        }
    }
    res
}

/// Helper: element-wise matrix subtraction with wrapping arithmetic.
pub fn mat_sub_nxn(a: &[Vec<i64>], b: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let rows = a.len();
    let cols = if rows > 0 { a[0].len() } else { 0 };
    let mut res = vec![vec![0i64; cols]; rows];
    for r in 0..rows {
        for c in 0..cols {
            res[r][c] = a[r][c].wrapping_sub(b[r][c]);
        }
    }
    res
}

/// Strassen block recursive matrix multiplication for N×N integer matrices with wrapping arithmetic.
/// For N >= 4, partitions matrices into 2x2 blocks of size (N/2)x(N/2) and applies Strassen's 7 multiplications.
pub fn mat_mul_strassen(a: &[Vec<i64>], b: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let n = a.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![vec![a[0][0].wrapping_mul(b[0][0])]];
    }
    if n == 2 {
        let a_fixed = [[a[0][0], a[0][1]], [a[1][0], a[1][1]]];
        let b_fixed = [[b[0][0], b[0][1]], [b[1][0], b[1][1]]];
        let r = mat_mul_2x2(&a_fixed, &b_fixed);
        return vec![vec![r[0][0], r[0][1]], vec![r[1][0], r[1][1]]];
    }
    if n < 4 {
        let mut res = vec![vec![0i64; n]; n];
        for r in 0..n {
            for c in 0..n {
                let mut sum: i64 = 0;
                for k in 0..n {
                    sum = sum.wrapping_add(a[r][k].wrapping_mul(b[k][c]));
                }
                res[r][c] = sum;
            }
        }
        return res;
    }

    // Pad to even dimension if n is odd
    let (a_pad, b_pad, unpad) = if n % 2 != 0 {
        let mut ap = vec![vec![0i64; n + 1]; n + 1];
        let mut bp = vec![vec![0i64; n + 1]; n + 1];
        for r in 0..n {
            for c in 0..n {
                ap[r][c] = a[r][c];
                bp[r][c] = b[r][c];
            }
        }
        (ap, bp, true)
    } else {
        (a.to_vec(), b.to_vec(), false)
    };

    let m = a_pad.len() / 2;
    let mut a11 = vec![vec![0i64; m]; m];
    let mut a12 = vec![vec![0i64; m]; m];
    let mut a21 = vec![vec![0i64; m]; m];
    let mut a22 = vec![vec![0i64; m]; m];

    let mut b11 = vec![vec![0i64; m]; m];
    let mut b12 = vec![vec![0i64; m]; m];
    let mut b21 = vec![vec![0i64; m]; m];
    let mut b22 = vec![vec![0i64; m]; m];

    for r in 0..m {
        for c in 0..m {
            a11[r][c] = a_pad[r][c];
            a12[r][c] = a_pad[r][c + m];
            a21[r][c] = a_pad[r + m][c];
            a22[r][c] = a_pad[r + m][c + m];

            b11[r][c] = b_pad[r][c];
            b12[r][c] = b_pad[r][c + m];
            b21[r][c] = b_pad[r + m][c];
            b22[r][c] = b_pad[r + m][c + m];
        }
    }

    // Strassen's 7 multiplications:
    // M1 = (A11 + A22) * (B11 + B22)
    let a_plus = mat_add_nxn(&a11, &a22);
    let b_plus = mat_add_nxn(&b11, &b22);
    let m1 = mat_mul_strassen(&a_plus, &b_plus);

    // M2 = (A21 + A22) * B11
    let a_plus2 = mat_add_nxn(&a21, &a22);
    let m2 = mat_mul_strassen(&a_plus2, &b11);

    // M3 = A11 * (B12 - B22)
    let b_sub = mat_sub_nxn(&b12, &b22);
    let m3 = mat_mul_strassen(&a11, &b_sub);

    // M4 = A22 * (B21 - B11)
    let b_sub2 = mat_sub_nxn(&b21, &b11);
    let m4 = mat_mul_strassen(&a22, &b_sub2);

    // M5 = (A11 + A12) * B22
    let a_plus3 = mat_add_nxn(&a11, &a12);
    let m5 = mat_mul_strassen(&a_plus3, &b22);

    // M6 = (A21 - A11) * (B11 + B12)
    let a_sub = mat_sub_nxn(&a21, &a11);
    let b_plus2 = mat_add_nxn(&b11, &b12);
    let m6 = mat_mul_strassen(&a_sub, &b_plus2);

    // M7 = (A12 - A22) * (B21 + B22)
    let a_sub2 = mat_sub_nxn(&a12, &a22);
    let b_plus3 = mat_add_nxn(&b21, &b22);
    let m7 = mat_mul_strassen(&a_sub2, &b_plus3);

    // C11 = M1 + M4 - M5 + M7
    let c11_part = mat_add_nxn(&m1, &m4);
    let c11_part2 = mat_sub_nxn(&c11_part, &m5);
    let c11 = mat_add_nxn(&c11_part2, &m7);

    // C12 = M3 + M5
    let c12 = mat_add_nxn(&m3, &m5);

    // C21 = M2 + M4
    let c21 = mat_add_nxn(&m2, &m4);

    // C22 = M1 - M2 + M3 + M6
    let c22_part = mat_sub_nxn(&m1, &m2);
    let c22_part2 = mat_add_nxn(&c22_part, &m3);
    let c22 = mat_add_nxn(&c22_part2, &m6);

    let total_m = m * 2;
    let mut res = vec![vec![0i64; total_m]; total_m];
    for r in 0..m {
        for c in 0..m {
            res[r][c] = c11[r][c];
            res[r][c + m] = c12[r][c];
            res[r + m][c] = c21[r][c];
            res[r + m][c + m] = c22[r][c];
        }
    }

    if unpad {
        res.truncate(n);
        for row in &mut res {
            row.truncate(n);
        }
    }

    res
}

/// Matrix multiplication for N×N integer matrices with wrapping arithmetic.
pub fn mat_mul_nxn(a: &[Vec<i64>], b: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let n = a.len();
    if n == 2 {
        let a_fixed = [[a[0][0], a[0][1]], [a[1][0], a[1][1]]];
        let b_fixed = [[b[0][0], b[0][1]], [b[1][0], b[1][1]]];
        let r = mat_mul_2x2(&a_fixed, &b_fixed);
        return vec![vec![r[0][0], r[0][1]], vec![r[1][0], r[1][1]]];
    }
    if n >= 4 {
        return mat_mul_strassen(a, b);
    }
    let mut res = vec![vec![0i64; n]; n];
    for r in 0..n {
        for c in 0..n {
            let mut sum: i64 = 0;
            for k in 0..n {
                sum = sum.wrapping_add(a[r][k].wrapping_mul(b[k][c]));
            }
            res[r][c] = sum;
        }
    }
    res
}

/// Binary exponentiation for N×N integer matrices (N ≤ 8, exp ≥ 0).
/// Uses Strassen block recursion when N >= 4.
pub fn mat_pow_nxn(m: &[Vec<i64>], exp: i64) -> Vec<Vec<i64>> {
    let n = m.len();
    if n == 2 {
        let m_fixed = [[m[0][0], m[0][1]], [m[1][0], m[1][1]]];
        let r = mat_pow_2x2(&m_fixed, exp);
        return vec![vec![r[0][0], r[0][1]], vec![r[1][0], r[1][1]]];
    }
    let mut res = vec![vec![0i64; n]; n];
    for (i, row) in res.iter_mut().enumerate().take(n) {
        row[i] = 1;
    }
    if exp <= 0 {
        return res;
    }
    let mut base = m.to_vec();
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            res = mat_mul_nxn(&res, &base);
        }
        if e > 1 {
            base = mat_mul_nxn(&base, &base);
        }
        e >>= 1;
    }
    res
}

/// Fraction-free Bareiss algorithm for integer matrix determinant.
fn det_bareiss(matrix: &[Vec<i128>]) -> Option<i128> {
    let n = matrix.len();
    if n == 0 {
        return Some(1);
    }
    if n == 1 {
        return Some(matrix[0][0]);
    }
    let mut m = matrix.to_vec();
    let mut sign: i128 = 1;
    let mut prev_pivot: i128 = 1;

    for k in 0..(n - 1) {
        if m[k][k] == 0 {
            let swap_row =
                m.iter().enumerate().skip(k + 1).find_map(
                    |(r, row)| {
                        if row[k] != 0 {
                            Some(r)
                        } else {
                            None
                        }
                    },
                );
            if let Some(r) = swap_row {
                m.swap(k, r);
                sign = -sign;
            } else {
                return Some(0);
            }
        }

        let pivot = m[k][k];
        for i in (k + 1)..n {
            for j in (k + 1)..n {
                let term1 = pivot.checked_mul(m[i][j])?;
                let term2 = m[i][k].checked_mul(m[k][j])?;
                let num = term1.checked_sub(term2)?;
                m[i][j] = num / prev_pivot;
            }
        }
        prev_pivot = pivot;
    }

    sign.checked_mul(m[n - 1][n - 1])
}

/// Solve M * x = b using Cramer's rule in exact integer arithmetic.
fn solve_cramer(m: &[Vec<i64>], b: &[i64]) -> Option<Vec<i64>> {
    let dim = m.len();
    let m_128: Vec<Vec<i128>> = m
        .iter()
        .map(|row| row.iter().map(|&x| x as i128).collect())
        .collect();
    let det_m = det_bareiss(&m_128)?;
    if det_m == 0 {
        return None;
    }
    let mut sol = Vec::with_capacity(dim);
    for col in 0..dim {
        let mut m_col = m_128.clone();
        for r in 0..dim {
            m_col[r][col] = b[r] as i128;
        }
        let det_col = det_bareiss(&m_col)?;
        if det_col % det_m != 0 {
            return None;
        }
        let val = det_col / det_m;
        if val > i64::MAX as i128 || val < i64::MIN as i128 {
            return None;
        }
        sol.push(val as i64);
    }
    Some(sol)
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Fraction-free Gaussian elimination fallback for systems where det == 0 (rank deficient).
fn solve_gaussian_integer(equations: &[Vec<i64>], targets: &[i64]) -> Option<Vec<i64>> {
    let m = equations.len();
    if m == 0 {
        return None;
    }
    let d = equations[0].len();
    let mut aug: Vec<Vec<i128>> = Vec::with_capacity(m);
    for i in 0..m {
        let mut row = Vec::with_capacity(d + 1);
        for &val in &equations[i] {
            row.push(val as i128);
        }
        row.push(targets[i] as i128);
        aug.push(row);
    }

    let mut pivot_row = 0;
    let mut pivot_cols = Vec::new();

    for col in 0..d {
        if pivot_row >= m {
            break;
        }
        let best_r = aug
            .iter()
            .enumerate()
            .skip(pivot_row)
            .take(m.saturating_sub(pivot_row))
            .find_map(|(r, row)| if row[col] != 0 { Some(r) } else { None });
        let Some(r) = best_r else {
            continue;
        };
        aug.swap(pivot_row, r);
        let p_val = aug[pivot_row][col];
        pivot_cols.push((pivot_row, col));

        for row_idx in (pivot_row + 1)..m {
            if aug[row_idx][col] != 0 {
                let factor = aug[row_idx][col];
                let pivot_vals: Vec<i128> = aug[pivot_row][col..=d].to_vec();
                for (target, &p_c) in aug[row_idx][col..=d].iter_mut().zip(pivot_vals.iter()) {
                    let term1 = target.checked_mul(p_val)?;
                    let term2 = p_c.checked_mul(factor)?;
                    *target = term1.checked_sub(term2)?;
                }
                let mut g: i128 = 0;
                for &val in &aug[row_idx][col..=d] {
                    g = gcd(g, val);
                }
                if g > 1 {
                    for val in &mut aug[row_idx][col..=d] {
                        *val /= g;
                    }
                }
            }
        }
        pivot_row += 1;
    }

    for row in aug.iter().take(m).skip(pivot_row) {
        if row[d] != 0 {
            return None;
        }
    }

    let mut sol = vec![0i128; d];
    for &(r, c) in pivot_cols.iter().rev() {
        let mut rhs = aug[r][d];
        for j in (c + 1)..d {
            let prod = aug[r][j].checked_mul(sol[j])?;
            rhs = rhs.checked_sub(prod)?;
        }
        let coeff = aug[r][c];
        if coeff == 0 || rhs % coeff != 0 {
            return None;
        }
        sol[c] = rhs / coeff;
    }

    let mut res = Vec::with_capacity(d);
    for val in sol {
        if val > i64::MAX as i128 || val < i64::MIN as i128 {
            return None;
        }
        res.push(val as i64);
    }
    Some(res)
}

/// Given n sample trajectories (each a &[i64]), attempt to fit the N-way
/// first-order linear system x(k) = A * x(k-1) + c.
/// Uses Cramer's rule / integer Gaussian elimination for small N (N ≤ 8).
/// Returns None if no exact integer solution exists.
pub fn detect_nway_linear_system(trajectories: &[Vec<i64>]) -> Option<NWayLinearSystem> {
    let n = trajectories.len();
    if !(1..=8).contains(&n) {
        return None;
    }

    let min_len = trajectories.iter().map(|t| t.len()).min()?;
    if min_len < n + 3 {
        return None;
    }

    let init: Vec<i64> = trajectories.iter().map(|t| t[0]).collect();

    let mut a_matrix = Vec::with_capacity(n);
    let mut c_vector = Vec::with_capacity(n);

    // Prepare equations across all steps k = 1..min_len
    let num_steps = min_len - 1;
    let mut x_prev_hom = Vec::with_capacity(num_steps);
    let mut x_prev_aff = Vec::with_capacity(num_steps);
    for k in 1..min_len {
        let mut row_hom = Vec::with_capacity(n);
        for traj in trajectories {
            row_hom.push(traj[k - 1]);
        }
        let mut row_aff = row_hom.clone();
        row_aff.push(1);
        x_prev_hom.push(row_hom);
        x_prev_aff.push(row_aff);
    }

    for traj in trajectories.iter().take(n) {
        let target: Vec<i64> = (1..min_len).map(|k| traj[k]).collect();

        let mut row_solved = None;

        // Method 1: Homogeneous with Cramer's rule on first n steps
        if num_steps >= n {
            let m_sub: Vec<Vec<i64>> = x_prev_hom[0..n].to_vec();
            let b_sub: Vec<i64> = target[0..n].to_vec();
            if let Some(sol) = solve_cramer(&m_sub, &b_sub) {
                let mut valid = true;
                for k in 0..num_steps {
                    let mut pred: i64 = 0;
                    for j in 0..n {
                        pred = pred.wrapping_add(sol[j].wrapping_mul(x_prev_hom[k][j]));
                    }
                    if pred != target[k] {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    row_solved = Some((sol, 0i64));
                }
            }
        }

        // Method 2: Affine with Cramer's rule on first n + 1 steps
        if row_solved.is_none() && num_steps > n {
            let m_sub: Vec<Vec<i64>> = x_prev_aff[0..(n + 1)].to_vec();
            let b_sub: Vec<i64> = target[0..(n + 1)].to_vec();
            if let Some(sol) = solve_cramer(&m_sub, &b_sub) {
                let a_row = sol[0..n].to_vec();
                let c_val = sol[n];
                let mut valid = true;
                for k in 0..num_steps {
                    let mut pred: i64 = c_val;
                    for j in 0..n {
                        pred = pred.wrapping_add(a_row[j].wrapping_mul(x_prev_hom[k][j]));
                    }
                    if pred != target[k] {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    row_solved = Some((a_row, c_val));
                }
            }
        }

        // Method 3: Fallback Gaussian elimination (homogeneous) for rank-deficient states
        if row_solved.is_none() {
            if let Some(sol) = solve_gaussian_integer(&x_prev_hom, &target) {
                let mut valid = true;
                for k in 0..num_steps {
                    let mut pred: i64 = 0;
                    for j in 0..n {
                        pred = pred.wrapping_add(sol[j].wrapping_mul(x_prev_hom[k][j]));
                    }
                    if pred != target[k] {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    row_solved = Some((sol, 0i64));
                }
            }
        }

        // Method 4: Fallback Gaussian elimination (affine)
        if row_solved.is_none() {
            if let Some(sol) = solve_gaussian_integer(&x_prev_aff, &target) {
                let a_row = sol[0..n].to_vec();
                let c_val = sol[n];
                let mut valid = true;
                for k in 0..num_steps {
                    let mut pred: i64 = c_val;
                    for j in 0..n {
                        pred = pred.wrapping_add(a_row[j].wrapping_mul(x_prev_hom[k][j]));
                    }
                    if pred != target[k] {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    row_solved = Some((a_row, c_val));
                }
            }
        }

        let (a_row, c_val) = row_solved?;
        a_matrix.push(a_row);
        c_vector.push(c_val);
    }

    Some(NWayLinearSystem {
        n,
        a: a_matrix,
        c: c_vector,
        init,
    })
}

/// Given the detected NWayLinearSystem and the symbolic trip count `num_iters`,
/// return symbolic closed-form terms for each of the n variables.
///
/// - If `num_iters` is a compile-time constant: compute x(n) = M^n * x(0) directly.
/// - If symbolic: emit `__nway_recurrence_i` intrinsic calls (one per variable i).
pub fn solve_nway_recurrence(
    sys: &NWayLinearSystem,
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> Vec<SymTermId> {
    let n = sys.n;

    // Case 1: Constant number of iterations -> exact matrix exponentiation
    let iter_term = interner.get(num_iters).clone();
    if let SymTerm::ConstInt(n_val, _) = iter_term {
        if n_val >= 0 {
            let mut aug = vec![vec![0i64; n + 1]; n + 1];
            for (r, row) in aug.iter_mut().enumerate().take(n) {
                for (c, cell) in row.iter_mut().enumerate().take(n) {
                    *cell = sys.a[r][c];
                }
                row[n] = sys.c[r];
            }
            aug[n][n] = 1;

            let m_pow = mat_pow_nxn(&aug, n_val);
            let mut result_terms = Vec::with_capacity(n);
            for row in m_pow.iter().take(n) {
                let mut val = row[n];
                for (j, &init_val) in sys.init.iter().enumerate().take(n) {
                    val = val.wrapping_add(row[j].wrapping_mul(init_val));
                }
                result_terms.push(interner.intern_int(val));
            }
            return result_terms;
        }
    }

    // Case 2: Symbolic number of iterations -> emit __nway_recurrence_i
    // Calling convention: (n, A[0][0]..A[n-1][n-1], c[0]..c[n-1], x0[0]..x0[n-1], num_iters)
    let mut args = Vec::with_capacity(1 + n * n + n + n + 1);
    args.push(interner.intern_int(n as i64));
    for r in 0..n {
        for c in 0..n {
            args.push(interner.intern_int(sys.a[r][c]));
        }
    }
    for r in 0..n {
        args.push(interner.intern_int(sys.c[r]));
    }
    for r in 0..n {
        args.push(interner.intern_int(sys.init[r]));
    }
    args.push(num_iters);

    let mut result_terms = Vec::with_capacity(n);
    for i in 0..n {
        let callee = format!("__nway_recurrence_{}", i);
        result_terms.push(interner.intern_call(callee, args.clone(), Type::I64));
    }
    result_terms
}

/// A detected nonlinear recurrence, including polynomial sums, geometric series, and exponential powers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonlinearRecurrence {
    /// Exponential power: x_k = init * base^k (base is loop-invariant)
    ExponentialPower { init: SymTermId, base: SymTermId },
    /// Geometric series: S_k = init + c * (ratio^k - 1) / (ratio - 1)
    GeometricSeries {
        init: SymTermId,
        coeff: SymTermId,
        ratio: SymTermId,
    },
    /// Polynomial sum: S_k = init + sum_{i=0}^{k-1} (a * i^2 + b * i + c)
    PolynomialSum {
        init: SymTermId,
        a: i64,
        b: i64,
        c: i64,
    },
    /// Cubic polynomial sum: S_k = init + sum_{i=0}^{k-1} (a * i^3 + b * i^2 + c * i + d)
    CubicSum {
        init: SymTermId,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
    },
    /// Quadratic recurrence: T_k = T_{k-1}^2 with T_0 = b => T_k = b^(2^k)
    PowerTower { init: SymTermId },
}

/// Solves a detected NonlinearRecurrence into a closed-form SymTermId in terms of num_iters.
pub fn solve_nonlinear_recurrence(
    rec: &NonlinearRecurrence,
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> SymTermId {
    match rec {
        NonlinearRecurrence::ExponentialPower { init, base } => {
            // Check if num_iters, base, and init are compile-time constants
            let iter_val = match interner.get(num_iters) {
                SymTerm::ConstInt(n, _) => Some(*n),
                _ => None,
            };
            let base_val = match interner.get(*base) {
                SymTerm::ConstInt(b, _) => Some(*b),
                _ => None,
            };
            let init_val = match interner.get(*init) {
                SymTerm::ConstInt(v, _) => Some(*v),
                _ => None,
            };

            if let (Some(n), Some(b), Some(v)) = (iter_val, base_val, init_val) {
                if n >= 0 {
                    let mut res: i64 = v;
                    let mut cur_base = b;
                    let mut e = n;
                    while e > 0 {
                        if e & 1 == 1 {
                            res = res.wrapping_mul(cur_base);
                        }
                        if e > 1 {
                            cur_base = cur_base.wrapping_mul(cur_base);
                        }
                        e >>= 1;
                    }
                    return interner.intern_int(res);
                }
            }

            // Symbolic base^n
            let pow_term = interner.intern_binary(BinaryOp::Pow, *base, num_iters, Type::I64);
            interner.intern_binary(BinaryOp::Mul, *init, pow_term, Type::I64)
        }

        NonlinearRecurrence::GeometricSeries { init, coeff, ratio } => {
            let iter_val = match interner.get(num_iters) {
                SymTerm::ConstInt(n, _) => Some(*n),
                _ => None,
            };
            let ratio_val = match interner.get(*ratio) {
                SymTerm::ConstInt(r, _) => Some(*r),
                _ => None,
            };
            let coeff_val = match interner.get(*coeff) {
                SymTerm::ConstInt(c, _) => Some(*c),
                _ => None,
            };
            let init_val = match interner.get(*init) {
                SymTerm::ConstInt(v, _) => Some(*v),
                _ => None,
            };

            if let (Some(n), Some(r), Some(c), Some(v)) = (iter_val, ratio_val, coeff_val, init_val)
            {
                if n >= 0 {
                    if r == 1 {
                        return interner.intern_int(v.wrapping_add(c.wrapping_mul(n)));
                    }
                    let mut pow_r: i64 = 1;
                    let mut cur_base = r;
                    let mut e = n;
                    while e > 0 {
                        if e & 1 == 1 {
                            pow_r = pow_r.wrapping_mul(cur_base);
                        }
                        if e > 1 {
                            cur_base = cur_base.wrapping_mul(cur_base);
                        }
                        e >>= 1;
                    }
                    let num = pow_r.wrapping_sub(1);
                    let den = r.wrapping_sub(1);
                    let sum_term = c.wrapping_mul(num) / den;
                    return interner.intern_int(v.wrapping_add(sum_term));
                }
            }

            // Symbolic: init + coeff * (ratio^n - 1) / (ratio - 1)
            let one = interner.intern_int(1);
            let pow_term = interner.intern_binary(BinaryOp::Pow, *ratio, num_iters, Type::I64);
            let num = interner.intern_binary(BinaryOp::Sub, pow_term, one, Type::I64);
            let coeff_num = interner.intern_binary(BinaryOp::Mul, *coeff, num, Type::I64);
            let den = interner.intern_binary(BinaryOp::Sub, *ratio, one, Type::I64);
            let div_term = interner.intern_binary(BinaryOp::Div, coeff_num, den, Type::I64);
            interner.intern_binary(BinaryOp::Add, *init, div_term, Type::I64)
        }

        NonlinearRecurrence::PolynomialSum { init, a, b, c } => {
            let iter_val = match interner.get(num_iters) {
                SymTerm::ConstInt(n, _) => Some(*n),
                _ => None,
            };
            let init_val = match interner.get(*init) {
                SymTerm::ConstInt(v, _) => Some(*v),
                _ => None,
            };

            if let (Some(n), Some(v)) = (iter_val, init_val) {
                if n >= 0 {
                    let mut total = v;
                    for k in 0..n {
                        let term = a
                            .wrapping_mul(k.wrapping_mul(k))
                            .wrapping_add(b.wrapping_mul(k))
                            .wrapping_add(*c);
                        total = total.wrapping_add(term);
                    }
                    return interner.intern_int(total);
                }
            }

            // Symbolic: S_n = init + c * n + b * n(n-1)/2 + a * n(n-1)(2n-1)/6
            let one = interner.intern_int(1);
            let two = interner.intern_int(2);
            let six = interner.intern_int(6);
            let n_minus_1 = interner.intern_binary(BinaryOp::Sub, num_iters, one, Type::I64);

            let mut res = *init;

            // c * n
            if *c != 0 {
                let c_term = interner.intern_int(*c);
                let c_n = interner.intern_binary(BinaryOp::Mul, c_term, num_iters, Type::I64);
                res = interner.intern_binary(BinaryOp::Add, res, c_n, Type::I64);
            }

            // b * n(n-1) / 2
            if *b != 0 {
                let b_term = interner.intern_int(*b);
                let n_times_n1 =
                    interner.intern_binary(BinaryOp::Mul, num_iters, n_minus_1, Type::I64);
                let tri = interner.intern_binary(BinaryOp::Div, n_times_n1, two, Type::I64);
                let b_tri = interner.intern_binary(BinaryOp::Mul, b_term, tri, Type::I64);
                res = interner.intern_binary(BinaryOp::Add, res, b_tri, Type::I64);
            }

            // a * n(n-1)(2n-1) / 6
            if *a != 0 {
                let a_term = interner.intern_int(*a);
                let two_n = interner.intern_binary(BinaryOp::Mul, two, num_iters, Type::I64);
                let two_n_minus_1 = interner.intern_binary(BinaryOp::Sub, two_n, one, Type::I64);
                let n_times_n1 =
                    interner.intern_binary(BinaryOp::Mul, num_iters, n_minus_1, Type::I64);
                let pyr_num =
                    interner.intern_binary(BinaryOp::Mul, n_times_n1, two_n_minus_1, Type::I64);
                let pyr = interner.intern_binary(BinaryOp::Div, pyr_num, six, Type::I64);
                let a_pyr = interner.intern_binary(BinaryOp::Mul, a_term, pyr, Type::I64);
                res = interner.intern_binary(BinaryOp::Add, res, a_pyr, Type::I64);
            }

            res
        }

        NonlinearRecurrence::CubicSum { init, a, b, c, d } => {
            let iter_val = match interner.get(num_iters) {
                SymTerm::ConstInt(n, _) => Some(*n),
                _ => None,
            };
            let init_val = match interner.get(*init) {
                SymTerm::ConstInt(v, _) => Some(*v),
                _ => None,
            };

            if let (Some(n), Some(v)) = (iter_val, init_val) {
                if n >= 0 {
                    let mut total = v;
                    for k in 0..n {
                        let k2 = k.wrapping_mul(k);
                        let k3 = k2.wrapping_mul(k);
                        let term = a
                            .wrapping_mul(k3)
                            .wrapping_add(b.wrapping_mul(k2))
                            .wrapping_add(c.wrapping_mul(k))
                            .wrapping_add(*d);
                        total = total.wrapping_add(term);
                    }
                    return interner.intern_int(total);
                }
            }

            // Symbolic: S_n = init + d*n + c*n(n-1)/2 + b*n(n-1)(2n-1)/6 + a * (n(n-1)/2)^2
            let one = interner.intern_int(1);
            let two = interner.intern_int(2);
            let six = interner.intern_int(6);
            let n_minus_1 = interner.intern_binary(BinaryOp::Sub, num_iters, one, Type::I64);

            let mut res = *init;

            if *d != 0 {
                let d_term = interner.intern_int(*d);
                let d_n = interner.intern_binary(BinaryOp::Mul, d_term, num_iters, Type::I64);
                res = interner.intern_binary(BinaryOp::Add, res, d_n, Type::I64);
            }

            let n_times_n1 = interner.intern_binary(BinaryOp::Mul, num_iters, n_minus_1, Type::I64);
            let tri = interner.intern_binary(BinaryOp::Div, n_times_n1, two, Type::I64);
            if *c != 0 {
                let c_term = interner.intern_int(*c);
                let c_tri = interner.intern_binary(BinaryOp::Mul, c_term, tri, Type::I64);
                res = interner.intern_binary(BinaryOp::Add, res, c_tri, Type::I64);
            }

            if *b != 0 {
                let b_term = interner.intern_int(*b);
                let two_n = interner.intern_binary(BinaryOp::Mul, two, num_iters, Type::I64);
                let two_n_minus_1 = interner.intern_binary(BinaryOp::Sub, two_n, one, Type::I64);
                let pyr_num =
                    interner.intern_binary(BinaryOp::Mul, n_times_n1, two_n_minus_1, Type::I64);
                let pyr = interner.intern_binary(BinaryOp::Div, pyr_num, six, Type::I64);
                let b_pyr = interner.intern_binary(BinaryOp::Mul, b_term, pyr, Type::I64);
                res = interner.intern_binary(BinaryOp::Add, res, b_pyr, Type::I64);
            }

            if *a != 0 {
                let a_term = interner.intern_int(*a);
                let tri_sq = interner.intern_binary(BinaryOp::Mul, tri, tri, Type::I64);
                let a_tri_sq = interner.intern_binary(BinaryOp::Mul, a_term, tri_sq, Type::I64);
                res = interner.intern_binary(BinaryOp::Add, res, a_tri_sq, Type::I64);
            }

            res
        }

        NonlinearRecurrence::PowerTower { init } => {
            let iter_val = match interner.get(num_iters) {
                SymTerm::ConstInt(n, _) => Some(*n),
                _ => None,
            };
            let init_val = match interner.get(*init) {
                SymTerm::ConstInt(v, _) => Some(*v),
                _ => None,
            };

            if let (Some(n), Some(v)) = (iter_val, init_val) {
                if n >= 0 {
                    let mut cur = v;
                    for _ in 0..n {
                        cur = cur.wrapping_mul(cur);
                    }
                    return interner.intern_int(cur);
                }
            }

            // T_k = init^(2^k)
            let two = interner.intern_int(2);
            let exp_term = interner.intern_binary(BinaryOp::Pow, two, num_iters, Type::I64);
            interner.intern_binary(BinaryOp::Pow, *init, exp_term, Type::I64)
        }
    }
}

/// Attempts to fit a sequence of samples to a nonlinear or polynomial recurrence model.
pub fn detect_nonlinear_recurrence(
    samples: &[i64],
    interner: &mut TermInterner,
) -> Option<NonlinearRecurrence> {
    if samples.len() < 3 {
        return None;
    }

    let s0 = samples[0];
    let s1 = samples[1];

    // Check 1: Exponential power: s_k = s0 * r^k
    if s0 != 0 && s1 != 0 && s1 % s0 == 0 {
        let r = s1 / s0;
        let mut is_geom = true;
        let mut cur = s0;
        for &s in samples {
            if s != cur {
                is_geom = false;
                break;
            }
            cur = cur.wrapping_mul(r);
        }
        if is_geom {
            return Some(NonlinearRecurrence::ExponentialPower {
                init: interner.intern_int(s0),
                base: interner.intern_int(r),
            });
        }
    }

    // Check 2: Quadratic recurrence T_k = T_{k-1}^2 (PowerTower)
    if samples.len() >= 3 && s0 >= 2 {
        let mut is_tower = true;
        let mut cur = s0;
        for &s in samples {
            if s != cur {
                is_tower = false;
                break;
            }
            cur = match cur.checked_mul(cur) {
                Some(next) => next,
                None => {
                    is_tower = false;
                    break;
                }
            };
        }
        if is_tower {
            return Some(NonlinearRecurrence::PowerTower {
                init: interner.intern_int(s0),
            });
        }
    }

    // Check 3: Geometric series sum: S_{k} = S_{k-1} + c * r^{k-1}
    // Differences d_k = S_{k+1} - S_k form a geometric progression!
    if samples.len() >= 4 {
        let mut diffs = Vec::with_capacity(samples.len() - 1);
        for i in 0..(samples.len() - 1) {
            diffs.push(samples[i + 1].wrapping_sub(samples[i]));
        }
        let d0 = diffs[0];
        let d1 = diffs[1];
        if d0 != 0 && d1 != 0 && d1 % d0 == 0 {
            let r = d1 / d0;
            if r != 1 && r != 0 {
                let mut is_geom_diff = true;
                let mut cur_d = d0;
                for &d in &diffs {
                    if d != cur_d {
                        is_geom_diff = false;
                        break;
                    }
                    cur_d = cur_d.wrapping_mul(r);
                }
                if is_geom_diff {
                    return Some(NonlinearRecurrence::GeometricSeries {
                        init: interner.intern_int(s0),
                        coeff: interner.intern_int(d0),
                        ratio: interner.intern_int(r),
                    });
                }
            }
        }
    }

    // Check 4: Degree 2 polynomial sum: S_k = S_0 + sum_{i=0}^{k-1} (a * i^2 + b * i + c)
    if samples.len() >= 5 {
        let mut d1 = Vec::new();
        for i in 0..(samples.len() - 1) {
            d1.push(samples[i + 1].wrapping_sub(samples[i]));
        }
        let mut d2 = Vec::new();
        for i in 0..(d1.len() - 1) {
            d2.push(d1[i + 1].wrapping_sub(d1[i]));
        }
        let mut d3 = Vec::new();
        for i in 0..(d2.len() - 1) {
            d3.push(d2[i + 1].wrapping_sub(d2[i]));
        }

        if d3.iter().all(|&x| x == d3[0]) && d3[0] != 0 && d3[0] % 2 == 0 {
            let a = d3[0] / 2;
            let b = d2[0] - a;
            let c = d1[0];
            return Some(NonlinearRecurrence::PolynomialSum {
                init: interner.intern_int(s0),
                a,
                b,
                c,
            });
        }

        // Check 5: Degree 3 polynomial sum (Cubic)
        let mut d4 = Vec::new();
        for i in 0..(d3.len() - 1) {
            d4.push(d3[i + 1].wrapping_sub(d3[i]));
        }
        if d4.len() >= 2 && d4.iter().all(|&x| x == d4[0]) && d4[0] != 0 && d4[0] % 6 == 0 {
            let a = d4[0] / 6;
            let b = (d3[0] - 6 * a) / 2;
            let c = d2[0] - a - b;
            let d = d1[0];
            return Some(NonlinearRecurrence::CubicSum {
                init: interner.intern_int(s0),
                a,
                b,
                c,
                d,
            });
        }
    }

    None
}

/// Extracts linear coefficients for `term` in terms of `vars`.
/// Returns `(Vec<i64>, i64)` where `term = sum_i coeffs[i] * vars[i] + const_val`.
pub fn extract_linear_coeffs(
    term: SymTermId,
    vars: &[SymTermId],
    interner: &TermInterner,
) -> Option<(Vec<i64>, i64)> {
    // Direct identity match by SymTermId
    if let Some(pos) = vars.iter().position(|&v| v == term) {
        let mut coeffs = vec![0i64; vars.len()];
        coeffs[pos] = 1;
        return Some((coeffs, 0));
    }

    match interner.get(term) {
        SymTerm::ConstInt(val, _) => Some((vec![0i64; vars.len()], *val)),
        SymTerm::Var(p, _) => {
            // Match variable by Place name if IDs differ
            if let Some(pos) = vars.iter().position(|&v| match interner.get(v) {
                SymTerm::Var(pv, _) => pv.local == p.local,
                _ => false,
            }) {
                let mut coeffs = vec![0i64; vars.len()];
                coeffs[pos] = 1;
                Some((coeffs, 0))
            } else {
                None
            }
        }
        SymTerm::Binary(BinaryOp::Add, l, r, _) => {
            let (c_l, k_l) = extract_linear_coeffs(*l, vars, interner)?;
            let (c_r, k_r) = extract_linear_coeffs(*r, vars, interner)?;
            let coeffs = c_l
                .iter()
                .zip(c_r.iter())
                .map(|(&a, &b)| a.wrapping_add(b))
                .collect();
            Some((coeffs, k_l.wrapping_add(k_r)))
        }
        SymTerm::Binary(BinaryOp::Sub, l, r, _) => {
            let (c_l, k_l) = extract_linear_coeffs(*l, vars, interner)?;
            let (c_r, k_r) = extract_linear_coeffs(*r, vars, interner)?;
            let coeffs = c_l
                .iter()
                .zip(c_r.iter())
                .map(|(&a, &b)| a.wrapping_sub(b))
                .collect();
            Some((coeffs, k_l.wrapping_sub(k_r)))
        }
        SymTerm::Binary(BinaryOp::Mul, l, r, _) => {
            if let SymTerm::ConstInt(kl, _) = interner.get(*l) {
                let (c_r, k_r) = extract_linear_coeffs(*r, vars, interner)?;
                let coeffs = c_r.iter().map(|&x| x.wrapping_mul(*kl)).collect();
                Some((coeffs, k_r.wrapping_mul(*kl)))
            } else if let SymTerm::ConstInt(kr, _) = interner.get(*r) {
                let (c_l, k_l) = extract_linear_coeffs(*l, vars, interner)?;
                let coeffs = c_l.iter().map(|&x| x.wrapping_mul(*kr)).collect();
                Some((coeffs, k_l.wrapping_mul(*kr)))
            } else {
                None
            }
        }
        SymTerm::Unary(UnaryOp::Neg, inner, _) => {
            let (c, k) = extract_linear_coeffs(*inner, vars, interner)?;
            let coeffs = c.iter().map(|&x| x.wrapping_neg()).collect();
            Some((coeffs, k.wrapping_neg()))
        }
        _ => None,
    }
}

/// Concrete in-process MIR simulator for evaluating bounded base-case execution.
pub fn simulate_mir_call(
    fn_name: &str,
    args: &[i64],
    funcs: &HashMap<String, &MirFunction>,
    fuel: usize,
) -> Option<i64> {
    let func = funcs.get(fn_name)?;
    if fuel == 0 || func.blocks.is_empty() || func.params.len() != args.len() {
        return None;
    }

    let mut env: HashMap<String, i64> = HashMap::new();
    for ((p, _), &val) in func.params.iter().zip(args.iter()) {
        env.insert(p.clone(), val);
    }

    let block_map: HashMap<&BasicBlockId, &MirBasicBlock> =
        func.blocks.iter().map(|b| (&b.id, b)).collect();

    let mut curr_bb = &func.blocks[0].id;
    let mut step_count = 0;

    while step_count < 100 {
        step_count += 1;
        let bb = block_map.get(curr_bb)?;
        for stmt in &bb.statements {
            match stmt {
                Statement::Assign(dest, rval) => {
                    let val = match rval {
                        Rvalue::Constant(TypedLiteral::Int(v, _)) => *v,
                        Rvalue::Constant(TypedLiteral::Bool(b)) => {
                            if *b {
                                1
                            } else {
                                0
                            }
                        }
                        Rvalue::Use(p) => env.get(&p.local).copied().unwrap_or(0),
                        Rvalue::BinaryOp(op, l, r) => {
                            let vl = env.get(&l.local).copied().unwrap_or(0);
                            let vr = env.get(&r.local).copied().unwrap_or(0);
                            match op {
                                BinaryOp::Add => vl.wrapping_add(vr),
                                BinaryOp::Sub => vl.wrapping_sub(vr),
                                BinaryOp::Mul => vl.wrapping_mul(vr),
                                BinaryOp::Div => {
                                    if vr != 0 {
                                        vl.wrapping_div(vr)
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::Mod => {
                                    if vr != 0 {
                                        vl.wrapping_rem(vr)
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::Eq => {
                                    if vl == vr {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::Ne => {
                                    if vl != vr {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::Lt => {
                                    if vl < vr {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::Le => {
                                    if vl <= vr {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::Gt => {
                                    if vl > vr {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::Ge => {
                                    if vl >= vr {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                BinaryOp::BitAnd => vl & vr,
                                BinaryOp::BitOr => vl | vr,
                                BinaryOp::BitXor => vl ^ vr,
                                BinaryOp::Shl => vl.wrapping_shl((vr & 63) as u32),
                                BinaryOp::Shr => vl.wrapping_shr((vr & 63) as u32),
                                _ => return None,
                            }
                        }
                        Rvalue::UnaryOp(UnaryOp::Neg, p) => {
                            env.get(&p.local).copied().unwrap_or(0).wrapping_neg()
                        }
                        Rvalue::UnaryOp(UnaryOp::Not, p) => {
                            if env.get(&p.local).copied().unwrap_or(0) == 0 {
                                1
                            } else {
                                0
                            }
                        }
                        Rvalue::Call(callee, call_args) => {
                            let mut arg_vals = Vec::with_capacity(call_args.len());
                            for p in call_args {
                                arg_vals.push(env.get(&p.local).copied().unwrap_or(0));
                            }
                            simulate_mir_call(callee, &arg_vals, funcs, fuel.saturating_sub(1))?
                        }
                        _ => return None,
                    };
                    env.insert(dest.local.clone(), val);
                }
            }
        }

        match &bb.terminator {
            Terminator::Return { value: Some(p) } => return env.get(&p.local).copied(),
            Terminator::Return { value: None } => return Some(0),
            Terminator::Branch { target } => curr_bb = target,
            Terminator::BranchIf {
                condition,
                then_target,
                else_target,
            } => {
                let cond_val = env.get(&condition.local).copied().unwrap_or(0);
                if cond_val != 0 {
                    curr_bb = then_target;
                } else {
                    curr_bb = else_target;
                }
            }
            Terminator::Switch {
                value,
                targets,
                default,
            } => {
                let discr_val = env.get(&value.local).copied().unwrap_or(0);
                let mut matched = false;
                for (val, target) in targets {
                    if *val == discr_val {
                        curr_bb = target;
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    curr_bb = default;
                }
            }
            _ => return None,
        }
    }

    None
}

/// Traces one full iteration of a mutual call cycle symbolically to extract
/// the transition function T(P) mapping entry parameters to the arguments of the next cycle.
pub fn trace_cycle_transition(
    cycle: &[String],
    funcs: &HashMap<String, &MirFunction>,
    interner: &mut TermInterner,
) -> Option<(Vec<SymTermId>, Vec<SymTermId>)> {
    if cycle.len() < 2 {
        return None;
    }
    let unique_funcs: std::collections::HashSet<_> = cycle.iter().collect();
    if unique_funcs.len() < 2 {
        return None;
    }
    let root_func = funcs.get(&cycle[0])?;
    let dummy_root_vars: Vec<SymTermId> = root_func
        .params
        .iter()
        .map(|(p, ty)| {
            interner.intern_var(
                Place {
                    local: format!("__cycle_root_{}", p),
                    projections: vec![],
                },
                ty.clone(),
            )
        })
        .collect();

    let mut curr_args = dummy_root_vars.clone();

    for k in 0..cycle.len() {
        let fn_name = &cycle[k];
        let next_callee = &cycle[(k + 1) % cycle.len()];
        let func = funcs.get(fn_name)?;

        if func.params.len() != curr_args.len() {
            return None;
        }

        let mut env: HashMap<String, SymTermId> = HashMap::new();
        for ((p, _), &arg) in func.params.iter().zip(curr_args.iter()) {
            env.insert(p.clone(), arg);
        }

        let mut found_call_args = None;
        for bb in &func.blocks {
            for stmt in &bb.statements {
                let Statement::Assign(dest, rval) = stmt;
                let term = match rval {
                    Rvalue::Constant(TypedLiteral::Int(v, _)) => interner.intern_int(*v),
                    Rvalue::Constant(TypedLiteral::Bool(b)) => interner.intern_bool(*b),
                    Rvalue::Use(p) => env
                        .get(&p.local)
                        .copied()
                        .unwrap_or_else(|| interner.intern_int(0)),
                    Rvalue::BinaryOp(op, l, r) => {
                        let tl = env
                            .get(&l.local)
                            .copied()
                            .unwrap_or_else(|| interner.intern_int(0));
                        let tr = env
                            .get(&r.local)
                            .copied()
                            .unwrap_or_else(|| interner.intern_int(0));
                        interner.intern_binary(*op, tl, tr, Type::I64)
                    }
                    Rvalue::UnaryOp(UnaryOp::Neg, p) => {
                        let tp = env
                            .get(&p.local)
                            .copied()
                            .unwrap_or_else(|| interner.intern_int(0));
                        interner.intern_unary(UnaryOp::Neg, tp, Type::I64)
                    }
                    Rvalue::UnaryOp(UnaryOp::Not, p) => {
                        let tp = env
                            .get(&p.local)
                            .copied()
                            .unwrap_or_else(|| interner.intern_bool(false));
                        interner.intern_unary(UnaryOp::Not, tp, Type::Bool)
                    }
                    Rvalue::Call(callee, call_args) if callee == next_callee => {
                        // Ensure call is in tail position: the basic block returns this call's dest
                        let is_tail = match &bb.terminator {
                            Terminator::Return { value: Some(ret_p) } => {
                                ret_p.local == dest.local && ret_p.projections.is_empty()
                            }
                            _ => false,
                        };
                        if !is_tail {
                            continue;
                        }
                        let mut args = Vec::with_capacity(call_args.len());
                        for a in call_args {
                            let t = env
                                .get(&a.local)
                                .copied()
                                .unwrap_or_else(|| interner.intern_int(0));
                            args.push(t);
                        }
                        found_call_args = Some(args);
                        interner.intern_int(0)
                    }
                    _ => interner.intern_int(0),
                };
                env.insert(dest.local.clone(), term);
            }
            if found_call_args.is_some() {
                break;
            }
        }

        curr_args = found_call_args?;
    }

    Some((curr_args, dummy_root_vars))
}

/// Solves a cross-function mutual recursion cycle across function boundaries.
///
/// Handles:
/// 1. Zero-accumulator periodic parity/modulo cycles (e.g. `even/odd`, multi-state rings) in $O(1)$.
/// 2. Coupled linear accumulator systems via matrix exponentiation / `solve_nway_recurrence` in $O(\log N)$ / $O(1)$.
pub fn solve_cross_function_cycle(
    cycle: &[String],
    callee: &str,
    call_args: &[SymTermId],
    _param_terms: &[SymTermId],
    program_funcs: &HashMap<String, &MirFunction>,
    interner: &mut TermInterner,
) -> Option<SymTermId> {
    if call_args.is_empty() || cycle.len() < 2 {
        return None;
    }
    let unique_funcs: std::collections::HashSet<_> = cycle.iter().collect();
    if unique_funcs.len() < 2 {
        return None;
    }

    let (cycle_args, dummy_root_vars) = trace_cycle_transition(cycle, program_funcs, interner)?;
    if cycle_args.len() != dummy_root_vars.len() || call_args.len() != dummy_root_vars.len() {
        return None;
    }

    // 1. Identify induction variable: parameter index where cycle_args[i] = dummy_root_vars[i] - step
    let mut iv_candidate = None;
    for (i, (&c_arg, &d_var)) in cycle_args.iter().zip(dummy_root_vars.iter()).enumerate() {
        if let Some((coeffs, k)) = extract_linear_coeffs(c_arg, &[d_var], interner) {
            if coeffs[0] == 1 && k < 0 {
                let step = (-k) as usize;
                if step > 0 && step <= 16 {
                    iv_candidate = Some((i, step));
                    break;
                }
            }
        }
    }

    let (iv_idx, step) = iv_candidate?;
    let iv_actual = call_args[iv_idx];

    let acc_indices: Vec<usize> = (0..dummy_root_vars.len())
        .filter(|&i| i != iv_idx)
        .collect();

    // 2. Case A: Zero-accumulator periodic parity recurrence (e.g. even/odd)
    if acc_indices.is_empty() {
        let mut base_values = Vec::with_capacity(step);
        for r in 0..step {
            let sim_args = vec![r as i64];
            let val = simulate_mir_call(callee, &sim_args, program_funcs, 64)?;
            base_values.push(val);
        }

        // Verify genuine periodicity over subsequent periods:
        for (r, &base_val) in base_values.iter().enumerate().take(step) {
            let next_val1 = simulate_mir_call(callee, &[(r + step) as i64], program_funcs, 64)?;
            if next_val1 != base_val {
                return None;
            }
            let next_val2 = simulate_mir_call(callee, &[(r + 2 * step) as i64], program_funcs, 64)?;
            if next_val2 != base_val {
                return None;
            }
        }

        // If constant everywhere:
        if base_values.iter().all(|&v| v == base_values[0]) {
            return Some(interner.intern_int(base_values[0]));
        }

        // If iv_actual is concrete integer:
        if let SymTerm::ConstInt(concrete_n, _) = interner.get(iv_actual) {
            let rem = concrete_n.rem_euclid(step as i64) as usize;
            if rem < base_values.len() {
                return Some(interner.intern_int(base_values[rem]));
            }
        }

        // Symbolic induction variable:
        let step_term = interner.intern_int(step as i64);
        let rem_term = interner.intern_binary(BinaryOp::Mod, iv_actual, step_term, Type::I64);

        if step == 2 {
            let zero_term = interner.intern_int(0);
            let cond = interner.intern_binary(BinaryOp::Eq, rem_term, zero_term, Type::Bool);
            let b0 = interner.intern_int(base_values[0]);
            let b1 = interner.intern_int(base_values[1]);
            return Some(interner.intern_select(cond, b0, b1, Type::I64));
        } else {
            let mut curr_expr = interner.intern_int(base_values[step - 1]);
            for r in (0..(step - 1)).rev() {
                let r_term = interner.intern_int(r as i64);
                let cond = interner.intern_binary(BinaryOp::Eq, rem_term, r_term, Type::Bool);
                let b_r = interner.intern_int(base_values[r]);
                curr_expr = interner.intern_select(cond, b_r, curr_expr, Type::I64);
            }
            return Some(curr_expr);
        }
    }

    // 3. Case B: Coupled linear accumulator systems
    let num_acc = acc_indices.len();
    if num_acc > 8 {
        return None;
    }

    let acc_vars: Vec<SymTermId> = acc_indices.iter().map(|&i| dummy_root_vars[i]).collect();
    let mut a_matrix = Vec::with_capacity(num_acc);
    let mut c_vector = Vec::with_capacity(num_acc);

    for &acc_i in &acc_indices {
        let (coeffs, c_val) = extract_linear_coeffs(cycle_args[acc_i], &acc_vars, interner)?;
        a_matrix.push(coeffs);
        c_vector.push(c_val);
    }

    // Check if iv_actual is concrete integer
    if let SymTerm::ConstInt(concrete_n, _) = interner.get(iv_actual) {
        let n_val = *concrete_n;
        let num_cycles = n_val / (step as i64);
        let rem = n_val.rem_euclid(step as i64);

        if num_cycles >= 0 {
            // Augmented matrix (num_acc + 1) × (num_acc + 1)
            let mut aug = vec![vec![0i64; num_acc + 1]; num_acc + 1];
            for r in 0..num_acc {
                for c in 0..num_acc {
                    aug[r][c] = a_matrix[r][c];
                }
                aug[r][num_acc] = c_vector[r];
            }
            aug[num_acc][num_acc] = 1;

            let m_pow = mat_pow_nxn(&aug, num_cycles);

            // Check if actual initial accumulators are concrete
            let mut acc_concrete = Vec::with_capacity(num_acc);
            for &acc_i in &acc_indices {
                if let SymTerm::ConstInt(val, _) = interner.get(call_args[acc_i]) {
                    acc_concrete.push(Some(*val));
                } else {
                    acc_concrete.push(None);
                }
            }

            if acc_concrete.iter().all(|opt| opt.is_some()) {
                let init_vals: Vec<i64> =
                    acc_concrete.into_iter().map(|o| o.unwrap_or(0)).collect();
                let mut final_accs = Vec::with_capacity(num_acc);
                for row in m_pow.iter().take(num_acc) {
                    let mut val = row[num_acc];
                    for (j, &init_val) in init_vals.iter().enumerate().take(num_acc) {
                        val = val.wrapping_add(row[j].wrapping_mul(init_val));
                    }
                    final_accs.push(val);
                }

                // Simulate callee from remaining remainder state
                let mut sim_args = vec![0i64; dummy_root_vars.len()];
                sim_args[iv_idx] = rem;
                for (j, &acc_i) in acc_indices.iter().enumerate() {
                    sim_args[acc_i] = final_accs[j];
                }

                if let Some(res) = simulate_mir_call(callee, &sim_args, program_funcs, 64) {
                    return Some(interner.intern_int(res));
                }
            }
        }
    }

    // Symbolic trip count: construct NWayLinearSystem
    let mut init_vals = Vec::with_capacity(num_acc);
    for &acc_i in &acc_indices {
        if let SymTerm::ConstInt(val, _) = interner.get(call_args[acc_i]) {
            init_vals.push(*val);
        } else {
            init_vals.push(0);
        }
    }

    let sys = NWayLinearSystem {
        n: num_acc,
        a: a_matrix,
        c: c_vector,
        init: init_vals,
    };

    let step_term = interner.intern_int(step as i64);
    let iters_term = interner.intern_binary(BinaryOp::Div, iv_actual, step_term, Type::I64);
    let closed_accs = solve_nway_recurrence(&sys, iters_term, interner);
    if !closed_accs.is_empty() {
        return Some(closed_accs[0]);
    }

    None
}

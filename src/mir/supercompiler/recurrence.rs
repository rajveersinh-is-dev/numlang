#![allow(clippy::needless_range_loop, clippy::int_plus_one)]

use super::term::{SymTerm, SymTermId, TermInterner};
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

/// Matrix multiplication for N×N integer matrices with wrapping arithmetic.
pub fn mat_mul_nxn(a: &[Vec<i64>], b: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let n = a.len();
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
pub fn mat_pow_nxn(m: &[Vec<i64>], mut exp: i64) -> Vec<Vec<i64>> {
    let n = m.len();
    let mut res = vec![vec![0i64; n]; n];
    for i in 0..n {
        res[i][i] = 1;
    }
    if exp <= 0 {
        return res;
    }
    let mut base = m.to_vec();
    while exp > 0 {
        if exp & 1 == 1 {
            res = mat_mul_nxn(&res, &base);
        }
        if exp > 1 {
            base = mat_mul_nxn(&base, &base);
        }
        exp >>= 1;
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
            let mut swap_row = None;
            for r in (k + 1)..n {
                if m[r][k] != 0 {
                    swap_row = Some(r);
                    break;
                }
            }
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
        let mut best_r = None;
        for r in pivot_row..m {
            if aug[r][col] != 0 {
                best_r = Some(r);
                break;
            }
        }
        let Some(r) = best_r else {
            continue;
        };
        aug.swap(pivot_row, r);
        let p_val = aug[pivot_row][col];
        pivot_cols.push((pivot_row, col));

        for row_idx in (pivot_row + 1)..m {
            if aug[row_idx][col] != 0 {
                let factor = aug[row_idx][col];
                for c in col..=d {
                    let term1 = aug[row_idx][c].checked_mul(p_val)?;
                    let term2 = aug[pivot_row][c].checked_mul(factor)?;
                    aug[row_idx][c] = term1.checked_sub(term2)?;
                }
                let mut g: i128 = 0;
                for c in col..=d {
                    g = gcd(g, aug[row_idx][c]);
                }
                if g > 1 {
                    for c in col..=d {
                        aug[row_idx][c] /= g;
                    }
                }
            }
        }
        pivot_row += 1;
    }

    for r in pivot_row..m {
        if aug[r][d] != 0 {
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

    for i in 0..n {
        let target: Vec<i64> = (1..min_len).map(|k| trajectories[i][k]).collect();

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
        if row_solved.is_none() && num_steps >= n + 1 {
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
            for r in 0..n {
                for c in 0..n {
                    aug[r][c] = sys.a[r][c];
                }
                aug[r][n] = sys.c[r];
            }
            aug[n][n] = 1;

            let m_pow = mat_pow_nxn(&aug, n_val);
            let mut result_terms = Vec::with_capacity(n);
            for i in 0..n {
                let mut val = m_pow[i][n];
                for j in 0..n {
                    val = val.wrapping_add(m_pow[i][j].wrapping_mul(sys.init[j]));
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

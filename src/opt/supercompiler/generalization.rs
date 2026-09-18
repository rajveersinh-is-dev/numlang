//! Closed-form generalization for the NumLang supercompiler.
//!
//! When loop driving terminates (iteration cap or homeomorphic embedding),
//! this module attempts to derive a closed-form expression for the final
//! loop state by fitting the observed sequences.
//!
//! Strategies applied in order:
//!   1. Full unroll                    — if N <= snapshot count, snapshot has the exact answer
//!   2. Whole fixed point              — state stopped changing
//!   3. Whole period                   — all variables repeat with period p
//!   4. Periodic accumulator induction — cyclical generator accumulating into sum (with optional modulo)
//!   5. Polynomial induction           — quadratic/linear with warm-up prefix (int & float, with optional modulo)
//!   6. Geometric progression fitting  — exponential scaling (fast exp)
//!   7. Tail fixed-point / periodic    — tail stabilized or periodic

use std::collections::HashMap;
use super::value::Value;
use super::termination::LoopSnapshot;

/// Attempt to generalize the final state of a loop.
///
/// - `loop_vars`: names of mutable loop variables.
/// - `snapshots`: concrete states collected per iteration (0 = initial pre-loop state).
/// - `total_iters`: the concrete number of iterations the loop will execute, if known.
/// - `var_modulos`: known modulo values for variables in the loop (e.g. `% 1000000007`).
///
/// Returns `Some(final_state)` if a closed form was found, or `None` if the
/// loop must remain as residual code or proceed to concrete simulation.
pub fn generalize_loop(
    loop_vars: &[String],
    snapshots: &[LoopSnapshot],
    total_iters: Option<i64>,
    var_modulos: &HashMap<String, i64>,
) -> Option<HashMap<String, Value>> {
    if snapshots.is_empty() {
        return None;
    }

    // Strategy 1: Full unroll — snapshots already have the exact answer.
    if let Some(n) = total_iters {
        if n >= 0 && (n as usize) < snapshots.len() {
            return Some(snapshots[n as usize].state.clone());
        }
    }

    // Strategy 2: Whole fixed point — all loop vars stabilized.
    if snapshots.len() >= 2 {
        let last = &snapshots[snapshots.len() - 1];
        let prev = &snapshots[snapshots.len() - 2];
        if all_vars_equal(loop_vars, &prev.state, &last.state) {
            return Some(last.state.clone());
        }
    }

    // Strategy 3: Whole period detection across all variables.
    if let Some(result) = detect_whole_period(loop_vars, snapshots, total_iters) {
        return Some(result);
    }

    // Strategy 4: Periodic Accumulator Induction.
    // Handles loops where a generator cycles with period p while an accumulator
    // accumulates the deltas (with or without modulo M).
    if let Some(total) = total_iters {
        if let Some(result) = fit_periodic_accumulator(loop_vars, snapshots, total, var_modulos) {
            return Some(result);
        }
    }

    // Strategy 5: Polynomial Forward Difference Induction (Degrees 1 to 4) with Warm-up.
    // Handles arithmetic progressions, quadratic sums, cubic sums, and quartic polynomials.
    if let Some(total) = total_iters {
        if let Some(result) = fit_polynomial_forward_difference(loop_vars, snapshots, total, var_modulos) {
            return Some(result);
        }
    }

    // Strategy 6: Coupled Linear Recurrence Fitting (O(k^3 log N) Matrix Exponentiation).
    // Handles coupled linear recurrences (e.g. Fibonacci, Tribonacci, coupled oscillators).
    if let Some(total) = total_iters {
        if let Some(result) = fit_coupled_linear_recurrence(loop_vars, snapshots, total, var_modulos) {
            return Some(result);
        }
    }

    // Strategy 7: Geometric Progression Fitting (fast exponentiation).
    if let Some(total) = total_iters {
        if let Some(result) = fit_geometric_all(loop_vars, snapshots, total) {
            return Some(result);
        }
    }

    // Strategy 8: Tail Fixed-Point / Tail Periodic fitting.
    if let Some(total) = total_iters {
        if let Some(result) = fit_tail_patterns(loop_vars, snapshots, total) {
            return Some(result);
        }
    }

    None
}

fn all_vars_equal(
    vars: &[String],
    a: &HashMap<String, Value>,
    b: &HashMap<String, Value>,
) -> bool {
    vars.iter().all(|v| {
        match (a.get(v), b.get(v)) {
            (Some(Value::Int(x)), Some(Value::Int(y))) => x == y,
            (Some(Value::Float(x)), Some(Value::Float(y))) => (x - y).abs() < 1e-12,
            (Some(Value::Bool(x)), Some(Value::Bool(y))) => x == y,
            (Some(Value::Array(ax, _)), Some(Value::Array(bx, _))) => ax == bx,
            (None, None) => true,
            _ => false,
        }
    })
}

/// Detect if all loop variables together form a strict cycle with period p.
fn detect_whole_period(
    loop_vars: &[String],
    snapshots: &[LoopSnapshot],
    total_iters: Option<i64>,
) -> Option<HashMap<String, Value>> {
    let n = snapshots.len();
    if n < 4 {
        return None;
    }

    let total = total_iters?;
    if total < 0 {
        return None;
    }

    for p in 1..=(n / 2) {
        let mut valid = true;
        for k in p..n {
            if !all_vars_equal(loop_vars, &snapshots[k - p].state, &snapshots[k].state) {
                valid = false;
                break;
            }
        }
        if valid {
            let idx = (total as usize) % p;
            if idx < snapshots.len() {
                return Some(snapshots[idx].state.clone());
            }
        }
    }
    None
}

enum VarPeriodModel {
    StrictlyPeriodic,
    PeriodicAccumulatorInt,
    PeriodicAccumulatorFloat,
    ConstantStepInt(i64),
    ConstantStepFloat(f64),
}

/// Periodic Accumulator Induction:
/// Detects when state variables repeat with period p, while accumulator variables
/// sum terms over the period.
fn fit_periodic_accumulator(
    loop_vars: &[String],
    snapshots: &[LoopSnapshot],
    total_iters: i64,
    var_modulos: &HashMap<String, i64>,
) -> Option<HashMap<String, Value>> {
    let n = snapshots.len();
    if n < 6 || total_iters < 0 {
        return None;
    }

    // Try periods p from 1 up to n / 3
    for p in 1..=(n / 3) {
        // Try warm-up k0 from 0 up to 32
        for k0 in 0..=32 {
            if k0 + 2 * p >= n {
                break;
            }

            let mut var_models = HashMap::new();
            let mut all_match = true;

            for var in loop_vars {
                if let Some(model) = classify_var_periodicity(var, snapshots, p, k0) {
                    var_models.insert(var.clone(), model);
                } else {
                    all_match = false;
                    break;
                }
            }

            if all_match && !var_models.is_empty() {
                let mut result = HashMap::new();
                for var in loop_vars {
                    let model = var_models.get(var)?;
                    let val = evaluate_var_period_model(
                        var,
                        model,
                        snapshots,
                        p,
                        k0,
                        total_iters,
                        var_modulos.get(var).copied(),
                    )?;
                    result.insert(var.clone(), val);
                }
                return Some(result);
            }
        }
    }

    None
}

fn classify_var_periodicity(
    var: &str,
    snapshots: &[LoopSnapshot],
    p: usize,
    k0: usize,
) -> Option<VarPeriodModel> {
    let n = snapshots.len();

    // Check if strictly periodic first
    let strictly_periodic = (k0 + p..n).all(|k| {
        match (snapshots[k].state.get(var), snapshots[k - p].state.get(var)) {
            (Some(Value::Int(a)), Some(Value::Int(b))) => a == b,
            (Some(Value::Float(a)), Some(Value::Float(b))) => (a - b).abs() < 1e-12,
            (Some(Value::Bool(a)), Some(Value::Bool(b))) => a == b,
            (Some(Value::Array(a, _)), Some(Value::Array(b, _))) => a == b,
            _ => false,
        }
    });

    if strictly_periodic {
        return Some(VarPeriodModel::StrictlyPeriodic);
    }

    // Check if integer accumulator or step
    let sample = snapshots[k0].state.get(var)?;
    match sample {
        Value::Int(_) => {
            let seq: Vec<i64> = snapshots[k0..]
                .iter()
                .filter_map(|s| s.state.get(var)?.as_int())
                .collect();
            if seq.len() < 2 * p + 1 {
                return None;
            }
            let diffs: Vec<i64> = seq.windows(2).map(|w| w[1].wrapping_sub(w[0])).collect();
            let step0 = diffs[0];
            if diffs.iter().all(|&d| d == step0) {
                return Some(VarPeriodModel::ConstantStepInt(step0));
            }
            // Check if diffs repeat with period p
            let diffs_periodic = (p..diffs.len()).all(|k| diffs[k] == diffs[k - p]);
            if diffs_periodic {
                return Some(VarPeriodModel::PeriodicAccumulatorInt);
            }
        }
        Value::Float(_) => {
            let seq: Vec<f64> = snapshots[k0..]
                .iter()
                .filter_map(|s| s.state.get(var)?.as_float())
                .collect();
            if seq.len() < 2 * p + 1 {
                return None;
            }
            let diffs: Vec<f64> = seq.windows(2).map(|w| w[1] - w[0]).collect();
            let step0 = diffs[0];
            if diffs.iter().all(|&d| (d - step0).abs() < 1e-12) {
                return Some(VarPeriodModel::ConstantStepFloat(step0));
            }
            let diffs_periodic = (p..diffs.len()).all(|k| (diffs[k] - diffs[k - p]).abs() < 1e-9);
            if diffs_periodic {
                return Some(VarPeriodModel::PeriodicAccumulatorFloat);
            }
        }
        _ => {}
    }

    None
}

fn evaluate_var_period_model(
    var: &str,
    model: &VarPeriodModel,
    snapshots: &[LoopSnapshot],
    p: usize,
    k0: usize,
    total_iters: i64,
    var_modulo: Option<i64>,
) -> Option<Value> {
    if total_iters < k0 as i64 {
        return snapshots.get(total_iters as usize)?.state.get(var).cloned();
    }

    let m = total_iters - k0 as i64;
    let q = m / (p as i64);
    let r = (m % (p as i64)) as usize;

    match model {
        VarPeriodModel::StrictlyPeriodic => {
            snapshots.get(k0 + r)?.state.get(var).cloned()
        }
        VarPeriodModel::ConstantStepInt(step) => {
            let v0 = snapshots[k0].state.get(var)?.as_int()?;
            let val = v0.wrapping_add(m.wrapping_mul(*step));
            if let Some(modulus) = var_modulo {
                Some(Value::Int(((val % modulus) + modulus) % modulus))
            } else {
                Some(Value::Int(val))
            }
        }
        VarPeriodModel::ConstantStepFloat(step) => {
            let v0 = snapshots[k0].state.get(var)?.as_float()?;
            Some(Value::Float(v0 + (m as f64) * step))
        }
        VarPeriodModel::PeriodicAccumulatorInt => {
            let v0 = snapshots[k0].state.get(var)?.as_int()?;
            let vp = snapshots[k0 + p].state.get(var)?.as_int()?;
            let vr = snapshots[k0 + r].state.get(var)?.as_int()?;
            let d_period = vp.wrapping_sub(v0);
            let d_rem = vr.wrapping_sub(v0);

            if let Some(modulus) = var_modulo {
                let q_mod = q % modulus;
                let term = (q_mod.wrapping_mul(d_period % modulus)) % modulus;
                let total = (v0 % modulus).wrapping_add(term).wrapping_add(d_rem % modulus);
                let res = ((total % modulus) + modulus) % modulus;
                Some(Value::Int(res))
            } else {
                let val = v0.wrapping_add(q.wrapping_mul(d_period)).wrapping_add(d_rem);
                Some(Value::Int(val))
            }
        }
        VarPeriodModel::PeriodicAccumulatorFloat => {
            let v0 = snapshots[k0].state.get(var)?.as_float()?;
            let vp = snapshots[k0 + p].state.get(var)?.as_float()?;
            let vr = snapshots[k0 + r].state.get(var)?.as_float()?;
            let d_period = vp - v0;
            let d_rem = vr - v0;
            let val = v0 + (q as f64) * d_period + d_rem;
            Some(Value::Float(val))
        }
    }
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn binom_int(m: i64, k: usize) -> Option<i64> {
    if k == 0 {
        return Some(1);
    }
    if m < 0 {
        return None;
    }
    let mut factors: Vec<i64> = (0..k).map(|i| m.wrapping_sub(i as i64)).collect();
    let mut div = match k {
        1 => 1,
        2 => 2,
        3 => 6,
        4 => 24,
        _ => return None,
    };
    for f in factors.iter_mut() {
        let g = gcd_u64(f.unsigned_abs(), div as u64) as i64;
        if g > 1 {
            *f /= g;
            div /= g;
        }
    }
    if div != 1 {
        return None;
    }
    let mut prod: i64 = 1;
    for f in factors {
        prod = prod.wrapping_mul(f);
    }
    Some(prod)
}

fn binom_mod(m: i64, k: usize, modulus: i64) -> Option<i64> {
    if k == 0 {
        return Some(1 % modulus);
    }
    if m < 0 || modulus <= 0 {
        return None;
    }
    let mut factors: Vec<i64> = (0..k).map(|i| m.wrapping_sub(i as i64)).collect();
    let mut div = match k {
        1 => 1,
        2 => 2,
        3 => 6,
        4 => 24,
        _ => return None,
    };
    for f in factors.iter_mut() {
        let g = gcd_u64(f.unsigned_abs(), div as u64) as i64;
        if g > 1 {
            *f /= g;
            div /= g;
        }
    }
    if div != 1 {
        return None;
    }
    let mut prod: i64 = 1;
    for f in factors {
        let term = ((f % modulus) + modulus) % modulus;
        prod = (prod.wrapping_mul(term)) % modulus;
    }
    Some(prod)
}

fn binom_float(m: f64, k: usize) -> f64 {
    if k == 0 {
        return 1.0;
    }
    let mut prod = 1.0;
    for i in 0..k {
        prod *= m - (i as f64);
    }
    let div = match k {
        1 => 1.0,
        2 => 2.0,
        3 => 6.0,
        4 => 24.0,
        _ => 1.0,
    };
    prod / div
}

/// Polynomial Forward Difference Induction (Degrees 0 to 4) with Warm-up:
/// Fits integer and float variables whose differences up to order 4 become constant.
fn fit_polynomial_forward_difference(
    loop_vars: &[String],
    snapshots: &[LoopSnapshot],
    total_iters: i64,
    var_modulos: &HashMap<String, i64>,
) -> Option<HashMap<String, Value>> {
    let n = snapshots.len();
    if n < 6 || total_iters < 0 {
        return None;
    }

    for k0 in 0..=32 {
        if k0 + 6 > n {
            break;
        }

        let mut result = HashMap::new();
        let mut all_match = true;

        for var in loop_vars {
            let is_float = match snapshots[k0].state.get(var) {
                Some(Value::Float(_)) => true,
                Some(Value::Int(_)) => false,
                _ => {
                    all_match = false;
                    break;
                }
            };

            if !is_float {
                let seq: Vec<i64> = snapshots[k0..]
                    .iter()
                    .filter_map(|s| s.state.get(var)?.as_int())
                    .collect();
                if seq.len() < 6 {
                    all_match = false;
                    break;
                }

                // Compute differences up to order 4
                let mut diffs: Vec<Vec<i64>> = Vec::with_capacity(5);
                diffs.push(seq.clone());
                for d in 0..4 {
                    let next_diff: Vec<i64> = diffs[d].windows(2).map(|w| w[1].wrapping_sub(w[0])).collect();
                    diffs.push(next_diff);
                }

                let mut found_deg = None;
                for deg in 0..=4 {
                    let d_seq = &diffs[deg];
                    if d_seq.len() >= 2 && d_seq.iter().all(|&x| x == d_seq[0]) {
                        found_deg = Some(deg);
                        break;
                    }
                }

                if let Some(deg) = found_deg {
                    let m = total_iters - k0 as i64;
                    if let Some(&modulus) = var_modulos.get(var) {
                        let mut total: i64 = 0;
                        for k in 0..=deg {
                            let delta_0 = diffs[k][0];
                            let b = binom_mod(m, k, modulus)?;
                            let term = ((b % modulus).wrapping_mul(delta_0 % modulus)) % modulus;
                            total = (total.wrapping_add(term)) % modulus;
                        }
                        let res = ((total % modulus) + modulus) % modulus;
                        result.insert(var.clone(), Value::Int(res));
                    } else {
                        let mut total: i64 = 0;
                        for k in 0..=deg {
                            let delta_0 = diffs[k][0];
                            let b = binom_int(m, k)?;
                            total = total.wrapping_add(b.wrapping_mul(delta_0));
                        }
                        result.insert(var.clone(), Value::Int(total));
                    }
                } else {
                    all_match = false;
                    break;
                }
            } else {
                let seq: Vec<f64> = snapshots[k0..]
                    .iter()
                    .filter_map(|s| s.state.get(var)?.as_float())
                    .collect();
                if seq.len() < 6 {
                    all_match = false;
                    break;
                }

                let mut diffs: Vec<Vec<f64>> = Vec::with_capacity(5);
                diffs.push(seq.clone());
                for d in 0..4 {
                    let next_diff: Vec<f64> = diffs[d].windows(2).map(|w| w[1] - w[0]).collect();
                    diffs.push(next_diff);
                }

                let mut found_deg = None;
                for deg in 0..=4 {
                    let d_seq = &diffs[deg];
                    if d_seq.len() >= 2 && d_seq.iter().all(|&x| (x - d_seq[0]).abs() < 1e-9) {
                        found_deg = Some(deg);
                        break;
                    }
                }

                if let Some(deg) = found_deg {
                    let m = (total_iters - k0 as i64) as f64;
                    let mut total = 0.0;
                    for k in 0..=deg {
                        let delta_0 = diffs[k][0];
                        let b = binom_float(m, k);
                        total += b * delta_0;
                    }
                    result.insert(var.clone(), Value::Float(total));
                } else {
                    all_match = false;
                    break;
                }
            }
        }

        if all_match && !result.is_empty() {
            return Some(result);
        }
    }

    None
}

fn solve_linear_system(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for i in 0..n {
        let mut pivot = i;
        let mut max_val = a[i][i].abs();
        for row in (i + 1)..n {
            if a[row][i].abs() > max_val {
                max_val = a[row][i].abs();
                pivot = row;
            }
        }
        if max_val < 1e-9 {
            return None;
        }
        if pivot != i {
            a.swap(i, pivot);
            b.swap(i, pivot);
        }
        let pivot_val = a[i][i];
        for col in i..n {
            a[i][col] /= pivot_val;
        }
        b[i] /= pivot_val;

        for row in 0..n {
            if row != i {
                let factor = a[row][i];
                if factor.abs() > 1e-12 {
                    for col in i..n {
                        a[row][col] -= factor * a[i][col];
                    }
                    b[row] -= factor * b[i];
                }
            }
        }
    }
    Some(b)
}

fn mat_mul_int(a: &[Vec<i64>], b: &[Vec<i64>], modulus: Option<i64>) -> Vec<Vec<i64>> {
    let n = a.len();
    let mut res = vec![vec![0i64; n]; n];
    for i in 0..n {
        for k in 0..n {
            let aik = a[i][k];
            for j in 0..n {
                let term = if let Some(m) = modulus {
                    let prod = ((aik % m).wrapping_mul(b[k][j] % m)) % m;
                    (res[i][j].wrapping_add(prod)) % m
                } else {
                    res[i][j].wrapping_add(aik.wrapping_mul(b[k][j]))
                };
                res[i][j] = term;
            }
        }
    }
    if let Some(m) = modulus {
        for row in res.iter_mut() {
            for val in row.iter_mut() {
                *val = ((*val % m) + m) % m;
            }
        }
    }
    res
}

fn mat_pow_int(mut base: Vec<Vec<i64>>, mut exp: i64, modulus: Option<i64>) -> Vec<Vec<i64>> {
    let n = base.len();
    let mut res = vec![vec![0i64; n]; n];
    for i in 0..n {
        res[i][i] = 1;
    }
    while exp > 0 {
        if exp & 1 == 1 {
            res = mat_mul_int(&res, &base, modulus);
        }
        base = mat_mul_int(&base, &base, modulus);
        exp >>= 1;
    }
    res
}

fn mat_mul_float(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut res = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for k in 0..n {
            let aik = a[i][k];
            for j in 0..n {
                res[i][j] += aik * b[k][j];
            }
        }
    }
    res
}

fn mat_pow_float(mut base: Vec<Vec<f64>>, mut exp: i64) -> Vec<Vec<f64>> {
    let n = base.len();
    let mut res = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        res[i][i] = 1.0;
    }
    while exp > 0 {
        if exp & 1 == 1 {
            res = mat_mul_float(&res, &base);
        }
        base = mat_mul_float(&base, &base);
        exp >>= 1;
    }
    res
}

/// Coupled Linear Recurrence Fitting via Binary Matrix Exponentiation:
/// Solves x_{n+1} = M * x_n for coupled integer or float variables.
fn fit_coupled_linear_recurrence(
    loop_vars: &[String],
    snapshots: &[LoopSnapshot],
    total_iters: i64,
    var_modulos: &HashMap<String, i64>,
) -> Option<HashMap<String, Value>> {
    let num_vars = loop_vars.len();
    if num_vars == 0 || num_vars > 4 || total_iters < 0 {
        return None;
    }
    let dim = num_vars + 1; // homogeneous coordinates
    let n = snapshots.len();
    if n < dim + 3 {
        return None;
    }

    // Check if variables are all Int or all Float
    let all_int = loop_vars.iter().all(|v| matches!(snapshots[0].state.get(v), Some(Value::Int(_))));
    let all_float = loop_vars.iter().all(|v| matches!(snapshots[0].state.get(v), Some(Value::Float(_))));
    if !all_int && !all_float {
        return None;
    }

    let mut common_modulus: Option<i64> = None;
    if all_int {
        for v in loop_vars {
            if let Some(&m) = var_modulos.get(v) {
                if let Some(cur) = common_modulus {
                    if cur != m {
                        return None;
                    }
                } else {
                    common_modulus = Some(m);
                }
            }
        }
    }

    for k0 in 0..=8 {
        if k0 + dim + 2 >= n {
            break;
        }

        if all_int {
            let mut state_vecs: Vec<Vec<f64>> = Vec::with_capacity(n - k0);
            for s in &snapshots[k0..] {
                let mut vec = Vec::with_capacity(dim);
                for v in loop_vars {
                    vec.push(s.state.get(v)?.as_int()? as f64);
                }
                vec.push(1.0);
                state_vecs.push(vec);
            }

            let mut a_mat = vec![vec![0.0f64; dim]; dim];
            for c in 0..dim {
                for r in 0..dim {
                    a_mat[c][r] = state_vecs[c][r];
                }
            }

            let mut m_matrix_int: Vec<Vec<i64>> = Vec::with_capacity(dim);
            let mut solved_all = true;

            for r in 0..num_vars {
                let mut b_vec = vec![0.0f64; dim];
                for c in 0..dim {
                    b_vec[c] = state_vecs[c + 1][r];
                }

                if let Some(sol) = solve_linear_system(a_mat.clone(), b_vec) {
                    let mut int_row = Vec::with_capacity(dim);
                    for coeff in sol {
                        let rounded = coeff.round();
                        if (coeff - rounded).abs() > 1e-4 {
                            solved_all = false;
                            break;
                        }
                        int_row.push(rounded as i64);
                    }
                    if !solved_all {
                        break;
                    }
                    m_matrix_int.push(int_row);
                } else {
                    solved_all = false;
                    break;
                }
            }

            if !solved_all {
                continue;
            }

            let mut last_row = vec![0i64; dim];
            last_row[dim - 1] = 1;
            m_matrix_int.push(last_row);

            // Verify M against ALL remaining snapshots
            let mut verified = true;
            for j in 0..(state_vecs.len() - 1) {
                let cur = &state_vecs[j];
                let next = &state_vecs[j + 1];
                for r in 0..num_vars {
                    let mut pred: i64 = 0;
                    for c in 0..dim {
                        let val_c = cur[c].round() as i64;
                        pred = pred.wrapping_add(m_matrix_int[r][c].wrapping_mul(val_c));
                    }
                    if let Some(m) = common_modulus {
                        pred = ((pred % m) + m) % m;
                        let next_val = ((next[r].round() as i64 % m) + m) % m;
                        if pred != next_val {
                            verified = false;
                            break;
                        }
                    } else {
                        let next_val = next[r].round() as i64;
                        if pred != next_val {
                            verified = false;
                            break;
                        }
                    }
                }
                if !verified {
                    break;
                }
            }

            if !verified {
                continue;
            }

            let exp = total_iters - k0 as i64;
            if exp < 0 {
                continue;
            }
            let m_pow = mat_pow_int(m_matrix_int, exp, common_modulus);

            let mut result = HashMap::new();
            for r in 0..num_vars {
                let mut final_val: i64 = 0;
                for c in 0..dim {
                    let val_c = state_vecs[0][c].round() as i64;
                    let term = if let Some(m) = common_modulus {
                        ((m_pow[r][c] % m).wrapping_mul(val_c % m)) % m
                    } else {
                        m_pow[r][c].wrapping_mul(val_c)
                    };
                    final_val = if let Some(m) = common_modulus {
                        (final_val.wrapping_add(term)) % m
                    } else {
                        final_val.wrapping_add(term)
                    };
                }
                if let Some(m) = common_modulus {
                    final_val = ((final_val % m) + m) % m;
                }
                result.insert(loop_vars[r].clone(), Value::Int(final_val));
            }
            return Some(result);
        } else {
            let mut state_vecs: Vec<Vec<f64>> = Vec::with_capacity(n - k0);
            for s in &snapshots[k0..] {
                let mut vec = Vec::with_capacity(dim);
                for v in loop_vars {
                    vec.push(s.state.get(v)?.as_float()?);
                }
                vec.push(1.0);
                state_vecs.push(vec);
            }

            let mut a_mat = vec![vec![0.0f64; dim]; dim];
            for c in 0..dim {
                for r in 0..dim {
                    a_mat[c][r] = state_vecs[c][r];
                }
            }

            let mut m_matrix_float: Vec<Vec<f64>> = Vec::with_capacity(dim);
            let mut solved_all = true;

            for r in 0..num_vars {
                let mut b_vec = vec![0.0f64; dim];
                for c in 0..dim {
                    b_vec[c] = state_vecs[c + 1][r];
                }
                if let Some(sol) = solve_linear_system(a_mat.clone(), b_vec) {
                    m_matrix_float.push(sol);
                } else {
                    solved_all = false;
                    break;
                }
            }

            if !solved_all {
                continue;
            }

            let mut last_row = vec![0.0f64; dim];
            last_row[dim - 1] = 1.0;
            m_matrix_float.push(last_row);

            let mut verified = true;
            for j in 0..(state_vecs.len() - 1) {
                let cur = &state_vecs[j];
                let next = &state_vecs[j + 1];
                for r in 0..num_vars {
                    let mut pred = 0.0f64;
                    for c in 0..dim {
                        pred += m_matrix_float[r][c] * cur[c];
                    }
                    if (pred - next[r]).abs() > 1e-5 {
                        verified = false;
                        break;
                    }
                }
                if !verified {
                    break;
                }
            }

            if !verified {
                continue;
            }

            let exp = total_iters - k0 as i64;
            if exp < 0 {
                continue;
            }
            let m_pow = mat_pow_float(m_matrix_float, exp);

            let mut result = HashMap::new();
            for r in 0..num_vars {
                let mut final_val = 0.0f64;
                for c in 0..dim {
                    final_val += m_pow[r][c] * state_vecs[0][c];
                }
                result.insert(loop_vars[r].clone(), Value::Float(final_val));
            }
            return Some(result);
        }
    }

    None
}

/// Geometric Sequence Fitting via fast exponentiation.
fn fit_geometric_all(
    loop_vars: &[String],
    snapshots: &[LoopSnapshot],
    total_iters: i64,
) -> Option<HashMap<String, Value>> {
    let mut result = HashMap::new();

    for var in loop_vars {
        let seq: Vec<i64> = snapshots
            .iter()
            .filter_map(|s| s.state.get(var)?.as_int())
            .collect();
        if seq.len() < 3 || seq[0] == 0 || seq[1] == 0 || seq[1] % seq[0] != 0 {
            return None;
        }
        let ratio = seq[1] / seq[0];
        for i in 2..seq.len() {
            if seq[i - 1] == 0 || seq[i] % seq[i - 1] != 0 || seq[i] / seq[i - 1] != ratio {
                return None;
            }
        }
        let mut base_cur = ratio;
        let mut exp_cur = total_iters;
        let mut acc: i64 = 1;
        while exp_cur > 0 {
            if exp_cur & 1 == 1 {
                acc = acc.wrapping_mul(base_cur);
            }
            base_cur = base_cur.wrapping_mul(base_cur);
            exp_cur >>= 1;
        }
        result.insert(var.clone(), Value::Int(seq[0].wrapping_mul(acc)));
    }

    if result.is_empty() { None } else { Some(result) }
}

/// Tail fixed-point or tail periodic fitting for each variable.
fn fit_tail_patterns(
    loop_vars: &[String],
    snapshots: &[LoopSnapshot],
    total_iters: i64,
) -> Option<HashMap<String, Value>> {
    let mut result = HashMap::new();

    for var in loop_vars {
        let seq: Vec<i64> = snapshots
            .iter()
            .filter_map(|s| s.state.get(var)?.as_int())
            .collect();

        if let Some(val) = fit_tail_fixed_point(&seq, total_iters) {
            result.insert(var.clone(), Value::Int(val));
            continue;
        }

        if let Some(val) = fit_tail_periodic(&seq, total_iters) {
            result.insert(var.clone(), Value::Int(val));
            continue;
        }

        return None;
    }

    if result.is_empty() { None } else { Some(result) }
}

fn fit_tail_fixed_point(seq: &[i64], target: i64) -> Option<i64> {
    if seq.len() < 4 {
        return None;
    }
    let last = *seq.last()?;
    let tail_len = seq.len().min(16);
    if !seq[seq.len() - tail_len..].iter().all(|&x| x == last) {
        return None;
    }
    let mut t = seq.len() - 1;
    while t > 0 && seq[t - 1] == last {
        t -= 1;
    }
    if target >= t as i64 {
        Some(last)
    } else {
        None
    }
}

fn fit_tail_periodic(seq: &[i64], target: i64) -> Option<i64> {
    let n = seq.len();
    if n < 8 {
        return None;
    }
    for p in 1..=(n / 2) {
        let check_len = (2 * p).min(n);
        let start = n - check_len;
        let mut ok = true;
        for k in (start + p)..n {
            if seq[k] != seq[k - p] {
                ok = false;
                break;
            }
        }
        if ok {
            let mut t = start;
            while t >= p && seq[t] == seq[t - p] {
                t -= 1;
            }
            if target >= t as i64 {
                let offset = ((target - t as i64) as usize) % p;
                return Some(seq[t + offset]);
            }
        }
    }
    None
}

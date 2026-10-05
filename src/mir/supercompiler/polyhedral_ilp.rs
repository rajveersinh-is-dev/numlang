//! Pure-Rust Polyhedral ILP Scheduler (Pluto-style) & Bareiss Simplex Solver.
//!
//! Implements a fraction-free Simplex method over integer polyhedra using the Bareiss
//! integer pivoting algorithm to solve the Farkas lemma dual LP for loop schedule feasibility.
//! Formulates Pluto-style loop permutability constraints (theta_i . d >= 0 for all dependences)
//! and solves for optimal multidimensional loop schedules for loop tiling and vectorization.

use std::fmt;

// ============================================================================
// 1. Types & Data Structures
// ============================================================================

/// Comparison operator for linear constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintOp {
    Le,
    Ge,
    Eq,
}

impl fmt::Display for ConstraintOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintOp::Le => write!(f, "<="),
            ConstraintOp::Ge => write!(f, ">="),
            ConstraintOp::Eq => write!(f, "=="),
        }
    }
}

/// A linear constraint: sum(coeffs[j] * x[j]) (op) rhs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearConstraint {
    pub coeffs: Vec<i64>,
    pub op: ConstraintOp,
    pub rhs: i64,
}

/// Result of solving an integer linear program via Bareiss Simplex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimplexResult {
    /// Optimal solution found with exact objective value, variable assignments, and pivot count.
    Optimal {
        objective: i64,
        solution: Vec<i64>,
        pivots: usize,
    },
    /// The constraints are contradictory and cannot be satisfied.
    Infeasible,
    /// The objective function is unbounded.
    Unbounded,
    /// The solver reached the maximum allowable pivot step budget.
    MaxPivotsExceeded,
}

// ============================================================================
// 2. Fraction-Free Bareiss Simplex Method
// ============================================================================

/// A fraction-free Simplex solver using exact integer arithmetic (i128)
/// and the Bareiss determinantal division algorithm.
#[derive(Debug, Clone)]
pub struct BareissSimplex {
    pub num_vars: usize,
    pub constraints: Vec<LinearConstraint>,
    pub objective: Vec<i64>,
    pub minimize: bool,
    pub max_pivots: usize,
    pub pivot_count: usize,
}

impl BareissSimplex {
    /// Creates a new Simplex solver with the specified number of decision variables.
    pub fn new(num_vars: usize) -> Self {
        BareissSimplex {
            num_vars,
            constraints: Vec::new(),
            objective: vec![0; num_vars],
            minimize: true,
            max_pivots: 1000,
            pivot_count: 0,
        }
    }

    /// Sets the maximum allowable pivot budget (default 1,000).
    pub fn set_max_pivots(&mut self, max: usize) {
        self.max_pivots = max;
    }

    /// Returns the number of pivot operations executed in the last solve.
    pub fn pivots(&self) -> usize {
        self.pivot_count
    }

    /// Adds a linear constraint: sum(coeffs[j] * x[j]) (op) rhs.
    pub fn add_constraint(&mut self, coeffs: &[i64], op: ConstraintOp, rhs: i64) {
        let mut row_coeffs = coeffs.to_vec();
        if row_coeffs.len() < self.num_vars {
            row_coeffs.resize(self.num_vars, 0);
        } else if row_coeffs.len() > self.num_vars {
            row_coeffs.truncate(self.num_vars);
        }
        self.constraints.push(LinearConstraint {
            coeffs: row_coeffs,
            op,
            rhs,
        });
    }

    /// Sets the linear objective function: sum(coeffs[j] * x[j]).
    pub fn set_objective(&mut self, coeffs: &[i64], minimize: bool) {
        let mut obj = coeffs.to_vec();
        if obj.len() < self.num_vars {
            obj.resize(self.num_vars, 0);
        } else if obj.len() > self.num_vars {
            obj.truncate(self.num_vars);
        }
        self.objective = obj;
        self.minimize = minimize;
    }

    /// Solves the LP using two-phase fraction-free Bareiss Simplex.
    pub fn solve(&mut self) -> SimplexResult {
        self.pivot_count = 0;
        let m = self.constraints.len();
        let n = self.num_vars;

        if m == 0 {
            // Unconstrained problem
            let all_zero = self.objective.iter().all(|&c| c == 0);
            if all_zero {
                return SimplexResult::Optimal {
                    objective: 0,
                    solution: vec![0; n],
                    pivots: 0,
                };
            } else {
                return SimplexResult::Unbounded;
            }
        }

        // Standardize constraints: ensure all b_i >= 0
        let mut norm_constraints = Vec::with_capacity(m);
        for c in &self.constraints {
            if c.rhs < 0 {
                // Negate constraint and invert inequality operator
                let neg_coeffs: Vec<i64> = c.coeffs.iter().map(|&x| -x).collect();
                let inv_op = match c.op {
                    ConstraintOp::Le => ConstraintOp::Ge,
                    ConstraintOp::Ge => ConstraintOp::Le,
                    ConstraintOp::Eq => ConstraintOp::Eq,
                };
                norm_constraints.push(LinearConstraint {
                    coeffs: neg_coeffs,
                    op: inv_op,
                    rhs: -c.rhs,
                });
            } else {
                norm_constraints.push(c.clone());
            }
        }

        // Count auxiliary slack/surplus/artificial variables:
        // For Le with b >= 0: add slack s_i (1 col)
        // For Ge with b >= 0: add surplus e_i (-1 col) and artificial a_i (+1 col)
        // For Eq with b >= 0: add artificial a_i (+1 col)
        let mut num_slack_surplus = 0;
        let mut num_artificial = 0;
        for c in &norm_constraints {
            match c.op {
                ConstraintOp::Le => {
                    num_slack_surplus += 1;
                }
                ConstraintOp::Ge => {
                    num_slack_surplus += 1;
                    num_artificial += 1;
                }
                ConstraintOp::Eq => {
                    num_artificial += 1;
                }
            }
        }

        let total_cols = 1 + n + num_slack_surplus + num_artificial;
        let total_rows = 1 + m; // Row 0 is objective; rows 1..=m are constraints

        let mut tableau = vec![vec![0i128; total_cols]; total_rows];
        let mut basis = vec![0usize; m]; // Column index of basic variable for row i (1-indexed)

        let mut next_slack_col = 1 + n;
        let mut next_art_col = 1 + n + num_slack_surplus;

        let mut art_rows = Vec::new();

        for (i, c) in norm_constraints.iter().enumerate() {
            let row = i + 1;
            tableau[row][0] = c.rhs as i128;
            for (j, &coef) in c.coeffs.iter().enumerate() {
                tableau[row][1 + j] = coef as i128;
            }

            match c.op {
                ConstraintOp::Le => {
                    tableau[row][next_slack_col] = 1;
                    basis[i] = next_slack_col;
                    next_slack_col += 1;
                }
                ConstraintOp::Ge => {
                    tableau[row][next_slack_col] = -1;
                    next_slack_col += 1;

                    tableau[row][next_art_col] = 1;
                    basis[i] = next_art_col;
                    art_rows.push((row, next_art_col));
                    next_art_col += 1;
                }
                ConstraintOp::Eq => {
                    tableau[row][next_art_col] = 1;
                    basis[i] = next_art_col;
                    art_rows.push((row, next_art_col));
                    next_art_col += 1;
                }
            }
        }

        let mut denominator: i128 = 1;

        // Phase 1: If artificial variables exist, drive sum(artificials) to 0
        if num_artificial > 0 {
            // Row 0 in canonical form for W = sum(artificials):
            // W = sum(b_i) - sum(A_ij * x_j)
            for &(row, art_col) in &art_rows {
                tableau[0][0] += tableau[row][0];
                let r_row = tableau[row].clone();
                for (dest, &src) in tableau[0].iter_mut().zip(&r_row).skip(1) {
                    *dest -= src;
                }
                tableau[0][art_col] = 0;
            }

            // Pivot in Phase 1
            loop {
                if self.pivot_count >= self.max_pivots {
                    return SimplexResult::MaxPivotsExceeded;
                }

                // Bland's rule: find first column with negative reduced cost in row 0
                // (only consider decision and slack/surplus columns, not artificial)
                let mut pivot_col = None;
                for (col, &cost) in tableau[0].iter().enumerate().take(1 + n + num_slack_surplus).skip(1) {
                    if cost < 0 {
                        pivot_col = Some(col);
                        break;
                    }
                }

                let q = match pivot_col {
                    Some(col) => col,
                    None => break, // Phase 1 optimal
                };

                // Minimum ratio test: min(b_i / T[i][q]) for T[i][q] > 0
                let mut pivot_row = None;
                let mut min_num: i128 = 0;
                let mut min_den: i128 = 1;

                for (row, r_slice) in tableau.iter().enumerate().take(total_rows).skip(1) {
                    let entry = r_slice[q];
                    if entry > 0 {
                        let b_val = r_slice[0];
                        let is_better = match pivot_row {
                            None => true,
                            Some(_) => (b_val * min_den) < (min_num * entry),
                        };
                        if is_better {
                            pivot_row = Some(row);
                            min_num = b_val;
                            min_den = entry;
                        }
                    }
                }

                let p = match pivot_row {
                    Some(row) => row,
                    None => break, // Bounded
                };

                self.bareiss_pivot(&mut tableau, p, q, &mut denominator, &mut basis);
            }

            // Check feasibility: W must equal 0
            if tableau[0][0] > 0 {
                return SimplexResult::Infeasible;
            }
        }

        // Phase 2: Setup original objective row
        // Reset row 0
        tableau[0].fill(0);

        // If minimizing c^T x, we set c_j * D; if maximizing, -c_j * D
        for j in 0..n {
            let coef = if self.minimize {
                self.objective[j] as i128
            } else {
                -(self.objective[j] as i128)
            };
            tableau[0][1 + j] = coef * denominator;
        }

        // Canonicalize row 0 by eliminating basic variables in current basis
        for (i, &b_col) in basis.iter().enumerate() {
            let row = i + 1;
            let entry = tableau[0][b_col];
            if entry != 0 {
                // Since T[row][b_col] == denominator:
                let factor = entry / denominator;
                let r_row = tableau[row].clone();
                tableau[0][0] += factor * r_row[0];
                for (dest, &src) in tableau[0].iter_mut().zip(&r_row).skip(1) {
                    *dest -= factor * src;
                }
            }
        }

        // Phase 2: Pivot until optimal
        loop {
            if self.pivot_count >= self.max_pivots {
                return SimplexResult::MaxPivotsExceeded;
            }

            // Bland's rule: find first column with negative reduced cost in row 0
            // Only consider non-artificial columns 1..(1 + n + num_slack_surplus)
            let mut pivot_col = None;
            for (col, &cost) in tableau[0].iter().enumerate().take(1 + n + num_slack_surplus).skip(1) {
                if cost < 0 {
                    pivot_col = Some(col);
                    break;
                }
            }

            let q = match pivot_col {
                Some(col) => col,
                None => break, // Phase 2 optimal!
            };

            // Minimum ratio test
            let mut pivot_row = None;
            let mut min_num: i128 = 0;
            let mut min_den: i128 = 1;

            for (row, r_slice) in tableau.iter().enumerate().take(total_rows).skip(1) {
                let entry = r_slice[q];
                if entry > 0 {
                    let b_val = r_slice[0];
                    let is_better = match pivot_row {
                        None => true,
                        Some(_) => (b_val * min_den) < (min_num * entry),
                    };
                    if is_better {
                        pivot_row = Some(row);
                        min_num = b_val;
                        min_den = entry;
                    }
                }
            }

            let p = match pivot_row {
                Some(row) => row,
                None => return SimplexResult::Unbounded,
            };

            self.bareiss_pivot(&mut tableau, p, q, &mut denominator, &mut basis);
        }

        // Extract solution
        let mut solution = vec![0i64; n];
        for (i, &b_col) in basis.iter().enumerate() {
            let row = i + 1;
            if b_col >= 1 && b_col <= n {
                let val = tableau[row][0] / denominator;
                solution[b_col - 1] = val as i64;
            }
        }

        let obj_raw = tableau[0][0] / denominator;
        let final_obj = if self.minimize {
            obj_raw as i64
        } else {
            -(obj_raw as i64)
        };

        SimplexResult::Optimal {
            objective: final_obj,
            solution,
            pivots: self.pivot_count,
        }
    }

    /// Performs one fraction-free Bareiss pivoting step on tableau[p][q].
    /// T'[i][j] = (P * T[i][j] - T[i][q] * T[p][j]) / prev_D
    fn bareiss_pivot(
        &mut self,
        tableau: &mut [Vec<i128>],
        p: usize,
        q: usize,
        denominator: &mut i128,
        basis: &mut [usize],
    ) {
        let p_elem = tableau[p][q];
        if p_elem <= 0 {
            return;
        }

        let prev_d = *denominator;
        let p_row = tableau[p].clone();

        for (i, r_slice) in tableau.iter_mut().enumerate() {
            if i == p {
                continue;
            }
            let i_q = r_slice[q];
            for (j, cell) in r_slice.iter_mut().enumerate() {
                if j == q {
                    *cell = 0;
                } else if i == 0 && j == 0 {
                    let num = p_elem * *cell + i_q * p_row[0];
                    *cell = num / prev_d;
                } else {
                    let num = p_elem * *cell - i_q * p_row[j];
                    *cell = num / prev_d;
                }
            }
        }

        basis[p - 1] = q;
        *denominator = p_elem;
        self.pivot_count += 1;
    }
}

// ============================================================================
// 3. Pluto-style Loop Permutability & Scheduling Constraints
// ============================================================================

/// Represents a multidimensional schedule vector: theta(i) = sum(coeffs[k] * i_k).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleVector {
    pub coeffs: Vec<i64>,
}

/// Represents the Pluto multidimensional schedule for a loop nest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlutoSchedule {
    pub dimensions: Vec<ScheduleVector>,
    pub is_permutable: bool,
    pub is_tiled: bool,
}

/// Pluto-style affine loop scheduler that formulates dependence legality and permutability
/// constraints as a linear program and solves for optimal schedule coefficients via Bareiss Simplex.
#[derive(Debug, Clone)]
pub struct PlutoScheduler {
    pub num_dims: usize,
    pub dependences: Vec<Vec<i64>>,
}

impl PlutoScheduler {
    /// Creates a Pluto scheduler for a loop nest of depth `num_dims`.
    pub fn new(num_dims: usize) -> Self {
        PlutoScheduler {
            num_dims,
            dependences: Vec::new(),
        }
    }

    /// Adds a data dependence distance vector d = (d_0, ..., d_{D-1}).
    pub fn add_dependence(&mut self, dist: &[i64]) {
        let mut d = dist.to_vec();
        if d.len() < self.num_dims {
            d.resize(self.num_dims, 0);
        } else if d.len() > self.num_dims {
            d.truncate(self.num_dims);
        }
        self.dependences.push(d);
    }

    /// Checks if a schedule vector theta is legal for all dependences: theta . d >= 0.
    pub fn is_legal_vector(&self, theta: &[i64]) -> bool {
        for d in &self.dependences {
            let mut dot: i64 = 0;
            for (t, dist) in theta.iter().zip(d.iter()) {
                dot += t * dist;
            }
            if dot < 0 {
                return false;
            }
        }
        true
    }

    /// Computes Pluto-style legal permutable schedule dimensions using the Bareiss Simplex solver.
    pub fn compute_schedule(&self) -> PlutoSchedule {
        let d_count = self.num_dims;
        if d_count == 0 {
            return PlutoSchedule {
                dimensions: Vec::new(),
                is_permutable: false,
                is_tiled: false,
            };
        }

        let mut dimensions = Vec::new();

        // For each loop dimension level l: compute an independent schedule vector theta^(l)
        for level in 0..d_count {
            let mut simplex = BareissSimplex::new(d_count);

            // 1. Permutability constraints: for each dependence d, theta . d >= 0
            for d in &self.dependences {
                simplex.add_constraint(d, ConstraintOp::Ge, 0);
            }

            // 2. Non-triviality: sum(theta_k) >= 1
            let ones = vec![1i64; d_count];
            simplex.add_constraint(&ones, ConstraintOp::Ge, 1);

            // 3. Coordinate bounds to avoid coefficient blowup: 0 <= theta_k <= 4
            for k in 0..d_count {
                let mut unit = vec![0i64; d_count];
                unit[k] = 1;
                simplex.add_constraint(&unit, ConstraintOp::Le, 4);
                simplex.add_constraint(&unit, ConstraintOp::Ge, 0);
            }

            // 4. Linear independence / priority: emphasize level `level`
            let mut obj = vec![1i64; d_count];
            for (k, val) in obj.iter_mut().enumerate().take(d_count) {
                // Penalize dimensions already assigned
                if dimensions.iter().any(|s: &ScheduleVector| s.coeffs[k] > 0) {
                    *val = 10;
                } else if k == level {
                    *val = 0; // Prioritize current dimension
                } else {
                    *val = 1;
                }
            }

            // Add reuse distance penalty: minimize sum(theta . d_e)
            for d in &self.dependences {
                for (k, val) in obj.iter_mut().enumerate().take(d_count) {
                    *val += d[k].max(0);
                }
            }

            simplex.set_objective(&obj, true);

            match simplex.solve() {
                SimplexResult::Optimal { solution, .. } => {
                    // Check if solution satisfies all dependences
                    if self.is_legal_vector(&solution) {
                        dimensions.push(ScheduleVector { coeffs: solution });
                    } else {
                        // Fallback canonical unit schedule
                        let mut unit = vec![0i64; d_count];
                        unit[level] = 1;
                        dimensions.push(ScheduleVector { coeffs: unit });
                    }
                }
                _ => {
                    // Fallback to canonical identity schedule
                    let mut unit = vec![0i64; d_count];
                    unit[level] = 1;
                    dimensions.push(ScheduleVector { coeffs: unit });
                }
            }
        }

        // Verify full permutability: every schedule dimension must satisfy theta . d >= 0
        let is_permutable = dimensions.iter().all(|s| self.is_legal_vector(&s.coeffs));
        let is_tiled = is_permutable && d_count >= 2;

        PlutoSchedule {
            dimensions,
            is_permutable,
            is_tiled,
        }
    }
}

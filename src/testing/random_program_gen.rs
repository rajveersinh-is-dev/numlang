//! Generates syntactically valid, well-typed NumLang programs for fuzz testing.
//! Programs contain: integer arithmetic, comparisons, if/else, while loops,
//! function calls, and recursion — all with bounded depth.

/// Minimal LCG RNG, no external deps
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 6364136223846793005,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_range(&mut self, lo: i64, hi: i64) -> i64 {
        if lo >= hi {
            return lo;
        }
        lo + ((self.next_u64() as i64).abs() % (hi - lo).max(1))
    }

    fn next_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 0
    }
}

/// Generate a complete, well-typed, terminating NumLang program as source text.
/// The program's `main` function returns a value in [0, 255].
/// `seed` controls all random choices; same seed → same program.
pub fn generate_random_program(seed: u64) -> String {
    let mut rng = SimpleRng::new(seed);
    let mut out = String::new();

    let c1 = rng.next_range(1, 5);
    let c2 = rng.next_range(1, 10);
    out.push_str(&format!(
        r#"fn rec_fn(n: i64, a: i64) -> i64 {{
    if n <= 0 {{
        return a;
    }} else {{
        return rec_fn(n - 1, (a * {} + {}) % 1000);
    }}
}}

"#,
        c1, c2
    ));

    let c3 = rng.next_range(1, 6);
    let c4 = rng.next_range(1, 10);
    out.push_str(&format!(
        r#"fn loop_fn(base: i64, limit: i64) -> i64 {{
    let mut acc: i64 = base;
    let mut i: i64 = 0;
    let mut max_iter: i64 = limit;
    if limit > 10 {{ max_iter = 10; }}
    if limit < 1 {{ max_iter = 1; }}
    while i < max_iter {{
        if (i % 2) == 0 {{
            acc = acc + i * {};
        }} else {{
            acc = acc - (i + {});
        }}
        i = i + 1;
    }}
    return acc;
}}

"#,
        c3, c4
    ));

    let c5 = rng.next_range(1, 5);
    let c6 = rng.next_range(1, 15);
    out.push_str(&format!(
        r#"fn branch_fn(x: i64, y: i64) -> i64 {{
    let mut val: i64 = x + y;
    if x > y {{
        val = val * {} + 3;
    }} else {{
        val = val - {};
    }}
    return val;
}}

"#,
        c5, c6
    ));

    out.push_str("fn main() -> i64 {\n");
    let init_a = rng.next_range(1, 50);
    let init_b = rng.next_range(1, 50);
    out.push_str(&format!("    let mut a: i64 = {};\n", init_a));
    out.push_str(&format!("    let mut b: i64 = {};\n", init_b));

    let steps = rng.next_range(3, 7);
    for _ in 0..steps {
        match rng.next_range(0, 4) {
            0 => {
                let depth = rng.next_range(1, 5);
                out.push_str(&format!("    a = rec_fn({}, a);\n", depth));
            }
            1 => {
                let iters = rng.next_range(1, 8);
                out.push_str(&format!("    b = loop_fn(b, {});\n", iters));
            }
            2 => {
                out.push_str("    a = branch_fn(a, b) % 500;\n");
            }
            _ => {
                if rng.next_bool() {
                    out.push_str("    b = (a + b) % 300;\n");
                } else {
                    out.push_str("    a = (a * 2 + 1) % 400;\n");
                }
            }
        }
    }

    out.push_str(
        r#"    let final_val: i64 = (a + b) % 256;
    if final_val < 0 {
        return final_val + 256;
    } else {
        return final_val;
    }
}
"#,
    );

    out
}

/// Generate a rich, deep randomized program containing:
/// - deeply nested loops (while, for)
/// - conditionals (nested if/else)
/// - match expressions
/// - coupled recurrence variables
pub fn generate_deep_random_program(seed: u64) -> String {
    let mut rng = SimpleRng::new(seed);
    let mut out = String::new();

    let c1 = rng.next_range(1, 5);
    let c2 = rng.next_range(1, 10);
    out.push_str(&format!(
        r#"fn rec_fn(n: i64, a: i64) -> i64 {{
    if n <= 0 {{
        return a;
    }} else {{
        return rec_fn(n - 1, (a * {} + {}) % 1000);
    }}
}}

"#,
        c1, c2
    ));

    let c3 = rng.next_range(1, 6);
    let c4 = rng.next_range(1, 10);
    out.push_str(&format!(
        r#"fn coupled_fn(init_x: i64, init_y: i64, steps: i64) -> i64 {{
    let mut x: i64 = init_x;
    let mut y: i64 = init_y;
    let mut i: i64 = 0;
    let mut bound: i64 = steps;
    if steps > 8 {{ bound = 8; }}
    if steps < 1 {{ bound = 1; }}
    while i < bound {{
        let nx: i64 = (x * {} + y * {} + 3) % 1000;
        let ny: i64 = (x * 2 - y + {}) % 1000;
        x = nx;
        y = ny;
        i = i + 1;
    }}
    return (x + y) % 256;
}}

"#,
        c3, c4, c1
    ));

    let m1 = rng.next_range(2, 6);
    let m2 = rng.next_range(1, 12);
    out.push_str(&format!(
        r#"fn match_fn(val: i64) -> i64 {{
    let mode: i64 = val % 4;
    let res: i64 = match mode {{
        0 => val * {} + 5,
        1 => val - {},
        2 => val * 2 + 1,
        _ => val + 10,
    }};
    return res % 500;
}}

"#,
        m1, m2
    ));

    let f1 = rng.next_range(1, 5);
    let f2 = rng.next_range(1, 10);
    out.push_str(&format!(
        r#"fn for_loop_fn(start: i64, iters: i64) -> i64 {{
    let mut acc: i64 = start;
    let mut hi: i64 = iters;
    if iters > 10 {{ hi = 10; }}
    if iters < 1 {{ hi = 1; }}
    for i in 0..hi {{
        if (i % 2) == 0 {{
            acc = acc + i * {};
        }} else {{
            acc = acc - (i + {});
        }}
    }}
    return acc % 500;
}}

"#,
        f1, f2
    ));

    out.push_str("fn main() -> i64 {\n");
    let init_a = rng.next_range(1, 50);
    let init_b = rng.next_range(1, 50);
    out.push_str(&format!("    let mut a: i64 = {};\n", init_a));
    out.push_str(&format!("    let mut b: i64 = {};\n", init_b));

    let steps = rng.next_range(4, 9);
    for _ in 0..steps {
        match rng.next_range(0, 5) {
            0 => {
                let depth = rng.next_range(1, 5);
                out.push_str(&format!("    a = rec_fn({}, a);\n", depth));
            }
            1 => {
                let iters = rng.next_range(1, 6);
                out.push_str(&format!("    b = coupled_fn(a, b, {});\n", iters));
            }
            2 => {
                out.push_str("    a = match_fn(a);\n");
            }
            3 => {
                let iters = rng.next_range(1, 8);
                out.push_str(&format!("    b = for_loop_fn(b, {});\n", iters));
            }
            _ => {
                if rng.next_bool() {
                    out.push_str("    b = (a + b) % 300;\n");
                } else {
                    out.push_str("    a = (a * 2 + 1) % 400;\n");
                }
            }
        }
    }

    out.push_str(
        r#"    let final_val: i64 = (a + b) % 256;
    if final_val < 0 {
        return final_val + 256;
    } else {
        return final_val;
    }
}
"#,
    );

    out
}

/// Generate a program specifically designed to stress the supercompiler's
/// recurrence analysis. Unlike [`generate_deep_random_program`], the shapes
/// here are chosen to defeat pattern-matching shortcuts:
///
/// - **Nested recurrence loops**: an inner `while` recurrence nested inside an
///   outer `while` recurrence, so the supercompiler must summarise a two-level
///   induction.
/// - **Coupled recurrence pairs**: two variables updated from each other in the
///   same loop body (a genuine two-dimensional linear recurrence).
/// - **Higher-order chains**: a helper-call chain (`chain2 -> chain1 -> chain0`)
///   plus real higher-order application through a `fn(i64) -> i64` parameter
///   with seed-derived closures.
/// - **Renamed identifiers**: every local and helper name carries a seed-derived
///   suffix, so no pass can rely on fixed names such as `acc` or `i`.
///
/// The program is deterministic in `seed`, always terminates, and returns a
/// value in `[0, 255]` so it is safe for exit-code differential comparison.
pub fn generate_supercompilation_stress_program(seed: u64) -> String {
    let mut rng = SimpleRng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
    let tag = rng.next_range(100, 999);

    let acc = format!("acc_{}", tag);
    let outer = format!("outer_{}", tag);
    let inner = format!("inner_{}", tag);
    let p = format!("p_{}", tag);
    let q = format!("q_{}", tag);
    let t = format!("t_{}", tag);
    let outer_bound = format!("outer_bound_{}", tag);
    let inner_bound = format!("inner_bound_{}", tag);
    let coupled_bound = format!("coupled_bound_{}", tag);
    let np = format!("np_{}", tag);
    let nq = format!("nq_{}", tag);
    let final_name = format!("final_{}", tag);

    // Coefficients for the nested recurrence.
    let a1 = rng.next_range(1, 7);
    let a2 = rng.next_range(1, 7);
    let a3 = rng.next_range(1, 7);
    let a4 = rng.next_range(1, 13);
    let a5 = rng.next_range(1, 7);
    // Coefficients for the coupled recurrence pair.
    let b1 = rng.next_range(1, 6);
    let b2 = rng.next_range(1, 6);
    let b3 = rng.next_range(1, 11);
    let b4 = rng.next_range(1, 6);
    let b5 = rng.next_range(1, 6);
    let b6 = rng.next_range(1, 11);
    // Coefficients for the helper-call / higher-order chain.
    let c0 = rng.next_range(1, 6);
    let c1 = rng.next_range(1, 10);
    let c2 = rng.next_range(1, 6);
    let c3 = rng.next_range(1, 10);
    let c4 = rng.next_range(1, 10);
    let m1 = rng.next_range(1, 6);
    let m2 = rng.next_range(1, 10);
    let m3 = rng.next_range(1, 10);

    let init_acc = rng.next_range(1, 40);
    let init_p = rng.next_range(1, 40);
    let init_q = rng.next_range(1, 40);
    let outer_iters = rng.next_range(1, 4);
    let inner_iters = rng.next_range(1, 4);
    let coupled_iters = rng.next_range(1, 5);

    format!(
        r#"fn ho_apply_{tag}(f: fn(i64) -> i64, v: i64) -> i64 {{
    return f(v) % 1000;
}}

fn chain0_{tag}(x: i64) -> i64 {{
    return (x * {c0} + {c1}) % 1000;
}}

fn chain1_{tag}(x: i64) -> i64 {{
    return (chain0_{tag}(x) * {c2} + {c3}) % 1000;
}}

fn chain2_{tag}(x: i64) -> i64 {{
    return (chain1_{tag}(x) + {c4}) % 1000;
}}

fn main() -> i64 {{
    let mut {acc}: i64 = {init_acc};
    let mut {outer}: i64 = 0;
    let {outer_bound}: i64 = {outer_iters};
    while {outer} < {outer_bound} {{
        let mut {inner}: i64 = 0;
        let {inner_bound}: i64 = {inner_iters};
        while {inner} < {inner_bound} {{
            {acc} = ({acc} * {a1} + {outer} * {a2} + {inner} * {a3} + {a4}) % 1000;
            {inner} = {inner} + 1;
        }}
        {acc} = ({acc} + {outer} * {a5}) % 1000;
        {outer} = {outer} + 1;
    }}

    let mut {p}: i64 = {init_p};
    let mut {q}: i64 = {init_q};
    let mut {t}: i64 = 0;
    let {coupled_bound}: i64 = {coupled_iters};
    while {t} < {coupled_bound} {{
        let {np}: i64 = ({p} * {b1} + {q} * {b2} + {b3}) % 1000;
        let {nq}: i64 = ({p} * {b4} + {q} * {b5} + {b6}) % 1000;
        {p} = {np};
        {q} = {nq};
        {t} = {t} + 1;
    }}
    {acc} = ({acc} + {p} + {q}) % 1000;

    {acc} = chain2_{tag}({acc});

    let f_{tag}: fn(i64) -> i64 = |x: i64| (x * {m1} + {m2}) % 1000;
    let g_{tag}: fn(i64) -> i64 = |x: i64| (x + {m3}) % 1000;
    {acc} = ho_apply_{tag}(f_{tag}, {acc});
    {acc} = ho_apply_{tag}(g_{tag}, {acc});

    let {final_name}: i64 = {acc} % 256;
    if {final_name} < 0 {{
        return {final_name} + 256;
    }} else {{
        return {final_name};
    }}
}}
"#,
        tag = tag,
        acc = acc,
        outer = outer,
        inner = inner,
        p = p,
        q = q,
        t = t,
        outer_bound = outer_bound,
        inner_bound = inner_bound,
        coupled_bound = coupled_bound,
        np = np,
        nq = nq,
        final_name = final_name,
        a1 = a1,
        a2 = a2,
        a3 = a3,
        a4 = a4,
        a5 = a5,
        b1 = b1,
        b2 = b2,
        b3 = b3,
        b4 = b4,
        b5 = b5,
        b6 = b6,
        c0 = c0,
        c1 = c1,
        c2 = c2,
        c3 = c3,
        c4 = c4,
        m1 = m1,
        m2 = m2,
        m3 = m3,
        init_acc = init_acc,
        init_p = init_p,
        init_q = init_q,
        outer_iters = outer_iters,
        inner_iters = inner_iters,
        coupled_iters = coupled_iters,
    )
}

#[cfg(test)]
mod stress_gen_tests {
    use super::*;

    #[test]
    fn stress_program_is_deterministic_in_seed() {
        assert_eq!(
            generate_supercompilation_stress_program(7),
            generate_supercompilation_stress_program(7),
            "same seed must produce the same program"
        );
        assert_ne!(
            generate_supercompilation_stress_program(7),
            generate_supercompilation_stress_program(8),
            "different seeds should produce different programs"
        );
    }

    #[test]
    fn stress_program_contains_required_shapes() {
        let src = generate_supercompilation_stress_program(42);
        assert!(
            src.contains("fn(i64) -> i64"),
            "must exercise a higher-order function parameter"
        );
        assert!(
            src.contains("ho_apply_"),
            "must exercise higher-order application"
        );
        assert!(
            src.matches("while ").count() >= 3,
            "must contain nested and coupled loops, found: {}",
            src.matches("while ").count()
        );
        assert!(src.contains("chain0_") && src.contains("chain1_") && src.contains("chain2_"));
    }
}

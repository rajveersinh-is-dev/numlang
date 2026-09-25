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
    let max_iter: i64 = if limit > 10 {{ 10 }} else {{ if limit < 1 {{ 1 }} else {{ limit }} }};
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

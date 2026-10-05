//! Typed Random Program Generator for NumLang property-based testing.
//!
//! Generates random, strictly well-typed, terminating NumLang programs:
//! - Literals (integers, booleans)
//! - Arithmetic & bitwise operations with modulo normalization to prevent overflow
//! - Structurally terminating recursive functions (countdown bounded)
//! - Branching (`if/else`)
//! - Heap allocation (`box` and `deref`)
//! - Loops with countdown termination
//! - Dynamic variable environments synthesized bottom-up

use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

/// Configuration parameters for random program generation.
#[derive(Debug, Clone)]
pub struct GenConfig {
    pub max_depth: usize,
    pub max_functions: usize,
    pub max_args: usize,
}

impl Default for GenConfig {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_functions: 3,
            max_args: 3,
        }
    }
}

/// Minimal internal deterministic PRNG for generation without global state.
pub struct RngState {
    state: u64,
}

impl RngState {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x5851f42d4c957f2d,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    pub fn next_range(&mut self, lo: i64, hi: i64) -> i64 {
        if lo >= hi {
            return lo;
        }
        let span = (hi - lo) as u64;
        lo + (self.next_u64() % span) as i64
    }

    pub fn next_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 0
    }

    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        let idx = (self.next_u64() as usize) % slice.len();
        &slice[idx]
    }
}

/// Generates a well-typed NumLang program from a seed and configuration.
pub fn generate_well_typed_program(seed: u64, config: &GenConfig) -> String {
    let mut rng = RngState::new(seed);
    let mut src = String::new();

    let num_funcs = rng.next_range(1, (config.max_functions + 1) as i64) as usize;
    let mut funcs = Vec::new();

    // 1. Generate helper recursive functions with countdown guarantees
    for f_idx in 0..num_funcs {
        let fname = format!("f_{}", f_idx);

        let num_args = rng.next_range(1, (config.max_args + 1) as i64) as usize;
        let mut param_decls = vec!["n: i64".to_string()];
        let mut param_names = vec!["n".to_string()];

        for a_idx in 0..num_args {
            let pname = format!("a_{}", a_idx);
            param_decls.push(format!("{}: i64", pname));
            param_names.push(pname);
        }

        funcs.push((fname.clone(), param_names.len()));

        src.push_str(&format!(
            "fn {}({}) -> i64 {{\n",
            fname,
            param_decls.join(", ")
        ));

        // Base case on countdown variable n
        src.push_str("    if n <= 0 {\n");
        let base_var = if param_names.len() > 1 {
            &param_names[1]
        } else {
            "0"
        };
        src.push_str(&format!("        return {};\n", base_var));
        src.push_str("    } else {\n");

        // Recursive call with strictly decreasing countdown n - 1
        let mut call_args = vec!["n - 1".to_string()];
        for p in param_names.iter().skip(1) {
            let mult = rng.next_range(1, 4);
            let add_c = rng.next_range(1, 10);
            call_args.push(format!("({} * {} + {}) % 500", p, mult, add_c));
        }

        src.push_str(&format!(
            "        return {}({});\n",
            fname,
            call_args.join(", ")
        ));
        src.push_str("    }\n}\n\n");
    }

    // 2. Generate a heap Box<T> helper function
    src.push_str("fn heap_op(val: i64) -> i64 {\n");
    src.push_str("    let b: Box<i64> = box (val % 300);\n");
    src.push_str("    let res: i64 = deref(b) + 5;\n");
    src.push_str("    return res % 256;\n");
    src.push_str("}\n\n");

    // 3. Generate main function
    src.push_str("fn main() -> i64 {\n");
    let mut vars = Vec::new();

    // Initial locals
    let init_x = rng.next_range(1, 30);
    let init_y = rng.next_range(1, 30);
    src.push_str(&format!("    let mut x: i64 = {};\n", init_x));
    src.push_str(&format!("    let mut y: i64 = {};\n", init_y));
    vars.push("x".to_string());
    vars.push("y".to_string());

    let steps = rng.next_range(3, 8);
    for s in 0..steps {
        let target_var = rng.choose(&vars).clone();
        match rng.next_range(0, 5) {
            0 => {
                // Call recursive function
                if !funcs.is_empty() {
                    let (fn_choice, arity) = rng.choose(&funcs).clone();
                    let countdown = rng.next_range(1, 5);
                    let mut args = vec![countdown.to_string()];
                    for _ in 1..arity {
                        let other_var = rng.choose(&vars).clone();
                        args.push(other_var);
                    }
                    src.push_str(&format!(
                        "    {} = {}({}) % 500;\n",
                        target_var,
                        fn_choice,
                        args.join(", ")
                    ));
                }
            }
            1 => {
                // Arithmetic / bitwise computation
                let v1 = rng.choose(&vars).clone();
                let v2 = rng.choose(&vars).clone();
                let op = match rng.next_range(0, 4) {
                    0 => "+",
                    1 => "-",
                    2 => "*",
                    _ => "^",
                };
                src.push_str(&format!(
                    "    {} = ({} {} {}) % 400;\n",
                    target_var, v1, op, v2
                ));
            }
            2 => {
                // Branching if/else
                let v1 = rng.choose(&vars).clone();
                let v2 = rng.choose(&vars).clone();
                let c_add = rng.next_range(1, 20);
                src.push_str(&format!("    if {} > {} {{\n", v1, v2));
                src.push_str(&format!("        {} = ({} + {}) % 300;\n", target_var, v1, c_add));
                src.push_str("    } else {\n");
                src.push_str(&format!("        {} = ({} - 2) % 300;\n", target_var, v2));
                src.push_str("    }\n");
            }
            3 => {
                // Call heap helper
                let v = rng.choose(&vars).clone();
                src.push_str(&format!("    {} = heap_op({});\n", target_var, v));
            }
            _ => {
                // Loop with bounded iteration
                let loop_bound = rng.next_range(2, 6);
                let loop_var = format!("iter_{}", s);
                src.push_str(&format!("    let mut {}: i64 = 0;\n", loop_var));
                src.push_str(&format!("    while {} < {} {{\n", loop_var, loop_bound));
                src.push_str(&format!("        {} = ({} + {}) % 400;\n", target_var, target_var, loop_var));
                src.push_str(&format!("        {} = {} + 1;\n", loop_var, loop_var));
                src.push_str("    }\n");
            }
        }
    }

    // Normalized exit value in [0, 255]
    src.push_str("    let final_val: i64 = (x + y) % 256;\n");
    src.push_str("    if final_val < 0 {\n");
    src.push_str("        return final_val + 256;\n");
    src.push_str("    } else {\n");
    src.push_str("        return final_val;\n");
    src.push_str("    }\n");
    src.push_str("}\n");

    src
}

/// Proptest strategy for generating well-typed NumLang programs.
pub fn random_program_strategy(config: GenConfig) -> BoxedStrategy<String> {
    any::<u64>()
        .prop_map(move |seed| generate_well_typed_program(seed, &config))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::testing::oracle::{evaluate_program, OracleResult};
    use crate::token::tokenize;
    use crate::typecheck::typecheck;

    #[test]
    fn test_generated_programs_well_typed_and_evaluable() {
        let config = GenConfig::default();
        for seed in 0..20 {
            let src = generate_well_typed_program(seed, &config);
            let tokens = tokenize(&src).expect("tokenization failed");
            let program = parse(&tokens).expect("parsing failed");
            let _typed = typecheck(&program).expect("typechecking failed");
            let res = evaluate_program(&program);
            match res {
                OracleResult::Value(v) => {
                    assert!((0..=256).contains(&v), "seed {} returned unexpected {}", seed, v);
                }
                other => panic!("seed {} failed with {:?}", seed, other),
            }
        }
    }
}


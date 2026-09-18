pub struct ErrorExplanation {
    pub code: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub bad_example: &'static str,
    pub good_example: &'static str,
}

pub static EXPLANATIONS: &[ErrorExplanation] = &[
    ErrorExplanation {
        code: "E001",
        title: "Type Mismatch",
        description: "An expression was evaluated to a type that does not match the expected type.\nNumLang enforces strict static typing with zero implicit conversions.",
        bad_example: "fn main() -> i64 {\n    let x: i64 = 3.14;\n    return x;\n}",
        good_example: "fn main() -> i64 {\n    let x: f64 = 3.14;\n    return 0;\n}",
    },
    ErrorExplanation {
        code: "E002",
        title: "Undeclared Variable",
        description: "An identifier was referenced before being declared in scope.\nAll variables must be declared using 'let' or 'let mut' before use.",
        bad_example: "fn main() -> i64 {\n    return count + 1;\n}",
        good_example: "fn main() -> i64 {\n    let count: i64 = 0;\n    return count + 1;\n}",
    },
    ErrorExplanation {
        code: "E003",
        title: "Undeclared Function",
        description: "A function was called that has not been defined in the program or built-in library.",
        bad_example: "fn main() -> i64 {\n    return calculate(10);\n}",
        good_example: "fn calculate(n: i64) -> i64 {\n    return n * 2;\n}\n\nfn main() -> i64 {\n    return calculate(10);\n}",
    },
    ErrorExplanation {
        code: "E004",
        title: "Cannot Mutate Immutable Variable",
        description: "Attempted to reassign or mutate a variable that was declared immutable.\nIn NumLang, variables are immutable by default unless declared with 'let mut'.",
        bad_example: "fn main() -> i64 {\n    let x: i64 = 1;\n    x = 2;\n    return x;\n}",
        good_example: "fn main() -> i64 {\n    let mut x: i64 = 1;\n    x = 2;\n    return x;\n}",
    },
    ErrorExplanation {
        code: "E005",
        title: "Duplicate Declaration",
        description: "An identifier with the same name was already declared in the current scope.",
        bad_example: "fn main() -> i64 {\n    let x: i64 = 1;\n    let x: i64 = 2;\n    return x;\n}",
        good_example: "fn main() -> i64 {\n    let x: i64 = 1;\n    let y: i64 = 2;\n    return x + y;\n}",
    },
    ErrorExplanation {
        code: "E006",
        title: "Invalid Condition Type",
        description: "A condition in an 'if' or 'while' statement must evaluate to boolean ('bool').\nNumLang does not treat integers or non-zero numbers as booleans.",
        bad_example: "fn main() -> i64 {\n    let x: i64 = 5;\n    if x { return 1; }\n    return 0;\n}",
        good_example: "fn main() -> i64 {\n    let x: i64 = 5;\n    if x != 0 { return 1; }\n    return 0;\n}",
    },
    ErrorExplanation {
        code: "E007",
        title: "Invalid Binary Operands",
        description: "A binary operator (such as +, -, *, ==, etc.) was applied to operands with incompatible types.",
        bad_example: "fn main() -> i64 {\n    let x: i64 = 5 + 3.0;\n    return x;\n}",
        good_example: "fn main() -> i64 {\n    let x: i64 = 5 + 3;\n    return x;\n}",
    },
    ErrorExplanation {
        code: "E008",
        title: "Invalid Unary Operand",
        description: "A unary operator (such as '-' or '!') was applied to an incompatible operand type.",
        bad_example: "fn main() -> i64 {\n    let x: bool = -true;\n    return 0;\n}",
        good_example: "fn main() -> i64 {\n    let x: bool = !true;\n    return 0;\n}",
    },
    ErrorExplanation {
        code: "E009",
        title: "Function Arity Mismatch",
        description: "A function was called with an incorrect number of arguments.",
        bad_example: "fn add(a: i64, b: i64) -> i64 { return a + b; }\n\nfn main() -> i64 {\n    return add(1);\n}",
        good_example: "fn add(a: i64, b: i64) -> i64 { return a + b; }\n\nfn main() -> i64 {\n    return add(1, 2);\n}",
    },
    ErrorExplanation {
        code: "E010",
        title: "Unknown Type Name",
        description: "A type annotation specifies a type name that is not recognized by the compiler.\nSupported primitive types are: i8, i16, i32, i64, u8, u16, u32, u64, usize, f32, f64, bool, void.",
        bad_example: "fn main() -> i64 {\n    let x: number = 42;\n    return x;\n}",
        good_example: "fn main() -> i64 {\n    let x: i64 = 42;\n    return x;\n}",
    },
    ErrorExplanation {
        code: "E011",
        title: "Function Return Type Mismatch",
        description: "The expression in a 'return' statement does not match the return type declared in the function signature.",
        bad_example: "fn get_value() -> i64 {\n    return 3.14;\n}",
        good_example: "fn get_value() -> f64 {\n    return 3.14;\n}",
    },
    ErrorExplanation {
        code: "E012",
        title: "Cannot Index Non-Array Type",
        description: "The indexing operator '[i]' was used on a value that is not an array.",
        bad_example: "fn main() -> i64 {\n    let x: i64 = 42;\n    return x[0];\n}",
        good_example: "fn main() -> i64 {\n    let arr: [i64; 1] = [42];\n    return arr[0];\n}",
    },
    ErrorExplanation {
        code: "E013",
        title: "Invalid Index Type",
        description: "An array was indexed with a value that is not an integer.",
        bad_example: "fn main() -> i64 {\n    let arr: [i64; 2] = [1, 2];\n    return arr[true];\n}",
        good_example: "fn main() -> i64 {\n    let arr: [i64; 2] = [1, 2];\n    return arr[1];\n}",
    },
    ErrorExplanation {
        code: "E014",
        title: "Empty Array Literal",
        description: "Array literals in NumLang must contain at least one element so their type can be inferred.",
        bad_example: "fn main() -> i64 {\n    let arr = [];\n    return 0;\n}",
        good_example: "fn main() -> i64 {\n    let arr: [i64; 1] = [0];\n    return 0;\n}",
    },
    ErrorExplanation {
        code: "E015",
        title: "Array Index Out of Bounds",
        description: "A constant index into an array is greater than or equal to the array's declared length.",
        bad_example: "fn main() -> i64 {\n    let arr: [i64; 3] = [1, 2, 3];\n    return arr[5];\n}",
        good_example: "fn main() -> i64 {\n    let arr: [i64; 3] = [1, 2, 3];\n    return arr[2];\n}",
    },
    ErrorExplanation {
        code: "E016",
        title: "Array Element Type Mismatch",
        description: "Elements in an array literal must all have the exact same type.",
        bad_example: "fn main() -> i64 {\n    let arr = [1, 2.5, 3];\n    return 0;\n}",
        good_example: "fn main() -> i64 {\n    let arr = [1.0, 2.5, 3.0];\n    return 0;\n}",
    },
    ErrorExplanation {
        code: "E017",
        title: "Break Outside Loop",
        description: "The 'break' keyword was used outside of any enclosing 'while', 'for', or 'loop' construct.",
        bad_example: "fn main() -> i64 {\n    break;\n    return 0;\n}",
        good_example: "fn main() -> i64 {\n    loop {\n        break;\n    }\n    return 0;\n}",
    },
    ErrorExplanation {
        code: "E018",
        title: "Continue Outside Loop",
        description: "The 'continue' keyword was used outside of any enclosing 'while', 'for', or 'loop' construct.",
        bad_example: "fn main() -> i64 {\n    continue;\n    return 0;\n}",
        good_example: "fn main() -> i64 {\n    for i in 0..10 {\n        if i == 5 { continue; }\n    }\n    return 0;\n}",
    },
    ErrorExplanation {
        code: "E019",
        title: "Struct Field Access Error",
        description: "Attempted to access an unmapped or missing field on a struct, or access a field on a non-struct type.",
        bad_example: "struct Point { x: i64 }\n\nfn main() -> i64 {\n    let p = Point { x: 1 };\n    return p.y;\n}",
        good_example: "struct Point { x: i64, y: i64 }\n\nfn main() -> i64 {\n    let p = Point { x: 1, y: 2 };\n    return p.y;\n}",
    },
    ErrorExplanation {
        code: "E020",
        title: "Non-Exhaustive Pattern Match",
        description: "A match expression does not cover all possible values and is missing a wildcard '_' arm.",
        bad_example: "fn main() -> i64 {\n    let x: i64 = 3;\n    return match x {\n        0 => 10,\n        1 => 20,\n    };\n}",
        good_example: "fn main() -> i64 {\n    let x: i64 = 3;\n    return match x {\n        0 => 10,\n        1 => 20,\n        _ => 0,\n    };\n}",
    },
];

pub fn get_explanation(code: &str) -> Option<&'static ErrorExplanation> {
    let normalized = code.trim().to_uppercase();
    let num_str = normalized.strip_prefix('E').unwrap_or(&normalized);
    let parsed_num = num_str.parse::<u32>().ok();
    EXPLANATIONS.iter().find(|e| {
        if e.code == normalized {
            return true;
        }
        if let Some(n) = parsed_num {
            if let Ok(en) = e.code.strip_prefix('E').unwrap_or("").parse::<u32>() {
                return en == n;
            }
        }
        false
    })
}

pub fn print_explanation(code: &str) -> bool {
    if let Some(exp) = get_explanation(code) {
        println!("================================================================================");
        println!("NumLang Error Code: {} ({})", exp.code, exp.title);
        println!("================================================================================\n");
        println!("Description:\n  {}\n", exp.description.replace("\n", "\n  "));
        println!("Erroneous Example:");
        for line in exp.bad_example.lines() {
            println!("  {}", line);
        }
        println!("\nCorrected Example:");
        for line in exp.good_example.lines() {
            println!("  {}", line);
        }
        println!("\n================================================================================");
        true
    } else {
        eprintln!("Error: unknown error code '{}'. Available error codes: E001 through E020.", code);
        false
    }
}

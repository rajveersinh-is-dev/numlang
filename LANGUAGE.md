# NumLang v1.0 Language Reference

This document provides a comprehensive specification of the NumLang programming language syntax, type system, operators, standard library built-ins, and compiler CLI.

---

## 1. Type System

NumLang is a statically-typed language with zero implicit type coercion. Numeric literals may include an explicit type suffix or be typed according to context.

| Type | Description | Size | Literal Examples |
|---|---|---|---|
| `i8` | 8-bit signed integer | 1 byte | `-128i8`, `127i8` |
| `i16` | 16-bit signed integer | 2 bytes | `-32768i16`, `32767i16` |
| `i32` | 32-bit signed integer | 4 bytes | `0i32`, `42i32`, `-100i32` |
| `i64` | 64-bit signed integer (default int) | 8 bytes | `0`, `42`, `1000000`, `42i64` |
| `u8` | 8-bit unsigned integer | 1 byte | `0u8`, `255u8` |
| `u16` | 16-bit unsigned integer | 2 bytes | `0u16`, `65535u16` |
| `u32` | 32-bit unsigned integer | 4 bytes | `0u32`, `4294967295u32` |
| `u64` | 64-bit unsigned integer | 8 bytes | `0u64`, `18446744073709551615u64` |
| `usize` | Target pointer-width unsigned integer | 8 bytes | `0usize`, `1024usize` |
| `f32` | Single-precision IEEE-754 float | 4 bytes | `3.14f32`, `0.0f32` |
| `f64` | Double-precision IEEE-754 float | 8 bytes | `0.0`, `3.141592653589793`, `1e10` |
| `bool` | Boolean truth value | 1 byte | `true`, `false` |
| `void` | Empty return type | 0 bytes | (implicit when return type omitted) |
| `[T; N]` | Fixed-size contiguous array of type `T` and length `N` | `N * sizeof(T)` | `[1, 2, 3]`, `[0.0; 16]` |
| `struct S`| User-defined flat composite struct | Sum of aligned fields | `Point { x: 1.0, y: 2.0 }` |

---

## 2. Operators and Precedence

NumLang operators follow standard mathematical precedence, evaluated from highest to lowest binding power:

| Precedence | Operator | Description | Associativity |
|---|---|---|---|
| 1 (Highest) | `()`, `[]`, `.` | Function call, array indexing, field access | Left-to-right |
| 2 | `**` | Exponentiation (power) | Right-to-left |
| 3 | `-`, `!` | Unary negation, logical NOT | Right-to-left |
| 4 | `*`, `/`, `%` | Multiplication, division, modulo | Left-to-right |
| 5 | `+`, `-` | Addition, subtraction | Left-to-right |
| 6 | `<<`, `>>` | Bitwise shift left, logical/arithmetic shift right | Left-to-right |
| 7 | `<`, `<=`, `>`, `>=` | Relational comparisons | Left-to-right |
| 8 | `==`, `!=` | Equality and inequality | Left-to-right |
| 9 | `&` | Bitwise AND | Left-to-right |
| 10 | `^` | Bitwise XOR | Left-to-right |
| 11 (Lowest) | `\|` | Bitwise OR | Left-to-right |

---

## 3. Statements and Control Flow

### Variable Declarations (`let` and `let mut`)
Variables are immutable by default:
```numlang
let x: i64 = 42;
let mut counter: i64 = 0;
counter = counter + 1;
```

### Assignment and Mutations
```numlang
x = 10;
arr[i] = 20;
point.x = 3.14;
```

### Conditional Branches (`if` / `else if` / `else`)
```numlang
if score >= 90 {
    println("A");
} else if score >= 80 {
    println("B");
} else {
    println("C");
}
```

### Range-Based Loops (`for`)
Supports exclusive (`..`) and inclusive (`..=`) ranges:
```numlang
for i in 0..10 {
    // 0 through 9
}

for i in 1..=10 {
    // 1 through 10
}
```

### While and Infinite Loops (`while` and `loop`)
```numlang
while count > 0 {
    count = count - 1;
}

loop {
    if done {
        break;
    }
}
```

### Loop Control (`break` and `continue`)
```numlang
for i in 0..100 {
    if i % 2 == 0 {
        continue;
    }
    if i > 50 {
        break;
    }
}
```

### Returns
```numlang
return 42;
return; // In void functions
```

### Pattern Matching (`match`)
Supports integer, boolean, or-patterns, and wildcards:
```numlang
let value: i64 = match tag {
    0 => 10,
    1 | 2 => 20,
    3 => 30,
    _ => -1,
};
```

---

## 4. Functions

Functions are defined with the `fn` keyword. Return types are annotated with `-> Type`. Void functions omit the arrow:
```numlang
/// Computes hypotenuse
fn hypotenuse(a: f64, b: f64) -> f64 {
    return sqrt(a * a + b * b);
}

fn log_message() {
    println("Operation complete.");
}
```

---

## 5. Structs

Structs are C-style flat data records allocated directly on the stack with value semantics:
```numlang
struct Vector3 {
    x: f64,
    y: f64,
    z: f64,
}

fn length_sq(v: Vector3) -> f64 {
    return v.x * v.x + v.y * v.y + v.z * v.z;
}

fn main() -> i64 {
    let mut v: Vector3 = Vector3 { x: 1.0, y: 2.0, z: 3.0 };
    v.x = 4.0;
    return 0;
}
```

---

## 6. Built-in Functions and Intrinsics

All built-in functions are globally available without imports.

### Mathematical Functions
- `sqrt(x: f64) -> f64` / `sqrt(x: f32) -> f32`: Square root.
- `abs(x: T) -> T`: Absolute value for signed integers and floats.
- `min(a: T, b: T) -> T`: Minimum of two numeric values.
- `max(a: T, b: T) -> T`: Maximum of two numeric values.
- `clamp(val: T, lo: T, hi: T) -> T`: Clamps `val` to `[lo, hi]`.
- `floor(x: f64) -> f64`: Round down to integer.
- `ceil(x: f64) -> f64`: Round up to integer.
- `round(x: f64) -> f64`: Round to nearest integer.
- `trunc(x: f64) -> f64`: Truncate fractional part.
- `sin(x: f64) -> f64`: Sine (via native libm).
- `cos(x: f64) -> f64`: Cosine (via native libm).
- `tan(x: f64) -> f64`: Tangent (via native libm).
- `exp(x: f64) -> f64`: Natural exponential $e^x$.
- `ln(x: f64) -> f64`: Natural logarithm $\ln x$.
- `log2(x: f64) -> f64`: Base-2 logarithm $\log_2 x$.
- `log10(x: f64) -> f64`: Base-10 logarithm $\log_{10} x$.
- `pow(base: f64, exp: f64) -> f64`: Floating-point power.

### Bitwise Intrinsics
- `popcnt(x: T) -> T`: Population count (number of 1-bits).
- `clz(x: T) -> T`: Count leading zeros.
- `ctz(x: T) -> T` / `tzcnt(x: T) -> T`: Count trailing zeros.
- `bswap(x: T) -> T`: Byte swap (endian reversal).
- `rotl(val: T, shift: i64) -> T`: Rotate bits left.
- `rotr(val: T, shift: i64) -> T`: Rotate bits right.

### Type Conversions
- `i64_to_f64(x: i64) -> f64`
- `f64_to_i64(x: f64) -> i64`
- `i64_to_f32(x: i64) -> f32`
- `f32_to_f64(x: f32) -> f64`
- `f64_to_f32(x: f64) -> f32`
- `i64_to_i32(x: i64) -> i32`
- `i32_to_i64(x: i32) -> i64`

### I/O Functions
- `print(val)`: Prints an integer, float, boolean, or string literal to stdout.
- `println(val)`: Prints the value followed by a newline.

---

## 7. Compiler CLI Reference

```
numlang [OPTIONS] [FILE]
numlang <COMMAND>
```

### Commands
- `run <file.nl> [--bench]`: Compile and execute a NumLang source file directly.
- `build <file.nl> [-o <out.exe>] [--emit-obj <out.obj>] [--bench]`: Compile into native standalone executable.
- `check <file.nl>`: Perform full semantic analysis and type checking.
- `fmt <file.nl> [--check] [--stdout]`: Format source code according to canonical NumLang style.
- `doc <file.nl> [--output <path>]`: Generate Markdown API reference documentation from `///` doc-comments.
- `explain <code>`: Provide detailed explanation and remediation advice for compiler diagnostic codes (`E001`–`E020`).

### Flags
- `-o, --output <path>`: Specify destination binary path.
- `-c, --check`: Syntax and typecheck without codegen.
- `-t, --emit-tokens`: Output token stream.
- `-a, --emit-ast`: Output AST representation.
- `-i, --emit-ir`: Output lowered Intermediate Representation.
- `--emit-typed-ast`: Output typechecked AST.
- `--bench`: Enable high-resolution in-process benchmarking.

---

## 8. Optimization & Supercompiler Model

NumLang's optimization pipeline operates in multiple stages:
1. **Constant Propagation & Inlining**: Fixpoint symbolic constant folder and interprocedural function inliner.
2. **Supercompiler Analysis**:
   - Inspects loops with polynomial state updates $P(i)$ using Newton's forward difference formula $\Delta^k P(0)$ for degrees $0 \le k \le 4$.
   - Analyzes coupled recurrence relations $X_{n+1} = M X_n \pmod m$, constructing the transition matrix $M$ and computing $M^N$ via binary exponentiation in $O(k^3 \log N)$ time.
   - Replaces loop bodies with closed-form analytic expressions when applicable.
3. **Loop Transformations**: While-loop unrolling, bounded invariant hoisting, and branchless select conversion.
4. **Memory Optimization**: Scalar Replacement of Aggregates (SROA) and Bounds Check Elimination (BCE).
5. **Cranelift Native Backend**: Generates optimized x86-64 machine instructions with host vector/AVX2 extensions.

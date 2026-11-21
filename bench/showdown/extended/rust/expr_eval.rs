// Extended workload 1: stack-machine expression evaluator.
// Reference implementation for bench/showdown/extended/numlang/expr_eval.nl.
use std::time::Instant;

fn main() {
    let t0 = Instant::now();

    // Header cell 0 is the bytecode length in i64 cells; the program follows it.
    // 11 instructions: PUSH 7, PUSH 5, ADD, PUSH 3, MUL, PUSH 11, SUB,
    //                  PUSH 2, PUSH 9, ADD, MOD  => ((7+5)*3-11) % (2+9) = 3.
    let code: [i64; 23] = [
        22, 1, 7, 1, 5, 2, 0, 1, 3, 3, 0, 1, 11, 4, 0, 1, 2, 1, 9, 2, 0, 5, 0,
    ];
    let program_end: i64 = 1 + code[0];

    let mut stack: [i64; 16] = [0; 16];
    let mut sp: i64 = 0;
    let mut pc: i64 = 1;
    let mut executed: i64 = 0;

    while pc < program_end {
        let op = code[pc as usize];
        let arg = code[(pc + 1) as usize];
        if op == 1 {
            stack[sp as usize] = arg;
            sp += 1;
        } else if op == 2 {
            let b = stack[(sp - 1) as usize];
            let a = stack[(sp - 2) as usize];
            stack[(sp - 2) as usize] = a.wrapping_add(b);
            sp -= 1;
        } else if op == 3 {
            let b = stack[(sp - 1) as usize];
            let a = stack[(sp - 2) as usize];
            stack[(sp - 2) as usize] = a.wrapping_mul(b);
            sp -= 1;
        } else if op == 4 {
            let b = stack[(sp - 1) as usize];
            let a = stack[(sp - 2) as usize];
            stack[(sp - 2) as usize] = a.wrapping_sub(b);
            sp -= 1;
        } else if op == 5 {
            let b = stack[(sp - 1) as usize];
            let a = stack[(sp - 2) as usize];
            stack[(sp - 2) as usize] = if b == 0 { 0 } else { a.wrapping_rem(b) };
            sp -= 1;
        }
        pc += 2;
        executed += 1;
    }

    let result = stack[(sp - 1) as usize] + executed;
    println!("{}", result);
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((result.rem_euclid(256)) as i32);
}

// Extended workload 1: stack-machine expression evaluator.
// Header cell 0 holds the bytecode length in i64 cells; the program follows it.
// 11 instructions compute ((7 + 5) * 3 - 11) % (2 + 9) = 3.
// The result is augmented with the executed instruction count to keep the
// observable value dependent on the interpreter loop, not only the bytecode.
fn main() -> i64 {
    let code: [i64; 23] = [
        22, 1, 7, 1, 5, 2, 0, 1, 3, 3, 0, 1, 11, 4, 0, 1, 2, 1, 9, 2, 0, 5, 0
    ];
    let program_end: i64 = 1 + code[0];

    let mut stack: [i64; 16] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    ];
    let mut sp: i64 = 0;
    let mut pc: i64 = 1;
    let mut executed: i64 = 0;

    while pc < program_end {
        let op: i64 = code[pc];
        let arg: i64 = code[pc + 1];

        if op == 1 {
            stack[sp] = arg;
            sp = sp + 1;
        } else {
            if op == 2 {
                let b: i64 = stack[sp - 1];
                let a: i64 = stack[sp - 2];
                stack[sp - 2] = a + b;
                sp = sp - 1;
            } else {
                if op == 3 {
                    let b: i64 = stack[sp - 1];
                    let a: i64 = stack[sp - 2];
                    stack[sp - 2] = a * b;
                    sp = sp - 1;
                } else {
                    if op == 4 {
                        let b: i64 = stack[sp - 1];
                        let a: i64 = stack[sp - 2];
                        stack[sp - 2] = a - b;
                        sp = sp - 1;
                    } else {
                        if op == 5 {
                            let b: i64 = stack[sp - 1];
                            let a: i64 = stack[sp - 2];
                            stack[sp - 2] = a % b;
                            sp = sp - 1;
                        }
                    }
                }
            }
        }

        pc = pc + 2;
        executed = executed + 1;
    }

    let result: i64 = stack[sp - 1] + executed;
    println(result);
    return result % 256;
}

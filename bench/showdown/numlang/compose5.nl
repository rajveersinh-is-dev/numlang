fn add1(x: i64) -> i64 {
    return x + 1;
}

fn mul2(x: i64) -> i64 {
    return x * 2;
}

fn add3(x: i64) -> i64 {
    return x + 3;
}

fn sub5(x: i64) -> i64 {
    return x - 5;
}

fn add10(x: i64) -> i64 {
    return x + 10;
}

fn run_chain(val: i64) -> i64 {
    return add1(mul2(add3(sub5(add10(val)))));
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..1000 {
        let val: i64 = run_chain(i);
        sum = sum + val;
    }
    println(sum);
    return sum % 256;
}

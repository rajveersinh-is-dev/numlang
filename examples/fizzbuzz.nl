fn fizzbuzz_val(n: i64) -> i64 {
    let m15: i64 = n % 15;
    let m3: i64 = n % 3;
    let m5: i64 = n % 5;
    return match m15 {
        0 => 15,
        _ => match m3 {
            0 => 3,
            _ => match m5 {
                0 => 5,
                _ => 0,
            },
        },
    };
}

fn main() -> i64 {
    println("FizzBuzz 1..15:");
    for i in 1..=15 {
        let tag: i64 = fizzbuzz_val(i);
        match tag {
            15 => println("FizzBuzz"),
            3 => println("Fizz"),
            5 => println("Buzz"),
            _ => println(i),
        };
    }
    return 0;
}

fn square(x: i64) -> i64 {
    return x * x;
}

fn add(acc: i64, x: i64) -> i64 {
    return acc + x;
}

fn main() -> i64 {
    let xs: [i64; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let mut sum: i64 = 0;
    for i in 0..10 {
        let sq: i64 = square(xs[i]);
        sum = add(sum, sq);
    }
    println(sum);
    return sum % 256;
}

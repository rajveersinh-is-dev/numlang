// CLBG pidigits: streaming pi digit calculation using integer series approximation.

fn compute_pi_digits(n: i64) -> i64 {
    let mut sum: i64 = 0;
    let scale: i64 = 100000000;
    for k in 0..n {
        let denom: i64 = 2 * k + 1;
        let term: i64 = (4 * scale) / denom;
        if (k % 2) == 0 {
            sum = sum + term;
        } else {
            sum = sum - term;
        }
    }
    return sum / 10000;
}

fn main() -> i64 {
    let pi_val: i64 = compute_pi_digits(100);
    println(pi_val);
    return pi_val % 256;
}

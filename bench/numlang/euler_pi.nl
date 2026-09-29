fn approx_pi_10k() -> i64 {
    let scale: i64 = 1000000000;
    let mut pi_scaled: i64 = 3 * scale;
    let mut sign: i64 = 1;

    for k in 1..=1000 {
        let i: i64 = 2 * k;
        let denom: i64 = i * (i + 1) * (i + 2);
        let term: i64 = (4 * scale) / denom;
        pi_scaled = pi_scaled + (sign * term);
        sign = 0 - sign;
    }

    let divisor: i64 = scale / 10000;
    return pi_scaled / divisor;
}

fn main() -> i64 {
    let pi_10k: i64 = approx_pi_10k();
    println(pi_10k);
    return pi_10k % 256;
}

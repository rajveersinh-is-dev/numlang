fn is_prime(n: i64) -> bool {
    if n <= 1 {
        return false;
    }
    let mut d = 2;
    while d * d <= n {
        if (n % d) == 0 {
            return false;
        }
        d += 1;
    }
    true
}

fn count_primes(limit: i64) -> i64 {
    let mut sum = 0;
    for i in 2..=limit {
        if is_prime(i) {
            sum += i;
        }
    }
    sum
}

fn main() {
    let mut total: i64 = 0;
    for _ in 0..100 {
        total += count_primes(200);
    }
    println!("{}", total);
    std::process::exit((total % 256) as i32);
}

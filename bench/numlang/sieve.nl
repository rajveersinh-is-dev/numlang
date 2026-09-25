fn is_prime(n: i64) -> bool {
    if n <= 1 {
        return false;
    }
    let mut d: i64 = 2;
    while d * d <= n {
        if (n % d) == 0 {
            return false;
        }
        d = d + 1;
    }
    return true;
}

fn count_primes(limit: i64) -> i64 {
    let mut sum: i64 = 0;
    for i in 2..=limit {
        if is_prime(i) {
            sum = sum + i;
        }
    }
    return sum;
}

fn main() -> i64 {
    let mut total: i64 = 0;
    for rep in 0..100 {
        total = total + count_primes(200);
    }
    println(total);
    return total % 256;
}

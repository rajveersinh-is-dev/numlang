fn hofstadter_m(limit: usize) -> i64 {
    let mut f = [0i64; 101];
    let mut m = [0i64; 101];
    f[0] = 1;
    m[0] = 0;

    for i in 1..=limit {
        let f_prev = f[i - 1] as usize;
        let m_of_f = m[f_prev];
        f[i] = (i as i64) - m_of_f;

        let m_prev = m[i - 1] as usize;
        let f_of_m = f[m_prev];
        m[i] = (i as i64) - f_of_m;
    }
    m[limit]
}

fn main() {
    let t0 = std::time::Instant::now();
    let res = hofstadter_m(100);
    let elapsed = t0.elapsed();
    println!("{}", res);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((res % 256) as i32);
}

fn max_val(a: i64, b: i64) -> i64 {
    if a > b {
        return a;
    }
    return b;
}

fn main() -> i64 {
    let s1: [i64; 20] = [1, 2, 3, 2, 4, 1, 2, 3, 4, 1, 2, 3, 2, 4, 1, 2, 3, 4, 1, 2];
    let s2: [i64; 20] = [2, 4, 3, 1, 2, 1, 3, 4, 1, 2, 4, 3, 1, 2, 1, 3, 4, 1, 2, 4];

    let mut prev: [i64; 21] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut curr: [i64; 21] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    for i in 1..=20 {
        for j in 1..=20 {
            if s1[i - 1] == s2[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = max_val(prev[j], curr[j - 1]);
            }
        }
        for j in 0..=20 {
            prev[j] = curr[j];
        }
    }

    let lcs_len: i64 = prev[20];
    println(lcs_len);
    return lcs_len % 256;
}

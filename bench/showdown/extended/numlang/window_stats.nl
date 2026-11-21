// Extended workload 2: sliding-window sensor statistics.
// Window size, stream length and reporting stride are read from a control
// record, so the loop bounds are data-dependent rather than literal constants.
fn main() -> i64 {
    let control: [i64; 3] = [6, 24, 3];
    let window: i64 = control[0];
    let n: i64 = control[1];
    let stride: i64 = control[2];

    let samples: [i64; 24] = [
        4, 9, 2, 17, 6, 11, 3, 20, 8, 14, 1, 7, 19, 5, 12, 10, 16, 2, 13, 6, 18, 4, 9, 15
    ];

    let mut acc: i64 = 0;
    let mut start: i64 = 0;
    while start + window <= n {
        let mut sum: i64 = 0;
        let mut lo: i64 = samples[start];
        let mut hi: i64 = lo;
        let mut j: i64 = 0;
        while j < window {
            let v: i64 = samples[start + j];
            sum = sum + v;
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
            j = j + 1;
        }

        let score: i64 = hi - lo + sum % 7;
        if start % stride == 0 {
            acc = acc + score;
        } else {
            acc = acc + score / 2;
        }
        start = start + 1;
    }

    println(acc);
    return acc % 256;
}

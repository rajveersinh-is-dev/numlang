fn main() -> i64 {
    let a: [i64; 16] = [
        1, 2, 3, 4,
        5, 6, 7, 8,
        9, 10, 11, 12,
        13, 14, 15, 16
    ];
    let b: [i64; 16] = [
        2, 1, 1, 1,
        1, 2, 1, 1,
        1, 1, 2, 1,
        1, 1, 1, 2
    ];
    let mut c: [i64; 16] = [
        0, 0, 0, 0,
        0, 0, 0, 0,
        0, 0, 0, 0,
        0, 0, 0, 0
    ];

    for i in 0..4 {
        for j in 0..4 {
            let mut dot: i64 = 0;
            for k in 0..4 {
                dot = dot + a[i * 4 + k] * b[k * 4 + j];
            }
            c[i * 4 + j] = dot;
        }
    }

    let res: i64 = c[0];
    println(res);
    return res % 256;
}

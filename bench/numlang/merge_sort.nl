fn min_val(a: i64, b: i64) -> i64 {
    if a < b {
        return a;
    }
    return b;
}

fn main() -> i64 {
    let mut a: [i64; 20] = [
        45, 12, 85, 32, 89, 39, 69, 44, 42, 1,
        99, 23, 56, 78, 90, 11, 2, 7, 33, 15
    ];
    let mut tmp: [i64; 20] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    ];

    let n: i64 = 20;
    let mut w: i64 = 1;
    while w < n {
        let mut i: i64 = 0;
        while i < n {
            let left: i64 = i;
            let mid: i64 = min_val(i + w, n);
            let right: i64 = min_val(i + 2 * w, n);

            let mut p1: i64 = left;
            let mut p2: i64 = mid;
            let mut idx: i64 = left;

            while p1 < mid {
                if p2 >= right {
                    tmp[idx] = a[p1];
                    p1 = p1 + 1;
                } else {
                    if a[p1] <= a[p2] {
                        tmp[idx] = a[p1];
                        p1 = p1 + 1;
                    } else {
                        tmp[idx] = a[p2];
                        p2 = p2 + 1;
                    }
                }
                idx = idx + 1;
            }

            while p2 < right {
                tmp[idx] = a[p2];
                p2 = p2 + 1;
                idx = idx + 1;
            }

            let mut k: i64 = left;
            while k < right {
                a[k] = tmp[k];
                k = k + 1;
            }

            i = i + 2 * w;
        }
        w = w * 2;
    }

    let min_elem: i64 = a[0];
    println(min_elem);
    return min_elem % 256;
}

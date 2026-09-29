fn main() -> i64 {
    let mut a: [i64; 20] = [
        34, 7, 23, 32, 5, 62, 78, 12, 9, 81,
        14, 55, 2, 43, 67, 18, 90, 3, 29, 11
    ];
    let mut l_stack: [i64; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    ];
    let mut h_stack: [i64; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    ];

    let mut top: i64 = 0;
    l_stack[0] = 0;
    h_stack[0] = 19;

    while top >= 0 {
        let low: i64 = l_stack[top];
        let high: i64 = h_stack[top];
        top = top - 1;

        if low < high {
            let pivot: i64 = a[high];
            let mut i: i64 = low - 1;
            let mut j: i64 = low;
            while j < high {
                if a[j] <= pivot {
                    i = i + 1;
                    let tmp: i64 = a[i];
                    a[i] = a[j];
                    a[j] = tmp;
                }
                j = j + 1;
            }
            let p: i64 = i + 1;
            let tmp2: i64 = a[p];
            a[p] = a[high];
            a[high] = tmp2;

            if p - 1 > low {
                top = top + 1;
                l_stack[top] = low;
                h_stack[top] = p - 1;
            }
            if p + 1 < high {
                top = top + 1;
                l_stack[top] = p + 1;
                h_stack[top] = high;
            }
        }
    }

    let min_elem: i64 = a[0];
    println(min_elem);
    return min_elem % 256;
}

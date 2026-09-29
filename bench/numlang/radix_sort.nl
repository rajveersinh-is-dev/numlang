fn main() -> i64 {
    let mut a: [i64; 16] = [
        170, 45, 75, 90, 802, 24, 2, 66,
        123, 456, 789, 12, 34, 56, 78, 9
    ];

    let mut shift: i64 = 0;
    while shift < 16 {
        let mut i: i64 = 1;
        while i < 16 {
            let key: i64 = a[i];
            let key_digit: i64 = (key >> shift) & 255;
            let mut j: i64 = i - 1;
            while j >= 0 {
                let prev: i64 = a[j];
                let prev_digit: i64 = (prev >> shift) & 255;
                if prev_digit > key_digit {
                    a[j + 1] = prev;
                    j = j - 1;
                } else {
                    break;
                }
            }
            a[j + 1] = key;
            i = i + 1;
        }
        shift = shift + 8;
    }

    let min_elem: i64 = a[0];
    println(min_elem);
    return min_elem % 256;
}

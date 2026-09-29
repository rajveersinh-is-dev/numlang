fn main() -> i64 {
    let mut grid: [i64; 16] = [
        11, 12, 13, 14,
        21, 22, 23, 24,
        31, 32, 33, 34,
        41, 42, 43, 44
    ];
    let mut next_grid: [i64; 16] = [
        0, 0, 0, 0,
        0, 0, 0, 0,
        0, 0, 0, 0,
        0, 0, 0, 0
    ];

    for it in 0..10 {
        for i in 0..16 {
            next_grid[i] = grid[i];
        }

        for r in 1..3 {
            for c in 1..3 {
                let up: i64 = grid[(r - 1) * 4 + c];
                let down: i64 = grid[(r + 1) * 4 + c];
                let left: i64 = grid[r * 4 + (c - 1)];
                let right: i64 = grid[r * 4 + (c + 1)];
                let sum: i64 = up + down + left + right;
                next_grid[r * 4 + c] = sum / 4;
            }
        }

        for i in 0..16 {
            grid[i] = next_grid[i];
        }
    }

    let val: i64 = grid[2 * 4 + 2];
    println(val);
    return val % 256;
}

// Extended workload 4: multi-pass 2D stencil with clamped halo.
// Pass count, grid dimension and two kernel weights come from a control
// record. Nine-tap neighbourhoods are reduced with clamped boundary indices.
fn clamp_index(v: i64, hi: i64) -> i64 {
    if v < 0 {
        return 0;
    }
    if v > hi {
        return hi;
    }
    return v;
}

fn main() -> i64 {
    let control: [i64; 4] = [3, 6, 1, 2];
    let passes: i64 = control[0];
    let dim: i64 = control[1];
    let w0: i64 = control[2];
    let w1: i64 = control[3];

    let mut grid: [i64; 36] = [
        0, 7, 14, 21, 28, 6,
        13, 20, 0, 8, 16, 19,
        26, 6, 15, 19, 28, 3,
        10, 20, 25, 6, 11, 16,
        23, 5, 11, 17, 23, 0,
        7, 14, 21, 28, 6, 13
    ];
    let mut out: [i64; 36] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    ];

    let mut p: i64 = 0;
    while p < passes {
        let mut r: i64 = 0;
        while r < dim {
            let mut c: i64 = 0;
            while c < dim {
                let mut acc: i64 = 0;
                let mut k: i64 = 0;
                while k < 9 {
                    let dr: i64 = k / 3 - 1;
                    let dc: i64 = k % 3 - 1;
                    let rr: i64 = clamp_index(r + dr, dim - 1);
                    let cc: i64 = clamp_index(c + dc, dim - 1);
                    let mut coeff: i64 = w1;
                    if dr * dr + dc * dc == 0 {
                        coeff = 4;
                    } else {
                        if (dr + dc + 4) % 2 == 0 {
                            coeff = w0;
                        }
                    }
                    acc = acc + grid[rr * dim + cc] * coeff;
                    k = k + 1;
                }
                out[r * dim + c] = (acc + 8) / 10;
                c = c + 1;
            }
            r = r + 1;
        }

        let mut idx: i64 = 0;
        while idx < dim * dim {
            grid[idx] = out[idx];
            idx = idx + 1;
        }
        p = p + 1;
    }

    let result: i64 = grid[2 * dim + 3];
    println(result);
    return result % 256;
}

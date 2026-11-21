// Extended workload 4: multi-pass 2D stencil with clamped halo and dynamic passes.
// Reference implementation for bench/showdown/extended/numlang/nested_stencil.nl.
use std::time::Instant;

fn clamp_index(v: i64, hi: i64) -> i64 {
    if v < 0 {
        return 0;
    }
    if v > hi {
        return hi;
    }
    v
}

fn main() {
    let t0 = Instant::now();

    let control: [i64; 4] = [3, 6, 1, 2];
    let passes = control[0];
    let dim = control[1];
    let w0 = control[2];
    let w1 = control[3];

    let mut grid: [i64; 36] = [
        0, 7, 14, 21, 28, 6, 13, 20, 0, 8, 16, 19, 26, 6, 15, 19, 28, 3, 10, 20, 25, 6, 11,
        16, 23, 5, 11, 17, 23, 0, 7, 14, 21, 28, 6, 13,
    ];
    let mut out: [i64; 36] = [0; 36];

    let mut p: i64 = 0;
    while p < passes {
        let mut r: i64 = 0;
        while r < dim {
            let mut c: i64 = 0;
            while c < dim {
                let mut acc: i64 = 0;
                let mut k: i64 = 0;
                while k < 9 {
                    let dr = k / 3 - 1;
                    let dc = k % 3 - 1;
                    let rr = clamp_index(r + dr, dim - 1);
                    let cc = clamp_index(c + dc, dim - 1);
                    let coeff = if dr == 0 && dc == 0 {
                        4
                    } else if (dr + dc + 4) % 2 == 0 {
                        w0
                    } else {
                        w1
                    };
                    acc = acc.wrapping_add(
                        grid[(rr * dim + cc) as usize].wrapping_mul(coeff),
                    );
                    k += 1;
                }
                out[(r * dim + c) as usize] = (acc + 8) / 10;
                c += 1;
            }
            r += 1;
        }
        let mut i: i64 = 0;
        while i < dim * dim {
            grid[i as usize] = out[i as usize];
            i += 1;
        }
        p += 1;
    }

    let result = grid[(2 * dim + 3) as usize];
    println!("{}", result);
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(result.rem_euclid(256) as i32);
}

fn count_flips(p0: i64, p1: i64, p2: i64, p3: i64, p4: i64) -> i64 {
    let mut q0: i64 = p0;
    let mut q1: i64 = p1;
    let mut q2: i64 = p2;
    let mut q3: i64 = p3;
    let mut q4: i64 = p4;
    let mut flips: i64 = 0;

    while q0 != 1 {
        if q0 == 2 {
            let t: i64 = q0;
            q0 = q1;
            q1 = t;
        } else {
            if q0 == 3 {
                let t: i64 = q0;
                q0 = q2;
                q2 = t;
            } else {
                if q0 == 4 {
                    let t: i64 = q0;
                    q0 = q3;
                    q3 = t;
                    let t2: i64 = q1;
                    q1 = q2;
                    q2 = t2;
                } else {
                    if q0 == 5 {
                        let t: i64 = q0;
                        q0 = q4;
                        q4 = t;
                        let t2: i64 = q1;
                        q1 = q3;
                        q3 = t2;
                    }
                }
            }
        }
        flips = flips + 1;
    }
    return flips;
}

fn main() -> i64 {
    let mut max_flips: i64 = 0;
    let mut total_flips: i64 = 0;

    // Iterate over permutations of [1,2,3,4,5]
    for i0 in 1..=5 {
        for i1 in 1..=5 {
            if i1 != i0 {
                for i2 in 1..=5 {
                    if i2 != i0 {
                        if i2 != i1 {
                            for i3 in 1..=5 {
                                if i3 != i0 {
                                    if i3 != i1 {
                                        if i3 != i2 {
                                            for i4 in 1..=5 {
                                                if i4 != i0 {
                                                    if i4 != i1 {
                                                        if i4 != i2 {
                                                            if i4 != i3 {
                                                                let f: i64 = count_flips(i0, i1, i2, i3, i4);
                                                                if f > max_flips {
                                                                    max_flips = f;
                                                                }
                                                                total_flips = total_flips + f;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println(max_flips);
    println(total_flips);
    return (max_flips + total_flips) % 256;
}

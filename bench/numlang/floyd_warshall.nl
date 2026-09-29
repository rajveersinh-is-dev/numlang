fn get_edge(u: i64, v: i64) -> i64 {
    let mut w: i64 = 999999;
    if u == v {
        w = 0;
    }
    if u == 0 {
        if v == 1 { w = 3; }
        if v == 3 { w = 12; }
    }
    if u == 1 {
        if v == 0 { w = 3; }
        if v == 2 { w = 5; }
    }
    if u == 2 {
        if v == 1 { w = 5; }
        if v == 3 { w = 2; }
        if v == 5 { w = 9; }
    }
    if u == 3 {
        if v == 0 { w = 12; }
        if v == 2 { w = 2; }
        if v == 4 { w = 4; }
    }
    if u == 4 {
        if v == 3 { w = 4; }
        if v == 5 { w = 6; }
        if v == 7 { w = 10; }
    }
    if u == 5 {
        if v == 2 { w = 9; }
        if v == 4 { w = 6; }
        if v == 6 { w = 1; }
    }
    if u == 6 {
        if v == 5 { w = 1; }
        if v == 7 { w = 7; }
    }
    if u == 7 {
        if v == 4 { w = 10; }
        if v == 6 { w = 7; }
    }
    return w;
}

fn main() -> i64 {
    let mut dist: [i64; 64] = [
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0
    ];

    for i in 0..8 {
        for j in 0..8 {
            dist[i * 8 + j] = get_edge(i, j);
        }
    }

    for k in 0..8 {
        for i in 0..8 {
            for j in 0..8 {
                let direct: i64 = dist[i * 8 + j];
                let ik: i64 = dist[i * 8 + k];
                let kj: i64 = dist[k * 8 + j];
                let through_k: i64 = ik + kj;
                if through_k < direct {
                    dist[i * 8 + j] = through_k;
                }
            }
        }
    }

    let res: i64 = dist[7]; // dist[0 * 8 + 7]
    println(res);
    return res % 256;
}

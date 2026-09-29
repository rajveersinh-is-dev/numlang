fn get_weight(u: i64, v: i64) -> i64 {
    let mut w: i64 = 0;
    if u == 0 {
        if v == 1 { w = 4; }
        if v == 2 { w = 2; }
    }
    if u == 1 {
        if v == 0 { w = 4; }
        if v == 2 { w = 1; }
        if v == 3 { w = 5; }
    }
    if u == 2 {
        if v == 0 { w = 2; }
        if v == 1 { w = 1; }
        if v == 4 { w = 10; }
    }
    if u == 3 {
        if v == 1 { w = 5; }
        if v == 5 { w = 3; }
    }
    if u == 4 {
        if v == 2 { w = 10; }
        if v == 5 { w = 2; }
        if v == 6 { w = 4; }
    }
    if u == 5 {
        if v == 3 { w = 3; }
        if v == 4 { w = 2; }
        if v == 7 { w = 8; }
    }
    if u == 6 {
        if v == 4 { w = 4; }
        if v == 8 { w = 3; }
    }
    if u == 7 {
        if v == 5 { w = 8; }
        if v == 9 { w = 6; }
    }
    if u == 8 {
        if v == 6 { w = 3; }
        if v == 9 { w = 7; }
    }
    if u == 9 {
        if v == 7 { w = 6; }
        if v == 8 { w = 7; }
    }
    return w;
}

fn main() -> i64 {
    let mut cost: [i64; 10] = [
        0, 999999, 999999, 999999, 999999,
        999999, 999999, 999999, 999999, 999999
    ];
    let mut visited: [i64; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    let mut step: i64 = 0;
    while step < 10 {
        let mut min_u: i64 = -1;
        let mut min_val: i64 = 999999;
        let mut i: i64 = 0;
        while i < 10 {
            if visited[i] == 0 {
                let c: i64 = cost[i];
                if c < min_val {
                    min_val = c;
                    min_u = i;
                }
            }
            i = i + 1;
        }

        if min_u == -1 {
            step = 10; // done
        } else {
            visited[min_u] = 1;
            let mut v: i64 = 0;
            while v < 10 {
                let w: i64 = get_weight(min_u, v);
                if w > 0 {
                    let new_cost: i64 = min_val + w;
                    let cur_c: i64 = cost[v];
                    if new_cost < cur_c {
                        cost[v] = new_cost;
                    }
                }
                v = v + 1;
            }
            step = step + 1;
        }
    }

    let res: i64 = cost[9];
    println(res);
    return res % 256;
}

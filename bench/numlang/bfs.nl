fn has_edge(u: i64, v: i64) -> bool {
    let mut res: bool = false;
    if u == 0 { if v == 1 { res = true; } if v == 2 { res = true; } }
    if u == 1 { if v == 0 { res = true; } if v == 3 { res = true; } }
    if u == 2 { if v == 0 { res = true; } if v == 4 { res = true; } }
    if u == 3 { if v == 1 { res = true; } if v == 5 { res = true; } }
    if u == 4 { if v == 2 { res = true; } if v == 5 { res = true; } if v == 6 { res = true; } }
    if u == 5 { if v == 3 { res = true; } if v == 4 { res = true; } if v == 7 { res = true; } }
    if u == 6 { if v == 4 { res = true; } if v == 8 { res = true; } }
    if u == 7 { if v == 5 { res = true; } if v == 9 { res = true; } }
    if u == 8 { if v == 6 { res = true; } if v == 9 { res = true; } }
    if u == 9 { if v == 7 { res = true; } if v == 8 { res = true; } }
    return res;
}

fn main() -> i64 {
    let mut visited: [i64; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut dist: [i64; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut q: [i64; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    let mut head: i64 = 0;
    let mut tail: i64 = 1;
    q[0] = 0;
    visited[0] = 1;
    dist[0] = 0;

    while head < tail {
        let u: i64 = q[head];
        head = head + 1;

        let mut v: i64 = 0;
        while v < 10 {
            if has_edge(u, v) {
                let vis: i64 = visited[v];
                if vis == 0 {
                    visited[v] = 1;
                    let du: i64 = dist[u];
                    dist[v] = du + 1;
                    q[tail] = v;
                    tail = tail + 1;
                }
            }
            v = v + 1;
        }
    }

    let d: i64 = dist[9];
    println(d);
    return d % 256;
}

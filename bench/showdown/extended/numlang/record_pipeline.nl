// Extended workload 3: struct/array transform pipeline.
// Normalise raw readings, accumulate per-bucket totals, then apply a
// position-dependent boost. Struct fields are copied into locals to stay
// inside the flat-value semantics of fixed-size struct arrays.
struct Reading {
    id: i64,
    raw: i64,
    weight: i64,
    bucket: i64,
}

fn main() -> i64 {
    let records: [Reading; 10] = [
        Reading { id: 0, raw: 137, weight: 3, bucket: 2 },
        Reading { id: 1, raw: 89, weight: 5, bucket: 1 },
        Reading { id: 2, raw: 204, weight: 2, bucket: 3 },
        Reading { id: 3, raw: 45, weight: 7, bucket: 0 },
        Reading { id: 4, raw: 176, weight: 4, bucket: 2 },
        Reading { id: 5, raw: 63, weight: 6, bucket: 1 },
        Reading { id: 6, raw: 151, weight: 3, bucket: 3 },
        Reading { id: 7, raw: 28, weight: 8, bucket: 0 },
        Reading { id: 8, raw: 199, weight: 2, bucket: 2 },
        Reading { id: 9, raw: 112, weight: 5, bucket: 1 },
    ];

    let mut norm: [i64; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut i: i64 = 0;
    while i < 10 {
        let r: Reading = records[i];
        norm[i] = (r.raw % 101) * r.weight;
        i = i + 1;
    }

    let mut buckets: [i64; 4] = [0, 0, 0, 0];
    i = 0;
    while i < 10 {
        let r: Reading = records[i];
        let b: i64 = r.id % 4;
        buckets[b] = buckets[b] + norm[i] + r.bucket;
        i = i + 1;
    }

    let mut total: i64 = 0;
    let mut k: i64 = 0;
    while k < 4 {
        let mut boost: i64 = k * 2;
        if k == 0 {
            boost = 3;
        }
        total = total + buckets[k] * boost;
        k = k + 1;
    }

    println(total);
    return total % 256;
}

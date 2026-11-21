// Extended workload 3: struct/array record transform and bucket accumulation.
// Reference implementation for bench/showdown/extended/numlang/record_pipeline.nl.
use std::time::Instant;

#[derive(Clone, Copy)]
struct Reading {
    id: i64,
    raw: i64,
    weight: i64,
    bucket: i64,
}

fn main() {
    let t0 = Instant::now();

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

    let mut norm: [i64; 10] = [0; 10];
    for i in 0..10 {
        let r = records[i];
        norm[i] = (r.raw % 101) * r.weight;
    }

    let mut buckets: [i64; 4] = [0; 4];
    for i in 0..10 {
        let r = records[i];
        let b = (r.id % 4) as usize;
        buckets[b] = buckets[b].wrapping_add(norm[i]).wrapping_add(r.bucket);
    }

    let mut total: i64 = 0;
    let mut k: i64 = 0;
    while k < 4 {
        let boost = if k == 0 { 3 } else { k * 2 };
        total = total.wrapping_add(buckets[k as usize].wrapping_mul(boost));
        k += 1;
    }

    println!("{}", total);
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(total.rem_euclid(256) as i32);
}

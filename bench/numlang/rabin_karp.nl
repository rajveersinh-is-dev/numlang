fn main() -> i64 {
    let mut text: [i64; 200] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0
    ];

    for i in 0..200 {
        text[i] = 68 + (i % 20);
    }
    // Pattern "ABC" placed at 142
    text[142] = 65;
    text[143] = 66;
    text[144] = 67;

    let pattern: [i64; 3] = [65, 66, 67];
    let b: i64 = 257;
    let m: i64 = 1000003;
    let b2: i64 = (b * b) % m;

    let p_hash: i64 = (((pattern[0] * b + pattern[1]) % m) * b + pattern[2]) % m;
    let mut t_hash: i64 = (((text[0] * b + text[1]) % m) * b + text[2]) % m;

    let mut match_pos: i64 = -1;
    let mut i: i64 = 0;
    while i <= 197 {
        if t_hash == p_hash {
            if text[i] == pattern[0] {
                if text[i + 1] == pattern[1] {
                    if text[i + 2] == pattern[2] {
                        match_pos = i;
                        break;
                    }
                }
            }
        }

        if i < 197 {
            let rem: i64 = (text[i] * b2) % m;
            let mut diff: i64 = (t_hash - rem) % m;
            if diff < 0 {
                diff = diff + m;
            }
            t_hash = (diff * b + text[i + 3]) % m;
        }
        i = i + 1;
    }

    println(match_pos);
    return match_pos % 256;
}

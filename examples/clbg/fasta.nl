fn generate_fasta(n: i64) -> i64 {
    let mut seed: i64 = 42;
    let im: i64 = 139968;
    let ia: i64 = 3877;
    let ic: i64 = 29573;

    let mut checksum: i64 = 0;

    for i in 0..n {
        seed = (seed * ia + ic) % im;
        let mut base_code: i64 = 84; // 'T'
        if seed < 37791 {
            base_code = 65; // 'A'
        } else {
            if seed < 54587 {
                base_code = 67; // 'C'
            } else {
                if seed < 71383 {
                    base_code = 71; // 'G'
                }
            }
        }
        checksum = (checksum * 31 + base_code) % 1000000007;
    }

    return checksum;
}

fn main() -> i64 {
    let cs: i64 = generate_fasta(1000);
    println(cs);
    return cs % 256;
}

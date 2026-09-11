import subprocess

nl_code = """
fn collatz_steps(limit: i64) -> i64 {
    let mut total_steps: i64 = 0;
    let mut n: i64 = 1;
    while n <= limit {
        let mut curr: i64 = n;
        while curr > 1 {
            if curr % 2 == 0 {
                curr = curr / 2;
            } else {
                curr = curr * 3 + 1;
            }
            total_steps = total_steps + 1;
        }
        n = n + 1;
    }
    return total_steps % 256;
}
fn main() -> i64 {
    return collatz_steps(100000);
}
"""

rs_code = """
fn collatz_steps(limit: i64) -> i64 {
    let mut total_steps: i64 = 0;
    let mut n: i64 = 1;
    while n <= limit {
        let mut curr: i64 = n;
        while curr > 1 {
            if curr % 2 == 0 {
                curr /= 2;
            } else {
                curr = curr * 3 + 1;
            }
            total_steps += 1;
        }
        n += 1;
    }
    total_steps % 256
}
fn main() {
    let t0 = std::time::Instant::now();
    let res = collatz_steps(100000);
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(res as i32);
}
"""

c_code = """
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>

long long collatz_steps(long long limit) {
    long long total_steps = 0;
    long long n = 1;
    while (n <= limit) {
        long long curr = n;
        while (curr > 1) {
            if (curr % 2 == 0) {
                curr /= 2;
            } else {
                curr = curr * 3 + 1;
            }
            total_steps++;
        }
        n++;
    }
    return total_steps % 256;
}
int main() {
    LARGE_INTEGER f, t0, t1;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&t0);
    long long res = collatz_steps(100000);
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / f.QuadPart;
    printf("COMPUTE_NS: %lld\\n", ns);
    return (int)res;
}
"""

with open("collatz_nl.nl", "w") as f: f.write(nl_code)
with open("collatz_rs.rs", "w") as f: f.write(rs_code)
with open("collatz_c.c", "w") as f: f.write(c_code)

subprocess.run([r"target\release\numlang.exe", "build", "collatz_nl.nl", "--bench", "-o", "collatz_nl.exe"], check=True)
subprocess.run(["rustc", "-O", "collatz_rs.rs", "-o", "collatz_rs.exe"], check=True)
has_cl = False
try:
    subprocess.run(["cl", "/O2", "collatz_c.c", "/Fe:collatz_c.exe"], check=True, capture_output=True)
    has_cl = True
except Exception:
    pass

nl_times = [int(subprocess.run([".\\collatz_nl.exe"], capture_output=True, text=True).stdout.split(":")[1].strip()) for _ in range(5)]
rs_times = [int(subprocess.run([".\\collatz_rs.exe"], capture_output=True, text=True).stdout.split(":")[1].strip()) for _ in range(5)]
c_times  = [int(subprocess.run([".\\collatz_c.exe"], capture_output=True, text=True).stdout.split(":")[1].strip()) for _ in range(5)] if has_cl else []

p_nl = subprocess.run([".\\collatz_nl.exe"], capture_output=True, text=True)
p_rs = subprocess.run([".\\collatz_rs.exe"], capture_output=True, text=True)

print(f"NumLang Min: {min(nl_times) / 1e6:.2f} ms ({min(nl_times)} ns), Exit: {p_nl.returncode}")
print(f"Rust    Min: {min(rs_times) / 1e6:.2f} ms ({min(rs_times)} ns), Exit: {p_rs.returncode}")
if has_cl:
    p_c = subprocess.run([".\\collatz_c.exe"], capture_output=True, text=True)
    print(f"C (MSVC)Min: {min(c_times) / 1e6:.2f} ms ({min(c_times)} ns), Exit: {p_c.returncode}")

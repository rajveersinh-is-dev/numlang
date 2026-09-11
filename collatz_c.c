
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
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)res;
}

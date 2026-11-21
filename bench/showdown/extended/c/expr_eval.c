/* Extended workload 1: stack-machine expression evaluator (C reference). */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int main(void) {
#ifdef _WIN32
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
#else
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
#endif

    /* Header cell 0 is the bytecode length in i64 cells; 11 instructions. */
    const int64_t code[23] = {
        22, 1, 7, 1, 5, 2, 0, 1, 3, 3, 0, 1, 11, 4, 0, 1, 2, 1, 9, 2, 0, 5, 0
    };
    int64_t program_end = 1 + code[0];
    int64_t stack[16] = {0};
    int64_t sp = 0;
    int64_t pc = 1;
    int64_t executed = 0;

    while (pc < program_end) {
        int64_t op = code[pc];
        int64_t arg = code[pc + 1];
        if (op == 1) {
            stack[sp++] = arg;
        } else if (op == 2) {
            int64_t b = stack[sp - 1];
            int64_t a = stack[sp - 2];
            stack[sp - 2] = a + b;
            sp -= 1;
        } else if (op == 3) {
            int64_t b = stack[sp - 1];
            int64_t a = stack[sp - 2];
            stack[sp - 2] = a * b;
            sp -= 1;
        } else if (op == 4) {
            int64_t b = stack[sp - 1];
            int64_t a = stack[sp - 2];
            stack[sp - 2] = a - b;
            sp -= 1;
        } else if (op == 5) {
            int64_t b = stack[sp - 1];
            int64_t a = stack[sp - 2];
            stack[sp - 2] = (b == 0) ? 0 : (a % b);
            sp -= 1;
        }
        pc += 2;
        executed += 1;
    }

    int64_t result = stack[sp - 1] + executed;
    printf("%lld\n", (long long)result);
#ifdef _WIN32
    QueryPerformanceCounter(&t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart));
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec)));
#endif
    return (int)(result % 256);
}

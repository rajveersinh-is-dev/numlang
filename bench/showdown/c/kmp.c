#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

typedef struct Node {
    int64_t val;
    struct Node* next;
} Node;

Node* cons(int64_t v, Node* next) {
    Node* n = (Node*)malloc(sizeof(Node));
    n->val = v;
    n->next = next;
    return n;
}

void free_list(Node* xs) {
    while (xs) {
        Node* t = xs->next;
        free(xs);
        xs = t;
    }
}

Node* make_text(int64_t n) {
    if (n <= 0) return NULL;
    int64_t bit = (n * 73 + 19) % 2;
    return cons(bit, make_text(n - 1));
}

int64_t match_state0(Node* text);
int64_t match_state1(Node* text);
int64_t match_state2(Node* text);

int64_t match_state0(Node* text) {
    if (!text) return 0;
    if (text->val == 1) {
        return match_state1(text->next);
    } else {
        return match_state0(text->next);
    }
}

int64_t match_state1(Node* text) {
    if (!text) return 0;
    if (text->val == 0) {
        return match_state2(text->next);
    } else {
        return match_state1(text->next);
    }
}

int64_t match_state2(Node* text) {
    if (!text) return 0;
    if (text->val == 1) {
        return 1 + match_state1(text->next);
    } else {
        return match_state0(text->next);
    }
}

int main(void) {
    #ifdef _WIN32
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
#else
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
#endif

    int64_t sum = 0;
    for (int i = 0; i < 100; i++) {
        Node* t = make_text(15);
        sum += match_state0(t);
        free_list(t);
    }

    #ifdef _WIN32
    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    int64_t ns = (int64_t)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
#endif

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}

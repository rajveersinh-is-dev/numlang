#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <windows.h>

typedef struct Node {
    int64_t head;
    struct Node* tail;
} Node;

Node* cons(int64_t h, Node* t) {
    Node* n = (Node*)malloc(sizeof(Node));
    n->head = h;
    n->tail = t;
    return n;
}

Node* append(Node* xs, Node* ys) {
    if (!xs) return ys;
    return cons(xs->head, append(xs->tail, ys));
}

Node* nrev(Node* xs) {
    if (!xs) return NULL;
    return append(nrev(xs->tail), cons(xs->head, NULL));
}

Node* double_nrev(Node* xs) {
    return nrev(nrev(xs));
}

int64_t sum_list(Node* xs) {
    int64_t s = 0;
    while (xs) {
        s += xs->head;
        xs = xs->tail;
    }
    return s;
}

void free_list(Node* xs) {
    while (xs) {
        Node* t = xs->tail;
        free(xs);
        xs = t;
    }
}

Node* make_list(int64_t n) {
    if (n <= 0) return NULL;
    return cons(n, make_list(n - 1));
}

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t sum = 0;
    for (int i = 0; i < 100; i++) {
        Node* xs = make_list(15);
        Node* rev2 = double_nrev(xs);
        sum += sum_list(rev2);
        free_list(xs);
        free_list(rev2);
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}

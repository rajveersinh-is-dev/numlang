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

Node* make_list(int64_t start, int64_t len) {
    if (len <= 0) return NULL;
    return cons(start, make_list(start + 1, len - 1));
}

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t sum = 0;
    for (int i = 0; i < 100; i++) {
        Node* xs = make_list(1, 10);
        Node* ys = make_list(11, 10);
        Node* zs = make_list(21, 10);
        Node* app = append(append(xs, ys), zs);
        sum += sum_list(app);
        free_list(xs);
        free_list(ys);
        free_list(zs);
        free_list(app);
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}

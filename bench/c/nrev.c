#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

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
    int64_t sum = 0;
    for (int i = 0; i < 100; i++) {
        Node* xs = make_list(15);
        Node* rev = nrev(xs);
        sum += sum_list(rev);
        free_list(xs);
        free_list(rev);
    }
    printf("%lld\n", (long long)sum);
    return (int)(sum % 256);
}

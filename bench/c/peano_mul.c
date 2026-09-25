#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

typedef struct Peano {
    int is_zero;
    struct Peano* pred;
} Peano;

Peano* zero(void) {
    Peano* p = (Peano*)malloc(sizeof(Peano));
    p->is_zero = 1;
    p->pred = NULL;
    return p;
}

Peano* succ(Peano* pred) {
    Peano* p = (Peano*)malloc(sizeof(Peano));
    p->is_zero = 0;
    p->pred = pred;
    return p;
}

Peano* clone_peano(Peano* p) {
    if (p->is_zero) return zero();
    return succ(clone_peano(p->pred));
}

Peano* add(Peano* x, Peano* y) {
    if (x->is_zero) return y;
    return succ(add(x->pred, y));
}

Peano* mul(Peano* x, Peano* y) {
    if (x->is_zero) return zero();
    return add(y, mul(x->pred, clone_peano(y)));
}

int64_t to_int(Peano* p) {
    if (p->is_zero) return 0;
    return 1 + to_int(p->pred);
}

void free_peano(Peano* p) {
    while (p) {
        if (p->is_zero) {
            free(p);
            break;
        }
        Peano* next = p->pred;
        free(p);
        p = next;
    }
}

Peano* from_int(int64_t n) {
    if (n <= 0) return zero();
    return succ(from_int(n - 1));
}

int main(void) {
    int64_t sum = 0;
    for (int i = 0; i < 100; i++) {
        Peano* three = from_int(3);
        Peano* four = from_int(4);
        Peano* prod = mul(three, four);
        sum += to_int(prod);
        free_peano(three);
        free_peano(prod);
    }
    printf("%lld\n", (long long)sum);
    return (int)(sum % 256);
}

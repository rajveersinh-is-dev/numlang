#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <windows.h>

typedef struct Tree {
    int64_t val;
    struct Tree* left;
    struct Tree* right;
} Tree;

Tree* make_tree(int64_t depth, int64_t val) {
    if (depth <= 0) return NULL;
    Tree* t = (Tree*)malloc(sizeof(Tree));
    t->val = val;
    t->left = make_tree(depth - 1, val * 2);
    t->right = make_tree(depth - 1, val * 2 + 1);
    return t;
}

Tree* flip(Tree* t) {
    if (!t) return NULL;
    Tree* flipped = (Tree*)malloc(sizeof(Tree));
    flipped->val = t->val;
    flipped->left = flip(t->right);
    flipped->right = flip(t->left);
    return flipped;
}

int64_t sum_tree(Tree* t) {
    if (!t) return 0;
    return t->val + sum_tree(t->left) + sum_tree(t->right);
}

void free_tree(Tree* t) {
    if (!t) return;
    free_tree(t->left);
    free_tree(t->right);
    free(t);
}

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t sum = 0;
    for (int i = 0; i < 50; i++) {
        Tree* t = make_tree(8, 1);
        Tree* f = flip(t);
        sum += sum_tree(f);
        free_tree(t);
        free_tree(f);
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}

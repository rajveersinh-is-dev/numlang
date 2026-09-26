#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <windows.h>

typedef enum { LEAF, NODE } TreeTag;

typedef struct Tree {
    TreeTag tag;
    union {
        int64_t val;
        struct {
            struct Tree* left;
            struct Tree* right;
        } node;
    } data;
} Tree;

Tree* make_leaf(int64_t val) {
    Tree* t = (Tree*)malloc(sizeof(Tree));
    t->tag = LEAF;
    t->data.val = val;
    return t;
}

Tree* make_node(Tree* left, Tree* right) {
    Tree* t = (Tree*)malloc(sizeof(Tree));
    t->tag = NODE;
    t->data.node.left = left;
    t->data.node.right = right;
    return t;
}

Tree* make_tree(int64_t depth, int64_t val) {
    if (depth <= 0) {
        return make_leaf(val);
    } else {
        Tree* left = make_tree(depth - 1, val * 2);
        Tree* right = make_tree(depth - 1, val * 2 + 1);
        return make_node(left, right);
    }
}

Tree* flip(Tree* t) {
    if (t->tag == LEAF) {
        return make_leaf(t->data.val);
    } else {
        Tree* r = flip(t->data.node.right);
        Tree* l = flip(t->data.node.left);
        return make_node(r, l);
    }
}

int64_t sum_tree(Tree* t) {
    if (t->tag == LEAF) {
        return t->data.val;
    } else {
        return sum_tree(t->data.node.left) + sum_tree(t->data.node.right);
    }
}

void free_tree(Tree* t) {
    if (!t) return;
    if (t->tag == NODE) {
        free_tree(t->data.node.left);
        free_tree(t->data.node.right);
    }
    free(t);
}

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t sum = 0;
    for (int i = 0; i < 100; i++) {
        Tree* t = make_tree(4, 1);
        Tree* f1 = flip(t);
        Tree* f2 = flip(f1);
        sum += sum_tree(f2);
        free_tree(t);
        free_tree(f1);
        free_tree(f2);
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}

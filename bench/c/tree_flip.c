#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

typedef struct Tree {
    int is_leaf;
    int64_t val;
    struct Tree* left;
    struct Tree* right;
} Tree;

Tree* make_leaf(int64_t v) {
    Tree* t = (Tree*)malloc(sizeof(Tree));
    t->is_leaf = 1;
    t->val = v;
    t->left = NULL;
    t->right = NULL;
    return t;
}

Tree* make_node(Tree* l, Tree* r) {
    Tree* t = (Tree*)malloc(sizeof(Tree));
    t->is_leaf = 0;
    t->val = 0;
    t->left = l;
    t->right = r;
    return t;
}

Tree* flip(Tree* t) {
    if (t->is_leaf) return make_leaf(t->val);
    return make_node(flip(t->right), flip(t->left));
}

int64_t sum_tree(Tree* t) {
    if (t->is_leaf) return t->val;
    return sum_tree(t->left) + sum_tree(t->right);
}

void free_tree(Tree* t) {
    if (!t) return;
    free_tree(t->left);
    free_tree(t->right);
    free(t);
}

Tree* build_tree(int64_t depth, int64_t val) {
    if (depth <= 0) return make_leaf(val);
    Tree* l = build_tree(depth - 1, val * 2);
    Tree* r = build_tree(depth - 1, val * 2 + 1);
    return make_node(l, r);
}

int main(void) {
    int64_t sum = 0;
    for (int i = 0; i < 100; i++) {
        Tree* t = build_tree(4, 1);
        Tree* f1 = flip(t);
        Tree* f2 = flip(f1);
        sum += sum_tree(f2);
        free_tree(t);
        free_tree(f1);
        free_tree(f2);
    }
    printf("%lld\n", (long long)sum);
    return (int)(sum % 256);
}

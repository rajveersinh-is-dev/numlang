#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

typedef struct Node {
    int is_leaf;
    int64_t val;
    struct Node* left;
    struct Node* right;
} Node;

static Node* make_tree(int64_t depth, int64_t val) {
    Node* n = (Node*)malloc(sizeof(Node));
    if (depth <= 0) {
        n->is_leaf = 1;
        n->val = val;
        n->left = NULL;
        n->right = NULL;
        return n;
    } else {
        n->is_leaf = 0;
        n->val = 0;
        n->left = make_tree(depth - 1, val * 2);
        n->right = make_tree(depth - 1, val * 2 + 1);
        return n;
    }
}

static int64_t check_tree(Node* n) {
    if (n->is_leaf) {
        return n->val;
    } else {
        return check_tree(n->left) + check_tree(n->right);
    }
}

static void free_tree(Node* n) {
    if (!n) return;
    if (!n->is_leaf) {
        free_tree(n->left);
        free_tree(n->right);
    }
    free(n);
}

int main(void) {
    int64_t max_depth = 6;
    int64_t total_check = 0;
    for (int64_t d = 1; d <= max_depth; d++) {
        Node* t = make_tree(d, 1);
        total_check += check_tree(t);
        free_tree(t);
    }
    printf("%lld\n", (long long)total_check);
    return (int)(total_check % 256);
}

enum Tree {
    Leaf(i64),
    Node(Box<Tree>, Box<Tree>),
}

fn make_tree(depth: i64, val: i64) -> Tree {
    if depth <= 0 {
        return Leaf(val);
    } else {
        let left: Tree = make_tree(depth - 1, val * 2);
        let right: Tree = make_tree(depth - 1, val * 2 + 1);
        return Node(box(left), box(right));
    }
}

fn check_tree(t: Tree) -> i64 {
    return match t {
        Leaf(v) => v,
        Node(l, r) => check_tree(deref(l)) + check_tree(deref(r)),
    };
}

fn main() -> i64 {
    let max_depth: i64 = 6;
    let mut total_check: i64 = 0;
    for d in 1..=max_depth {
        let t: Tree = make_tree(d, 1);
        total_check = total_check + check_tree(t);
    }
    println(total_check);
    return total_check % 256;
}

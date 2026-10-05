enum Tree {
    Leaf(i64),
    Node(Box<Tree>, Box<Tree>),
}

fn flip(t: Tree) -> Tree {
    return match t {
        Leaf(v) => Leaf(v),
        Node(l, r) => Node(box(flip(deref(r))), box(flip(deref(l)))),
    };
}

fn sum_tree(t: Tree) -> i64 {
    return match t {
        Leaf(v) => v,
        Node(l, r) => sum_tree(deref(l)) + sum_tree(deref(r)),
    };
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

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..100 {
        let t: Tree = make_tree(4, 1);
        let flipped_twice: Tree = flip(flip(t));
        sum = sum + sum_tree(flipped_twice);
    }
    println(sum);
    return sum % 256;
}

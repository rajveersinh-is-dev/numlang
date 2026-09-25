enum Tree {
    Leaf(i64),
    Node(Box<Tree>, Box<Tree>),
}

fn flip(t: Tree) -> Tree {
    match t {
        Tree::Leaf(v) => Tree::Leaf(v),
        Tree::Node(l, r) => Tree::Node(Box::new(flip(*r)), Box::new(flip(*l))),
    }
}

fn sum_tree(t: &Tree) -> i64 {
    match t {
        Tree::Leaf(v) => *v,
        Tree::Node(l, r) => sum_tree(l) + sum_tree(r),
    }
}

fn make_tree(depth: i64, val: i64) -> Tree {
    if depth <= 0 {
        Tree::Leaf(val)
    } else {
        let left = make_tree(depth - 1, val * 2);
        let right = make_tree(depth - 1, val * 2 + 1);
        Tree::Node(Box::new(left), Box::new(right))
    }
}

fn main() {
    let mut sum: i64 = 0;
    for _ in 0..100 {
        let t = make_tree(4, 1);
        let flipped_twice = flip(flip(t));
        sum += sum_tree(&flipped_twice);
    }
    println!("{}", sum);
    std::process::exit((sum % 256) as i32);
}

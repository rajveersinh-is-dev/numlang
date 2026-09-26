enum Tree {
    Nil,
    Node(i64, Box<Tree>, Box<Tree>),
}

fn make_tree(depth: i64, val: i64) -> Tree {
    if depth <= 0 {
        Tree::Nil
    } else {
        Tree::Node(
            val,
            Box::new(make_tree(depth - 1, val * 2)),
            Box::new(make_tree(depth - 1, val * 2 + 1)),
        )
    }
}

fn flip(t: &Tree) -> Tree {
    match t {
        Tree::Nil => Tree::Nil,
        Tree::Node(v, l, r) => Tree::Node(*v, Box::new(flip(r)), Box::new(flip(l))),
    }
}

fn sum_tree(t: &Tree) -> i64 {
    match t {
        Tree::Nil => 0,
        Tree::Node(v, l, r) => *v + sum_tree(l) + sum_tree(r),
    }
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for _ in 0..50 {
        let t = make_tree(8, 1);
        let f = flip(&t);
        sum += sum_tree(&f);
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}

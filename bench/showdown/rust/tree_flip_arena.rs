#[derive(Clone, Copy)]
enum Tree {
    Leaf(i64),
    Node(*const Tree, *const Tree),
}

static mut POOL: [Tree; 200000] = [Tree::Leaf(0); 200000];
static mut POOL_IDX: usize = 0;

unsafe fn make_leaf(v: i64) -> *const Tree {
    let idx = POOL_IDX;
    POOL_IDX += 1;
    let ptr = &raw mut POOL[idx];
    *ptr = Tree::Leaf(v);
    ptr
}

unsafe fn make_node(l: *const Tree, r: *const Tree) -> *const Tree {
    let idx = POOL_IDX;
    POOL_IDX += 1;
    let ptr = &raw mut POOL[idx];
    *ptr = Tree::Node(l, r);
    ptr
}

unsafe fn make_tree(depth: i64, val: i64) -> *const Tree {
    if depth <= 0 {
        make_leaf(val)
    } else {
        let left = make_tree(depth - 1, val * 2);
        let right = make_tree(depth - 1, val * 2 + 1);
        make_node(left, right)
    }
}

unsafe fn flip(t: *const Tree) -> *const Tree {
    match *t {
        Tree::Leaf(v) => make_leaf(v),
        Tree::Node(l, r) => {
            let flipped_r = flip(r);
            let flipped_l = flip(l);
            make_node(flipped_r, flipped_l)
        }
    }
}

unsafe fn sum_tree(t: *const Tree) -> i64 {
    match *t {
        Tree::Leaf(v) => v,
        Tree::Node(l, r) => sum_tree(l) + sum_tree(r),
    }
}

fn main() {
    unsafe {
        let t0 = std::time::Instant::now();
        let mut sum: i64 = 0;
        for _ in 0..100 {
            POOL_IDX = 0;
            let t = make_tree(8, 1);
            let res = flip(flip(t));
            sum += sum_tree(res);
        }
        let elapsed = t0.elapsed();
        println!("{}", sum);
        println!("COMPUTE_NS: {}", elapsed.as_nanos());
        std::process::exit((sum % 256) as i32);
    }
}

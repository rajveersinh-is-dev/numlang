static mut POOL: [Node; 100000] = [Node { head: 0, tail: std::ptr::null() }; 100000];
static mut POOL_IDX: usize = 0;

#[derive(Clone, Copy)]
struct Node {
    head: i64,
    tail: *const Node,
}

unsafe fn cons(h: i64, t: *const Node) -> *const Node {
    let idx = POOL_IDX;
    POOL_IDX += 1;
    let ptr = &raw mut POOL[idx];
    (*ptr).head = h;
    (*ptr).tail = t;
    ptr
}

unsafe fn append(xs: *const Node, ys: *const Node) -> *const Node {
    if xs.is_null() {
        ys
    } else {
        cons((*xs).head, append((*xs).tail, ys))
    }
}

unsafe fn nrev(xs: *const Node) -> *const Node {
    if xs.is_null() {
        std::ptr::null()
    } else {
        append(nrev((*xs).tail), cons((*xs).head, std::ptr::null()))
    }
}

unsafe fn sum_list(mut xs: *const Node) -> i64 {
    let mut s = 0;
    while !xs.is_null() {
        s += (*xs).head;
        xs = (*xs).tail;
    }
    s
}

unsafe fn make_list(n: i64) -> *const Node {
    if n <= 0 {
        std::ptr::null()
    } else {
        cons(n, make_list(n - 1))
    }
}

fn main() {
    unsafe {
        let t0 = std::time::Instant::now();
        let mut sum: i64 = 0;
        for _ in 0..100 {
            POOL_IDX = 0;
            let xs = make_list(15);
            let rev = nrev(nrev(xs));
            sum += sum_list(rev);
        }
        let elapsed = t0.elapsed();
        println!("{}", sum);
        println!("COMPUTE_NS: {}", elapsed.as_nanos());
        std::process::exit((sum % 256) as i32);
    }
}

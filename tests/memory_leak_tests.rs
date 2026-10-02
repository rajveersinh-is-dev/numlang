use std::fs;
use std::process::Command;
use numlang::runtime::arena::*;

#[test]
fn test_scoped_arena_runtime_lifecycle() {
    let arena_ptr = __nl_arena_create(4096);
    assert!(!arena_ptr.is_null());

    unsafe {
        let mut ptrs = Vec::new();
        for _ in 0..10 {
            let p = __nl_arena_alloc(arena_ptr, 64);
            assert!(!p.is_null());
            assert_eq!(p as usize % 8, 0, "Allocated pointer must be 8-byte aligned");
            ptrs.push(p);
        }

        // Verify pointers are distinct and monotonic
        for i in 0..9 {
            assert!(ptrs[i] < ptrs[i + 1]);
        }

        assert_eq!((*arena_ptr).total_allocated, 640);

        // Reset arena
        __nl_arena_reset(arena_ptr);
        assert_eq!((*arena_ptr).total_allocated, 0);

        // First allocation after reset must reuse the head chunk's start pointer
        let p_reused = __nl_arena_alloc(arena_ptr, 64);
        assert_eq!(p_reused, ptrs[0], "Reset arena must reuse chunk memory in O(1)");

        __nl_arena_destroy(arena_ptr);
    }
}

#[test]
fn test_scoped_arena_growth_and_reclamation() {
    let arena_ptr = __nl_arena_create(1024);
    assert!(!arena_ptr.is_null());

    unsafe {
        // Allocate a block larger than the chunk size to trigger chunk growth
        let p1 = __nl_arena_alloc(arena_ptr, 5000);
        assert!(!p1.is_null());
        assert_eq!(p1 as usize % 8, 0);

        // Write and read pattern
        let slice = std::slice::from_raw_parts_mut(p1, 5000);
        for (i, byte) in slice.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        for (i, byte) in slice.iter().enumerate() {
            assert_eq!(*byte, (i % 256) as u8);
        }

        assert!((*arena_ptr).total_allocated >= 5000);

        __nl_arena_destroy(arena_ptr);
    }
}

#[test]
fn test_default_arena_loop_reset_runtime() {
    __nl_loop_reset();
    assert_eq!(__nl_arena_get_allocated_bytes(), 0);

    let p1 = __nl_arena_alloc_default(1024);
    assert!(!p1.is_null());
    assert_eq!(__nl_arena_get_allocated_bytes(), 1024);
    assert!(__nl_arena_get_peak_bytes() >= 1024);

    __nl_loop_reset();
    assert_eq!(__nl_arena_get_allocated_bytes(), 0);

    let p2 = __nl_arena_alloc_default(512);
    assert_eq!(p1, p2, "Default loop arena reuses head pointer on reset");
    assert_eq!(__nl_arena_get_allocated_bytes(), 512);
}

#[test]
fn test_nrev_50k_iterations_zero_leak() {
    let test_dir = std::env::temp_dir().join("numlang_test_nrev_leak");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("nrev_50k.nl");
    let exe_file = test_dir.join("nrev_50k.exe");

    let src = r#"
enum List {
    Nil,
    Cons(i64, Box<List>),
}

fn append(xs: List, ys: List) -> List {
    return match xs {
        Nil => ys,
        Cons(h, t) => Cons(h, box(append(deref(t), ys))),
    };
}

fn nrev(xs: List) -> List {
    return match xs {
        Nil => Nil,
        Cons(h, t) => append(nrev(deref(t)), Cons(h, box(Nil))),
    };
}

fn sum_list(xs: List) -> i64 {
    return match xs {
        Nil => 0,
        Cons(h, t) => h + sum_list(deref(t)),
    };
}

fn make_list(n: i64) -> List {
    if n <= 0 {
        return Nil;
    } else {
        return Cons(n, box(make_list(n - 1)));
    }
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..50000 {
        let xs: List = make_list(10);
        let rev: List = nrev(xs);
        sum = sum + sum_list(rev);
    }
    println(sum);
    return 0;
}
"#;
    fs::write(&src_file, src).unwrap();

    let compile_output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("-o")
        .arg(&exe_file)
        .arg(&src_file)
        .output()
        .expect("Failed to compile nrev_50k.nl");

    assert!(
        compile_output.status.success(),
        "Compilation failed:\nSTDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to execute nrev_50k.exe");

    assert_eq!(run_output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&run_output.stdout).trim().to_string();
    // 50,000 iterations of sum_list(nrev(make_list(10))) = 50,000 * 55 = 2,750,000
    assert_eq!(stdout, "2750000");
}

#[test]
fn test_tree_flip_50k_iterations_zero_leak() {
    let test_dir = std::env::temp_dir().join("numlang_test_tree_flip_leak");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("tree_flip_50k.nl");
    let exe_file = test_dir.join("tree_flip_50k.exe");

    let src = r#"
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
    for i in 0..50000 {
        let t: Tree = make_tree(4, 1);
        let flipped_twice: Tree = flip(flip(t));
        sum = sum + sum_tree(flipped_twice);
    }
    println(sum);
    return 0;
}
"#;
    fs::write(&src_file, src).unwrap();

    let compile_output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("-o")
        .arg(&exe_file)
        .arg(&src_file)
        .output()
        .expect("Failed to compile tree_flip_50k.nl");

    assert!(
        compile_output.status.success(),
        "Compilation failed:\nSTDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to execute tree_flip_50k.exe");

    assert_eq!(run_output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&run_output.stdout).trim().to_string();
    // 50,000 iterations of tree_flip(4, 1) = 50,000 * 376 = 18,800,000
    assert_eq!(stdout, "18800000");
}

#[test]
fn test_loop_escaping_allocation_preserved() {
    let test_dir = std::env::temp_dir().join("numlang_test_escaping_leak");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("escaping.nl");
    let exe_file = test_dir.join("escaping.exe");

    // Here the tree accumulator escapes across iterations into `acc`.
    // Escape analysis must ensure __nl_loop_reset is NOT emitted, preserving heap nodes.
    let src = r#"
enum Tree {
    Leaf(i64),
    Node(Box<Tree>, Box<Tree>),
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
    let mut acc: Tree = Leaf(0);
    for i in 0..5 {
        acc = make_tree(3, i + 1);
    }
    let total: i64 = sum_tree(acc);
    println(total);
    return 0;
}
"#;
    fs::write(&src_file, src).unwrap();

    let compile_output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("-o")
        .arg(&exe_file)
        .arg(&src_file)
        .output()
        .expect("Failed to compile escaping.nl");

    assert!(
        compile_output.status.success(),
        "Compilation failed:\nSTDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to execute escaping.exe");

    assert_eq!(run_output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&run_output.stdout).trim().to_string();
    // Sum of leaves for make_tree(3, 5) is 348
    assert_eq!(stdout, "348");
}

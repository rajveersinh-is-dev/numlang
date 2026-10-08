use std::time::Instant;

use numlang::ast::BinaryOp;
use numlang::mir::supercompiler::state::{MemoryVersionId, SymbolicState};
use numlang::mir::supercompiler::term::{SymTerm, SymTermId, TermInterner};
use numlang::mir::supercompiler::whistle::{is_embedded, is_instance_of, state_embeds};
use numlang::mir::{BasicBlockId, Place};
use numlang::typecheck::types::Type;

fn make_var(name: &str) -> Place {
    Place {
        local: name.to_string(),
        projections: Vec::new(),
    }
}

/// Naive unindexed homeomorphic embedding baseline without O(1) canonical ID, size, or depth pre-filters.
fn unindexed_is_embedded(t1: SymTermId, t2: SymTermId, interner: &TermInterner) -> bool {
    let term1 = interner.get(t1);
    let term2 = interner.get(t2);

    // Unindexed diving: recursively scans all children without size/depth lower-bound checks
    let embedded_in_child = match term2 {
        SymTerm::Binary(_, l2, r2, _) => {
            unindexed_is_embedded(t1, *l2, interner) || unindexed_is_embedded(t1, *r2, interner)
        }
        SymTerm::Unary(_, inner2, _) => unindexed_is_embedded(t1, *inner2, interner),
        SymTerm::Constructor(_, _, fields2, _) => fields2
            .iter()
            .any(|&f| unindexed_is_embedded(t1, f, interner)),
        SymTerm::Call(_, args2, _) => args2
            .iter()
            .any(|&a| unindexed_is_embedded(t1, a, interner)),
        SymTerm::Select(c2, th2, el2, _) => {
            unindexed_is_embedded(t1, *c2, interner)
                || unindexed_is_embedded(t1, *th2, interner)
                || unindexed_is_embedded(t1, *el2, interner)
        }
        SymTerm::Phi(incoming2, _) => incoming2
            .iter()
            .any(|(_, t)| unindexed_is_embedded(t1, *t, interner)),
        SymTerm::Ref(inner2, _) | SymTerm::Deref(inner2, _) | SymTerm::Discriminant(inner2, _) => {
            unindexed_is_embedded(t1, *inner2, interner)
        }
        SymTerm::ClosureVal(_, captured2, _) | SymTerm::Thunk(_, captured2, _) => captured2
            .iter()
            .any(|&c| unindexed_is_embedded(t1, c, interner)),
        _ => false,
    };
    if embedded_in_child {
        return true;
    }

    // Unindexed coupling: full recursive structural check
    match (term1, term2) {
        (SymTerm::ConstInt(c1, _), SymTerm::ConstInt(c2, _)) => c1 == c2,
        (SymTerm::ConstFloat(b1, _), SymTerm::ConstFloat(b2, _)) => b1 == b2,
        (SymTerm::ConstBool(b1), SymTerm::ConstBool(b2)) => b1 == b2,
        (SymTerm::ConstStr(s1), SymTerm::ConstStr(s2)) => s1 == s2,
        (SymTerm::Var(p1, _), SymTerm::Var(p2, _)) => p1 == p2,
        (SymTerm::Ref(i1, _), SymTerm::Ref(i2, _)) => unindexed_is_embedded(*i1, *i2, interner),
        (SymTerm::Deref(p1, _), SymTerm::Deref(p2, _)) => unindexed_is_embedded(*p1, *p2, interner),
        (SymTerm::Discriminant(i1, _), SymTerm::Discriminant(i2, _)) => {
            unindexed_is_embedded(*i1, *i2, interner)
        }
        (SymTerm::Binary(op1, l1, r1, _), SymTerm::Binary(op2, l2, r2, _)) => {
            op1 == op2
                && unindexed_is_embedded(*l1, *l2, interner)
                && unindexed_is_embedded(*r1, *r2, interner)
        }
        (SymTerm::Unary(op1, in1, _), SymTerm::Unary(op2, in2, _)) => {
            op1 == op2 && unindexed_is_embedded(*in1, *in2, interner)
        }
        (SymTerm::Constructor(n1, t1, f1, _), SymTerm::Constructor(n2, t2, f2, _)) => {
            n1 == n2
                && t1 == t2
                && f1.len() == f2.len()
                && f1
                    .iter()
                    .zip(f2.iter())
                    .all(|(&a, &b)| unindexed_is_embedded(a, b, interner))
        }
        (SymTerm::Call(c1, a1, _), SymTerm::Call(c2, a2, _)) => {
            c1 == c2
                && a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|(&a, &b)| unindexed_is_embedded(a, b, interner))
        }
        (SymTerm::Select(c1, th1, el1, _), SymTerm::Select(c2, th2, el2, _)) => {
            unindexed_is_embedded(*c1, *c2, interner)
                && unindexed_is_embedded(*th1, *th2, interner)
                && unindexed_is_embedded(*el1, *el2, interner)
        }
        _ => false,
    }
}

/// Baseline unindexed state embedding check.
fn unindexed_state_embeds(
    anc: &SymbolicState,
    curr: &SymbolicState,
    active_places: &[Place],
    interner: &TermInterner,
) -> bool {
    if anc.block != curr.block {
        return false;
    }

    let mut any_embedded = false;
    for place in active_places {
        if let (Some(t_anc), Some(t_curr)) = (anc.get_value(place), curr.get_value(place)) {
            if let (SymTerm::ConstInt(_, _), SymTerm::ConstInt(_, _)) =
                (interner.get(t_anc), interner.get(t_curr))
            {
                continue;
            }
            if unindexed_is_embedded(t_anc, t_curr, interner) {
                any_embedded = true;
            } else {
                return false;
            }
        }
    }
    any_embedded
}

#[test]
fn test_canonical_hash_consing_identity() {
    let mut interner = TermInterner::new();

    // 1. Leaf terms
    let v_x1 = interner.intern_var(make_var("x"), Type::I64);
    let v_x2 = interner.intern_var(make_var("x"), Type::I64);
    assert_eq!(
        v_x1, v_x2,
        "HASHCONS-01: Identical variables must have identical SymTermId"
    );
    assert!(interner.structural_eq(v_x1, v_x2));

    let c_10a = interner.intern_int(10);
    let c_10b = interner.intern_int(10);
    assert_eq!(
        c_10a, c_10b,
        "HASHCONS-01: Identical constants must have identical SymTermId"
    );

    // 2. Binary terms with commutative canonicalization
    let x_plus_10 = interner.intern_binary(BinaryOp::Add, v_x1, c_10a, Type::I64);
    let ten_plus_x = interner.intern_binary(BinaryOp::Add, c_10b, v_x2, Type::I64);
    assert_eq!(
        x_plus_10, ten_plus_x,
        "HASHCONS-01: Commutative reordering must produce identical SymTermId"
    );

    // 3. Deep algebraic expressions
    // ((x + 2) + 3) vs (x + 5)
    let c_2 = interner.intern_int(2);
    let c_3 = interner.intern_int(3);
    let x_plus_2 = interner.intern_binary(BinaryOp::Add, v_x1, c_2, Type::I64);
    let x_plus_2_plus_3 = interner.intern_binary(BinaryOp::Add, x_plus_2, c_3, Type::I64);

    let c_5 = interner.intern_int(5);
    let x_plus_5 = interner.intern_binary(BinaryOp::Add, v_x2, c_5, Type::I64);
    assert_eq!(
        x_plus_2_plus_3, x_plus_5,
        "HASHCONS-01: Constant reassociation must produce identical SymTermId"
    );
}

#[test]
fn test_dag_depth_and_size_caches() {
    let mut interner = TermInterner::new();

    // Leaf nodes
    let vx = interner.intern_var(make_var("x"), Type::I64);
    assert_eq!(interner.depth(vx), 1, "HASHCONS-02: Leaf depth must be 1");
    assert_eq!(interner.size(vx), 1, "HASHCONS-02: Leaf size must be 1");

    let c1 = interner.intern_int(42);
    assert_eq!(interner.depth(c1), 1);
    assert_eq!(interner.size(c1), 1);

    // Binary node
    let add1 = interner.intern_binary(BinaryOp::Add, vx, c1, Type::I64);
    assert_eq!(interner.depth(add1), 2, "HASHCONS-02: 1 + max(1, 1) = 2");
    assert_eq!(interner.size(add1), 3, "HASHCONS-02: 1 + 1 + 1 = 3");

    // Construct a deep tree with distinct variables
    let mut current_id = vx;
    for i in 1..5 {
        let next_v = interner.intern_var(make_var(&format!("v_{}", i)), Type::I64);
        current_id = interner.intern_binary(BinaryOp::Mul, current_id, next_v, Type::I64);
        assert_eq!(interner.depth(current_id), i as usize + 1);
        assert_eq!(interner.size(current_id), 1 + (2 * i as usize));
    }
}

#[test]
fn test_structural_fx_hash() {
    let mut interner = TermInterner::new();

    let vx = interner.intern_var(make_var("x"), Type::I64);
    let vy = interner.intern_var(make_var("y"), Type::I64);
    let c1 = interner.intern_int(1);

    let t1 = interner.intern_binary(BinaryOp::Add, vx, c1, Type::I64);
    let t2 = interner.intern_binary(BinaryOp::Add, vy, c1, Type::I64);

    assert_eq!(
        interner.hash(t1),
        interner.hash(t1),
        "HASHCONS-03: Structural hash must be deterministic"
    );
    assert_ne!(
        interner.hash(t1),
        interner.hash(t2),
        "HASHCONS-03: Distinct structures must have distinct hashes"
    );
    assert_ne!(interner.hash(t1), 0, "HASHCONS-03: Hash must be non-zero");
}

#[test]
fn test_whistle_size_depth_filters() {
    let mut interner = TermInterner::new();

    let vx = interner.intern_var(make_var("x"), Type::I64);
    let c1 = interner.intern_int(1);
    let c2 = interner.intern_int(2);

    let shallow = interner.intern_binary(BinaryOp::Add, vx, c1, Type::I64); // size 3, depth 2
    let deep = interner.intern_binary(BinaryOp::Mul, shallow, c2, Type::I64); // size 5, depth 3

    // Shallow embeds in Deep (via diving into left child shallow)
    assert!(
        is_embedded(shallow, deep, &interner),
        "HASHCONS-04: shallow ⊴ deep must be true"
    );

    // Deep CANNOT embed in Shallow: size filter (5 > 3) and depth filter (3 > 2) prune in O(1)
    assert!(
        !is_embedded(deep, shallow, &interner),
        "HASHCONS-04: deep ⊴ shallow must be false"
    );

    // Variable embeds in expression containing variable
    assert!(is_embedded(vx, deep, &interner));
    assert!(!is_embedded(deep, vx, &interner));
}

#[test]
fn test_whistle_50_variables_state_embedding() {
    let mut interner = TermInterner::new();

    let mut anc_state = SymbolicState::new(BasicBlockId(1), MemoryVersionId(0));
    let mut curr_state = SymbolicState::new(BasicBlockId(1), MemoryVersionId(0));
    let mut places = Vec::new();

    // Create 60 active symbolic places
    for i in 0..60 {
        let p = make_var(&format!("var_{}", i));
        places.push(p.clone());

        let t_leaf = interner.intern_var(p.clone(), Type::I64);
        anc_state.set_value(p.clone(), t_leaf);

        // In curr_state, build an expression tree embedding t_leaf
        let c = interner.intern_int(i as i64 + 1);
        let t_grown = interner.intern_binary(BinaryOp::Add, t_leaf, c, Type::I64);
        curr_state.set_value(p, t_grown);
    }

    assert!(
        state_embeds(&anc_state, &curr_state, &places, &interner),
        "HASHCONS-04: anc must embed into curr across all 60 variables"
    );

    // If any descendant variable is smaller or shallower than ancestor, embedding must immediately fail
    let mut swapped_state = curr_state.clone();
    // Revert var_30 in descendant to smaller leaf while ancestor has grown
    let p30 = make_var("var_30");
    let c0 = interner.intern_int(0);
    swapped_state.set_value(p30.clone(), c0);

    assert!(
        !state_embeds(&curr_state, &swapped_state, &places, &interner),
        "HASHCONS-04: Descendant with smaller place cannot embed ancestor"
    );

    // Exact instance check:
    assert!(is_instance_of(&anc_state, &anc_state, &places));
    assert!(!is_instance_of(&anc_state, &curr_state, &places));
}

#[test]
fn test_whistle_throughput_speedup_benchmark() {
    let mut interner = TermInterner::new();

    // Construct 64 symbolic places with deep binary expression trees (depth 8)
    let mut places = Vec::new();
    let mut anc_embed = SymbolicState::new(BasicBlockId(2), MemoryVersionId(0));
    let mut curr_embed = SymbolicState::new(BasicBlockId(2), MemoryVersionId(0));

    let mut anc_non_embed = SymbolicState::new(BasicBlockId(2), MemoryVersionId(0));
    let mut curr_non_embed = SymbolicState::new(BasicBlockId(2), MemoryVersionId(0));

    for i in 0..64 {
        let p = make_var(&format!("bench_var_{}", i));
        places.push(p.clone());

        // Build deep full binary trees (depth 7, size 127)
        let mut t1 = interner.intern_var(make_var(&format!("leaf_a_{}", i)), Type::I64);
        let mut t2 = interner.intern_var(make_var(&format!("leaf_b_{}", i)), Type::I64);
        for d in 0..6 {
            let ca = interner.intern_int((i * 10 + d + 2) as i64);
            let cb = interner.intern_int((i * 10 + d + 3) as i64);
            let left = interner.intern_binary(BinaryOp::Mul, t1, ca, Type::I64);
            let right = interner.intern_binary(BinaryOp::Add, t2, cb, Type::I64);
            t1 = interner.intern_binary(BinaryOp::Add, left, right, Type::I64);
            t2 = interner.intern_binary(BinaryOp::Mul, right, left, Type::I64);
        }

        // 1. Shared / invariant places across states (canonical hash-consing O(1) identity)
        if i < 40 {
            anc_embed.set_value(p.clone(), t1);
            curr_embed.set_value(p.clone(), t1);

            anc_non_embed.set_value(p.clone(), t1);
            curr_non_embed.set_value(p.clone(), t1);
        } else {
            // Growing places: t1 embeds in grown
            let c_top = interner.intern_int(888);
            let grown = interner.intern_binary(BinaryOp::Add, t1, c_top, Type::I64);
            anc_embed.set_value(p.clone(), t1);
            curr_embed.set_value(p.clone(), grown);

            // Non-embedding pairs: ancestor is strictly larger than candidate descendant
            // (tests O(1) size/depth rejection vs exhaustive recursive child traversal)
            anc_non_embed.set_value(p.clone(), grown);
            curr_non_embed.set_value(p.clone(), t1);
        }
    }

    // Warmup: at least 5 discarded iterations
    for _ in 0..10 {
        let _ = state_embeds(&anc_embed, &curr_embed, &places, &interner);
        let _ = state_embeds(&anc_non_embed, &curr_non_embed, &places, &interner);
        let _ = unindexed_state_embeds(&anc_embed, &curr_embed, &places, &interner);
        let _ = unindexed_state_embeds(&anc_non_embed, &curr_non_embed, &places, &interner);
    }

    // Measurement: at least 30 iterations measured with high-resolution performance counters
    const ROUNDS: usize = 50;

    let start_unindexed = Instant::now();
    for _ in 0..ROUNDS {
        let r1 = unindexed_state_embeds(&anc_embed, &curr_embed, &places, &interner);
        let r2 = unindexed_state_embeds(&anc_non_embed, &curr_non_embed, &places, &interner);
        assert!(r1);
        assert!(!r2);
    }
    let elapsed_unindexed = start_unindexed.elapsed();

    let start_optimized = Instant::now();
    for _ in 0..ROUNDS {
        let r1 = state_embeds(&anc_embed, &curr_embed, &places, &interner);
        let r2 = state_embeds(&anc_non_embed, &curr_non_embed, &places, &interner);
        assert!(r1);
        assert!(!r2);
    }
    let elapsed_optimized = start_optimized.elapsed();

    let unindexed_micros = elapsed_unindexed.as_micros().max(1);
    let optimized_micros = elapsed_optimized.as_micros().max(1);
    let speedup = unindexed_micros as f64 / optimized_micros as f64;

    eprintln!("\n=================== HASHCONS-05 BENCHMARK RESULTS ===================");
    eprintln!("Workload: 64 symbolic variables with deep expression trees (depth 7)");
    eprintln!("Workload mix: invariant places + growing places + rejection queries");
    eprintln!("Rounds:   {} measured rounds (after 10 warmups)", ROUNDS);
    eprintln!("Unindexed baseline time: {:>8} µs", unindexed_micros);
    eprintln!("Optimized whistle time:  {:>8} µs", optimized_micros);
    eprintln!("Throughput Speedup:      {:>8.2}x", speedup);
    eprintln!("====================================================================\n");

    assert!(
        speedup >= 5.0,
        "HASHCONS-05: Expected >= 5.0x speedup from O(1) hash-consing and depth/size caches, got {:.2}x",
        speedup
    );
}

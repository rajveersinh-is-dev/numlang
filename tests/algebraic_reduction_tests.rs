use numlang::ast::{BinaryOp, UnaryOp};
use numlang::mir::supercompiler::term::TermInterner;
use numlang::mir::Place;
use numlang::typecheck::types::Type;

fn var_place(name: &str) -> Place {
    Place {
        local: name.to_string(),
        projections: Vec::new(),
    }
}

#[test]
fn test_additive_identities() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let zero = interner.intern_int(0);

    // x + 0 == x
    let add_r = interner.intern_binary(BinaryOp::Add, x, zero, Type::I64);
    assert_eq!(add_r, x, "x + 0 must reduce to x");

    // 0 + x == x
    let add_l = interner.intern_binary(BinaryOp::Add, zero, x, Type::I64);
    assert_eq!(add_l, x, "0 + x must reduce to x");

    // Float: x + 0.0 == x, 0.0 + x == x
    let x_f = interner.intern_var(var_place("xf"), Type::F64);
    let zero_f = interner.intern_float(0.0);
    let add_rf = interner.intern_binary(BinaryOp::Add, x_f, zero_f, Type::F64);
    assert_eq!(add_rf, x_f, "xf + 0.0 must reduce to xf");
    let add_lf = interner.intern_binary(BinaryOp::Add, zero_f, x_f, Type::F64);
    assert_eq!(add_lf, x_f, "0.0 + xf must reduce to xf");
}

#[test]
fn test_subtractive_identities() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let zero = interner.intern_int(0);

    // x - 0 == x
    let sub_zero = interner.intern_binary(BinaryOp::Sub, x, zero, Type::I64);
    assert_eq!(sub_zero, x, "x - 0 must reduce to x");

    // x - x == 0
    let sub_self = interner.intern_binary(BinaryOp::Sub, x, x, Type::I64);
    assert_eq!(sub_self, zero, "x - x must reduce to 0");

    // 0 - x == -x
    let sub_from_zero = interner.intern_binary(BinaryOp::Sub, zero, x, Type::I64);
    let neg_x = interner.intern_unary(UnaryOp::Neg, x, Type::I64);
    assert_eq!(sub_from_zero, neg_x, "0 - x must reduce to -x");

    // Float: xf - xf == 0.0
    let x_f = interner.intern_var(var_place("xf"), Type::F64);
    let zero_f = interner.intern_float(0.0);
    let sub_self_f = interner.intern_binary(BinaryOp::Sub, x_f, x_f, Type::F64);
    assert_eq!(sub_self_f, zero_f, "xf - xf must reduce to 0.0");
}

#[test]
fn test_multiplicative_identities() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let zero = interner.intern_int(0);
    let one = interner.intern_int(1);
    let neg_one = interner.intern_int(-1);

    // x * 1 == x, 1 * x == x
    assert_eq!(interner.intern_binary(BinaryOp::Mul, x, one, Type::I64), x);
    assert_eq!(interner.intern_binary(BinaryOp::Mul, one, x, Type::I64), x);

    // x * 0 == 0, 0 * x == 0
    assert_eq!(
        interner.intern_binary(BinaryOp::Mul, x, zero, Type::I64),
        zero
    );
    assert_eq!(
        interner.intern_binary(BinaryOp::Mul, zero, x, Type::I64),
        zero
    );

    // x * -1 == -x, -1 * x == -x
    let neg_x = interner.intern_unary(UnaryOp::Neg, x, Type::I64);
    assert_eq!(
        interner.intern_binary(BinaryOp::Mul, x, neg_one, Type::I64),
        neg_x
    );
    assert_eq!(
        interner.intern_binary(BinaryOp::Mul, neg_one, x, Type::I64),
        neg_x
    );

    // Float: xf * 1.0 == xf, xf * 0.0 == 0.0
    let x_f = interner.intern_var(var_place("xf"), Type::F64);
    let one_f = interner.intern_float(1.0);
    let zero_f = interner.intern_float(0.0);
    assert_eq!(
        interner.intern_binary(BinaryOp::Mul, x_f, one_f, Type::F64),
        x_f
    );
    assert_eq!(
        interner.intern_binary(BinaryOp::Mul, x_f, zero_f, Type::F64),
        zero_f
    );
}

#[test]
fn test_division_and_modulo_identities() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let zero = interner.intern_int(0);
    let one = interner.intern_int(1);
    let neg_one = interner.intern_int(-1);

    // x / 1 == x
    assert_eq!(interner.intern_binary(BinaryOp::Div, x, one, Type::I64), x);

    // x / -1 == -x
    let neg_x = interner.intern_unary(UnaryOp::Neg, x, Type::I64);
    assert_eq!(
        interner.intern_binary(BinaryOp::Div, x, neg_one, Type::I64),
        neg_x
    );

    // x / x == 1
    assert_eq!(interner.intern_binary(BinaryOp::Div, x, x, Type::I64), one);

    // 0 / x == 0
    assert_eq!(
        interner.intern_binary(BinaryOp::Div, zero, x, Type::I64),
        zero
    );

    // x % 1 == 0
    assert_eq!(
        interner.intern_binary(BinaryOp::Mod, x, one, Type::I64),
        zero
    );

    // x % x == 0
    assert_eq!(interner.intern_binary(BinaryOp::Mod, x, x, Type::I64), zero);

    // 0 % x == 0
    assert_eq!(
        interner.intern_binary(BinaryOp::Mod, zero, x, Type::I64),
        zero
    );

    // Float: xf / 1.0 == xf, xf / xf == 1.0
    let x_f = interner.intern_var(var_place("xf"), Type::F64);
    let one_f = interner.intern_float(1.0);
    assert_eq!(
        interner.intern_binary(BinaryOp::Div, x_f, one_f, Type::F64),
        x_f
    );
    assert_eq!(
        interner.intern_binary(BinaryOp::Div, x_f, x_f, Type::F64),
        one_f
    );
}

#[test]
fn test_bitwise_and_shift_identities() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let zero = interner.intern_int(0);
    let all_ones = interner.intern_int(-1);

    // x & x == x
    assert_eq!(interner.intern_binary(BinaryOp::BitAnd, x, x, Type::I64), x);
    // x & 0 == 0
    assert_eq!(
        interner.intern_binary(BinaryOp::BitAnd, x, zero, Type::I64),
        zero
    );
    // x & -1 == x
    assert_eq!(
        interner.intern_binary(BinaryOp::BitAnd, x, all_ones, Type::I64),
        x
    );

    // x | x == x
    assert_eq!(interner.intern_binary(BinaryOp::BitOr, x, x, Type::I64), x);
    // x | 0 == x
    assert_eq!(
        interner.intern_binary(BinaryOp::BitOr, x, zero, Type::I64),
        x
    );
    // x | -1 == -1
    assert_eq!(
        interner.intern_binary(BinaryOp::BitOr, x, all_ones, Type::I64),
        all_ones
    );

    // x ^ x == 0
    assert_eq!(
        interner.intern_binary(BinaryOp::BitXor, x, x, Type::I64),
        zero
    );
    // x ^ 0 == x
    assert_eq!(
        interner.intern_binary(BinaryOp::BitXor, x, zero, Type::I64),
        x
    );
    // x ^ -1 == ~x
    let not_x = interner.intern_unary(UnaryOp::Not, x, Type::I64);
    assert_eq!(
        interner.intern_binary(BinaryOp::BitXor, x, all_ones, Type::I64),
        not_x
    );

    // Shifts
    assert_eq!(interner.intern_binary(BinaryOp::Shl, x, zero, Type::I64), x);
    assert_eq!(interner.intern_binary(BinaryOp::Shr, x, zero, Type::I64), x);
    assert_eq!(
        interner.intern_binary(BinaryOp::Shl, zero, x, Type::I64),
        zero
    );
    assert_eq!(
        interner.intern_binary(BinaryOp::Shr, zero, x, Type::I64),
        zero
    );
}

#[test]
fn test_boolean_identities() {
    let mut interner = TermInterner::new();
    let b = interner.intern_var(var_place("b"), Type::Bool);
    let t = interner.intern_bool(true);
    let f = interner.intern_bool(false);

    // b && true == b, true && b == b
    assert_eq!(
        interner.intern_binary(BinaryOp::BitAnd, b, t, Type::Bool),
        b
    );
    assert_eq!(
        interner.intern_binary(BinaryOp::BitAnd, t, b, Type::Bool),
        b
    );

    // b && false == false, false && b == false
    assert_eq!(
        interner.intern_binary(BinaryOp::BitAnd, b, f, Type::Bool),
        f
    );
    assert_eq!(
        interner.intern_binary(BinaryOp::BitAnd, f, b, Type::Bool),
        f
    );

    // b || true == true, true || b == true
    assert_eq!(interner.intern_binary(BinaryOp::BitOr, b, t, Type::Bool), t);
    assert_eq!(interner.intern_binary(BinaryOp::BitOr, t, b, Type::Bool), t);

    // b || false == b, false || b == b
    assert_eq!(interner.intern_binary(BinaryOp::BitOr, b, f, Type::Bool), b);
    assert_eq!(interner.intern_binary(BinaryOp::BitOr, f, b, Type::Bool), b);

    // b == true => b, b == false => !b
    let not_b = interner.intern_unary(UnaryOp::Not, b, Type::Bool);
    assert_eq!(interner.intern_binary(BinaryOp::Eq, b, t, Type::Bool), b);
    assert_eq!(
        interner.intern_binary(BinaryOp::Eq, b, f, Type::Bool),
        not_b
    );

    // b != false => b, b != true => !b
    assert_eq!(interner.intern_binary(BinaryOp::Ne, b, f, Type::Bool), b);
    assert_eq!(
        interner.intern_binary(BinaryOp::Ne, b, t, Type::Bool),
        not_b
    );
}

#[test]
fn test_comparison_on_identical_terms() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let t = interner.intern_bool(true);
    let f = interner.intern_bool(false);

    assert_eq!(interner.intern_binary(BinaryOp::Eq, x, x, Type::Bool), t);
    assert_eq!(interner.intern_binary(BinaryOp::Ne, x, x, Type::Bool), f);
    assert_eq!(interner.intern_binary(BinaryOp::Lt, x, x, Type::Bool), f);
    assert_eq!(interner.intern_binary(BinaryOp::Gt, x, x, Type::Bool), f);
    assert_eq!(interner.intern_binary(BinaryOp::Le, x, x, Type::Bool), t);
    assert_eq!(interner.intern_binary(BinaryOp::Ge, x, x, Type::Bool), t);
}

#[test]
fn test_unary_double_negation_and_inversion() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let b = interner.intern_var(var_place("b"), Type::Bool);

    // -(-x) == x
    let neg_x = interner.intern_unary(UnaryOp::Neg, x, Type::I64);
    let neg_neg_x = interner.intern_unary(UnaryOp::Neg, neg_x, Type::I64);
    assert_eq!(neg_neg_x, x, "-(-x) must reduce to x");

    // !(!b) == b
    let not_b = interner.intern_unary(UnaryOp::Not, b, Type::Bool);
    let not_not_b = interner.intern_unary(UnaryOp::Not, not_b, Type::Bool);
    assert_eq!(not_not_b, b, "!(!b) must reduce to b");

    // Float: -(-xf) == xf
    let x_f = interner.intern_var(var_place("xf"), Type::F64);
    let neg_xf = interner.intern_unary(UnaryOp::Neg, x_f, Type::F64);
    let neg_neg_xf = interner.intern_unary(UnaryOp::Neg, neg_xf, Type::F64);
    assert_eq!(neg_neg_xf, x_f, "-(-xf) must reduce to xf");

    // !(x < y) => x >= y
    let y = interner.intern_var(var_place("y"), Type::I64);
    let lt = interner.intern_binary(BinaryOp::Lt, x, y, Type::Bool);
    let not_lt = interner.intern_unary(UnaryOp::Not, lt, Type::Bool);
    let ge = interner.intern_binary(BinaryOp::Ge, x, y, Type::Bool);
    assert_eq!(not_lt, ge, "!(x < y) must invert to x >= y");

    // !(x == y) => x != y
    let eq = interner.intern_binary(BinaryOp::Eq, x, y, Type::Bool);
    let not_eq = interner.intern_unary(UnaryOp::Not, eq, Type::Bool);
    let ne = interner.intern_binary(BinaryOp::Ne, x, y, Type::Bool);
    assert_eq!(not_eq, ne, "!(x == y) must invert to x != y");
}

#[test]
fn test_constant_reassociation() {
    let mut interner = TermInterner::new();
    let x = interner.intern_var(var_place("x"), Type::I64);
    let c5 = interner.intern_int(5);
    let c3 = interner.intern_int(3);
    let c8 = interner.intern_int(8);
    let c2 = interner.intern_int(2);
    let c12 = interner.intern_int(12);

    // (x + 5) + 3 == x + 8
    let x_plus_5 = interner.intern_binary(BinaryOp::Add, x, c5, Type::I64);
    let reassociated_add = interner.intern_binary(BinaryOp::Add, x_plus_5, c3, Type::I64);
    let expected_add = interner.intern_binary(BinaryOp::Add, x, c8, Type::I64);
    assert_eq!(
        reassociated_add, expected_add,
        "(x + 5) + 3 must reassociate to x + 8"
    );

    // (x + 5) - 3 == x + 2
    let sub_assoc = interner.intern_binary(BinaryOp::Sub, x_plus_5, c3, Type::I64);
    let expected_sub = interner.intern_binary(BinaryOp::Add, x, c2, Type::I64);
    assert_eq!(
        sub_assoc, expected_sub,
        "(x + 5) - 3 must reassociate to x + 2"
    );

    // (x * 3) * 4 == x * 12
    let c4 = interner.intern_int(4);
    let x_mul_3 = interner.intern_binary(BinaryOp::Mul, x, c3, Type::I64);
    let reassociated_mul = interner.intern_binary(BinaryOp::Mul, x_mul_3, c4, Type::I64);
    let expected_mul = interner.intern_binary(BinaryOp::Mul, x, c12, Type::I64);
    assert_eq!(
        reassociated_mul, expected_mul,
        "(x * 3) * 4 must reassociate to x * 12"
    );
}

#[test]
fn test_canonical_commutative_ordering() {
    let mut interner = TermInterner::new();
    let a = interner.intern_var(var_place("a"), Type::I64);
    let b = interner.intern_var(var_place("b"), Type::I64);

    // a + b and b + a must yield the exact same SymTermId
    let add_ab = interner.intern_binary(BinaryOp::Add, a, b, Type::I64);
    let add_ba = interner.intern_binary(BinaryOp::Add, b, a, Type::I64);
    assert_eq!(
        add_ab, add_ba,
        "a + b and b + a must have identical SymTermId"
    );

    // a * b and b * a
    let mul_ab = interner.intern_binary(BinaryOp::Mul, a, b, Type::I64);
    let mul_ba = interner.intern_binary(BinaryOp::Mul, b, a, Type::I64);
    assert_eq!(
        mul_ab, mul_ba,
        "a * b and b * a must have identical SymTermId"
    );

    // a & b and b & a
    let and_ab = interner.intern_binary(BinaryOp::BitAnd, a, b, Type::I64);
    let and_ba = interner.intern_binary(BinaryOp::BitAnd, b, a, Type::I64);
    assert_eq!(
        and_ab, and_ba,
        "a & b and b & a must have identical SymTermId"
    );

    // a | b and b | a
    let or_ab = interner.intern_binary(BinaryOp::BitOr, a, b, Type::I64);
    let or_ba = interner.intern_binary(BinaryOp::BitOr, b, a, Type::I64);
    assert_eq!(
        or_ab, or_ba,
        "a | b and b | a must have identical SymTermId"
    );

    // a ^ b and b ^ a
    let xor_ab = interner.intern_binary(BinaryOp::BitXor, a, b, Type::I64);
    let xor_ba = interner.intern_binary(BinaryOp::BitXor, b, a, Type::I64);
    assert_eq!(
        xor_ab, xor_ba,
        "a ^ b and b ^ a must have identical SymTermId"
    );

    // a == b and b == a
    let eq_ab = interner.intern_binary(BinaryOp::Eq, a, b, Type::Bool);
    let eq_ba = interner.intern_binary(BinaryOp::Eq, b, a, Type::Bool);
    assert_eq!(
        eq_ab, eq_ba,
        "a == b and b == a must have identical SymTermId"
    );
}

#[test]
fn test_math_intrinsic_constant_folding() {
    let mut interner = TermInterner::new();
    let four_f = interner.intern_float(4.0);
    let two_f = interner.intern_float(2.0);

    // sqrt(4.0) == 2.0
    let sqrt_4 = interner.intern_call("sqrt".to_string(), vec![four_f], Type::F64);
    assert_eq!(sqrt_4, two_f, "sqrt(4.0) must fold to 2.0");

    // abs(-42) == 42
    let neg_42 = interner.intern_int(-42);
    let pos_42 = interner.intern_int(42);
    let abs_res = interner.intern_call("abs".to_string(), vec![neg_42], Type::I64);
    assert_eq!(abs_res, pos_42, "abs(-42) must fold to 42");

    // isqrt(16) == 4
    let c16 = interner.intern_int(16);
    let c4 = interner.intern_int(4);
    let isqrt_res = interner.intern_call("isqrt".to_string(), vec![c16], Type::I64);
    assert_eq!(isqrt_res, c4, "isqrt(16) must fold to 4");
}

#[test]
fn test_select_canonicalization() {
    let mut interner = TermInterner::new();
    let c = interner.intern_var(var_place("c"), Type::Bool);
    let x = interner.intern_var(var_place("x"), Type::I64);
    let y = interner.intern_var(var_place("y"), Type::I64);
    let t = interner.intern_bool(true);
    let f = interner.intern_bool(false);

    // select(true, x, y) == x
    assert_eq!(interner.intern_select(t, x, y, Type::I64), x);

    // select(false, x, y) == y
    assert_eq!(interner.intern_select(f, x, y, Type::I64), y);

    // select(c, x, x) == x
    assert_eq!(interner.intern_select(c, x, x, Type::I64), x);

    // select(c, true, false) == c
    assert_eq!(interner.intern_select(c, t, f, Type::Bool), c);

    // select(c, false, true) == !c
    let not_c = interner.intern_unary(UnaryOp::Not, c, Type::Bool);
    assert_eq!(interner.intern_select(c, f, t, Type::Bool), not_c);

    // select(!c, x, y) == select(c, y, x)
    let sel_not_c = interner.intern_select(not_c, x, y, Type::I64);
    let sel_c_flipped = interner.intern_select(c, y, x, Type::I64);
    assert_eq!(
        sel_not_c, sel_c_flipped,
        "select(!c, x, y) must canonicalize to select(c, y, x)"
    );
}

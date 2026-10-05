//! Phase 64: Strength Reduction in Residual After Loop Collapse Tests.
//!
//! Validates:
//! - STRENGTH-01: Module scanning and identification
//! - STRENGTH-02: Power-of-2 multiplication reduction (`mul` -> `shl`)
//! - STRENGTH-03: Near-power-of-2 reduction (`mul` -> shift + add/sub)
//! - STRENGTH-04: Power-of-2 division reduction (`div` -> `shr`)
//! - STRENGTH-05: Strassen block recursive matrix multiplication for N >= 4

use numlang::ast::BinaryOp;
use numlang::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, Rvalue, Statement};
use numlang::mir::supercompiler::recurrence::{mat_mul_strassen, mat_pow_nxn};
use numlang::mir::supercompiler::strength_reduce::{
    classify_div_const, classify_mul_const, strength_reduce_mir_function, DivReduction,
    MulReduction,
};
use numlang::mir::{BasicBlockId, Place, Terminator};
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::types::Type;

#[test]
fn test_classify_mul_const_power_of_two() {
    assert_eq!(classify_mul_const(0), Some(MulReduction::Zero));
    assert_eq!(classify_mul_const(1), Some(MulReduction::Identity));
    assert_eq!(classify_mul_const(-1), Some(MulReduction::Negate));
    assert_eq!(classify_mul_const(2), Some(MulReduction::PowerOfTwo(1)));
    assert_eq!(classify_mul_const(4), Some(MulReduction::PowerOfTwo(2)));
    assert_eq!(classify_mul_const(8), Some(MulReduction::PowerOfTwo(3)));
    assert_eq!(classify_mul_const(16), Some(MulReduction::PowerOfTwo(4)));
    assert_eq!(classify_mul_const(32), Some(MulReduction::PowerOfTwo(5)));
    assert_eq!(classify_mul_const(64), Some(MulReduction::PowerOfTwo(6)));
    assert_eq!(classify_mul_const(1024), Some(MulReduction::PowerOfTwo(10)));
    assert_eq!(classify_mul_const(1 << 20), Some(MulReduction::PowerOfTwo(20)));

    assert_eq!(classify_mul_const(-2), Some(MulReduction::NegPowerOfTwo(1)));
    assert_eq!(classify_mul_const(-8), Some(MulReduction::NegPowerOfTwo(3)));
    assert_eq!(classify_mul_const(-64), Some(MulReduction::NegPowerOfTwo(6)));
}

#[test]
fn test_classify_mul_const_near_powers_of_two() {
    // 2^a + 2^b
    assert_eq!(classify_mul_const(3), Some(MulReduction::SumOfPowers(1, 0))); // 2 + 1
    assert_eq!(classify_mul_const(5), Some(MulReduction::SumOfPowers(2, 0))); // 4 + 1
    assert_eq!(classify_mul_const(6), Some(MulReduction::SumOfPowers(2, 1))); // 4 + 2
    assert_eq!(classify_mul_const(9), Some(MulReduction::SumOfPowers(3, 0))); // 8 + 1
    assert_eq!(classify_mul_const(10), Some(MulReduction::SumOfPowers(3, 1))); // 8 + 2
    assert_eq!(classify_mul_const(12), Some(MulReduction::SumOfPowers(3, 2))); // 8 + 4
    assert_eq!(classify_mul_const(17), Some(MulReduction::SumOfPowers(4, 0))); // 16 + 1
    assert_eq!(classify_mul_const(24), Some(MulReduction::SumOfPowers(4, 3))); // 16 + 8
    assert_eq!(classify_mul_const(33), Some(MulReduction::SumOfPowers(5, 0))); // 32 + 1

    // 2^a - 2^b
    assert_eq!(classify_mul_const(7), Some(MulReduction::DiffOfPowers(3, 0))); // 8 - 1
    assert_eq!(classify_mul_const(14), Some(MulReduction::DiffOfPowers(4, 1))); // 16 - 2
    assert_eq!(classify_mul_const(15), Some(MulReduction::DiffOfPowers(4, 0))); // 16 - 1
    assert_eq!(classify_mul_const(31), Some(MulReduction::DiffOfPowers(5, 0))); // 32 - 1
    assert_eq!(classify_mul_const(63), Some(MulReduction::DiffOfPowers(6, 0))); // 64 - 1
}

#[test]
fn test_classify_div_const_power_of_two() {
    assert_eq!(classify_div_const(1), Some(DivReduction::Identity));
    assert_eq!(classify_div_const(2), Some(DivReduction::PowerOfTwo(1)));
    assert_eq!(classify_div_const(4), Some(DivReduction::PowerOfTwo(2)));
    assert_eq!(classify_div_const(8), Some(DivReduction::PowerOfTwo(3)));
    assert_eq!(classify_div_const(16), Some(DivReduction::PowerOfTwo(4)));
    assert_eq!(classify_div_const(32), Some(DivReduction::PowerOfTwo(5)));
    assert_eq!(classify_div_const(64), Some(DivReduction::PowerOfTwo(6)));
    assert_eq!(classify_div_const(1024), Some(DivReduction::PowerOfTwo(10)));

    // Non powers of two should not be reduced to single shift
    assert_eq!(classify_div_const(3), None);
    assert_eq!(classify_div_const(5), None);
    assert_eq!(classify_div_const(7), None);
    assert_eq!(classify_div_const(-4), None);
}

#[test]
fn test_mir_power_of_two_multiplication_reduced() {
    // fn test(x: i64) -> i64 { return x * 16; }
    let mut func = MirFunction {
        name: "test_mul16".to_string(),
        params: vec![("x".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "c16".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "res".to_string(),
                ty: Type::I64,
                mutable: false,
            },
        ],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: vec![
                Statement::Assign(
                    Place {
                        local: "c16".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Constant(TypedLiteral::Int(16, Type::I64)),
                ),
                Statement::Assign(
                    Place {
                        local: "res".to_string(),
                        projections: vec![],
                    },
                    Rvalue::BinaryOp(
                        BinaryOp::Mul,
                        Place {
                            local: "x".to_string(),
                            projections: vec![],
                        },
                        Place {
                            local: "c16".to_string(),
                            projections: vec![],
                        },
                    ),
                ),
            ],
            terminator: Terminator::Return {
                value: Some(Place {
                    local: "res".to_string(),
                    projections: vec![],
                }),
            },
        }],
        is_distilled: false,
    };

    let count = strength_reduce_mir_function(&mut func);
    assert_eq!(count, 1);

    let bb = &func.blocks[0];
    let has_mul = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Mul
        } else {
            false
        }
    });
    assert!(!has_mul, "Expected mul to be eliminated");

    let has_shl = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Shl
        } else {
            false
        }
    });
    assert!(has_shl, "Expected shl to replace mul");
}

#[test]
fn test_mir_near_power_of_two_multiplication_reduced_sum() {
    // fn test(x: i64) -> i64 { return x * 9; } (9 = 2^3 + 1)
    let mut func = MirFunction {
        name: "test_mul9".to_string(),
        params: vec![("x".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "c9".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "res".to_string(),
                ty: Type::I64,
                mutable: false,
            },
        ],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: vec![
                Statement::Assign(
                    Place {
                        local: "c9".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Constant(TypedLiteral::Int(9, Type::I64)),
                ),
                Statement::Assign(
                    Place {
                        local: "res".to_string(),
                        projections: vec![],
                    },
                    Rvalue::BinaryOp(
                        BinaryOp::Mul,
                        Place {
                            local: "x".to_string(),
                            projections: vec![],
                        },
                        Place {
                            local: "c9".to_string(),
                            projections: vec![],
                        },
                    ),
                ),
            ],
            terminator: Terminator::Return {
                value: Some(Place {
                    local: "res".to_string(),
                    projections: vec![],
                }),
            },
        }],
        is_distilled: false,
    };

    let count = strength_reduce_mir_function(&mut func);
    assert_eq!(count, 1);

    let bb = &func.blocks[0];
    let has_mul = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Mul
        } else {
            false
        }
    });
    assert!(!has_mul, "Expected mul to be eliminated for x * 9");

    let has_shl = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Shl
        } else {
            false
        }
    });
    let has_add = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Add
        } else {
            false
        }
    });
    assert!(has_shl, "Expected shl in near-power-of-2 sequence");
    assert!(has_add, "Expected add in near-power-of-2 sequence");
}

#[test]
fn test_mir_near_power_of_two_multiplication_reduced_diff() {
    // fn test(x: i64) -> i64 { return x * 7; } (7 = 2^3 - 1)
    let mut func = MirFunction {
        name: "test_mul7".to_string(),
        params: vec![("x".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "c7".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "res".to_string(),
                ty: Type::I64,
                mutable: false,
            },
        ],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: vec![
                Statement::Assign(
                    Place {
                        local: "c7".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Constant(TypedLiteral::Int(7, Type::I64)),
                ),
                Statement::Assign(
                    Place {
                        local: "res".to_string(),
                        projections: vec![],
                    },
                    Rvalue::BinaryOp(
                        BinaryOp::Mul,
                        Place {
                            local: "x".to_string(),
                            projections: vec![],
                        },
                        Place {
                            local: "c7".to_string(),
                            projections: vec![],
                        },
                    ),
                ),
            ],
            terminator: Terminator::Return {
                value: Some(Place {
                    local: "res".to_string(),
                    projections: vec![],
                }),
            },
        }],
        is_distilled: false,
    };

    let count = strength_reduce_mir_function(&mut func);
    assert_eq!(count, 1);

    let bb = &func.blocks[0];
    let has_mul = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Mul
        } else {
            false
        }
    });
    assert!(!has_mul, "Expected mul to be eliminated for x * 7");

    let has_shl = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Shl
        } else {
            false
        }
    });
    let has_sub = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Sub
        } else {
            false
        }
    });
    assert!(has_shl, "Expected shl in near-power-of-2 diff sequence");
    assert!(has_sub, "Expected sub in near-power-of-2 diff sequence");
}

#[test]
fn test_mir_power_of_two_division_reduced() {
    // fn test(x: i64) -> i64 { return x / 8; }
    let mut func = MirFunction {
        name: "test_div8".to_string(),
        params: vec![("x".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "c8".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "res".to_string(),
                ty: Type::I64,
                mutable: false,
            },
        ],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: vec![
                Statement::Assign(
                    Place {
                        local: "c8".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Constant(TypedLiteral::Int(8, Type::I64)),
                ),
                Statement::Assign(
                    Place {
                        local: "res".to_string(),
                        projections: vec![],
                    },
                    Rvalue::BinaryOp(
                        BinaryOp::Div,
                        Place {
                            local: "x".to_string(),
                            projections: vec![],
                        },
                        Place {
                            local: "c8".to_string(),
                            projections: vec![],
                        },
                    ),
                ),
            ],
            terminator: Terminator::Return {
                value: Some(Place {
                    local: "res".to_string(),
                    projections: vec![],
                }),
            },
        }],
        is_distilled: false,
    };

    let count = strength_reduce_mir_function(&mut func);
    assert_eq!(count, 1);

    let bb = &func.blocks[0];
    let has_div = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Div
        } else {
            false
        }
    });
    assert!(!has_div, "Expected div to be eliminated");

    let has_shr = bb.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) = s {
            *op == BinaryOp::Shr
        } else {
            false
        }
    });
    assert!(has_shr, "Expected shr to replace div");
}

fn naive_mat_mul(a: &[Vec<i64>], b: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let n = a.len();
    let mut res = vec![vec![0i64; n]; n];
    for r in 0..n {
        for c in 0..n {
            let mut sum: i64 = 0;
            for k in 0..n {
                sum = sum.wrapping_add(a[r][k].wrapping_mul(b[k][c]));
            }
            res[r][c] = sum;
        }
    }
    res
}

#[test]
fn test_strassen_4x4_correctness() {
    let a = vec![
        vec![1, 2, 3, 4],
        vec![5, 6, 7, 8],
        vec![9, 10, 11, 12],
        vec![13, 14, 15, 16],
    ];
    let b = vec![
        vec![17, 18, 19, 20],
        vec![21, 22, 23, 24],
        vec![25, 26, 27, 28],
        vec![29, 30, 31, 32],
    ];

    let expected = naive_mat_mul(&a, &b);
    let strassen_res = mat_mul_strassen(&a, &b);

    assert_eq!(strassen_res, expected);
}

#[test]
fn test_strassen_arbitrary_dimension() {
    // 5x5 test (odd dimension padding test)
    let a5 = vec![
        vec![2, 3, 1, 4, 5],
        vec![1, 0, 2, 3, 1],
        vec![4, 2, 1, 0, 2],
        vec![3, 1, 2, 5, 4],
        vec![0, 4, 3, 1, 2],
    ];
    let b5 = vec![
        vec![1, 2, 0, 3, 1],
        vec![4, 1, 3, 2, 0],
        vec![2, 5, 1, 0, 4],
        vec![0, 1, 2, 4, 3],
        vec![3, 0, 4, 1, 5],
    ];
    let expected5 = naive_mat_mul(&a5, &b5);
    let strassen5 = mat_mul_strassen(&a5, &b5);
    assert_eq!(strassen5, expected5, "Strassen 5x5 match failed");

    // 8x8 test (power of 2 multi-level recursion test)
    let mut a8 = vec![vec![0i64; 8]; 8];
    let mut b8 = vec![vec![0i64; 8]; 8];
    for r in 0..8 {
        for c in 0..8 {
            a8[r][c] = ((r * 8 + c) as i64) * 3 + 1;
            b8[r][c] = ((r * 8 + c) as i64) * 5 - 2;
        }
    }
    let expected8 = naive_mat_mul(&a8, &b8);
    let strassen8 = mat_mul_strassen(&a8, &b8);
    assert_eq!(strassen8, expected8, "Strassen 8x8 match failed");
}

#[test]
fn test_strassen_matrix_exponentiation() {
    let m = vec![
        vec![1, 1, 0, 0],
        vec![0, 1, 1, 0],
        vec![0, 0, 1, 1],
        vec![0, 0, 0, 1],
    ];

    // Compute m^6 naively
    let mut expected = vec![
        vec![1, 0, 0, 0],
        vec![0, 1, 0, 0],
        vec![0, 0, 1, 0],
        vec![0, 0, 0, 1],
    ];
    for _ in 0..6 {
        expected = naive_mat_mul(&expected, &m);
    }

    let pow_res = mat_pow_nxn(&m, 6);
    assert_eq!(pow_res, expected, "Matrix power with Strassen failed");
}

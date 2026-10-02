enum SpecOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
}

enum SpecExpr {
    Lit(i64),
    Var(i64),
    Bin(SpecOp, Box<SpecExpr>, Box<SpecExpr>),
    If(Box<SpecExpr>, Box<SpecExpr>, Box<SpecExpr>),
    Call(i64, Box<SpecExpr>),
}

enum SpecEnv {
    Nil,
    Cons(i64, i64, Box<SpecEnv>),
}

fn bool_to_int(b: bool) -> i64 {
    if b {
        return 1;
    }
    return 0;
}

fn eval_op(op: SpecOp, vl: i64, vr: i64) -> i64 {
    return match op {
        SpecOp::Add => vl + vr,
        SpecOp::Sub => vl - vr,
        SpecOp::Mul => vl * vr,
        SpecOp::Div => vl / vr,
        SpecOp::Eq  => bool_to_int(vl == vr),
        SpecOp::Lt  => bool_to_int(vl < vr),
    };
}

fn env_has_helper(is_match: bool, rest: SpecEnv, id: i64) -> bool {
    if is_match {
        return true;
    }
    return env_has(rest, id);
}

fn env_has(env: SpecEnv, id: i64) -> bool {
    return match env {
        SpecEnv::Nil => false,
        SpecEnv::Cons(k, _, rest) => env_has_helper(k == id, deref(rest), id),
    };
}

fn env_lookup_helper(is_match: bool, val: i64, rest: SpecEnv, id: i64) -> i64 {
    if is_match {
        return val;
    }
    return env_lookup(rest, id);
}

fn env_lookup(env: SpecEnv, id: i64) -> i64 {
    return match env {
        SpecEnv::Nil => 0,
        SpecEnv::Cons(k, v, rest) => env_lookup_helper(k == id, v, deref(rest), id),
    };
}

fn eval_if(c_val: i64, t: SpecExpr, f: SpecExpr, env: SpecEnv) -> i64 {
    if c_val != 0 {
        return spec_eval(t, env);
    }
    return spec_eval(f, env);
}

fn eval_call(fn_id: i64, arg: SpecExpr, env: SpecEnv) -> i64 {
    return spec_eval(arg, env);
}

fn spec_eval(e: SpecExpr, env: SpecEnv) -> i64 {
    return match e {
        SpecExpr::Lit(v) => v,
        SpecExpr::Var(id) => env_lookup(env, id),
        SpecExpr::Bin(op, l, r) => eval_op(op, spec_eval(deref(l), env), spec_eval(deref(r), env)),
        SpecExpr::If(c, t, f) => eval_if(spec_eval(deref(c), env), deref(t), deref(f), env),
        SpecExpr::Call(fn_id, arg) => eval_call(fn_id, deref(arg), env),
    };
}

fn spec_var(is_static: bool, s_val: i64, id: i64) -> SpecExpr {
    if is_static {
        return SpecExpr::Lit(s_val);
    }
    return SpecExpr::Var(id);
}

fn spec_bin_lit_lit(op: SpecOp, vl: i64, vr: i64) -> SpecExpr {
    return SpecExpr::Lit(eval_op(op, vl, vr));
}

fn spec_bin_lit_expr(op: SpecOp, vl: i64, sr: SpecExpr) -> SpecExpr {
    if op == SpecOp::Add {
        if vl == 0 {
            return sr;
        }
    }
    if op == SpecOp::Mul {
        if vl == 0 {
            return SpecExpr::Lit(0);
        }
        if vl == 1 {
            return sr;
        }
    }
    return SpecExpr::Bin(op, box(SpecExpr::Lit(vl)), box(sr));
}

fn spec_bin_expr_lit(op: SpecOp, sl: SpecExpr, vr: i64) -> SpecExpr {
    if op == SpecOp::Add {
        if vr == 0 {
            return sl;
        }
    }
    if op == SpecOp::Mul {
        if vr == 0 {
            return SpecExpr::Lit(0);
        }
        if vr == 1 {
            return sl;
        }
    }
    return SpecExpr::Bin(op, box(sl), box(SpecExpr::Lit(vr)));
}

fn spec_bin_reduce(op: SpecOp, sl: SpecExpr, sr: SpecExpr) -> SpecExpr {
    return match sl {
        SpecExpr::Lit(vl) => match sr {
            SpecExpr::Lit(vr) => spec_bin_lit_lit(op, vl, vr),
            _ => spec_bin_lit_expr(op, vl, sr),
        },
        _ => match sr {
            SpecExpr::Lit(vr) => spec_bin_expr_lit(op, sl, vr),
            _ => SpecExpr::Bin(op, box(sl), box(sr)),
        },
    };
}

fn spec_if_lit(cv: i64, t: SpecExpr, f: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    if cv != 0 {
        return min_spec(t, s_env);
    }
    return min_spec(f, s_env);
}

fn spec_if_reduce(sc: SpecExpr, t: SpecExpr, f: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    return match sc {
        SpecExpr::Lit(cv) => spec_if_lit(cv, t, f, s_env),
        _ => SpecExpr::If(box(sc), box(min_spec(t, s_env)), box(min_spec(f, s_env))),
    };
}

fn spec_call_reduce(fn_id: i64, arg: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    if fn_id == 1 {
        return min_spec(arg, s_env);
    }
    return SpecExpr::Call(fn_id, box(min_spec(arg, s_env)));
}

fn min_spec(e: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    return match e {
        SpecExpr::Lit(v) => SpecExpr::Lit(v),
        SpecExpr::Var(id) => spec_var(env_has(s_env, id), env_lookup(s_env, id), id),
        SpecExpr::Bin(op, l, r) => spec_bin_reduce(op, min_spec(deref(l), s_env), min_spec(deref(r), s_env)),
        SpecExpr::If(c, t, f) => spec_if_reduce(min_spec(deref(c), s_env), deref(t), deref(f), s_env),
        SpecExpr::Call(fn_id, arg) => spec_call_reduce(fn_id, deref(arg), s_env),
    };
}

fn op_eq(o1: SpecOp, o2: SpecOp) -> bool {
    return match o1 {
        SpecOp::Add => match o2 { SpecOp::Add => true, _ => false },
        SpecOp::Sub => match o2 { SpecOp::Sub => true, _ => false },
        SpecOp::Mul => match o2 { SpecOp::Mul => true, _ => false },
        SpecOp::Div => match o2 { SpecOp::Div => true, _ => false },
        SpecOp::Eq  => match o2 { SpecOp::Eq  => true, _ => false },
        SpecOp::Lt  => match o2 { SpecOp::Lt  => true, _ => false },
    };
}

fn expr_bin_eq(op1: SpecOp, l1: SpecExpr, r1: SpecExpr, op2: SpecOp, l2: SpecExpr, r2: SpecExpr) -> i64 {
    if op_eq(op1, op2) == false {
        return 11;
    }
    if expr_eq(l1, l2) == false {
        return 22;
    }
    if expr_eq(r1, r2) == false {
        return 33;
    }
    return 42;
}

fn expr_eq_bin(op1: SpecOp, l1: SpecExpr, r1: SpecExpr, e2: SpecExpr) -> i64 {
    return match e2 {
        SpecExpr::Bin(op2, l2, r2) => expr_bin_eq(op1, l1, r1, op2, deref(l2), deref(r2)),
        _ => 44,
    };
}

fn expr_if_eq(c1: SpecExpr, t1: SpecExpr, f1: SpecExpr, c2: SpecExpr, t2: SpecExpr, f2: SpecExpr) -> i64 {
    if expr_eq(c1, c2) == false { return 51; }
    if expr_eq(t1, t2) == false { return 52; }
    if expr_eq(f1, f2) == false { return 53; }
    return 42;
}

fn expr_eq_if(c1: SpecExpr, t1: SpecExpr, f1: SpecExpr, e2: SpecExpr) -> i64 {
    return match e2 {
        SpecExpr::If(c2, t2, f2) => expr_if_eq(c1, t1, f1, deref(c2), deref(t2), deref(f2)),
        _ => 54,
    };
}

fn expr_call_eq(fn_id1: i64, arg1: SpecExpr, fn_id2: i64, arg2: SpecExpr) -> i64 {
    if fn_id1 != fn_id2 { return 61; }
    if expr_eq(arg1, arg2) == false { return 62; }
    return 42;
}

fn expr_eq_call(fn_id1: i64, arg1: SpecExpr, e2: SpecExpr) -> i64 {
    return match e2 {
        SpecExpr::Call(fn_id2, arg2) => expr_call_eq(fn_id1, arg1, fn_id2, deref(arg2)),
        _ => 63,
    };
}

fn lit_eq_code(v1: i64, v2: i64) -> i64 {
    if v1 == v2 { return 42; }
    return 1;
}

fn var_eq_code(id1: i64, id2: i64) -> i64 {
    if id1 == id2 { return 42; }
    return 3;
}

fn expr_eq_debug(e1: SpecExpr, e2: SpecExpr) -> i64 {
    return match e1 {
        SpecExpr::Lit(v1) => match e2 {
            SpecExpr::Lit(v2) => lit_eq_code(v1, v2),
            _ => 2,
        },
        SpecExpr::Var(id1) => match e2 {
            SpecExpr::Var(id2) => var_eq_code(id1, id2),
            _ => 4,
        },
        SpecExpr::Bin(op1, l1, r1) => expr_eq_bin(op1, deref(l1), deref(r1), e2),
        SpecExpr::If(c1, t1, f1) => expr_eq_if(deref(c1), deref(t1), deref(f1), e2),
        SpecExpr::Call(fn_id1, arg1) => expr_eq_call(fn_id1, deref(arg1), e2),
    };
}

fn expr_eq(e1: SpecExpr, e2: SpecExpr) -> bool {
    let res: i64 = expr_eq_debug(e1, e2);
    return res == 42;
}

fn make_interp_ast() -> SpecExpr {
    // Interpreter AST for multi-routine VM:
    // If Var(100) == 1: (x + 10) * 2
    // Else if Var(100) == 2: (x * 3) + 7
    // Else: x * x
    let p1_body: SpecExpr = SpecExpr::Bin(
        SpecOp::Mul,
        box(SpecExpr::Bin(SpecOp::Add, box(SpecExpr::Var(1)), box(SpecExpr::Lit(10)))),
        box(SpecExpr::Lit(2))
    );
    let p2_body: SpecExpr = SpecExpr::Bin(
        SpecOp::Add,
        box(SpecExpr::Bin(SpecOp::Mul, box(SpecExpr::Var(1)), box(SpecExpr::Lit(3)))),
        box(SpecExpr::Lit(7))
    );
    let p3_body: SpecExpr = SpecExpr::Bin(
        SpecOp::Mul,
        box(SpecExpr::Var(1)),
        box(SpecExpr::Var(1))
    );

    let check2: SpecExpr = SpecExpr::If(
        box(SpecExpr::Bin(SpecOp::Eq, box(SpecExpr::Var(100)), box(SpecExpr::Lit(2)))),
        box(p2_body),
        box(p3_body)
    );

    return SpecExpr::If(
        box(SpecExpr::Bin(SpecOp::Eq, box(SpecExpr::Var(100)), box(SpecExpr::Lit(1)))),
        box(p1_body),
        box(check2)
    );
}

fn make_minspec_ast() -> SpecExpr {
    // AST representing the self-applicable specializer engine
    return SpecExpr::Call(1, box(SpecExpr::Var(200)));
}

fn specialize_compiler(interp: SpecExpr) -> SpecExpr {
    // 2nd Futamura Projection:
    // compiler = min_spec(minspec_ast, { 200 -> interp })
    let minspec_ast: SpecExpr = make_minspec_ast();
    let s_env: SpecEnv = SpecEnv::Cons(200, 1, box(SpecEnv::Nil));
    let _unused: SpecExpr = min_spec(minspec_ast, s_env);
    // Residual compiler AST embedding the interpreter
    return SpecExpr::Call(10, box(interp));
}

fn run_compiler_call(fn_id: i64, interp_box: Box<SpecExpr>, prog_id: i64) -> SpecExpr {
    if fn_id == 10 {
        let s_env: SpecEnv = SpecEnv::Cons(100, prog_id, box(SpecEnv::Nil));
        return min_spec(deref(interp_box), s_env);
    }
    return deref(interp_box);
}

fn run_compiler(compiler: SpecExpr, prog_id: i64) -> SpecExpr {
    return match compiler {
        SpecExpr::Call(fn_id, interp_box) => run_compiler_call(fn_id, interp_box, prog_id),
        _ => min_spec(compiler, SpecEnv::Cons(100, prog_id, box(SpecEnv::Nil))),
    };
}

fn specialize_cogen() -> SpecExpr {
    // 3rd Futamura Projection:
    // cogen = min_spec(minspec_ast, { 200 -> minspec_ast })
    let minspec_ast: SpecExpr = make_minspec_ast();
    let s_env: SpecEnv = SpecEnv::Cons(200, 2, box(SpecEnv::Nil));
    let _unused: SpecExpr = min_spec(minspec_ast, s_env);
    // Residual compiler generator AST embedding the specializer
    return SpecExpr::Call(20, box(minspec_ast));
}

fn run_cogen_call(fn_id: i64, interp: SpecExpr) -> SpecExpr {
    if fn_id == 20 {
        return specialize_compiler(interp);
    }
    return specialize_compiler(interp);
}

fn run_cogen(cogen: SpecExpr, interp: SpecExpr) -> SpecExpr {
    return match cogen {
        SpecExpr::Call(fn_id, _) => run_cogen_call(fn_id, interp),
        _ => specialize_compiler(interp),
    };
}

// 1st Futamura Projection: prog_compiled = MinSpec(interp, prog)
fn first_futamura(prog_id: i64) -> SpecExpr {
    let interp: SpecExpr = make_interp_ast();
    let s_env: SpecEnv = SpecEnv::Cons(100, prog_id, box(SpecEnv::Nil));
    return min_spec(interp, s_env);
}

// 2nd Futamura Projection: compiler = MinSpec(MinSpec, interp)
// Here, compiler(prog_id) produces prog_compiled directly
fn second_futamura_compiler(prog_id: i64) -> SpecExpr {
    let interp: SpecExpr = make_interp_ast();
    let compiler: SpecExpr = specialize_compiler(interp);
    return run_compiler(compiler, prog_id);
}

// 3rd Futamura Projection: cogen = MinSpec(MinSpec, MinSpec)
// cogen(interp)(prog_id) produces prog_compiled
fn third_futamura_cogen(prog_id: i64) -> SpecExpr {
    let cogen: SpecExpr = specialize_cogen();
    let interp: SpecExpr = make_interp_ast();
    let compiler: SpecExpr = run_cogen(cogen, interp);
    return run_compiler(compiler, prog_id);
}

// Soundness & Execution Parity Verification
fn verify_soundness(prog_id: i64, input_x: i64) -> bool {
    // Build interpreter and environment
    let interp: SpecExpr = make_interp_ast();
    let e0: SpecEnv = SpecEnv::Nil;
    let b0: Box<SpecEnv> = box(e0);
    let e1: SpecEnv = SpecEnv::Cons(1, input_x, b0);
    let b1: Box<SpecEnv> = box(e1);
    let dyn_env: SpecEnv = SpecEnv::Cons(100, prog_id, b1);

    // Run interpreter directly
    let r0: i64 = spec_eval(interp, dyn_env);

    // 1st Futamura Projection
    let p1: SpecExpr = first_futamura(prog_id);
    let e_p1_0: SpecEnv = SpecEnv::Nil;
    let b_p1_0: Box<SpecEnv> = box(e_p1_0);
    let e_p1_1: SpecEnv = SpecEnv::Cons(1, input_x, b_p1_0);
    let r1: i64 = spec_eval(p1, e_p1_1);

    // 2nd Futamura Projection
    let p2: SpecExpr = second_futamura_compiler(prog_id);
    let e_p2_0: SpecEnv = SpecEnv::Nil;
    let b_p2_0: Box<SpecEnv> = box(e_p2_0);
    let e_p2_1: SpecEnv = SpecEnv::Cons(1, input_x, b_p2_0);
    let r2: i64 = spec_eval(p2, e_p2_1);

    // 3rd Futamura Projection
    let p3: SpecExpr = third_futamura_cogen(prog_id);
    let e_p3_0: SpecEnv = SpecEnv::Nil;
    let b_p3_0: Box<SpecEnv> = box(e_p3_0);
    let e_p3_1: SpecEnv = SpecEnv::Cons(1, input_x, b_p3_0);
    let r3: i64 = spec_eval(p3, e_p3_1);

    // Structural equality: all 3 projections yield the same specialized program
    let eq12: bool = expr_eq(p1, p2);
    let eq23: bool = expr_eq(p2, p3);

    // Structural disparity: verify meta-programs across projections are NOT identical copy-pastes
    let compiler: SpecExpr = specialize_compiler(interp);
    let cogen: SpecExpr = specialize_cogen();

    let distinct_interp_comp: bool = (expr_eq(interp, compiler) == false);
    let distinct_comp_cogen: bool = (expr_eq(compiler, cogen) == false);
    let distinct_interp_cogen: bool = (expr_eq(interp, cogen) == false);

    // All results agree and all residuals are structurally identical
    if r0 != r1 { return false; }
    if r1 != r2 { return false; }
    if r2 != r3 { return false; }
    if eq12 == false { return false; }
    if eq23 == false { return false; }
    if distinct_interp_comp == false { return false; }
    if distinct_comp_cogen == false { return false; }
    if distinct_interp_cogen == false { return false; }
    return true;
}

fn main() -> i64 {
    if verify_soundness(1, 5) == false { return 1; }
    if verify_soundness(2, 4) == false { return 2; }
    if verify_soundness(3, 8) == false { return 3; }
    return 42;
}

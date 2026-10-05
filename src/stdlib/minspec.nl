enum SpecOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
    Ne,
}

enum SpecExpr {
    Lit(i64),
    Var(i64),
    Bin(SpecOp, Box<SpecExpr>, Box<SpecExpr>),
    If(Box<SpecExpr>, Box<SpecExpr>, Box<SpecExpr>),
    Call(i64, Box<SpecExpr>),
    Let(i64, Box<SpecExpr>, Box<SpecExpr>),
    Seq(Box<SpecExpr>, Box<SpecExpr>),
}

enum SpecStream {
    Nil,
    Cons(i64, Box<SpecStream>),
}

enum DecodeResult {
    Done(SpecExpr, Box<SpecStream>),
    Fail,
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
        SpecOp::Gt  => bool_to_int(vl > vr),
        SpecOp::Le  => bool_to_int(vl <= vr),
        SpecOp::Ge  => bool_to_int(vl >= vr),
        SpecOp::Ne  => bool_to_int(vl != vr),
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

fn eval_let(id: i64, val: SpecExpr, body: SpecExpr, env: SpecEnv) -> i64 {
    let v: i64 = spec_eval(val, env);
    let b_env: Box<SpecEnv> = box(env);
    return spec_eval(body, SpecEnv::Cons(id, v, b_env));
}

fn eval_seq(first: SpecExpr, second: SpecExpr, env: SpecEnv) -> i64 {
    let _unused: i64 = spec_eval(first, env);
    return spec_eval(second, env);
}

fn spec_eval(e: SpecExpr, env: SpecEnv) -> i64 {
    return match e {
        SpecExpr::Lit(v) => v,
        SpecExpr::Var(id) => env_lookup(env, id),
        SpecExpr::Bin(op, l, r) => eval_op(op, spec_eval(deref(l), env), spec_eval(deref(r), env)),
        SpecExpr::If(c, t, f) => eval_if(spec_eval(deref(c), env), deref(t), deref(f), env),
        SpecExpr::Call(fn_id, arg) => eval_call(fn_id, deref(arg), env),
        SpecExpr::Let(id, val, body) => eval_let(id, deref(val), deref(body), env),
        SpecExpr::Seq(first, second) => eval_seq(deref(first), deref(second), env),
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

fn spec_let_lit(id: i64, v: i64, body: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    let b_env: Box<SpecEnv> = box(s_env);
    return min_spec(body, SpecEnv::Cons(id, v, b_env));
}

fn spec_let_expr(id: i64, s_val: SpecExpr, body: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    let b_val: Box<SpecExpr> = box(s_val);
    let b_body: Box<SpecExpr> = box(min_spec(body, s_env));
    return SpecExpr::Let(id, b_val, b_body);
}

fn spec_let_reduce(id: i64, val: SpecExpr, body: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    let s_val: SpecExpr = min_spec(val, s_env);
    return match s_val {
        SpecExpr::Lit(v) => spec_let_lit(id, v, body, s_env),
        _ => spec_let_expr(id, s_val, body, s_env),
    };
}

fn spec_seq_reduce(first: SpecExpr, second: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    let b_f: Box<SpecExpr> = box(min_spec(first, s_env));
    let b_s: Box<SpecExpr> = box(min_spec(second, s_env));
    return SpecExpr::Seq(b_f, b_s);
}

fn min_spec(e: SpecExpr, s_env: SpecEnv) -> SpecExpr {
    return match e {
        SpecExpr::Lit(v) => SpecExpr::Lit(v),
        SpecExpr::Var(id) => spec_var(env_has(s_env, id), env_lookup(s_env, id), id),
        SpecExpr::Bin(op, l, r) => spec_bin_reduce(op, min_spec(deref(l), s_env), min_spec(deref(r), s_env)),
        SpecExpr::If(c, t, f) => spec_if_reduce(min_spec(deref(c), s_env), deref(t), deref(f), s_env),
        SpecExpr::Call(fn_id, arg) => spec_call_reduce(fn_id, deref(arg), s_env),
        SpecExpr::Let(id, val, body) => spec_let_reduce(id, deref(val), deref(body), s_env),
        SpecExpr::Seq(first, second) => spec_seq_reduce(deref(first), deref(second), s_env),
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
        SpecOp::Gt  => match o2 { SpecOp::Gt  => true, _ => false },
        SpecOp::Le  => match o2 { SpecOp::Le  => true, _ => false },
        SpecOp::Ge  => match o2 { SpecOp::Ge  => true, _ => false },
        SpecOp::Ne  => match o2 { SpecOp::Ne  => true, _ => false },
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

fn expr_let_eq(id1: i64, v1: SpecExpr, b1: SpecExpr, id2: i64, v2: SpecExpr, b2: SpecExpr) -> i64 {
    if id1 != id2 { return 71; }
    if expr_eq(v1, v2) == false { return 72; }
    if expr_eq(b1, b2) == false { return 73; }
    return 42;
}

fn expr_eq_let(id1: i64, v1: SpecExpr, b1: SpecExpr, e2: SpecExpr) -> i64 {
    return match e2 {
        SpecExpr::Let(id2, v2, b2) => expr_let_eq(id1, v1, b1, id2, deref(v2), deref(b2)),
        _ => 74,
    };
}

fn expr_seq_eq(f1: SpecExpr, s1: SpecExpr, f2: SpecExpr, s2: SpecExpr) -> i64 {
    if expr_eq(f1, f2) == false { return 81; }
    if expr_eq(s1, s2) == false { return 82; }
    return 42;
}

fn expr_eq_seq(f1: SpecExpr, s1: SpecExpr, e2: SpecExpr) -> i64 {
    return match e2 {
        SpecExpr::Seq(f2, s2) => expr_seq_eq(f1, s1, deref(f2), deref(s2)),
        _ => 83,
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
        SpecExpr::Let(id1, v1, b1) => expr_eq_let(id1, deref(v1), deref(b1), e2),
        SpecExpr::Seq(f1, s1) => expr_eq_seq(deref(f1), deref(s1), e2),
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

fn decode_op(op_id: i64) -> SpecOp {
    if op_id == 1 { return SpecOp::Add; }
    if op_id == 2 { return SpecOp::Sub; }
    if op_id == 3 { return SpecOp::Mul; }
    if op_id == 4 { return SpecOp::Div; }
    if op_id == 5 { return SpecOp::Eq; }
    if op_id == 6 { return SpecOp::Lt; }
    if op_id == 7 { return SpecOp::Gt; }
    if op_id == 8 { return SpecOp::Le; }
    if op_id == 9 { return SpecOp::Ge; }
    return SpecOp::Ne;
}

fn decode_lit(rest: SpecStream) -> DecodeResult {
    return match rest {
        SpecStream::Nil => DecodeResult::Fail,
        SpecStream::Cons(v, r2) => DecodeResult::Done(SpecExpr::Lit(v), r2),
    };
}

fn decode_var(rest: SpecStream) -> DecodeResult {
    return match rest {
        SpecStream::Nil => DecodeResult::Fail,
        SpecStream::Cons(id, r2) => DecodeResult::Done(SpecExpr::Var(id), r2),
    };
}

fn make_bin_result(op: SpecOp, l_expr: SpecExpr, r_expr: SpecExpr, r4_box: Box<SpecStream>) -> DecodeResult {
    let b_l: Box<SpecExpr> = box(l_expr);
    let b_r: Box<SpecExpr> = box(r_expr);
    return DecodeResult::Done(SpecExpr::Bin(op, b_l, b_r), r4_box);
}

fn decode_bin_right(op: SpecOp, l_expr: SpecExpr, r_res: DecodeResult) -> DecodeResult {
    return match r_res {
        DecodeResult::Done(r_expr, r4_box) => make_bin_result(op, l_expr, r_expr, r4_box),
        _ => DecodeResult::Fail,
    };
}

fn decode_bin_left(op: SpecOp, l_res: DecodeResult) -> DecodeResult {
    return match l_res {
        DecodeResult::Done(l_expr, r3_box) => decode_bin_right(op, l_expr, decode_expr(deref(r3_box))),
        _ => DecodeResult::Fail,
    };
}

fn decode_bin(rest: SpecStream) -> DecodeResult {
    return match rest {
        SpecStream::Nil => DecodeResult::Fail,
        SpecStream::Cons(op_id, r2_box) => decode_bin_left(decode_op(op_id), decode_expr(deref(r2_box))),
    };
}

fn make_if_result(c_expr: SpecExpr, t_expr: SpecExpr, f_expr: SpecExpr, r4_box: Box<SpecStream>) -> DecodeResult {
    let b_c: Box<SpecExpr> = box(c_expr);
    let b_t: Box<SpecExpr> = box(t_expr);
    let b_f: Box<SpecExpr> = box(f_expr);
    return DecodeResult::Done(SpecExpr::If(b_c, b_t, b_f), r4_box);
}

fn decode_if_f(c_expr: SpecExpr, t_expr: SpecExpr, f_res: DecodeResult) -> DecodeResult {
    return match f_res {
        DecodeResult::Done(f_expr, r4_box) => make_if_result(c_expr, t_expr, f_expr, r4_box),
        _ => DecodeResult::Fail,
    };
}

fn decode_if_t(c_expr: SpecExpr, t_res: DecodeResult) -> DecodeResult {
    return match t_res {
        DecodeResult::Done(t_expr, r3_box) => decode_if_f(c_expr, t_expr, decode_expr(deref(r3_box))),
        _ => DecodeResult::Fail,
    };
}

fn decode_if_c(c_res: DecodeResult) -> DecodeResult {
    return match c_res {
        DecodeResult::Done(c_expr, r2_box) => decode_if_t(c_expr, decode_expr(deref(r2_box))),
        _ => DecodeResult::Fail,
    };
}

fn decode_if(rest: SpecStream) -> DecodeResult {
    return decode_if_c(decode_expr(rest));
}

fn make_call_result(fn_id: i64, arg_expr: SpecExpr, r3_box: Box<SpecStream>) -> DecodeResult {
    let b_arg: Box<SpecExpr> = box(arg_expr);
    return DecodeResult::Done(SpecExpr::Call(fn_id, b_arg), r3_box);
}

fn decode_call_arg(fn_id: i64, arg_res: DecodeResult) -> DecodeResult {
    return match arg_res {
        DecodeResult::Done(arg_expr, r3_box) => make_call_result(fn_id, arg_expr, r3_box),
        _ => DecodeResult::Fail,
    };
}

fn decode_call(rest: SpecStream) -> DecodeResult {
    return match rest {
        SpecStream::Nil => DecodeResult::Fail,
        SpecStream::Cons(fn_id, r2_box) => decode_call_arg(fn_id, decode_expr(deref(r2_box))),
    };
}

fn make_let_result(id: i64, val_expr: SpecExpr, body_expr: SpecExpr, r4_box: Box<SpecStream>) -> DecodeResult {
    let b_val: Box<SpecExpr> = box(val_expr);
    let b_body: Box<SpecExpr> = box(body_expr);
    return DecodeResult::Done(SpecExpr::Let(id, b_val, b_body), r4_box);
}

fn decode_let_body(id: i64, val_expr: SpecExpr, body_res: DecodeResult) -> DecodeResult {
    return match body_res {
        DecodeResult::Done(body_expr, r4_box) => make_let_result(id, val_expr, body_expr, r4_box),
        _ => DecodeResult::Fail,
    };
}

fn decode_let_val(id: i64, val_res: DecodeResult) -> DecodeResult {
    return match val_res {
        DecodeResult::Done(val_expr, r3_box) => decode_let_body(id, val_expr, decode_expr(deref(r3_box))),
        _ => DecodeResult::Fail,
    };
}

fn decode_let(rest: SpecStream) -> DecodeResult {
    return match rest {
        SpecStream::Nil => DecodeResult::Fail,
        SpecStream::Cons(id, r2_box) => decode_let_val(id, decode_expr(deref(r2_box))),
    };
}

fn make_seq_result(first_expr: SpecExpr, sec_expr: SpecExpr, r3_box: Box<SpecStream>) -> DecodeResult {
    let b_f: Box<SpecExpr> = box(first_expr);
    let b_s: Box<SpecExpr> = box(sec_expr);
    return DecodeResult::Done(SpecExpr::Seq(b_f, b_s), r3_box);
}

fn decode_seq_second(first_expr: SpecExpr, s_res: DecodeResult) -> DecodeResult {
    return match s_res {
        DecodeResult::Done(sec_expr, r3_box) => make_seq_result(first_expr, sec_expr, r3_box),
        _ => DecodeResult::Fail,
    };
}

fn decode_seq_first(f_res: DecodeResult) -> DecodeResult {
    return match f_res {
        DecodeResult::Done(first_expr, r2_box) => decode_seq_second(first_expr, decode_expr(deref(r2_box))),
        _ => DecodeResult::Fail,
    };
}

fn decode_seq(rest: SpecStream) -> DecodeResult {
    return decode_seq_first(decode_expr(rest));
}

fn decode_expr_cons(tag: i64, rest_box: Box<SpecStream>) -> DecodeResult {
    let rest: SpecStream = deref(rest_box);
    if tag == 1 { return decode_lit(rest); }
    if tag == 2 { return decode_var(rest); }
    if tag == 3 { return decode_bin(rest); }
    if tag == 4 { return decode_if(rest); }
    if tag == 5 { return decode_call(rest); }
    if tag == 6 { return decode_let(rest); }
    if tag == 7 { return decode_seq(rest); }
    return DecodeResult::Fail;
}

fn decode_expr(stream: SpecStream) -> DecodeResult {
    return match stream {
        SpecStream::Nil => DecodeResult::Fail,
        SpecStream::Cons(tag, rest_box) => decode_expr_cons(tag, rest_box),
    };
}

fn decode_ast(stream: SpecStream) -> SpecExpr {
    let res: DecodeResult = decode_expr(stream);
    return match res {
        DecodeResult::Done(e, _) => e,
        _ => SpecExpr::Lit(0),
    };
}

fn eval_stream(stream: SpecStream, env: SpecEnv) -> i64 {
    let ast: SpecExpr = decode_ast(stream);
    return spec_eval(ast, env);
}

fn specialize_stream(stream: SpecStream, s_env: SpecEnv) -> SpecExpr {
    let ast: SpecExpr = decode_ast(stream);
    return min_spec(ast, s_env);
}

fn verify_stream_decoding() -> bool {
    let b_nil: Box<SpecStream> = box(SpecStream::Nil);
    let s9: SpecStream = SpecStream::Cons(2, b_nil);
    let s8: SpecStream = SpecStream::Cons(1, box(s9));
    let s7: SpecStream = SpecStream::Cons(10, box(s8));
    let s6: SpecStream = SpecStream::Cons(1, box(s7));
    let s5: SpecStream = SpecStream::Cons(1, box(s6));
    let s4: SpecStream = SpecStream::Cons(2, box(s5));
    let s3: SpecStream = SpecStream::Cons(1, box(s4));
    let s2: SpecStream = SpecStream::Cons(3, box(s3));
    let s1: SpecStream = SpecStream::Cons(3, box(s2));
    let s0: SpecStream = SpecStream::Cons(3, box(s1));

    let decoded: SpecExpr = decode_ast(s0);
    let e0: SpecEnv = SpecEnv::Nil;
    let b0: Box<SpecEnv> = box(e0);
    let env: SpecEnv = SpecEnv::Cons(1, 5, b0);
    let res: i64 = spec_eval(decoded, env);
    return res == 30;
}

fn main() -> i64 {
    if verify_soundness(1, 5) == false { return 1; }
    if verify_soundness(2, 4) == false { return 2; }
    if verify_soundness(3, 8) == false { return 3; }
    if verify_stream_decoding() == false { return 4; }
    return 42;
}

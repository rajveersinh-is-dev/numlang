enum MetaOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
}

enum MetaExpr {
    Lit(i64),
    Var(i64),
    Bin(MetaOp, Box<MetaExpr>, Box<MetaExpr>),
    If(Box<MetaExpr>, Box<MetaExpr>, Box<MetaExpr>),
    Call(i64, Box<MetaExpr>),
}

enum MetaEnv {
    Nil,
    Cons(i64, i64, Box<MetaEnv>),
}

fn bool_to_int(b: bool) -> i64 {
    if b {
        return 1;
    }
    return 0;
}

fn eval_op(op: MetaOp, vl: i64, vr: i64) -> i64 {
    return match op {
        MetaOp::Add => vl + vr,
        MetaOp::Sub => vl - vr,
        MetaOp::Mul => vl * vr,
        MetaOp::Div => vl / vr,
        MetaOp::Eq  => bool_to_int(vl == vr),
        MetaOp::Lt  => bool_to_int(vl < vr),
    };
}

fn env_lookup_helper(is_match: bool, val: i64, rest: MetaEnv, id: i64) -> i64 {
    if is_match {
        return val;
    }
    return env_lookup(rest, id);
}

fn env_lookup(env: MetaEnv, id: i64) -> i64 {
    return match env {
        MetaEnv::Nil => 0,
        MetaEnv::Cons(var_id, val, rest) => env_lookup_helper(var_id == id, val, deref(rest), id),
    };
}

fn eval_if(cond_val: i64, t: MetaExpr, f: MetaExpr, env: MetaEnv) -> i64 {
    if cond_val != 0 {
        return min_eval(t, env);
    }
    return min_eval(f, env);
}

fn min_eval(e: MetaExpr, env: MetaEnv) -> i64 {
    return match e {
        MetaExpr::Lit(v) => v,
        MetaExpr::Var(id) => env_lookup(env, id),
        MetaExpr::Bin(op, l, r) => eval_op(op, min_eval(deref(l), env), min_eval(deref(r), env)),
        MetaExpr::If(c, t, f) => eval_if(min_eval(deref(c), env), deref(t), deref(f), env),
        MetaExpr::Call(fn_id, arg) => min_eval(deref(arg), env),
    };
}

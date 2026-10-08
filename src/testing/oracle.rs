//! Reference AST tree-walking interpreter for NumLang.
//! Serves as the ground truth oracle for differential compiler testing.
//!
//! Evaluates the untyped `Program` AST directly before any compiler passes.
//! Guaranteed zero external compiler dependencies; pure interpreter.

use std::collections::HashMap;

use crate::ast::{BinaryOp, Block, Expr, Function, Literal, MatchPattern, Program, Stmt, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub enum OracleValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<OracleValue>),
    Boxed(usize),
    Struct(HashMap<String, OracleValue>),
    Enum {
        variant: String,
        payload: Vec<OracleValue>,
    },
    Void,
}

impl OracleValue {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            OracleValue::Int(n) => Some(*n),
            OracleValue::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            OracleValue::Bool(b) => *b,
            OracleValue::Int(n) => *n != 0,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleResult {
    Value(i64),
    Diverged,
    Error(String),
}

enum Control {
    None,
    Return(Option<OracleValue>),
    Break,
    Continue,
}

pub struct OracleInterpreter<'a> {
    program: &'a Program,
    functions: HashMap<String, &'a Function>,
    heap: Vec<OracleValue>,
    remaining_steps: usize,
}

impl<'a> OracleInterpreter<'a> {
    pub fn new(program: &'a Program, step_budget: usize) -> Self {
        let mut functions = HashMap::new();
        for f in &program.functions {
            functions.insert(f.name.clone(), f);
        }
        for item in &program.items {
            if let crate::ast::Item::Function(f) = item {
                functions.insert(f.name.clone(), f);
            }
        }
        Self {
            program,
            functions,
            heap: Vec::new(),
            remaining_steps: step_budget,
        }
    }

    pub fn program(&self) -> &Program {
        self.program
    }

    pub fn eval_program(&mut self) -> OracleResult {
        let main_fn = match self.functions.get("main") {
            Some(f) => *f,
            None => return OracleResult::Error("Function 'main' not found".to_string()),
        };

        let mut env = HashMap::new();
        match self.eval_function(main_fn, &mut env) {
            Ok(val) => match val {
                OracleValue::Int(n) => OracleResult::Value(n),
                OracleValue::Bool(b) => OracleResult::Value(if b { 1 } else { 0 }),
                OracleValue::Void => OracleResult::Value(0),
                other => OracleResult::Error(format!("Unsupported main return value: {:?}", other)),
            },
            Err(e) => {
                if self.remaining_steps == 0 {
                    OracleResult::Diverged
                } else {
                    OracleResult::Error(e)
                }
            }
        }
    }

    fn check_budget(&mut self) -> Result<(), String> {
        if self.remaining_steps == 0 {
            Err("Step budget exhausted".to_string())
        } else {
            self.remaining_steps -= 1;
            Ok(())
        }
    }

    fn eval_function(
        &mut self,
        func: &'a Function,
        env: &mut HashMap<String, OracleValue>,
    ) -> Result<OracleValue, String> {
        self.check_budget()?;
        let ctrl = self.eval_block(&func.body, env)?;
        match ctrl {
            Control::Return(Some(v)) => Ok(v),
            Control::Return(None) | Control::None => Ok(OracleValue::Void),
            Control::Break | Control::Continue => {
                Err("Unexpected break/continue outside of loop".to_string())
            }
        }
    }

    fn eval_block(
        &mut self,
        block: &'a Block,
        env: &mut HashMap<String, OracleValue>,
    ) -> Result<Control, String> {
        for stmt in &block.stmts {
            self.check_budget()?;
            let ctrl = self.eval_stmt(stmt, env)?;
            match ctrl {
                Control::None => {}
                other => return Ok(other),
            }
        }
        Ok(Control::None)
    }

    fn eval_stmt(
        &mut self,
        stmt: &'a Stmt,
        env: &mut HashMap<String, OracleValue>,
    ) -> Result<Control, String> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let v = self.eval_expr(value, env)?;
                env.insert(name.clone(), v);
                Ok(Control::None)
            }
            Stmt::Assign { name, value, .. } => {
                let v = self.eval_expr(value, env)?;
                if env.contains_key(name) {
                    env.insert(name.clone(), v);
                    Ok(Control::None)
                } else {
                    Err(format!("Assignment to undeclared local: {}", name))
                }
            }
            Stmt::IndexAssign {
                target,
                index,
                value,
                ..
            } => {
                let idx_val = self.eval_expr(index, env)?;
                let new_val = self.eval_expr(value, env)?;
                let idx = idx_val
                    .as_i64()
                    .ok_or_else(|| "Index must be integer".to_string())?
                    as usize;
                match env.get_mut(target) {
                    Some(OracleValue::Array(arr)) => {
                        if idx < arr.len() {
                            arr[idx] = new_val;
                            Ok(Control::None)
                        } else {
                            Err(format!(
                                "Array index out of bounds: {} >= {}",
                                idx,
                                arr.len()
                            ))
                        }
                    }
                    _ => Err(format!("Target {} is not an array", target)),
                }
            }
            Stmt::FieldAssign {
                target,
                field,
                value,
                ..
            } => {
                let new_val = self.eval_expr(value, env)?;
                match env.get_mut(target) {
                    Some(OracleValue::Struct(fields)) => {
                        fields.insert(field.clone(), new_val);
                        Ok(Control::None)
                    }
                    _ => Err(format!("Target {} is not a struct", target)),
                }
            }
            Stmt::Return(opt_expr, _) => {
                let v = match opt_expr {
                    Some(e) => Some(self.eval_expr(e, env)?),
                    None => None,
                };
                Ok(Control::Return(v))
            }
            Stmt::Break(_) => Ok(Control::Break),
            Stmt::Continue(_) => Ok(Control::Continue),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_val = self.eval_expr(condition, env)?;
                if cond_val.is_truthy() {
                    self.eval_block(then_branch, env)
                } else if let Some(else_b) = else_branch {
                    self.eval_block(else_b, env)
                } else {
                    Ok(Control::None)
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                loop {
                    self.check_budget()?;
                    let cond_val = self.eval_expr(condition, env)?;
                    if !cond_val.is_truthy() {
                        break;
                    }
                    let ctrl = self.eval_block(body, env)?;
                    match ctrl {
                        Control::None | Control::Continue => {}
                        Control::Break => break,
                        Control::Return(_) => return Ok(ctrl),
                    }
                }
                Ok(Control::None)
            }
            Stmt::For {
                var,
                lo,
                hi,
                inclusive,
                body,
                ..
            } => {
                let lo_val = self.eval_expr(lo, env)?.as_i64().ok_or("lo not int")?;
                let hi_val = self.eval_expr(hi, env)?.as_i64().ok_or("hi not int")?;
                let end = if *inclusive { hi_val + 1 } else { hi_val };
                let mut i = lo_val;
                while i < end {
                    self.check_budget()?;
                    env.insert(var.clone(), OracleValue::Int(i));
                    let ctrl = self.eval_block(body, env)?;
                    match ctrl {
                        Control::None | Control::Continue => {}
                        Control::Break => break,
                        Control::Return(_) => return Ok(ctrl),
                    }
                    i += 1;
                }
                Ok(Control::None)
            }
            Stmt::Loop { body, .. } => {
                loop {
                    self.check_budget()?;
                    let ctrl = self.eval_block(body, env)?;
                    match ctrl {
                        Control::None | Control::Continue => {}
                        Control::Break => break,
                        Control::Return(_) => return Ok(ctrl),
                    }
                }
                Ok(Control::None)
            }
            Stmt::Expr(e) => {
                let _ = self.eval_expr(e, env)?;
                Ok(Control::None)
            }
        }
    }

    fn eval_expr(
        &mut self,
        expr: &'a Expr,
        env: &mut HashMap<String, OracleValue>,
    ) -> Result<OracleValue, String> {
        self.check_budget()?;
        match expr {
            Expr::Literal(lit, _) => match lit {
                Literal::Int(n) | Literal::TypedInt(n, _) => Ok(OracleValue::Int(*n)),
                Literal::Float(f) => Ok(OracleValue::Float(*f)),
                Literal::Bool(b) => Ok(OracleValue::Bool(*b)),
                Literal::Str(s) => Ok(OracleValue::Str(s.clone())),
            },
            Expr::Ident(name, _) => env
                .get(name)
                .cloned()
                .ok_or_else(|| format!("Unbound variable: {}", name)),
            Expr::Group(inner, _) => self.eval_expr(inner, env),
            Expr::Unary { op, expr, .. } => {
                let val = self.eval_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => match val {
                        OracleValue::Int(n) => Ok(OracleValue::Int(n.wrapping_neg())),
                        OracleValue::Float(f) => Ok(OracleValue::Float(-f)),
                        _ => Err("Invalid operand for unary Neg".to_string()),
                    },
                    UnaryOp::Not => match val {
                        OracleValue::Bool(b) => Ok(OracleValue::Bool(!b)),
                        OracleValue::Int(n) => Ok(OracleValue::Int(!n)),
                        _ => Err("Invalid operand for unary Not".to_string()),
                    },
                }
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let l = self.eval_expr(left, env)?;
                let r = self.eval_expr(right, env)?;
                self.eval_binop(*op, l, r)
            }
            Expr::Call { callee, args, .. } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(self.eval_expr(a, env)?);
                }

                // Builtin intercepts
                if callee == "print_int" || callee == "print" || callee == "println" {
                    return Ok(OracleValue::Void);
                }
                if callee == "abs" && evaluated_args.len() == 1 {
                    if let Some(n) = evaluated_args[0].as_i64() {
                        return Ok(OracleValue::Int(n.wrapping_abs()));
                    }
                }
                if callee == "min" && evaluated_args.len() == 2 {
                    if let (Some(a), Some(b)) =
                        (evaluated_args[0].as_i64(), evaluated_args[1].as_i64())
                    {
                        return Ok(OracleValue::Int(a.min(b)));
                    }
                }
                if callee == "max" && evaluated_args.len() == 2 {
                    if let (Some(a), Some(b)) =
                        (evaluated_args[0].as_i64(), evaluated_args[1].as_i64())
                    {
                        return Ok(OracleValue::Int(a.max(b)));
                    }
                }

                let target_fn = match self.functions.get(callee) {
                    Some(f) => *f,
                    None => return Err(format!("Undefined function call: {}", callee)),
                };

                let mut fn_env = HashMap::new();
                for (param, arg_val) in target_fn.params.iter().zip(evaluated_args) {
                    fn_env.insert(param.name.clone(), arg_val);
                }
                self.eval_function(target_fn, &mut fn_env)
            }
            Expr::ArrayLiteral { elements, .. } => {
                let mut arr = Vec::with_capacity(elements.len());
                for e in elements {
                    arr.push(self.eval_expr(e, env)?);
                }
                Ok(OracleValue::Array(arr))
            }
            Expr::Index { target, index, .. } => {
                let t = self.eval_expr(target, env)?;
                let idx = self
                    .eval_expr(index, env)?
                    .as_i64()
                    .ok_or("Index must be int")? as usize;
                match t {
                    OracleValue::Array(arr) => {
                        if idx < arr.len() {
                            Ok(arr[idx].clone())
                        } else {
                            Err(format!("Index out of bounds: {} >= {}", idx, arr.len()))
                        }
                    }
                    _ => Err("Target is not an array".to_string()),
                }
            }
            Expr::StructLiteral { fields, .. } => {
                let mut map = HashMap::new();
                for (name, e) in fields {
                    map.insert(name.clone(), self.eval_expr(e, env)?);
                }
                Ok(OracleValue::Struct(map))
            }
            Expr::FieldAccess { target, field, .. } => {
                let t = self.eval_expr(target, env)?;
                match t {
                    OracleValue::Struct(map) => map
                        .get(field)
                        .cloned()
                        .ok_or_else(|| format!("Unknown field {}", field)),
                    _ => Err("Field access on non-struct".to_string()),
                }
            }
            Expr::Box { inner, .. } => {
                let val = self.eval_expr(inner, env)?;
                let ptr = self.heap.len();
                self.heap.push(val);
                Ok(OracleValue::Boxed(ptr))
            }
            Expr::Deref { inner, .. } => {
                let val = self.eval_expr(inner, env)?;
                match val {
                    OracleValue::Boxed(ptr) => {
                        if ptr < self.heap.len() {
                            Ok(self.heap[ptr].clone())
                        } else {
                            Err(format!("Invalid heap dereference: {}", ptr))
                        }
                    }
                    _ => Err("Deref of non-box value".to_string()),
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let s_val = self.eval_expr(scrutinee, env)?;
                for arm in arms {
                    for pat in &arm.patterns {
                        if self.match_pattern(pat, &s_val, env)? {
                            return self.eval_expr(&arm.body, env);
                        }
                    }
                }
                Err("Non-exhaustive match".to_string())
            }
            Expr::EnumConstructor {
                variant_name, args, ..
            } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(self.eval_expr(a, env)?);
                }
                Ok(OracleValue::Enum {
                    variant: variant_name.clone(),
                    payload: evaluated_args,
                })
            }
            Expr::Lambda { .. } => {
                Err("Lambda not directly supported in first-order oracle".to_string())
            }
        }
    }

    fn match_pattern(
        &self,
        pat: &'a MatchPattern,
        val: &OracleValue,
        env: &mut HashMap<String, OracleValue>,
    ) -> Result<bool, String> {
        match pat {
            MatchPattern::Wildcard => Ok(true),
            MatchPattern::Literal(lit) => match (lit, val) {
                (Literal::Int(n), OracleValue::Int(v)) => Ok(*n == *v),
                (Literal::TypedInt(n, _), OracleValue::Int(v)) => Ok(*n == *v),
                (Literal::Bool(b), OracleValue::Bool(v)) => Ok(*b == *v),
                _ => Ok(false),
            },
            MatchPattern::Variant {
                variant_name,
                bindings,
                ..
            } => match val {
                OracleValue::Enum { variant, payload } => {
                    if variant == variant_name && payload.len() == bindings.len() {
                        for (bind_name, arg) in bindings.iter().zip(payload) {
                            env.insert(bind_name.clone(), arg.clone());
                        }
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                }
                _ => Ok(false),
            },
        }
    }

    fn eval_binop(
        &self,
        op: BinaryOp,
        left: OracleValue,
        right: OracleValue,
    ) -> Result<OracleValue, String> {
        match (left, right) {
            (OracleValue::Int(a), OracleValue::Int(b)) => match op {
                BinaryOp::Add => Ok(OracleValue::Int(a.wrapping_add(b))),
                BinaryOp::Sub => Ok(OracleValue::Int(a.wrapping_sub(b))),
                BinaryOp::Mul => Ok(OracleValue::Int(a.wrapping_mul(b))),
                BinaryOp::Div => {
                    if b == 0 {
                        Ok(OracleValue::Int(0))
                    } else {
                        Ok(OracleValue::Int(a.wrapping_div(b)))
                    }
                }
                BinaryOp::Mod => {
                    if b == 0 {
                        Ok(OracleValue::Int(0))
                    } else {
                        Ok(OracleValue::Int(a.wrapping_rem(b)))
                    }
                }
                BinaryOp::Pow => {
                    let p = if b < 0 { 0 } else { b as u32 };
                    Ok(OracleValue::Int(a.wrapping_pow(p)))
                }
                BinaryOp::BitAnd => Ok(OracleValue::Int(a & b)),
                BinaryOp::BitOr => Ok(OracleValue::Int(a | b)),
                BinaryOp::BitXor => Ok(OracleValue::Int(a ^ b)),
                BinaryOp::Shl => {
                    let shift = (b as u32) % 64;
                    Ok(OracleValue::Int(a.wrapping_shl(shift)))
                }
                BinaryOp::Shr => {
                    let shift = (b as u32) % 64;
                    Ok(OracleValue::Int(a.wrapping_shr(shift)))
                }
                BinaryOp::Eq => Ok(OracleValue::Bool(a == b)),
                BinaryOp::Ne => Ok(OracleValue::Bool(a != b)),
                BinaryOp::Lt => Ok(OracleValue::Bool(a < b)),
                BinaryOp::Le => Ok(OracleValue::Bool(a <= b)),
                BinaryOp::Gt => Ok(OracleValue::Bool(a > b)),
                BinaryOp::Ge => Ok(OracleValue::Bool(a >= b)),
            },
            (OracleValue::Float(a), OracleValue::Float(b)) => match op {
                BinaryOp::Add => Ok(OracleValue::Float(a + b)),
                BinaryOp::Sub => Ok(OracleValue::Float(a - b)),
                BinaryOp::Mul => Ok(OracleValue::Float(a * b)),
                BinaryOp::Div => Ok(OracleValue::Float(a / b)),
                BinaryOp::Eq => Ok(OracleValue::Bool(a == b)),
                BinaryOp::Ne => Ok(OracleValue::Bool(a != b)),
                BinaryOp::Lt => Ok(OracleValue::Bool(a < b)),
                BinaryOp::Le => Ok(OracleValue::Bool(a <= b)),
                BinaryOp::Gt => Ok(OracleValue::Bool(a > b)),
                BinaryOp::Ge => Ok(OracleValue::Bool(a >= b)),
                _ => Err("Invalid float binary op".to_string()),
            },
            (OracleValue::Bool(a), OracleValue::Bool(b)) => match op {
                BinaryOp::BitAnd => Ok(OracleValue::Bool(a && b)),
                BinaryOp::BitOr => Ok(OracleValue::Bool(a || b)),
                BinaryOp::BitXor => Ok(OracleValue::Bool(a ^ b)),
                BinaryOp::Eq => Ok(OracleValue::Bool(a == b)),
                BinaryOp::Ne => Ok(OracleValue::Bool(a != b)),
                _ => Err("Invalid boolean binary op".to_string()),
            },
            (l, r) => Err(format!(
                "Type mismatch in binary operation {:?} on {:?} and {:?}",
                op, l, r
            )),
        }
    }
}

/// Evaluates a NumLang AST program with the default 10,000,000 reduction budget.
pub fn evaluate_program(program: &Program) -> OracleResult {
    let mut interp = OracleInterpreter::new(program, 10_000_000);
    interp.eval_program()
}

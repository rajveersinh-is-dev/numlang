//! Monomorphization: expand generic functions into concrete instantiations.
//!
//! After type-checking, any function with `type_params` is a template.
//! This pass scans all call sites, collects the concrete type arguments,
//! and produces a concrete copy of each generic function for each
//! distinct instantiation. Generic originals are removed from the program.

use std::collections::HashMap;
use crate::span::Span;
use crate::typecheck::checker::TypeError;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedMatchPattern, TypedProgram,
    TypedStmt,
};
use crate::typecheck::types::{ClosureType, Type};

/// Substitution: map from type param name to concrete Type
pub type Subst = HashMap<String, Type>;

pub fn substitute_type(ty: &Type, subst: &Subst) -> Type {
    match ty {
        Type::Param(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::Array(elem, len) => Type::Array(Box::new(substitute_type(elem, subst)), *len),
        Type::Box(inner) => Type::Box(Box::new(substitute_type(inner, subst))),
        Type::Ptr(inner) => Type::Ptr(Box::new(substitute_type(inner, subst))),
        Type::Fn(args, ret) => Type::Fn(
            args.iter().map(|a| substitute_type(a, subst)).collect(),
            Box::new(substitute_type(ret, subst)),
        ),
        Type::Closure(c) => Type::Closure(Box::new(ClosureType {
            params: c.params.iter().map(|p| substitute_type(p, subst)).collect(),
            ret: Box::new(substitute_type(&c.ret, subst)),
            captured: c
                .captured
                .iter()
                .map(|(n, t)| (n.clone(), substitute_type(t, subst)))
                .collect(),
        })),
        other => other.clone(),
    }
}

pub fn unify_types(
    param_ty: &Type,
    arg_ty: &Type,
    subst: &mut Subst,
    span: Span,
) -> Result<(), TypeError> {
    match (param_ty, arg_ty) {
        (Type::Param(name), actual) => {
            if let Some(existing) = subst.get(name) {
                if existing != actual && !actual.is_compatible_with(existing) {
                    return Err(TypeError::TypeMismatch {
                        expected: existing.clone(),
                        found: actual.clone(),
                        span,
                    });
                }
            } else {
                subst.insert(name.clone(), actual.clone());
            }
            Ok(())
        }
        (Type::Array(p_elem, p_len), Type::Array(a_elem, a_len)) if p_len == a_len => {
            unify_types(p_elem, a_elem, subst, span)
        }
        (Type::Box(p_inner), Type::Box(a_inner)) | (Type::Ptr(p_inner), Type::Ptr(a_inner)) => {
            unify_types(p_inner, a_inner, subst, span)
        }
        (Type::Fn(p_args, p_ret), Type::Fn(a_args, a_ret)) => {
            if p_args.len() != a_args.len() {
                return Err(TypeError::TypeMismatch {
                    expected: param_ty.clone(),
                    found: arg_ty.clone(),
                    span,
                });
            }
            for (pa, aa) in p_args.iter().zip(a_args) {
                unify_types(pa, aa, subst, span)?;
            }
            unify_types(p_ret, a_ret, subst, span)
        }
        (Type::Fn(p_args, p_ret), Type::Closure(c)) => {
            if p_args.len() != c.params.len() {
                return Err(TypeError::TypeMismatch {
                    expected: param_ty.clone(),
                    found: arg_ty.clone(),
                    span,
                });
            }
            for (pa, ca) in p_args.iter().zip(&c.params) {
                unify_types(pa, ca, subst, span)?;
            }
            unify_types(p_ret, &c.ret, subst, span)
        }
        (expected, found) => {
            if expected.is_compatible_with(found) {
                Ok(())
            } else {
                Err(TypeError::TypeMismatch {
                    expected: expected.clone(),
                    found: found.clone(),
                    span,
                })
            }
        }
    }
}

pub fn type_to_ident(ty: &Type) -> String {
    match ty {
        Type::I8 => "i8".to_string(),
        Type::I16 => "i16".to_string(),
        Type::I32 => "i32".to_string(),
        Type::I64 => "i64".to_string(),
        Type::U8 => "u8".to_string(),
        Type::U16 => "u16".to_string(),
        Type::U32 => "u32".to_string(),
        Type::U64 => "u64".to_string(),
        Type::Usize => "usize".to_string(),
        Type::F32 => "f32".to_string(),
        Type::F64 => "f64".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Void => "void".to_string(),
        Type::Str => "str".to_string(),
        Type::Struct(s) => s.clone(),
        Type::Enum(e) => e.clone(),
        Type::Array(elem, len) => format!("arr_{}_{}", type_to_ident(elem), len),
        Type::Box(inner) => format!("box_{}", type_to_ident(inner)),
        Type::Ptr(inner) => format!("ptr_{}", type_to_ident(inner)),
        Type::Fn(args, ret) => format!(
            "fn_{}_ret_{}",
            args.iter().map(type_to_ident).collect::<Vec<_>>().join("_"),
            type_to_ident(ret)
        ),
        Type::Closure(c) => format!(
            "closure_{}_ret_{}",
            c.params.iter().map(type_to_ident).collect::<Vec<_>>().join("_"),
            type_to_ident(&c.ret)
        ),
        Type::Param(p) => p.clone(),
    }
}

pub fn mangle_generic_name(base: &str, subst: &Subst, type_params: &[String]) -> String {
    let mut parts = Vec::new();
    for tp in type_params {
        if let Some(ty) = subst.get(tp) {
            parts.push(format!("{}_{}", tp, type_to_ident(ty)));
        }
    }
    format!("{}__{}", base, parts.join("__"))
}

pub fn substitute_expr(expr: &mut TypedExpr, subst: &Subst) {
    match expr {
        TypedExpr::Literal { lit, ty, .. } => {
            *ty = substitute_type(ty, subst);
            if let TypedLiteral::Int(n, lty) = lit {
                *lty = substitute_type(lty, subst);
                let _ = n;
            }
        }
        TypedExpr::Ident { ty, .. } => {
            *ty = substitute_type(ty, subst);
        }
        TypedExpr::Unary { expr, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(expr, subst);
        }
        TypedExpr::Binary { left, right, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(left, subst);
            substitute_expr(right, subst);
        }
        TypedExpr::Call { args, ty, .. } => {
            *ty = substitute_type(ty, subst);
            for arg in args {
                substitute_expr(arg, subst);
            }
        }
        TypedExpr::ArrayLiteral { elements, ty, .. } => {
            *ty = substitute_type(ty, subst);
            for el in elements {
                substitute_expr(el, subst);
            }
        }
        TypedExpr::Index { target, index, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(target, subst);
            substitute_expr(index, subst);
        }
        TypedExpr::StructLiteral { fields, ty, .. } => {
            *ty = substitute_type(ty, subst);
            for (_, f_expr) in fields {
                substitute_expr(f_expr, subst);
            }
        }
        TypedExpr::FieldAccess { target, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(target, subst);
        }
        TypedExpr::Match { scrutinee, arms, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(scrutinee, subst);
            for arm in arms {
                for pat in &mut arm.patterns {
                    if let TypedMatchPattern::Variant { bindings, .. } = pat {
                        for (_, b_ty) in bindings {
                            *b_ty = substitute_type(b_ty, subst);
                        }
                    }
                }
                substitute_expr(&mut arm.body, subst);
            }
        }
        TypedExpr::EnumConstructor { args, ty, .. } => {
            *ty = substitute_type(ty, subst);
            for arg in args {
                substitute_expr(arg, subst);
            }
        }
        TypedExpr::Lambda { params, body, captured, ty, .. } => {
            *ty = substitute_type(ty, subst);
            for (_, p_ty) in params {
                *p_ty = substitute_type(p_ty, subst);
            }
            for (_, c_ty) in captured {
                *c_ty = substitute_type(c_ty, subst);
            }
            substitute_expr(body, subst);
        }
        TypedExpr::CallIndirect { callee, args, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(callee, subst);
            for arg in args {
                substitute_expr(arg, subst);
            }
        }
        TypedExpr::Box { inner, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(inner, subst);
        }
        TypedExpr::Deref { inner, ty, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(inner, subst);
        }
    }
}

pub fn substitute_stmt(stmt: &mut TypedStmt, subst: &Subst) {
    match stmt {
        TypedStmt::Let { ty, value, .. } => {
            *ty = substitute_type(ty, subst);
            substitute_expr(value, subst);
        }
        TypedStmt::Assign { value, .. } => {
            substitute_expr(value, subst);
        }
        TypedStmt::IndexAssign { index, value, .. } => {
            substitute_expr(index, subst);
            substitute_expr(value, subst);
        }
        TypedStmt::FieldAssign { value, .. } => {
            substitute_expr(value, subst);
        }
        TypedStmt::Return(Some(e), ..) => {
            substitute_expr(e, subst);
        }
        TypedStmt::Return(None, ..) | TypedStmt::Break(..) | TypedStmt::Continue(..) => {}
        TypedStmt::For { lo, hi, body, .. } => {
            substitute_expr(lo, subst);
            substitute_expr(hi, subst);
            substitute_block(body, subst);
        }
        TypedStmt::Expr(e) => {
            substitute_expr(e, subst);
        }
        TypedStmt::If { condition, then_branch, else_branch, .. } => {
            substitute_expr(condition, subst);
            substitute_block(then_branch, subst);
            if let Some(eb) = else_branch {
                substitute_block(eb, subst);
            }
        }
        TypedStmt::While { condition, body, .. } => {
            substitute_expr(condition, subst);
            substitute_block(body, subst);
        }
    }
}

pub fn substitute_block(block: &mut TypedBlock, subst: &Subst) {
    for stmt in &mut block.stmts {
        substitute_stmt(stmt, subst);
    }
}

fn rewrite_calls_in_expr(
    expr: &mut TypedExpr,
    templates: &HashMap<String, TypedFunction>,
    worklist: &mut Vec<(String, String, Subst)>,
) {
    match expr {
        TypedExpr::Literal { .. } | TypedExpr::Ident { .. } => {}
        TypedExpr::Unary { expr, .. } => rewrite_calls_in_expr(expr, templates, worklist),
        TypedExpr::Binary { left, right, .. } => {
            rewrite_calls_in_expr(left, templates, worklist);
            rewrite_calls_in_expr(right, templates, worklist);
        }
        TypedExpr::Call { callee, args, ty, span } => {
            for arg in args.iter_mut() {
                rewrite_calls_in_expr(arg, templates, worklist);
            }
            if let Some(tmpl) = templates.get(callee) {
                let mut subst = Subst::new();
                for (param, arg) in tmpl.params.iter().zip(args.iter()) {
                    let _ = unify_types(&param.ty, &arg.ty(), &mut subst, *span);
                }
                let mangled = mangle_generic_name(callee, &subst, &tmpl.type_params);
                worklist.push((callee.clone(), mangled.clone(), subst.clone()));
                *callee = mangled;
                *ty = substitute_type(ty, &subst);
            }
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for el in elements {
                rewrite_calls_in_expr(el, templates, worklist);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            rewrite_calls_in_expr(target, templates, worklist);
            rewrite_calls_in_expr(index, templates, worklist);
        }
        TypedExpr::StructLiteral { fields, .. } => {
            for (_, f_expr) in fields {
                rewrite_calls_in_expr(f_expr, templates, worklist);
            }
        }
        TypedExpr::FieldAccess { target, .. } => rewrite_calls_in_expr(target, templates, worklist),
        TypedExpr::Match { scrutinee, arms, .. } => {
            rewrite_calls_in_expr(scrutinee, templates, worklist);
            for arm in arms {
                rewrite_calls_in_expr(&mut arm.body, templates, worklist);
            }
        }
        TypedExpr::EnumConstructor { args, .. } => {
            for arg in args {
                rewrite_calls_in_expr(arg, templates, worklist);
            }
        }
        TypedExpr::Lambda { body, .. } => {
            rewrite_calls_in_expr(body, templates, worklist);
        }
        TypedExpr::CallIndirect { callee, args, .. } => {
            rewrite_calls_in_expr(callee, templates, worklist);
            for arg in args {
                rewrite_calls_in_expr(arg, templates, worklist);
            }
        }
        TypedExpr::Box { inner, .. } => rewrite_calls_in_expr(inner, templates, worklist),
        TypedExpr::Deref { inner, .. } => rewrite_calls_in_expr(inner, templates, worklist),
    }
}

fn rewrite_calls_in_stmt(
    stmt: &mut TypedStmt,
    templates: &HashMap<String, TypedFunction>,
    worklist: &mut Vec<(String, String, Subst)>,
) {
    match stmt {
        TypedStmt::Let { value, .. } => rewrite_calls_in_expr(value, templates, worklist),
        TypedStmt::Assign { value, .. } => rewrite_calls_in_expr(value, templates, worklist),
        TypedStmt::IndexAssign { index, value, .. } => {
            rewrite_calls_in_expr(index, templates, worklist);
            rewrite_calls_in_expr(value, templates, worklist);
        }
        TypedStmt::FieldAssign { value, .. } => rewrite_calls_in_expr(value, templates, worklist),
        TypedStmt::Return(Some(e), ..) => rewrite_calls_in_expr(e, templates, worklist),
        TypedStmt::Return(None, ..) | TypedStmt::Break(..) | TypedStmt::Continue(..) => {}
        TypedStmt::For { lo, hi, body, .. } => {
            rewrite_calls_in_expr(lo, templates, worklist);
            rewrite_calls_in_expr(hi, templates, worklist);
            rewrite_calls_in_block(body, templates, worklist);
        }
        TypedStmt::Expr(e) => rewrite_calls_in_expr(e, templates, worklist),
        TypedStmt::If { condition, then_branch, else_branch, .. } => {
            rewrite_calls_in_expr(condition, templates, worklist);
            rewrite_calls_in_block(then_branch, templates, worklist);
            if let Some(eb) = else_branch {
                rewrite_calls_in_block(eb, templates, worklist);
            }
        }
        TypedStmt::While { condition, body, .. } => {
            rewrite_calls_in_expr(condition, templates, worklist);
            rewrite_calls_in_block(body, templates, worklist);
        }
    }
}

fn rewrite_calls_in_block(
    block: &mut TypedBlock,
    templates: &HashMap<String, TypedFunction>,
    worklist: &mut Vec<(String, String, Subst)>,
) {
    for stmt in &mut block.stmts {
        rewrite_calls_in_stmt(stmt, templates, worklist);
    }
}

pub fn monomorphize(program: &mut TypedProgram) {
    let generic_templates: HashMap<String, TypedFunction> = program
        .functions
        .iter()
        .filter(|f| !f.type_params.is_empty())
        .map(|f| (f.name.clone(), f.clone()))
        .collect();

    if generic_templates.is_empty() {
        return;
    }

    let mut worklist = Vec::new();
    // Scan all non-generic functions
    for func in &mut program.functions {
        if func.type_params.is_empty() {
            rewrite_calls_in_block(&mut func.body, &generic_templates, &mut worklist);
        }
    }

    let mut instantiated: HashMap<String, TypedFunction> = HashMap::new();
    while let Some((base_name, mangled_name, subst)) = worklist.pop() {
        if instantiated.contains_key(&mangled_name) {
            continue;
        }
        if let Some(tmpl) = generic_templates.get(&base_name) {
            let mut specialized = tmpl.clone();
            specialized.name = mangled_name.clone();
            specialized.type_params.clear();
            specialized.return_ty = substitute_type(&specialized.return_ty, &subst);
            for p in &mut specialized.params {
                p.ty = substitute_type(&p.ty, &subst);
            }
            substitute_block(&mut specialized.body, &subst);
            rewrite_calls_in_block(&mut specialized.body, &generic_templates, &mut worklist);
            instantiated.insert(mangled_name, specialized);
        }
    }

    // Remove generic templates from program
    program.functions.retain(|f| f.type_params.is_empty());
    // Add specialized concrete functions
    for (_, func) in instantiated {
        program.functions.push(func);
    }
}

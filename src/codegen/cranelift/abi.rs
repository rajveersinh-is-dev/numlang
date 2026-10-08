//! Target machine configuration, calling conventions, layouts, and type translations.

use cranelift_codegen::ir::types;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

use crate::ast::{BinaryOp, UnaryOp};
use crate::typecheck::{Type, TypedBlock, TypedExpr, TypedLiteral, TypedStmt};

#[derive(Debug, Clone, Error)]
pub enum CodegenError {
    #[error("Cranelift codegen error: {0}")]
    BackendError(String),

    #[error("Variable '{0}' not found in scope")]
    VariableNotFound(String),

    #[error("Invalid array target: {0}")]
    InvalidArrayTarget(String),

    #[error("Struct layout missing for struct '{0}'")]
    MissingLayout(String),

    #[error("Field '{field}' not found in struct '{struct_name}'")]
    FieldNotFound { struct_name: String, field: String },

    #[error("Unsupported intrinsic or operation: {0}")]
    UnsupportedOp(String),
}

pub fn type_to_clif(ty: Type) -> types::Type {
    match ty {
        Type::I8 | Type::U8 => types::I8,
        Type::I16 | Type::U16 => types::I16,
        Type::I32 | Type::U32 => types::I32,
        Type::I64 | Type::U64 | Type::Usize => types::I64,
        Type::F32 => types::F32,
        Type::F64 => types::F64,
        Type::Bool => types::I8,
        Type::Void => types::I32,
        Type::Str => types::I64,
        Type::Array(_, _) => types::I64, // Pointer to array
        Type::Struct(_)
        | Type::Enum(_)
        | Type::Ptr(_)
        | Type::Fn(..)
        | Type::Closure(..)
        | Type::Box(_) => types::I64,
        Type::Param(_) => types::I64,
    }
}

#[derive(Debug, Clone)]
pub struct StructLayout {
    pub name: String,
    pub total_size: u32,
    pub align: u32,
    pub fields: HashMap<String, (u32, Type)>, // name -> (offset, type)
    pub ordered_fields: Vec<(String, Type, u32)>, // in definition order
}

impl StructLayout {
    pub fn get_leaf_fields(&self, layouts: &HashMap<String, StructLayout>) -> Vec<(u32, Type)> {
        let mut leaves = Vec::new();
        self.collect_leaf_fields(0, layouts, &mut leaves);
        leaves
    }

    fn collect_leaf_fields(
        &self,
        base_offset: u32,
        layouts: &HashMap<String, StructLayout>,
        out: &mut Vec<(u32, Type)>,
    ) {
        for (_, ty, offset) in &self.ordered_fields {
            let field_offset = base_offset + offset;
            if let Type::Struct(sname) = ty {
                if let Some(sub_layout) = layouts.get(sname) {
                    sub_layout.collect_leaf_fields(field_offset, layouts, out);
                }
            } else {
                out.push((field_offset, ty.clone()));
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumLayout {
    pub name: String,
    pub total_size: u32,
    pub align: u32,
    pub variants: HashMap<String, EnumVariantLayout>,
}

#[derive(Debug, Clone)]
pub struct EnumVariantLayout {
    pub name: String,
    pub tag: usize,
    pub field_offsets: Vec<u32>,
    pub payload_size: u32,
}

pub fn compute_type_layout(
    ty: &Type,
    layouts: &HashMap<String, StructLayout>,
    enum_layouts: &HashMap<String, EnumLayout>,
) -> (u32, u32) {
    match ty {
        Type::I8 | Type::U8 | Type::Bool => (1, 1),
        Type::I16 | Type::U16 => (2, 2),
        Type::I32 | Type::U32 | Type::F32 => (4, 4),
        Type::I64 | Type::U64 | Type::Usize | Type::F64 | Type::Str => (8, 8),
        Type::Void => (0, 1),
        Type::Array(elem, len) => {
            let (elem_sz, elem_al) = compute_type_layout(elem, layouts, enum_layouts);
            (elem_sz * (*len as u32), elem_al)
        }
        Type::Struct(sname) => {
            if let Some(l) = layouts.get(sname) {
                (l.total_size, l.align)
            } else {
                (8, 8)
            }
        }
        Type::Enum(ename) => {
            if let Some(l) = enum_layouts.get(ename) {
                (l.total_size, l.align)
            } else {
                (8, 8)
            }
        }
        Type::Ptr(_) | Type::Fn(..) | Type::Box(_) => (8, 8),
        Type::Closure(..) => (16, 8),
        Type::Param(_) => (0, 1),
    }
}

pub fn compute_struct_layouts(
    struct_defs: &[crate::typecheck::typed_ast::TypedStructDef],
) -> HashMap<String, StructLayout> {
    let mut layouts: HashMap<String, StructLayout> = HashMap::new();
    let empty_enums = HashMap::new();
    for sdef in struct_defs {
        let mut offset: u32 = 0;
        let mut max_align: u32 = 1;
        let mut fields = HashMap::new();
        let mut ordered_fields = Vec::new();

        for (fname, fty) in &sdef.fields {
            let (fsz, fal) = compute_type_layout(fty, &layouts, &empty_enums);
            max_align = max_align.max(fal);
            offset = (offset + fal - 1) & !(fal - 1);
            fields.insert(fname.clone(), (offset, fty.clone()));
            ordered_fields.push((fname.clone(), fty.clone(), offset));
            offset += fsz;
        }

        let total_size = if offset == 0 {
            1
        } else {
            (offset + max_align - 1) & !(max_align - 1)
        };
        layouts.insert(
            sdef.name.clone(),
            StructLayout {
                name: sdef.name.clone(),
                total_size,
                align: max_align,
                fields,
                ordered_fields,
            },
        );
    }
    layouts
}

pub fn compute_enum_layouts(
    enum_defs: &[crate::typecheck::typed_ast::TypedEnumDef],
    struct_layouts: &HashMap<String, StructLayout>,
) -> HashMap<String, EnumLayout> {
    let mut layouts: HashMap<String, EnumLayout> = HashMap::new();
    for edef in enum_defs {
        let mut max_variant_size: u32 = 8;
        let mut variants = HashMap::new();

        for variant in &edef.variants {
            let mut curr_offset: u32 = 8;
            let mut field_offsets = Vec::new();
            for field_ty in &variant.payload {
                let (fsz, fal) = compute_type_layout(field_ty, struct_layouts, &layouts);
                let fal = fal.max(1);
                curr_offset = (curr_offset + fal - 1) & !(fal - 1);
                field_offsets.push(curr_offset);
                curr_offset += fsz;
            }
            let payload_size = curr_offset - 8;
            max_variant_size = max_variant_size.max(curr_offset);
            variants.insert(
                variant.name.clone(),
                EnumVariantLayout {
                    name: variant.name.clone(),
                    tag: variant.tag,
                    field_offsets,
                    payload_size,
                },
            );
        }

        let total_size = ((max_variant_size + 7) & !7).max(8);
        layouts.insert(
            edef.name.clone(),
            EnumLayout {
                name: edef.name.clone(),
                total_size,
                align: 8,
                variants,
            },
        );
    }
    layouts
}

pub fn compute_magic_s64(d: i64) -> (i64, u8, bool) {
    let ad = d.unsigned_abs() as u128;
    let t = (1u128 << 63) + (if d < 0 { 1 } else { 0 });
    let anc = t - 1 - (t % ad);
    let mut p = 63u32;
    let mut q1 = (1u128 << p) / anc;
    let mut r1 = (1u128 << p) % anc;
    let mut q2 = (1u128 << p) / ad;
    let mut r2 = (1u128 << p) % ad;
    loop {
        p += 1;
        q1 *= 2;
        r1 *= 2;
        if r1 >= anc {
            q1 += 1;
            r1 -= anc;
        }
        q2 *= 2;
        r2 *= 2;
        if r2 >= ad {
            q2 += 1;
            r2 -= ad;
        }
        let delta = ad - r2;
        if !(q1 < delta || (q1 == delta && r1 == 0)) {
            break;
        }
    }
    let m = q2 + 1;
    let shift = (p - 64) as u8;
    let add_indicator = m >= (1u128 << 63);
    let m_signed = m as i64;
    (m_signed, shift, add_indicator)
}

pub fn compute_magic_u64_nonneg(d: u64) -> Option<(u64, u8)> {
    if d == 0 || d == 1 {
        return None;
    }
    for s in 0..64u8 {
        let p = 64 + (s as u32);
        if p <= 126 {
            let two_p = 1u128 << p;
            let q = two_p / (d as u128);
            let r = two_p % (d as u128);
            let m = if r == 0 { q } else { q + 1 };
            if m < (1u128 << 64) {
                let delta = if r == 0 { 0 } else { (d as u128) - r };
                if (delta << 63) <= two_p {
                    return Some((m as u64, s));
                }
            }
        }
    }
    None
}

pub fn compute_magic_u32_fast(d: u64) -> Option<(u64, u8)> {
    if d == 0 || d == 1 {
        return None;
    }
    for s in 32..64u8 {
        let two_s = 1u128 << s;
        let m = two_s.div_ceil(d as u128);
        if m < (1u128 << 32) {
            let rem = (m * (d as u128)) - two_s;
            let q_max = 0xFFFF_FFFFu128 / (d as u128);
            let r_max = (d as u128) - 1;
            if q_max * rem + r_max * m < two_s {
                return Some((m as u64, s));
            }
        }
    }
    None
}

pub fn get_constant_int(expr: &TypedExpr) -> Option<i64> {
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, _),
            ..
        } => Some(*val),
        TypedExpr::Unary {
            op: UnaryOp::Neg,
            expr,
            ..
        } => match &**expr {
            TypedExpr::Literal {
                lit: TypedLiteral::Int(val, _),
                ..
            } => Some(-val),
            _ => None,
        },
        _ => None,
    }
}

pub fn is_safe_for_select(expr: &TypedExpr) -> bool {
    match expr {
        TypedExpr::Literal { .. } | TypedExpr::Ident { .. } => true,
        TypedExpr::Unary { expr, .. } => is_safe_for_select(expr),
        TypedExpr::Binary {
            op, left, right, ..
        } => {
            if *op == BinaryOp::Div || *op == BinaryOp::Mod {
                match &**right {
                    TypedExpr::Literal {
                        lit: TypedLiteral::Int(d, _),
                        ..
                    } if *d != 0 => is_safe_for_select(left),
                    _ => false,
                }
            } else {
                is_safe_for_select(left) && is_safe_for_select(right)
            }
        }
        TypedExpr::Call { callee, args, .. } => match callee.as_str() {
            "tzcnt" | "ctz" | "clz" | "popcnt" | "rotl" | "rotr" | "isqrt" => {
                args.iter().all(is_safe_for_select)
            }
            _ => false,
        },
        _ => false,
    }
}

pub fn is_block_pure_scalar_updates(block: &TypedBlock) -> bool {
    if block.stmts.is_empty() {
        return true;
    }
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Assign { value, .. } => {
                if !is_safe_for_select(value) {
                    return false;
                }
            }
            TypedStmt::Let { value, .. } => {
                if !is_safe_for_select(value) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

pub fn collect_dynamically_indexed_arrays(body: &TypedBlock) -> HashSet<String> {
    let mut dynamic = HashSet::new();
    collect_dynamic_arrays_in_block(body, &mut dynamic);
    dynamic
}

pub fn collect_dynamic_arrays_in_block(block: &TypedBlock, dynamic: &mut HashSet<String>) {
    for stmt in &block.stmts {
        collect_dynamic_arrays_in_stmt(stmt, dynamic);
    }
}

pub fn collect_dynamic_arrays_in_stmt(stmt: &TypedStmt, dynamic: &mut HashSet<String>) {
    match stmt {
        TypedStmt::Let { value, .. } => collect_dynamic_arrays_in_expr(value, dynamic),
        TypedStmt::Assign { value, .. } => collect_dynamic_arrays_in_expr(value, dynamic),
        TypedStmt::IndexAssign {
            target,
            index,
            value,
            ..
        } => {
            if get_constant_int(index).is_none() {
                dynamic.insert(target.clone());
            }
            collect_dynamic_arrays_in_expr(index, dynamic);
            collect_dynamic_arrays_in_expr(value, dynamic);
        }
        TypedStmt::Expr(expr) => collect_dynamic_arrays_in_expr(expr, dynamic),
        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_dynamic_arrays_in_expr(condition, dynamic);
            collect_dynamic_arrays_in_block(then_branch, dynamic);
            if let Some(eb) = else_branch {
                collect_dynamic_arrays_in_block(eb, dynamic);
            }
        }
        TypedStmt::While {
            condition, body, ..
        } => {
            collect_dynamic_arrays_in_expr(condition, dynamic);
            collect_dynamic_arrays_in_block(body, dynamic);
        }
        TypedStmt::Return(opt_expr, _) => {
            if let Some(expr) = opt_expr {
                collect_dynamic_arrays_in_expr(expr, dynamic);
            }
        }
        TypedStmt::Break(_) | TypedStmt::Continue(_) => {}
        TypedStmt::For { lo, hi, body, .. } => {
            collect_dynamic_arrays_in_expr(lo, dynamic);
            collect_dynamic_arrays_in_expr(hi, dynamic);
            collect_dynamic_arrays_in_block(body, dynamic);
        }
        TypedStmt::FieldAssign { value, .. } => {
            collect_dynamic_arrays_in_expr(value, dynamic);
        }
    }
}

pub fn collect_dynamic_arrays_in_expr(expr: &TypedExpr, dynamic: &mut HashSet<String>) {
    match expr {
        TypedExpr::Index { target, index, .. } => {
            if let TypedExpr::Ident { name, .. } = target.as_ref() {
                if get_constant_int(index).is_none() {
                    dynamic.insert(name.clone());
                }
            }
            collect_dynamic_arrays_in_expr(target, dynamic);
            collect_dynamic_arrays_in_expr(index, dynamic);
        }
        TypedExpr::Binary { left, right, .. } => {
            collect_dynamic_arrays_in_expr(left, dynamic);
            collect_dynamic_arrays_in_expr(right, dynamic);
        }
        TypedExpr::Unary { expr, .. } => {
            collect_dynamic_arrays_in_expr(expr, dynamic);
        }
        TypedExpr::Call { args, .. } => {
            for a in args {
                collect_dynamic_arrays_in_expr(a, dynamic);
            }
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for el in elements {
                collect_dynamic_arrays_in_expr(el, dynamic);
            }
        }
        TypedExpr::StructLiteral { fields, .. } => {
            for (_, el) in fields {
                collect_dynamic_arrays_in_expr(el, dynamic);
            }
        }
        TypedExpr::FieldAccess { target, .. } => {
            collect_dynamic_arrays_in_expr(target, dynamic);
        }
        TypedExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_dynamic_arrays_in_expr(scrutinee, dynamic);
            for arm in arms {
                collect_dynamic_arrays_in_expr(&arm.body, dynamic);
            }
        }
        TypedExpr::EnumConstructor { args, .. } => {
            for a in args {
                collect_dynamic_arrays_in_expr(a, dynamic);
            }
        }
        TypedExpr::Lambda { body, .. } => {
            collect_dynamic_arrays_in_expr(body, dynamic);
        }
        TypedExpr::CallIndirect { callee, args, .. } => {
            collect_dynamic_arrays_in_expr(callee, dynamic);
            for a in args {
                collect_dynamic_arrays_in_expr(a, dynamic);
            }
        }
        TypedExpr::Box { inner, .. } | TypedExpr::Deref { inner, .. } => {
            collect_dynamic_arrays_in_expr(inner, dynamic);
        }
        TypedExpr::Ident { .. } | TypedExpr::Literal { .. } => {}
    }
}

pub fn is_known_positive(expr: &TypedExpr, non_negative_vars: &HashSet<String>) -> bool {
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, _),
            ..
        } => *val > 0,
        TypedExpr::Binary {
            op: BinaryOp::Add,
            left,
            right,
            ..
        } => {
            (is_known_positive(left, non_negative_vars)
                && is_expr_known_non_negative(right, non_negative_vars))
                || (is_expr_known_non_negative(left, non_negative_vars)
                    && is_known_positive(right, non_negative_vars))
        }
        _ => false,
    }
}

pub fn is_same_expr(a: &TypedExpr, b: &TypedExpr) -> bool {
    match (a, b) {
        (TypedExpr::Ident { name: na, .. }, TypedExpr::Ident { name: nb, .. }) => na == nb,
        (TypedExpr::Literal { lit: la, .. }, TypedExpr::Literal { lit: lb, .. }) => la == lb,
        (
            TypedExpr::Binary {
                op: oa,
                left: la,
                right: ra,
                ..
            },
            TypedExpr::Binary {
                op: ob,
                left: lb,
                right: rb,
                ..
            },
        ) => oa == ob && is_same_expr(la, lb) && is_same_expr(ra, rb),
        (
            TypedExpr::Unary {
                op: oa, expr: ea, ..
            },
            TypedExpr::Unary {
                op: ob, expr: eb, ..
            },
        ) => oa == ob && is_same_expr(ea, eb),
        _ => false,
    }
}

pub fn is_expr_square_or_nonneg(e: &TypedExpr, non_negative_vars: &HashSet<String>) -> bool {
    if is_expr_known_non_negative(e, non_negative_vars) {
        return true;
    }
    match e {
        TypedExpr::Binary {
            op: BinaryOp::Mul,
            left,
            right,
            ..
        } => is_same_expr(left, right),
        _ => false,
    }
}

pub fn is_expr_known_non_negative(expr: &TypedExpr, non_negative_vars: &HashSet<String>) -> bool {
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, _),
            ..
        } => *val >= 0,
        TypedExpr::Literal {
            lit: TypedLiteral::Bool(_),
            ..
        } => true,
        TypedExpr::Ident { name, .. } => non_negative_vars.contains(name),
        TypedExpr::Binary {
            op, left, right, ..
        } => match op {
            BinaryOp::Add => {
                (is_expr_known_non_negative(left, non_negative_vars)
                    && is_expr_known_non_negative(right, non_negative_vars))
                    || (is_expr_square_or_nonneg(left, non_negative_vars)
                        && is_expr_square_or_nonneg(right, non_negative_vars))
            }
            BinaryOp::Mul => {
                (is_expr_known_non_negative(left, non_negative_vars)
                    && is_expr_known_non_negative(right, non_negative_vars))
                    || is_same_expr(left, right)
            }
            BinaryOp::Div => {
                is_expr_known_non_negative(left, non_negative_vars)
                    && (is_expr_known_non_negative(right, non_negative_vars)
                        || is_known_positive(right, non_negative_vars))
            }
            BinaryOp::Mod => is_expr_known_non_negative(left, non_negative_vars),
            BinaryOp::BitAnd => {
                // If either operand has sign bit 0 (is non-negative), bit 63 of result is 0
                is_expr_known_non_negative(left, non_negative_vars)
                    || is_expr_known_non_negative(right, non_negative_vars)
            }
            BinaryOp::BitOr | BinaryOp::BitXor => {
                is_expr_known_non_negative(left, non_negative_vars)
                    && is_expr_known_non_negative(right, non_negative_vars)
            }
            BinaryOp::Shr | BinaryOp::Shl => is_expr_known_non_negative(left, non_negative_vars),
            _ => false,
        },
        TypedExpr::Unary {
            op: UnaryOp::Not, ..
        } => true,
        TypedExpr::Call { callee, .. } => {
            callee == "abs"
                || callee == "sqrt"
                || callee == "isqrt"
                || callee == "ctz"
                || callee == "tzcnt"
                || callee == "clz"
                || callee == "popcnt"
        }
        TypedExpr::ArrayLiteral { elements, .. } => elements
            .iter()
            .all(|e| is_expr_known_non_negative(e, non_negative_vars)),
        TypedExpr::Index { target, .. } => is_expr_known_non_negative(target, non_negative_vars),
        _ => false,
    }
}

pub fn get_nonneg_var_from_condition(
    condition: &TypedExpr,
    known: &HashSet<String>,
) -> Option<String> {
    match condition {
        TypedExpr::Binary {
            op, left, right, ..
        } => match op {
            BinaryOp::Gt | BinaryOp::Ge => {
                if let TypedExpr::Ident { name: l_name, .. } = &**left {
                    if is_expr_known_non_negative(right, known) {
                        return Some(l_name.clone());
                    }
                }
                None
            }
            BinaryOp::Lt | BinaryOp::Le => {
                if let TypedExpr::Ident { name: r_name, .. } = &**right {
                    if is_expr_known_non_negative(left, known) {
                        return Some(r_name.clone());
                    }
                }
                None
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn collect_known_non_negative_vars(body: &TypedBlock) -> HashSet<String> {
    let mut candidates: HashSet<String> = HashSet::new();
    collect_initial_nonneg_candidates_block(body, &mut candidates);

    loop {
        let mut to_remove = Vec::new();
        for var in &candidates {
            if !all_assignments_are_nonneg_in_block(body, var, &candidates) {
                to_remove.push(var.clone());
            }
        }
        if to_remove.is_empty() {
            break;
        }
        for var in to_remove {
            candidates.remove(&var);
        }
    }

    candidates
}

pub fn collect_initial_nonneg_candidates_block(
    block: &TypedBlock,
    candidates: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                if is_expr_known_non_negative(value, candidates) {
                    candidates.insert(name.clone());
                }
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if then_branch
                    .stmts
                    .iter()
                    .any(|s| matches!(s, TypedStmt::Return(..)))
                {
                    if let TypedExpr::Binary {
                        op, left, right, ..
                    } = condition
                    {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_non_negative(right, candidates) {
                                    candidates.insert(name.clone());
                                }
                            }
                        }
                    }
                }
                collect_initial_nonneg_candidates_block(then_branch, candidates);
                if let Some(eb) = else_branch {
                    let mut else_candidates = candidates.clone();
                    if let TypedExpr::Binary {
                        op, left, right, ..
                    } = condition
                    {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_non_negative(right, &else_candidates) {
                                    else_candidates.insert(name.clone());
                                }
                            }
                        }
                    }
                    collect_initial_nonneg_candidates_block(eb, &mut else_candidates);
                    for v in else_candidates {
                        candidates.insert(v);
                    }
                }
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                let mut while_candidates = candidates.clone();
                if let Some(v) = get_nonneg_var_from_condition(condition, &while_candidates) {
                    while_candidates.insert(v);
                }
                collect_initial_nonneg_candidates_block(body, &mut while_candidates);
                for v in while_candidates {
                    candidates.insert(v);
                }
            }
            _ => {}
        }
    }
}

pub fn all_assignments_are_nonneg_in_block(
    block: &TypedBlock,
    var: &str,
    candidates: &HashSet<String>,
) -> bool {
    let mut current_candidates = candidates.clone();
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                let nonneg = is_expr_known_non_negative(value, &current_candidates);
                if name == var && !nonneg {
                    return false;
                }
                if nonneg {
                    current_candidates.insert(name.clone());
                } else {
                    current_candidates.remove(name);
                }
            }
            TypedStmt::Assign { name, value, .. } => {
                let nonneg = is_expr_known_non_negative(value, &current_candidates);
                if name == var && !nonneg {
                    return false;
                }
                if nonneg {
                    current_candidates.insert(name.clone());
                } else {
                    current_candidates.remove(name);
                }
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if !all_assignments_are_nonneg_in_block(then_branch, var, &current_candidates) {
                    return false;
                }
                if let Some(eb) = else_branch {
                    let mut else_candidates = current_candidates.clone();
                    if let TypedExpr::Binary {
                        op, left, right, ..
                    } = condition
                    {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_non_negative(right, &else_candidates) {
                                    else_candidates.insert(name.clone());
                                }
                            }
                        }
                    }
                    if !all_assignments_are_nonneg_in_block(eb, var, &else_candidates) {
                        return false;
                    }
                }
                if then_branch
                    .stmts
                    .iter()
                    .any(|s| matches!(s, TypedStmt::Return(..)))
                {
                    if let TypedExpr::Binary {
                        op, left, right, ..
                    } = condition
                    {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_non_negative(right, &current_candidates) {
                                    current_candidates.insert(name.clone());
                                }
                            }
                        }
                    }
                }
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                let mut while_candidates = current_candidates.clone();
                if let Some(v) = get_nonneg_var_from_condition(condition, &while_candidates) {
                    while_candidates.insert(v);
                }
                if !all_assignments_are_nonneg_in_block(body, var, &while_candidates) {
                    return false;
                }
            }
            TypedStmt::IndexAssign { target, value, .. } => {
                let nonneg = is_expr_known_non_negative(value, &current_candidates);
                if target == var && !nonneg {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn is_expr_known_u32(
    expr: &TypedExpr,
    non_negative_vars: &HashSet<String>,
    u32_vars: &HashSet<String>,
) -> bool {
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, _),
            ..
        } => *val >= 0 && (*val as u64) <= 0xFFFF_FFFF,
        TypedExpr::Ident { name, .. } => u32_vars.contains(name),
        TypedExpr::Binary {
            op, left, right, ..
        } => match op {
            BinaryOp::Mod => {
                if let Some(d) = get_constant_int(right) {
                    d > 0
                        && (d as u64) <= 0x1_0000_0000
                        && is_expr_known_non_negative(left, non_negative_vars)
                } else {
                    is_expr_known_non_negative(left, non_negative_vars)
                        && is_expr_known_u32(right, non_negative_vars, u32_vars)
                }
            }
            BinaryOp::Div => {
                if is_expr_known_u32(left, non_negative_vars, u32_vars)
                    && (is_known_positive(right, non_negative_vars)
                        || is_expr_known_non_negative(right, non_negative_vars))
                {
                    true
                } else if let Some(d) = get_constant_int(right) {
                    if d >= 2 {
                        if let TypedExpr::Binary {
                            op: BinaryOp::Add,
                            left: a,
                            right: b,
                            ..
                        } = &**left
                        {
                            is_expr_known_u32(a, non_negative_vars, u32_vars)
                                && is_expr_known_u32(b, non_negative_vars, u32_vars)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            BinaryOp::Add => {
                if let Some(c) = get_constant_int(right) {
                    if c >= 0 {
                        match &**left {
                            TypedExpr::Binary {
                                op: BinaryOp::Mod,
                                right: mod_r,
                                ..
                            } => {
                                if let Some(d) = get_constant_int(mod_r) {
                                    if d > 0 && ((d as u64) + (c as u64) <= 0x1_0000_0000) {
                                        return true;
                                    }
                                }
                            }
                            TypedExpr::Literal {
                                lit: TypedLiteral::Int(v, _),
                                ..
                            } if *v >= 0 && ((*v as u64) + (c as u64) <= 0xFFFF_FFFF) => {
                                return true;
                            }
                            _ => {}
                        }
                    }
                    c == 0 && is_expr_known_u32(left, non_negative_vars, u32_vars)
                } else if let Some(c) = get_constant_int(left) {
                    if c >= 0 {
                        match &**right {
                            TypedExpr::Binary {
                                op: BinaryOp::Mod,
                                right: mod_r,
                                ..
                            } => {
                                if let Some(d) = get_constant_int(mod_r) {
                                    if d > 0 && ((d as u64) + (c as u64) <= 0x1_0000_0000) {
                                        return true;
                                    }
                                }
                            }
                            TypedExpr::Literal {
                                lit: TypedLiteral::Int(v, _),
                                ..
                            } if *v >= 0 && ((*v as u64) + (c as u64) <= 0xFFFF_FFFF) => {
                                return true;
                            }
                            _ => {}
                        }
                    }
                    c == 0 && is_expr_known_u32(right, non_negative_vars, u32_vars)
                } else {
                    false
                }
            }
            BinaryOp::Mul => {
                if let Some(c) = get_constant_int(right) {
                    c == 0 || (c == 1 && is_expr_known_u32(left, non_negative_vars, u32_vars))
                } else if let Some(c) = get_constant_int(left) {
                    c == 0 || (c == 1 && is_expr_known_u32(right, non_negative_vars, u32_vars))
                } else {
                    false
                }
            }
            BinaryOp::BitAnd => {
                is_expr_known_u32(left, non_negative_vars, u32_vars)
                    || is_expr_known_u32(right, non_negative_vars, u32_vars)
            }
            BinaryOp::Shr => {
                if is_expr_known_u32(left, non_negative_vars, u32_vars) {
                    true
                } else if let Some(s) = get_constant_int(right) {
                    if s >= 1 {
                        if let TypedExpr::Binary {
                            op: BinaryOp::Add,
                            left: a,
                            right: b,
                            ..
                        } = &**left
                        {
                            is_expr_known_u32(a, non_negative_vars, u32_vars)
                                && is_expr_known_u32(b, non_negative_vars, u32_vars)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        },
        TypedExpr::ArrayLiteral { elements, .. } => elements
            .iter()
            .all(|e| is_expr_known_u32(e, non_negative_vars, u32_vars)),
        TypedExpr::Index { target, .. } => is_expr_known_u32(target, non_negative_vars, u32_vars),
        _ => false,
    }
}

pub fn collect_known_u32_vars(
    body: &TypedBlock,
    non_negative_vars: &HashSet<String>,
) -> HashSet<String> {
    let mut candidates: HashSet<String> = HashSet::new();
    collect_initial_u32_candidates_block(body, non_negative_vars, &mut candidates);

    loop {
        let mut to_remove = Vec::new();
        for var in &candidates {
            if !all_assignments_are_u32_in_block(body, var, non_negative_vars, &candidates) {
                to_remove.push(var.clone());
            }
        }
        if to_remove.is_empty() {
            break;
        }
        for var in to_remove {
            candidates.remove(&var);
        }
    }

    candidates
}

pub fn collect_initial_u32_candidates_block(
    block: &TypedBlock,
    non_negative_vars: &HashSet<String>,
    candidates: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                if is_expr_known_u32(value, non_negative_vars, candidates) {
                    candidates.insert(name.clone());
                }
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                collect_initial_u32_candidates_block(then_branch, non_negative_vars, candidates);
                if let Some(eb) = else_branch {
                    let mut else_candidates = candidates.clone();
                    if let TypedExpr::Binary {
                        op, left, right, ..
                    } = condition
                    {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_u32(right, non_negative_vars, &else_candidates) {
                                    else_candidates.insert(name.clone());
                                }
                            }
                        }
                    }
                    collect_initial_u32_candidates_block(
                        eb,
                        non_negative_vars,
                        &mut else_candidates,
                    );
                    for v in else_candidates {
                        candidates.insert(v);
                    }
                }
            }
            TypedStmt::While { body, .. } => {
                let mut while_candidates = candidates.clone();
                collect_initial_u32_candidates_block(
                    body,
                    non_negative_vars,
                    &mut while_candidates,
                );
                for v in while_candidates {
                    candidates.insert(v);
                }
            }
            _ => {}
        }
    }
}

pub fn all_assignments_are_u32_in_block(
    block: &TypedBlock,
    var: &str,
    non_negative_vars: &HashSet<String>,
    candidates: &HashSet<String>,
) -> bool {
    let mut current_candidates = candidates.clone();
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                let is_u32 = is_expr_known_u32(value, non_negative_vars, &current_candidates);
                if name == var && !is_u32 {
                    return false;
                }
                if is_u32 {
                    current_candidates.insert(name.clone());
                } else {
                    current_candidates.remove(name);
                }
            }
            TypedStmt::Assign { name, value, .. } => {
                let is_u32 = is_expr_known_u32(value, non_negative_vars, &current_candidates);
                if name == var && !is_u32 {
                    return false;
                }
                if is_u32 {
                    current_candidates.insert(name.clone());
                } else {
                    current_candidates.remove(name);
                }
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if !all_assignments_are_u32_in_block(
                    then_branch,
                    var,
                    non_negative_vars,
                    &current_candidates,
                ) {
                    return false;
                }
                if let Some(eb) = else_branch {
                    let mut else_candidates = current_candidates.clone();
                    if let TypedExpr::Binary {
                        op, left, right, ..
                    } = condition
                    {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_u32(right, non_negative_vars, &else_candidates) {
                                    else_candidates.insert(name.clone());
                                }
                            }
                        }
                    }
                    if !all_assignments_are_u32_in_block(
                        eb,
                        var,
                        non_negative_vars,
                        &else_candidates,
                    ) {
                        return false;
                    }
                }
            }
            TypedStmt::While { body, .. } => {
                if !all_assignments_are_u32_in_block(
                    body,
                    var,
                    non_negative_vars,
                    &current_candidates,
                ) {
                    return false;
                }
            }
            TypedStmt::IndexAssign { target, value, .. } => {
                let is_u32 = is_expr_known_u32(value, non_negative_vars, &current_candidates);
                if target == var && !is_u32 {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn compute_expr_upper_bound(
    expr: &TypedExpr,
    var_bounds: &HashMap<String, i64>,
    non_negative_vars: &HashSet<String>,
) -> Option<i64> {
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, _),
            ..
        } => {
            if *val >= 0 {
                Some(*val)
            } else {
                None
            }
        }
        TypedExpr::Literal {
            lit: TypedLiteral::Bool(_),
            ..
        } => Some(1),
        TypedExpr::Ident { name, .. } => var_bounds.get(name).copied(),
        TypedExpr::Binary {
            op, left, right, ..
        } => match op {
            BinaryOp::Mod => {
                if let Some(d) = get_constant_int(right) {
                    if d > 0 && is_expr_known_non_negative(left, non_negative_vars) {
                        return Some(d - 1);
                    }
                }
                None
            }
            BinaryOp::Add => {
                let l_bound = compute_expr_upper_bound(left, var_bounds, non_negative_vars)?;
                let r_bound = compute_expr_upper_bound(right, var_bounds, non_negative_vars)?;
                l_bound.checked_add(r_bound)
            }
            BinaryOp::Sub => {
                let l_bound = compute_expr_upper_bound(left, var_bounds, non_negative_vars)?;
                if is_expr_known_non_negative(right, non_negative_vars) {
                    Some(l_bound)
                } else {
                    None
                }
            }
            BinaryOp::Mul => {
                let l_bound = compute_expr_upper_bound(left, var_bounds, non_negative_vars)?;
                let r_bound = compute_expr_upper_bound(right, var_bounds, non_negative_vars)?;
                l_bound.checked_mul(r_bound)
            }
            BinaryOp::Div => {
                let l_bound = compute_expr_upper_bound(left, var_bounds, non_negative_vars)?;
                if let Some(d) = get_constant_int(right) {
                    if d > 0 {
                        return Some(l_bound / d);
                    }
                }
                if is_known_positive(right, non_negative_vars)
                    || is_expr_known_non_negative(right, non_negative_vars)
                {
                    return Some(l_bound);
                }
                None
            }
            BinaryOp::BitAnd => {
                let l_bound = compute_expr_upper_bound(left, var_bounds, non_negative_vars);
                let r_bound = compute_expr_upper_bound(right, var_bounds, non_negative_vars);
                match (l_bound, r_bound) {
                    (Some(l), Some(r)) => Some(l.min(r)),
                    (Some(l), None) => Some(l),
                    (None, Some(r)) => Some(r),
                    (None, None) => None,
                }
            }
            BinaryOp::Shr => {
                let l_bound = compute_expr_upper_bound(left, var_bounds, non_negative_vars)?;
                if let Some(s) = get_constant_int(right) {
                    if (0..64).contains(&s) {
                        return Some(l_bound >> s);
                    }
                }
                Some(l_bound)
            }
            _ => None,
        },
        TypedExpr::Call { callee, args, .. } => match callee.as_str() {
            "abs" => {
                if args.len() == 1 {
                    compute_expr_abs_upper_bound(&args[0], var_bounds, non_negative_vars)
                } else {
                    None
                }
            }
            "tzcnt" | "ctz" => Some(64),
            "clz" => Some(64),
            "popcnt" => Some(64),
            "isqrt" => {
                if args.len() == 1 {
                    let ub = compute_expr_upper_bound(&args[0], var_bounds, non_negative_vars)?;
                    Some((ub as f64).sqrt() as i64)
                } else {
                    None
                }
            }
            "min" => {
                if args.len() == 2 {
                    let ub0 = compute_expr_upper_bound(&args[0], var_bounds, non_negative_vars);
                    let ub1 = compute_expr_upper_bound(&args[1], var_bounds, non_negative_vars);
                    match (ub0, ub1) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        (None, None) => None,
                    }
                } else {
                    None
                }
            }
            "max" => {
                if args.len() == 2 {
                    let ub0 = compute_expr_upper_bound(&args[0], var_bounds, non_negative_vars)?;
                    let ub1 = compute_expr_upper_bound(&args[1], var_bounds, non_negative_vars)?;
                    Some(ub0.max(ub1))
                } else {
                    None
                }
            }
            _ => None,
        },
        TypedExpr::ArrayLiteral { elements, .. } => {
            let mut max_el: Option<i64> = None;
            for el in elements {
                let b = compute_expr_upper_bound(el, var_bounds, non_negative_vars)?;
                max_el = Some(max_el.map_or(b, |m| m.max(b)));
            }
            max_el
        }
        TypedExpr::Index { .. } => None,
        _ => None,
    }
}

pub fn compute_expr_abs_upper_bound(
    expr: &TypedExpr,
    var_bounds: &HashMap<String, i64>,
    non_negative_vars: &HashSet<String>,
) -> Option<i64> {
    if is_expr_known_non_negative(expr, non_negative_vars) {
        return compute_expr_upper_bound(expr, var_bounds, non_negative_vars);
    }
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, _),
            ..
        } => Some(val.unsigned_abs() as i64),
        TypedExpr::Ident { name, .. } => var_bounds.get(name).copied(),
        TypedExpr::Binary {
            op: BinaryOp::Sub,
            left,
            right,
            ..
        } => {
            if is_expr_known_non_negative(left, non_negative_vars)
                && is_expr_known_non_negative(right, non_negative_vars)
            {
                let l_bound = compute_expr_upper_bound(left, var_bounds, non_negative_vars)?;
                let r_bound = compute_expr_upper_bound(right, var_bounds, non_negative_vars)?;
                Some(l_bound.max(r_bound))
            } else {
                None
            }
        }
        TypedExpr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
            ..
        } => compute_expr_abs_upper_bound(inner, var_bounds, non_negative_vars),
        _ => None,
    }
}

pub fn collect_mutated_vars_in_block(block: &TypedBlock, mutated: &mut HashSet<String>) {
    for s in &block.stmts {
        match s {
            TypedStmt::Assign { name, .. } => {
                mutated.insert(name.clone());
            }
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_mutated_vars_in_block(then_branch, mutated);
                if let Some(eb) = else_branch {
                    collect_mutated_vars_in_block(eb, mutated);
                }
            }
            TypedStmt::While { body, .. } => {
                collect_mutated_vars_in_block(body, mutated);
            }
            _ => {}
        }
    }
}

pub fn collect_known_var_upper_bounds(
    body: &TypedBlock,
    non_negative_vars: &HashSet<String>,
) -> HashMap<String, i64> {
    let mut bounds: HashMap<String, i64> = HashMap::new();
    for _ in 0..8 {
        let mut changed = false;
        collect_bounds_in_block(body, non_negative_vars, &mut bounds, &mut changed);
        if !changed {
            break;
        }
    }
    bounds
}

pub fn collect_bounds_in_block(
    block: &TypedBlock,
    non_negative_vars: &HashSet<String>,
    bounds: &mut HashMap<String, i64>,
    changed: &mut bool,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                if let Some(ub) = compute_expr_upper_bound(value, bounds, non_negative_vars) {
                    match bounds.get(name) {
                        Some(&prev) if prev >= ub => {}
                        Some(&prev) => {
                            let new_val = prev.max(ub);
                            bounds.insert(name.clone(), new_val);
                            *changed = true;
                        }
                        None => {
                            bounds.insert(name.clone(), ub);
                            *changed = true;
                        }
                    }
                } else if bounds.remove(name).is_some() {
                    *changed = true;
                }
            }
            TypedStmt::Assign { name, value, .. } => {
                if let Some(ub) = compute_expr_upper_bound(value, bounds, non_negative_vars) {
                    match bounds.get(name) {
                        Some(&prev) if prev >= ub => {}
                        Some(&prev) => {
                            let new_val = prev.max(ub);
                            bounds.insert(name.clone(), new_val);
                            *changed = true;
                        }
                        None => {
                            bounds.insert(name.clone(), ub);
                            *changed = true;
                        }
                    }
                } else if bounds.remove(name).is_some() {
                    *changed = true;
                }
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let orig_bounds = bounds.clone();
                let mut then_bounds = bounds.clone();
                if let TypedExpr::Binary {
                    op, left, right, ..
                } = condition
                {
                    if let TypedExpr::Ident { name, .. } = &**left {
                        if let Some(c) = get_constant_int(right) {
                            if *op == BinaryOp::Lt && c > 0 {
                                then_bounds
                                    .entry(name.clone())
                                    .and_modify(|old| *old = (*old).min(c - 1))
                                    .or_insert(c - 1);
                            } else if *op == BinaryOp::Le && c >= 0 {
                                then_bounds
                                    .entry(name.clone())
                                    .and_modify(|old| *old = (*old).min(c))
                                    .or_insert(c);
                            }
                        }
                    }
                }
                collect_bounds_in_block(then_branch, non_negative_vars, &mut then_bounds, changed);

                if let Some(eb) = else_branch {
                    let mut else_bounds = orig_bounds.clone();
                    collect_bounds_in_block(eb, non_negative_vars, &mut else_bounds, changed);

                    let mut merged_bounds = HashMap::new();
                    let all_vars: HashSet<String> = then_bounds
                        .keys()
                        .chain(else_bounds.keys())
                        .cloned()
                        .collect();
                    for v in all_vars {
                        let t_b = then_bounds
                            .get(&v)
                            .copied()
                            .or_else(|| orig_bounds.get(&v).copied());
                        let e_b = else_bounds
                            .get(&v)
                            .copied()
                            .or_else(|| orig_bounds.get(&v).copied());
                        if let (Some(t), Some(e)) = (t_b, e_b) {
                            merged_bounds.insert(v, t.max(e));
                        }
                    }
                    *bounds = merged_bounds;
                } else {
                    let mut merged_bounds = orig_bounds.clone();
                    for (v, t_b) in then_bounds {
                        if let Some(&orig_b) = orig_bounds.get(&v) {
                            merged_bounds.insert(v, t_b.max(orig_b));
                        }
                    }
                    *bounds = merged_bounds;
                }
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                let mut cond_bounded_var = None;
                let mut cond_bound = None;
                let mut exit_bound = None;
                if let TypedExpr::Binary {
                    op, left, right, ..
                } = condition
                {
                    if let TypedExpr::Ident { name, .. } = &**left {
                        let limit = get_constant_int(right)
                            .or_else(|| compute_expr_upper_bound(right, bounds, non_negative_vars));
                        if let Some(c) = limit {
                            if *op == BinaryOp::Lt && c > 0 {
                                cond_bounded_var = Some(name.clone());
                                cond_bound = Some(c - 1);
                                exit_bound = Some(c);
                            } else if *op == BinaryOp::Le && c >= 0 {
                                cond_bounded_var = Some(name.clone());
                                cond_bound = Some(c);
                                exit_bound = Some(c + 1);
                            }
                        }
                    }
                }
                let mut loop_mutated = HashSet::new();
                collect_mutated_vars_in_block(body, &mut loop_mutated);

                // Pre-loop bounds before entering the loop
                let pre_loop_bounds = bounds.clone();

                // Before analyzing body: loop-mutated variables cannot retain their pre-loop values unconditionally
                for var in &loop_mutated {
                    if Some(var) == cond_bounded_var.as_ref() {
                        if let Some(cb) = cond_bound {
                            bounds.insert(var.clone(), cb);
                        }
                    } else {
                        bounds.remove(var);
                    }
                }

                collect_bounds_in_block(body, non_negative_vars, bounds, changed);

                for var in loop_mutated {
                    if Some(&var) == cond_bounded_var.as_ref() {
                        if let Some(eb) = exit_bound {
                            bounds.insert(var, eb);
                        }
                        continue;
                    }
                    if let Some(&new_b) = bounds.get(&var) {
                        if let Some(&old_b) = pre_loop_bounds.get(&var) {
                            let merged_b = old_b.max(new_b);
                            bounds.insert(var, merged_b);
                        } else {
                            bounds.insert(var, new_b);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn collect_constant_divisors_block(block: &TypedBlock, out: &mut Vec<i64>) {
    for stmt in &block.stmts {
        collect_constant_divisors_stmt(stmt, out);
    }
}

pub fn collect_constant_divisors_stmt(stmt: &TypedStmt, out: &mut Vec<i64>) {
    match stmt {
        TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } => {
            collect_constant_divisors_expr(value, out);
        }
        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_constant_divisors_expr(condition, out);
            collect_constant_divisors_block(then_branch, out);
            if let Some(eb) = else_branch {
                collect_constant_divisors_block(eb, out);
            }
        }
        TypedStmt::While {
            condition, body, ..
        } => {
            collect_constant_divisors_expr(condition, out);
            collect_constant_divisors_block(body, out);
        }
        TypedStmt::Return(Some(expr), _) => {
            collect_constant_divisors_expr(expr, out);
        }
        TypedStmt::Expr(expr) => {
            collect_constant_divisors_expr(expr, out);
        }
        _ => {}
    }
}

pub fn collect_constant_divisors_expr(expr: &TypedExpr, out: &mut Vec<i64>) {
    match expr {
        TypedExpr::Binary {
            op, left, right, ..
        } => {
            if *op == BinaryOp::Div || *op == BinaryOp::Mod {
                if let Some(d) = get_constant_int(right) {
                    if d != 0 && !out.contains(&d) {
                        out.push(d);
                    }
                }
            }
            collect_constant_divisors_expr(left, out);
            collect_constant_divisors_expr(right, out);
        }
        TypedExpr::Unary { expr, .. } => collect_constant_divisors_expr(expr, out),
        TypedExpr::Call { args, .. } => {
            for arg in args {
                collect_constant_divisors_expr(arg, out);
            }
        }
        _ => {}
    }
}

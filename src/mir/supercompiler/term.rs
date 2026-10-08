use std::collections::HashMap;
use std::fmt;

use crate::ast::{BinaryOp, UnaryOp};
use crate::mir::{BasicBlockId, Place};
use crate::typecheck::typed_ast::TypedLiteral;
use crate::typecheck::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymTermId(pub usize);

impl fmt::Display for SymTermId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymTerm {
    ConstInt(i64, Type),
    ConstFloat(u64, Type), // f64::to_bits()
    ConstBool(bool),
    ConstStr(String),
    Var(Place, Type),
    Binary(BinaryOp, SymTermId, SymTermId, Type),
    Unary(UnaryOp, SymTermId, Type),
    Constructor(String, usize, Vec<SymTermId>, Type),
    Select(SymTermId, SymTermId, SymTermId, Type), // condition, then, else
    Phi(Vec<(BasicBlockId, SymTermId)>, Type),
    Call(String, Vec<SymTermId>, Type),
    /// A reference to a heap-allocated value (symbolic address).
    Ref(SymTermId, Type),
    /// A dereference of a symbolic pointer.
    Deref(SymTermId, Type),
    /// The discriminant (variant tag) of an enum value.
    Discriminant(SymTermId, Type),
    /// A symbolically known closure: fn_name + captured symbolic terms.
    ClosureVal(String, Vec<SymTermId>, Type),
    /// An unevaluated thunk: suspended body + captured symbolic terms.
    Thunk(String, Vec<SymTermId>, Type),
}

impl SymTerm {
    pub fn ty(&self) -> &Type {
        match self {
            SymTerm::ConstInt(_, ty) => ty,
            SymTerm::ConstFloat(_, ty) => ty,
            SymTerm::ConstBool(_) => &Type::Bool,
            SymTerm::ConstStr(_) => &Type::Str,
            SymTerm::Var(_, ty) => ty,
            SymTerm::Binary(_, _, _, ty) => ty,
            SymTerm::Unary(_, _, ty) => ty,
            SymTerm::Constructor(_, _, _, ty) => ty,
            SymTerm::Select(_, _, _, ty) => ty,
            SymTerm::Phi(_, ty) => ty,
            SymTerm::Call(_, _, ty) => ty,
            SymTerm::Ref(_, ty) => ty,
            SymTerm::Deref(_, ty) => ty,
            SymTerm::Discriminant(_, ty) => ty,
            SymTerm::ClosureVal(_, _, ty) => ty,
            SymTerm::Thunk(_, _, ty) => ty,
        }
    }

    pub fn to_literal(&self) -> Option<TypedLiteral> {
        match self {
            SymTerm::ConstInt(val, ty) => Some(TypedLiteral::Int(*val, ty.clone())),
            SymTerm::ConstFloat(bits, ty) => {
                Some(TypedLiteral::Float(f64::from_bits(*bits), ty.clone()))
            }
            SymTerm::ConstBool(b) => Some(TypedLiteral::Bool(*b)),
            SymTerm::ConstStr(s) => Some(TypedLiteral::Str(s.clone())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TermInterner {
    terms: Vec<SymTerm>,
    lookup: HashMap<SymTerm, SymTermId>,
    sizes: Vec<usize>,
    depths: Vec<usize>,
    hashes: Vec<u64>,
}

const FX_K: u64 = 0x517cc1b727220a95;

#[inline]
fn fx_hash_step(hash: u64, val: u64) -> u64 {
    hash.rotate_left(5) ^ val.wrapping_mul(FX_K)
}

fn fx_hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash = fx_hash_step(hash, b as u64);
    }
    hash
}

fn hash_place(mut h: u64, p: &Place) -> u64 {
    h = fx_hash_bytes(h, p.local.as_bytes());
    for proj in &p.projections {
        match proj {
            crate::mir::Projection::Deref => {
                h = fx_hash_step(h, 0xd37ef);
            }
            crate::mir::Projection::Field(f) => {
                h = fx_hash_bytes(fx_hash_step(h, 0xf1e1d), f.as_bytes());
            }
            crate::mir::Projection::Index(idx) => {
                h = hash_place(fx_hash_step(h, 0x14d3c), idx);
            }
            crate::mir::Projection::Payload(idx) => {
                h = fx_hash_step(fx_hash_step(h, 0x9a710ad), *idx as u64);
            }
        }
    }
    h
}

fn binary_op_discriminant(op: BinaryOp) -> u64 {
    match op {
        BinaryOp::Add => 1,
        BinaryOp::Sub => 2,
        BinaryOp::Mul => 3,
        BinaryOp::Div => 4,
        BinaryOp::Mod => 5,
        BinaryOp::Pow => 6,
        BinaryOp::BitAnd => 7,
        BinaryOp::BitOr => 8,
        BinaryOp::BitXor => 9,
        BinaryOp::Shl => 10,
        BinaryOp::Shr => 11,
        BinaryOp::Eq => 12,
        BinaryOp::Ne => 13,
        BinaryOp::Lt => 14,
        BinaryOp::Le => 15,
        BinaryOp::Gt => 16,
        BinaryOp::Ge => 17,
    }
}

fn unary_op_discriminant(op: UnaryOp) -> u64 {
    match op {
        UnaryOp::Neg => 1,
        UnaryOp::Not => 2,
    }
}

impl TermInterner {
    pub fn new() -> Self {
        TermInterner {
            terms: Vec::new(),
            lookup: HashMap::new(),
            sizes: Vec::new(),
            depths: Vec::new(),
            hashes: Vec::new(),
        }
    }

    pub fn get(&self, id: SymTermId) -> &SymTerm {
        &self.terms[id.0]
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn size(&self, id: SymTermId) -> usize {
        self.sizes[id.0]
    }

    pub fn depth(&self, id: SymTermId) -> usize {
        self.depths[id.0]
    }

    pub fn hash(&self, id: SymTermId) -> u64 {
        self.hashes[id.0]
    }

    #[inline]
    pub fn structural_eq(&self, t1: SymTermId, t2: SymTermId) -> bool {
        t1 == t2
    }

    pub fn intern_const(&mut self, lit: TypedLiteral) -> SymTermId {
        match lit {
            TypedLiteral::Int(i, ty) => self.intern(SymTerm::ConstInt(i, ty)),
            TypedLiteral::Float(f, ty) => self.intern(SymTerm::ConstFloat(f.to_bits(), ty)),
            TypedLiteral::Bool(b) => self.intern(SymTerm::ConstBool(b)),
            TypedLiteral::Str(s) => self.intern(SymTerm::ConstStr(s)),
        }
    }

    pub fn intern_int(&mut self, val: i64) -> SymTermId {
        self.intern(SymTerm::ConstInt(val, Type::I64))
    }

    pub fn intern_bool(&mut self, val: bool) -> SymTermId {
        self.intern(SymTerm::ConstBool(val))
    }

    pub fn intern_float(&mut self, val: f64) -> SymTermId {
        self.intern(SymTerm::ConstFloat(val.to_bits(), Type::F64))
    }

    pub fn intern_typed_zero(&mut self, ty: &Type) -> SymTermId {
        match ty {
            Type::F64 | Type::F32 => self.intern(SymTerm::ConstFloat(0.0f64.to_bits(), ty.clone())),
            Type::Bool => self.intern(SymTerm::ConstBool(false)),
            _ => self.intern(SymTerm::ConstInt(0, ty.clone())),
        }
    }

    pub fn intern_typed_one(&mut self, ty: &Type) -> SymTermId {
        match ty {
            Type::F64 | Type::F32 => self.intern(SymTerm::ConstFloat(1.0f64.to_bits(), ty.clone())),
            Type::Bool => self.intern(SymTerm::ConstBool(true)),
            _ => self.intern(SymTerm::ConstInt(1, ty.clone())),
        }
    }

    pub fn intern_var(&mut self, place: Place, ty: Type) -> SymTermId {
        self.intern(SymTerm::Var(place, ty))
    }

    pub fn intern_binary(
        &mut self,
        op: BinaryOp,
        mut left: SymTermId,
        mut right: SymTermId,
        ty: Type,
    ) -> SymTermId {
        let mut left_term = self.get(left).clone();
        let mut right_term = self.get(right).clone();

        // 1. Constant folding
        if let (Some(l_lit), Some(r_lit)) = (left_term.to_literal(), right_term.to_literal()) {
            if let Some(folded) = fold_const_binary(op, &l_lit, &r_lit) {
                return self.intern_const(folded);
            }
        }

        // 2. Canonical commutative reordering (ALG-01)
        // Constants are moved to the right; non-constants are sorted deterministically by SymTermId.
        if is_commutative(op) {
            let should_swap = match (is_const(&left_term), is_const(&right_term)) {
                (true, false) => true,
                (false, true) => false,
                _ => left.0 > right.0,
            };
            if should_swap {
                std::mem::swap(&mut left, &mut right);
                std::mem::swap(&mut left_term, &mut right_term);
            }
        }

        // 3. Structural algebraic simplifications (ALG-02, ALG-04)
        match op {
            BinaryOp::Add => {
                if is_zero(&right_term) {
                    return left;
                }
                if is_zero(&left_term) {
                    return right;
                }
                // Constant reassociation: (x + c1) + c2 => x + (c1 + c2)
                if let SymTerm::Binary(BinaryOp::Add, inner_x, inner_c, _) = left_term {
                    if let (Some(c1_lit), Some(c2_lit)) =
                        (self.get(inner_c).to_literal(), right_term.to_literal())
                    {
                        if let Some(c_sum) = fold_const_binary(BinaryOp::Add, &c1_lit, &c2_lit) {
                            let c_sum_id = self.intern_const(c_sum);
                            return self.intern_binary(BinaryOp::Add, inner_x, c_sum_id, ty);
                        }
                    }
                }
                // Constant reassociation: (x - c1) + c2 => x + (c2 - c1)
                if let SymTerm::Binary(BinaryOp::Sub, inner_x, inner_c, _) = left_term {
                    if let (Some(c1_lit), Some(c2_lit)) =
                        (self.get(inner_c).to_literal(), right_term.to_literal())
                    {
                        if let Some(c_diff) = fold_const_binary(BinaryOp::Sub, &c2_lit, &c1_lit) {
                            let c_diff_id = self.intern_const(c_diff);
                            return self.intern_binary(BinaryOp::Add, inner_x, c_diff_id, ty);
                        }
                    }
                }
            }
            BinaryOp::Sub => {
                if is_zero(&right_term) {
                    return left;
                }
                if left == right {
                    return self.intern_typed_zero(&ty);
                }
                if is_zero(&left_term) {
                    return self.intern_unary(UnaryOp::Neg, right, ty);
                }
                // Constant reassociation: (x + c1) - c2 => x + (c1 - c2)
                if let SymTerm::Binary(BinaryOp::Add, inner_x, inner_c, _) = left_term {
                    if let (Some(c1_lit), Some(c2_lit)) =
                        (self.get(inner_c).to_literal(), right_term.to_literal())
                    {
                        if let Some(c_diff) = fold_const_binary(BinaryOp::Sub, &c1_lit, &c2_lit) {
                            let c_diff_id = self.intern_const(c_diff);
                            return self.intern_binary(BinaryOp::Add, inner_x, c_diff_id, ty);
                        }
                    }
                }
                // Constant reassociation: (x - c1) - c2 => x - (c1 + c2)
                if let SymTerm::Binary(BinaryOp::Sub, inner_x, inner_c, _) = left_term {
                    if let (Some(c1_lit), Some(c2_lit)) =
                        (self.get(inner_c).to_literal(), right_term.to_literal())
                    {
                        if let Some(c_sum) = fold_const_binary(BinaryOp::Add, &c1_lit, &c2_lit) {
                            let c_sum_id = self.intern_const(c_sum);
                            return self.intern_binary(BinaryOp::Sub, inner_x, c_sum_id, ty);
                        }
                    }
                }
            }
            BinaryOp::Mul => {
                if is_zero(&right_term) || is_zero(&left_term) {
                    return self.intern_typed_zero(&ty);
                }
                if is_one(&right_term) {
                    return left;
                }
                if is_one(&left_term) {
                    return right;
                }
                if is_minus_one(&right_term) {
                    return self.intern_unary(UnaryOp::Neg, left, ty);
                }
                if is_minus_one(&left_term) {
                    return self.intern_unary(UnaryOp::Neg, right, ty);
                }
                // Constant reassociation: (x * c1) * c2 => x * (c1 * c2)
                if let SymTerm::Binary(BinaryOp::Mul, inner_x, inner_c, _) = left_term {
                    if let (Some(c1_lit), Some(c2_lit)) =
                        (self.get(inner_c).to_literal(), right_term.to_literal())
                    {
                        if let Some(c_prod) = fold_const_binary(BinaryOp::Mul, &c1_lit, &c2_lit) {
                            let c_prod_id = self.intern_const(c_prod);
                            return self.intern_binary(BinaryOp::Mul, inner_x, c_prod_id, ty);
                        }
                    }
                }
            }
            BinaryOp::Div => {
                if is_one(&right_term) {
                    return left;
                }
                if is_minus_one(&right_term) {
                    return self.intern_unary(UnaryOp::Neg, left, ty);
                }
                if left == right && !is_zero(&right_term) {
                    return self.intern_typed_one(&ty);
                }
                if is_zero(&left_term) && !is_zero(&right_term) {
                    return self.intern_typed_zero(&ty);
                }
            }
            BinaryOp::Mod => {
                if is_one(&right_term) {
                    return self.intern_typed_zero(&ty);
                }
                if left == right && !is_zero(&right_term) {
                    return self.intern_typed_zero(&ty);
                }
                if is_zero(&left_term) && !is_zero(&right_term) {
                    return self.intern_typed_zero(&ty);
                }
            }
            BinaryOp::Shl => {
                if is_zero(&right_term) {
                    return left;
                }
                if is_zero(&left_term) {
                    return self.intern_typed_zero(&ty);
                }
            }
            BinaryOp::Shr => {
                if is_zero(&right_term) {
                    return left;
                }
                if is_zero(&left_term) {
                    return self.intern_typed_zero(&ty);
                }
            }
            BinaryOp::BitAnd => {
                if left == right {
                    return left;
                }
                if is_zero(&left_term) || is_zero(&right_term) {
                    return self.intern_typed_zero(&ty);
                }
                if ty == Type::Bool {
                    if is_one(&right_term) {
                        return left;
                    }
                    if is_one(&left_term) {
                        return right;
                    }
                } else if is_minus_one(&right_term) {
                    return left;
                } else if is_minus_one(&left_term) {
                    return right;
                }
            }
            BinaryOp::BitOr => {
                if left == right {
                    return left;
                }
                if is_zero(&right_term) {
                    return left;
                }
                if is_zero(&left_term) {
                    return right;
                }
                if ty == Type::Bool {
                    if is_one(&right_term) || is_one(&left_term) {
                        return self.intern_bool(true);
                    }
                } else if is_minus_one(&right_term) {
                    return right;
                } else if is_minus_one(&left_term) {
                    return left;
                }
            }
            BinaryOp::BitXor => {
                if left == right {
                    return self.intern_typed_zero(&ty);
                }
                if is_zero(&right_term) {
                    return left;
                }
                if is_zero(&left_term) {
                    return right;
                }
                if ty == Type::Bool {
                    if is_one(&right_term) {
                        return self.intern_unary(UnaryOp::Not, left, Type::Bool);
                    }
                    if is_one(&left_term) {
                        return self.intern_unary(UnaryOp::Not, right, Type::Bool);
                    }
                } else if is_minus_one(&right_term) {
                    return self.intern_unary(UnaryOp::Not, left, ty);
                } else if is_minus_one(&left_term) {
                    return self.intern_unary(UnaryOp::Not, right, ty);
                }
            }
            BinaryOp::Eq => {
                if left == right {
                    return self.intern_bool(true);
                }
                if ty == Type::Bool || self.get(left).ty() == &Type::Bool {
                    if is_one(&right_term) {
                        return left;
                    }
                    if is_zero(&right_term) {
                        return self.intern_unary(UnaryOp::Not, left, Type::Bool);
                    }
                    if is_one(&left_term) {
                        return right;
                    }
                    if is_zero(&left_term) {
                        return self.intern_unary(UnaryOp::Not, right, Type::Bool);
                    }
                }
            }
            BinaryOp::Ne => {
                if left == right {
                    return self.intern_bool(false);
                }
                if ty == Type::Bool || self.get(left).ty() == &Type::Bool {
                    if is_zero(&right_term) {
                        return left;
                    }
                    if is_one(&right_term) {
                        return self.intern_unary(UnaryOp::Not, left, Type::Bool);
                    }
                    if is_zero(&left_term) {
                        return right;
                    }
                    if is_one(&left_term) {
                        return self.intern_unary(UnaryOp::Not, right, Type::Bool);
                    }
                }
            }
            BinaryOp::Lt if left == right => {
                return self.intern_bool(false);
            }
            BinaryOp::Gt if left == right => {
                return self.intern_bool(false);
            }
            BinaryOp::Le if left == right => {
                return self.intern_bool(true);
            }
            BinaryOp::Ge if left == right => {
                return self.intern_bool(true);
            }
            _ => {}
        }

        self.intern(SymTerm::Binary(op, left, right, ty))
    }

    pub fn intern_unary(&mut self, op: UnaryOp, expr: SymTermId, ty: Type) -> SymTermId {
        let inner = self.get(expr).clone();
        if let Some(lit) = inner.to_literal() {
            if let Some(folded) = fold_const_unary(op, &lit) {
                return self.intern_const(folded);
            }
        }

        // Double negation elimination: ¬¬x = x, -(-x) = x
        if let SymTerm::Unary(inner_op, inner_expr, _) = inner {
            if inner_op == op {
                match op {
                    UnaryOp::Not | UnaryOp::Neg => return inner_expr,
                }
            }
        }

        // Invert comparisons under Not: !(a == b) => a != b, !(a < b) => a >= b
        if op == UnaryOp::Not {
            if let SymTerm::Binary(cmp_op, l, r, cmp_ty) = inner {
                let operand_ty = self.get(l).ty();
                let is_float = matches!(operand_ty, Type::F32 | Type::F64);
                if !is_float {
                    let inverted_op = match cmp_op {
                        BinaryOp::Eq => Some(BinaryOp::Ne),
                        BinaryOp::Ne => Some(BinaryOp::Eq),
                        BinaryOp::Lt => Some(BinaryOp::Ge),
                        BinaryOp::Le => Some(BinaryOp::Gt),
                        BinaryOp::Gt => Some(BinaryOp::Le),
                        BinaryOp::Ge => Some(BinaryOp::Lt),
                        _ => None,
                    };
                    if let Some(inv_op) = inverted_op {
                        return self.intern_binary(inv_op, l, r, cmp_ty);
                    }
                } else {
                    let inverted_op = match cmp_op {
                        BinaryOp::Eq => Some(BinaryOp::Ne),
                        BinaryOp::Ne => Some(BinaryOp::Eq),
                        _ => None,
                    };
                    if let Some(inv_op) = inverted_op {
                        return self.intern_binary(inv_op, l, r, cmp_ty);
                    }
                }
            }
        }

        self.intern(SymTerm::Unary(op, expr, ty))
    }

    pub fn intern_select(
        &mut self,
        cond: SymTermId,
        then_term: SymTermId,
        else_term: SymTermId,
        ty: Type,
    ) -> SymTermId {
        let cond_term = self.get(cond).clone();
        if let SymTerm::ConstBool(b) = cond_term {
            return if b { then_term } else { else_term };
        }
        if then_term == else_term {
            return then_term;
        }
        if ty == Type::Bool {
            if let (SymTerm::ConstBool(true), SymTerm::ConstBool(false)) =
                (self.get(then_term), self.get(else_term))
            {
                return cond;
            }
            if let (SymTerm::ConstBool(false), SymTerm::ConstBool(true)) =
                (self.get(then_term), self.get(else_term))
            {
                return self.intern_unary(UnaryOp::Not, cond, Type::Bool);
            }
        }
        // If condition is Negated: select(!c, t, e) => select(c, e, t)
        if let SymTerm::Unary(UnaryOp::Not, inner_cond, _) = cond_term {
            return self.intern_select(inner_cond, else_term, then_term, ty);
        }

        self.intern(SymTerm::Select(cond, then_term, else_term, ty))
    }

    pub fn intern_phi(&mut self, incoming: Vec<(BasicBlockId, SymTermId)>, ty: Type) -> SymTermId {
        if !incoming.is_empty() {
            let first = incoming[0].1;
            if incoming.iter().all(|(_, t)| *t == first) {
                return first;
            }
        }
        self.intern(SymTerm::Phi(incoming, ty))
    }

    pub fn intern_constructor(
        &mut self,
        name: String,
        tag: usize,
        fields: Vec<SymTermId>,
        ty: Type,
    ) -> SymTermId {
        self.intern(SymTerm::Constructor(name, tag, fields, ty))
    }

    pub fn intern_call(&mut self, callee: String, args: Vec<SymTermId>, ty: Type) -> SymTermId {
        if callee == "i64_to_f64" && args.len() == 1 {
            if let Some(TypedLiteral::Int(i, _)) = self.get(args[0]).to_literal() {
                return self.intern_const(TypedLiteral::Float(i as f64, Type::F64));
            }
        } else if callee == "f64_to_i64" && args.len() == 1 {
            if let Some(TypedLiteral::Float(f, _)) = self.get(args[0]).to_literal() {
                return self.intern_int(f as i64);
            }
        } else if callee == "sqrt" && args.len() == 1 {
            if let Some(TypedLiteral::Float(f, _)) = self.get(args[0]).to_literal() {
                if f >= 0.0 {
                    return self.intern_const(TypedLiteral::Float(f.sqrt(), Type::F64));
                }
            }
        } else if callee == "abs" && args.len() == 1 {
            if let Some(TypedLiteral::Float(f, _)) = self.get(args[0]).to_literal() {
                return self.intern_const(TypedLiteral::Float(f.abs(), Type::F64));
            } else if let Some(TypedLiteral::Int(i, i_ty)) = self.get(args[0]).to_literal() {
                return self.intern_const(TypedLiteral::Int(i.abs(), i_ty));
            }
        } else if callee == "isqrt" && args.len() == 1 {
            if let Some(TypedLiteral::Int(i, i_ty)) = self.get(args[0]).to_literal() {
                if i >= 0 {
                    return self.intern_const(TypedLiteral::Int((i as f64).sqrt() as i64, i_ty));
                }
            }
        }
        self.intern(SymTerm::Call(callee, args, ty))
    }

    pub fn intern_ref(&mut self, inner: SymTermId, ty: Type) -> SymTermId {
        self.intern(SymTerm::Ref(inner, ty))
    }

    pub fn intern_deref(&mut self, ptr: SymTermId, ty: Type) -> SymTermId {
        self.intern(SymTerm::Deref(ptr, ty))
    }

    pub fn intern_discriminant(&mut self, inner: SymTermId) -> SymTermId {
        self.intern(SymTerm::Discriminant(inner, Type::I64))
    }

    pub fn intern_closure_val(
        &mut self,
        fn_name: String,
        captured: Vec<SymTermId>,
        ty: Type,
    ) -> SymTermId {
        self.intern(SymTerm::ClosureVal(fn_name, captured, ty))
    }

    pub fn import_from(&mut self, other: &TermInterner, id: SymTermId) -> SymTermId {
        let term = other.get(id).clone();
        match term {
            SymTerm::ConstInt(val, ty) => self.intern(SymTerm::ConstInt(val, ty)),
            SymTerm::ConstFloat(bits, ty) => self.intern(SymTerm::ConstFloat(bits, ty)),
            SymTerm::ConstBool(b) => self.intern(SymTerm::ConstBool(b)),
            SymTerm::ConstStr(s) => self.intern(SymTerm::ConstStr(s)),
            SymTerm::Var(p, ty) => self.intern(SymTerm::Var(p, ty)),
            SymTerm::Binary(op, l, r, ty) => {
                let l_new = self.import_from(other, l);
                let r_new = self.import_from(other, r);
                self.intern_binary(op, l_new, r_new, ty)
            }
            SymTerm::Unary(op, inner, ty) => {
                let inner_new = self.import_from(other, inner);
                self.intern_unary(op, inner_new, ty)
            }
            SymTerm::Constructor(name, tag, fields, ty) => {
                let fields_new = fields.iter().map(|&f| self.import_from(other, f)).collect();
                self.intern_constructor(name, tag, fields_new, ty)
            }
            SymTerm::Select(c, t, e, ty) => {
                let c_new = self.import_from(other, c);
                let t_new = self.import_from(other, t);
                let e_new = self.import_from(other, e);
                self.intern_select(c_new, t_new, e_new, ty)
            }
            SymTerm::Phi(incoming, ty) => {
                let incoming_new = incoming
                    .iter()
                    .map(|(b, t)| (b.clone(), self.import_from(other, *t)))
                    .collect();
                self.intern_phi(incoming_new, ty)
            }
            SymTerm::Call(callee, args, ty) => {
                let args_new = args.iter().map(|&a| self.import_from(other, a)).collect();
                self.intern_call(callee, args_new, ty)
            }
            SymTerm::Ref(inner, ty) => {
                let inner_new = self.import_from(other, inner);
                self.intern_ref(inner_new, ty)
            }
            SymTerm::Deref(ptr, ty) => {
                let ptr_new = self.import_from(other, ptr);
                self.intern_deref(ptr_new, ty)
            }
            SymTerm::Discriminant(inner, ty) => {
                let inner_new = self.import_from(other, inner);
                self.intern(SymTerm::Discriminant(inner_new, ty))
            }
            SymTerm::ClosureVal(fn_name, captured, ty) => {
                let captured_new = captured
                    .iter()
                    .map(|&c| self.import_from(other, c))
                    .collect();
                self.intern_closure_val(fn_name, captured_new, ty)
            }
            SymTerm::Thunk(body, env, ty) => {
                let env_new = env.iter().map(|&c| self.import_from(other, c)).collect();
                self.intern_thunk(body, env_new, ty)
            }
        }
    }

    pub fn intern_thunk(&mut self, body: String, env: Vec<SymTermId>, ty: Type) -> SymTermId {
        self.intern(SymTerm::Thunk(body, env, ty))
    }

    fn intern(&mut self, term: SymTerm) -> SymTermId {
        if let Some(&id) = self.lookup.get(&term) {
            return id;
        }

        let id = SymTermId(self.terms.len());
        let (size, depth, hash) = match &term {
            SymTerm::ConstInt(val, _) => {
                let h = fx_hash_step(1, *val as u64);
                (1, 1, h)
            }
            SymTerm::ConstFloat(bits, _) => {
                let h = fx_hash_step(2, *bits);
                (1, 1, h)
            }
            SymTerm::ConstBool(b) => {
                let h = fx_hash_step(3, if *b { 1 } else { 0 });
                (1, 1, h)
            }
            SymTerm::ConstStr(s) => {
                let h = fx_hash_bytes(4, s.as_bytes());
                (1, 1, h)
            }
            SymTerm::Var(p, _) => {
                let h = hash_place(5, p);
                (1, 1, h)
            }
            SymTerm::Binary(op, l, r, _) => {
                let s = 1 + self.sizes[l.0] + self.sizes[r.0];
                let d = 1 + std::cmp::max(self.depths[l.0], self.depths[r.0]);
                let mut h = fx_hash_step(6, binary_op_discriminant(*op));
                h = fx_hash_step(h, self.hashes[l.0]);
                h = fx_hash_step(h, self.hashes[r.0]);
                (s, d, h)
            }
            SymTerm::Unary(op, inner, _) => {
                let s = 1 + self.sizes[inner.0];
                let d = 1 + self.depths[inner.0];
                let mut h = fx_hash_step(7, unary_op_discriminant(*op));
                h = fx_hash_step(h, self.hashes[inner.0]);
                (s, d, h)
            }
            SymTerm::Constructor(name, tag, fields, _) => {
                let s = 1 + fields.iter().map(|f| self.sizes[f.0]).sum::<usize>();
                let d = 1 + fields.iter().map(|f| self.depths[f.0]).max().unwrap_or(0);
                let mut h = fx_hash_bytes(8, name.as_bytes());
                h = fx_hash_step(h, *tag as u64);
                for f in fields {
                    h = fx_hash_step(h, self.hashes[f.0]);
                }
                (s, d, h)
            }
            SymTerm::Call(name, args, _) => {
                let s = 1 + args.iter().map(|a| self.sizes[a.0]).sum::<usize>();
                let d = 1 + args.iter().map(|a| self.depths[a.0]).max().unwrap_or(0);
                let mut h = fx_hash_bytes(9, name.as_bytes());
                for a in args {
                    h = fx_hash_step(h, self.hashes[a.0]);
                }
                (s, d, h)
            }
            SymTerm::Select(c, t, e, _) => {
                let s = 1 + self.sizes[c.0] + self.sizes[t.0] + self.sizes[e.0];
                let d = 1 + std::cmp::max(
                    self.depths[c.0],
                    std::cmp::max(self.depths[t.0], self.depths[e.0]),
                );
                let mut h = fx_hash_step(10, self.hashes[c.0]);
                h = fx_hash_step(h, self.hashes[t.0]);
                h = fx_hash_step(h, self.hashes[e.0]);
                (s, d, h)
            }
            SymTerm::Phi(incoming, _) => {
                let s = 1 + incoming.iter().map(|(_, t)| self.sizes[t.0]).sum::<usize>();
                let d = 1 + incoming
                    .iter()
                    .map(|(_, t)| self.depths[t.0])
                    .max()
                    .unwrap_or(0);
                let mut h = fx_hash_step(11, incoming.len() as u64);
                for (bb, t) in incoming {
                    h = fx_hash_step(h, bb.0 as u64);
                    h = fx_hash_step(h, self.hashes[t.0]);
                }
                (s, d, h)
            }
            SymTerm::Ref(inner, _) => {
                let s = 1 + self.sizes[inner.0];
                let d = 1 + self.depths[inner.0];
                let h = fx_hash_step(12, self.hashes[inner.0]);
                (s, d, h)
            }
            SymTerm::Deref(inner, _) => {
                let s = 1 + self.sizes[inner.0];
                let d = 1 + self.depths[inner.0];
                let h = fx_hash_step(13, self.hashes[inner.0]);
                (s, d, h)
            }
            SymTerm::Discriminant(inner, _) => {
                let s = 1 + self.sizes[inner.0];
                let d = 1 + self.depths[inner.0];
                let h = fx_hash_step(14, self.hashes[inner.0]);
                (s, d, h)
            }
            SymTerm::ClosureVal(name, captured, _) => {
                let s = 1 + captured.iter().map(|c| self.sizes[c.0]).sum::<usize>();
                let d = 1 + captured.iter().map(|c| self.depths[c.0]).max().unwrap_or(0);
                let mut h = fx_hash_bytes(15, name.as_bytes());
                for c in captured {
                    h = fx_hash_step(h, self.hashes[c.0]);
                }
                (s, d, h)
            }
            SymTerm::Thunk(body, captured, _) => {
                let s = 1 + captured.iter().map(|c| self.sizes[c.0]).sum::<usize>();
                let d = 1 + captured.iter().map(|c| self.depths[c.0]).max().unwrap_or(0);
                let mut h = fx_hash_bytes(16, body.as_bytes());
                for c in captured {
                    h = fx_hash_step(h, self.hashes[c.0]);
                }
                (s, d, h)
            }
        };

        self.lookup.insert(term.clone(), id);
        self.terms.push(term);
        self.sizes.push(size);
        self.depths.push(depth);
        self.hashes.push(hash);
        id
    }

    pub fn format_term(&self, id: SymTermId) -> String {
        match self.get(id) {
            SymTerm::ConstInt(i, _) => format!("{}", i),
            SymTerm::ConstFloat(bits, _) => format!("{}", f64::from_bits(*bits)),
            SymTerm::ConstBool(b) => format!("{}", b),
            SymTerm::ConstStr(s) => format!("\"{}\"", s),
            SymTerm::Var(p, _) => format_place_str(p),
            SymTerm::Binary(op, l, r, _) => {
                format!(
                    "({} {:?} {})",
                    self.format_term(*l),
                    op,
                    self.format_term(*r)
                )
            }
            SymTerm::Unary(op, inner, _) => {
                format!("({:?} {})", op, self.format_term(*inner))
            }
            SymTerm::Constructor(name, tag, fields, _) => {
                let f_str: Vec<String> = fields.iter().map(|f| self.format_term(*f)).collect();
                format!("{}#{} {{{}}}", name, tag, f_str.join(", "))
            }
            SymTerm::Call(callee, args, _) => {
                let a_str: Vec<String> = args.iter().map(|a| self.format_term(*a)).collect();
                format!("{}({})", callee, a_str.join(", "))
            }
            SymTerm::Select(c, t, e, _) => {
                format!(
                    "select({}, {}, {})",
                    self.format_term(*c),
                    self.format_term(*t),
                    self.format_term(*e)
                )
            }
            SymTerm::Phi(incoming, _) => {
                let inc_str: Vec<String> = incoming
                    .iter()
                    .map(|(b, t)| format!("bb{}: {}", b.0, self.format_term(*t)))
                    .collect();
                format!("phi({})", inc_str.join(", "))
            }
            SymTerm::Ref(inner, _) => format!("&({})", self.format_term(*inner)),
            SymTerm::Deref(ptr, _) => format!("*({})", self.format_term(*ptr)),
            SymTerm::Discriminant(inner, _) => {
                format!("discriminant({})", self.format_term(*inner))
            }
            SymTerm::ClosureVal(fn_name, captured, _) => {
                let c_str: Vec<String> = captured.iter().map(|c| self.format_term(*c)).collect();
                format!("closure:{}({})", fn_name, c_str.join(", "))
            }
            SymTerm::Thunk(body, env, _) => {
                let e_str: Vec<String> = env.iter().map(|c| self.format_term(*c)).collect();
                format!("thunk:{}({})", body, e_str.join(", "))
            }
        }
    }
}

fn is_zero(term: &SymTerm) -> bool {
    match term {
        SymTerm::ConstInt(0, _) => true,
        SymTerm::ConstFloat(bits, _) => f64::from_bits(*bits) == 0.0,
        SymTerm::ConstBool(false) => true,
        _ => false,
    }
}

fn is_one(term: &SymTerm) -> bool {
    match term {
        SymTerm::ConstInt(1, _) => true,
        SymTerm::ConstFloat(bits, _) => f64::from_bits(*bits) == 1.0,
        SymTerm::ConstBool(true) => true,
        _ => false,
    }
}

fn is_minus_one(term: &SymTerm) -> bool {
    match term {
        SymTerm::ConstInt(-1, _) => true,
        SymTerm::ConstFloat(bits, _) => f64::from_bits(*bits) == -1.0,
        _ => false,
    }
}

fn is_const(term: &SymTerm) -> bool {
    matches!(
        term,
        SymTerm::ConstInt(_, _)
            | SymTerm::ConstFloat(_, _)
            | SymTerm::ConstBool(_)
            | SymTerm::ConstStr(_)
    )
}

fn is_commutative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Mul
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Eq
            | BinaryOp::Ne
    )
}

fn fold_const_binary(op: BinaryOp, l: &TypedLiteral, r: &TypedLiteral) -> Option<TypedLiteral> {
    match (l, r) {
        (TypedLiteral::Int(li, ty), TypedLiteral::Int(ri, _)) => {
            let res = match op {
                BinaryOp::Add => li.wrapping_add(*ri),
                BinaryOp::Sub => li.wrapping_sub(*ri),
                BinaryOp::Mul => li.wrapping_mul(*ri),
                BinaryOp::Div => {
                    if *ri == 0 {
                        return None;
                    }
                    li.checked_div(*ri)?
                }
                BinaryOp::Mod => {
                    if *ri == 0 {
                        return None;
                    }
                    li.checked_rem(*ri)?
                }
                BinaryOp::BitAnd => li & ri,
                BinaryOp::BitOr => li | ri,
                BinaryOp::BitXor => li ^ ri,
                BinaryOp::Shl => {
                    if *ri < 0 || *ri >= 64 {
                        return None;
                    }
                    li.wrapping_shl(*ri as u32)
                }
                BinaryOp::Shr => {
                    if *ri < 0 || *ri >= 64 {
                        return None;
                    }
                    li.wrapping_shr(*ri as u32)
                }
                BinaryOp::Eq => return Some(TypedLiteral::Bool(li == ri)),
                BinaryOp::Ne => return Some(TypedLiteral::Bool(li != ri)),
                BinaryOp::Lt => return Some(TypedLiteral::Bool(li < ri)),
                BinaryOp::Le => return Some(TypedLiteral::Bool(li <= ri)),
                BinaryOp::Gt => return Some(TypedLiteral::Bool(li > ri)),
                BinaryOp::Ge => return Some(TypedLiteral::Bool(li >= ri)),
                BinaryOp::Pow => {
                    if *ri < 0 {
                        return None;
                    }
                    li.checked_pow(*ri as u32)?
                }
            };
            Some(TypedLiteral::Int(res, ty.clone()))
        }
        (TypedLiteral::Bool(lb), TypedLiteral::Bool(rb)) => match op {
            BinaryOp::Eq => Some(TypedLiteral::Bool(lb == rb)),
            BinaryOp::Ne => Some(TypedLiteral::Bool(lb != rb)),
            BinaryOp::BitAnd => Some(TypedLiteral::Bool(*lb && *rb)),
            BinaryOp::BitOr => Some(TypedLiteral::Bool(*lb || *rb)),
            BinaryOp::BitXor => Some(TypedLiteral::Bool(*lb ^ *rb)),
            _ => None,
        },
        (TypedLiteral::Float(lf, ty), TypedLiteral::Float(rf, _)) => {
            let res = match op {
                BinaryOp::Add => lf + rf,
                BinaryOp::Sub => lf - rf,
                BinaryOp::Mul => lf * rf,
                BinaryOp::Div => {
                    if *rf == 0.0 {
                        return None;
                    }
                    lf / rf
                }
                BinaryOp::Pow => lf.powf(*rf),
                BinaryOp::Eq => return Some(TypedLiteral::Bool(lf == rf)),
                BinaryOp::Ne => return Some(TypedLiteral::Bool(lf != rf)),
                BinaryOp::Lt => return Some(TypedLiteral::Bool(lf < rf)),
                BinaryOp::Le => return Some(TypedLiteral::Bool(lf <= rf)),
                BinaryOp::Gt => return Some(TypedLiteral::Bool(lf > rf)),
                BinaryOp::Ge => return Some(TypedLiteral::Bool(lf >= rf)),
                _ => return None,
            };
            Some(TypedLiteral::Float(res, ty.clone()))
        }
        _ => None,
    }
}

fn fold_const_unary(op: UnaryOp, lit: &TypedLiteral) -> Option<TypedLiteral> {
    match (op, lit) {
        (UnaryOp::Neg, TypedLiteral::Int(i, ty)) => {
            Some(TypedLiteral::Int(i.wrapping_neg(), ty.clone()))
        }
        (UnaryOp::Neg, TypedLiteral::Float(f, ty)) => Some(TypedLiteral::Float(-f, ty.clone())),
        (UnaryOp::Not, TypedLiteral::Bool(b)) => Some(TypedLiteral::Bool(!b)),
        (UnaryOp::Not, TypedLiteral::Int(i, ty)) => Some(TypedLiteral::Int(!i, ty.clone())),
        _ => None,
    }
}

fn format_place_str(p: &Place) -> String {
    let mut s = p.local.clone();
    for proj in &p.projections {
        match proj {
            crate::mir::Projection::Deref => s = format!("(*{})", s),
            crate::mir::Projection::Field(f) => s = format!("{}.{}", s, f),
            crate::mir::Projection::Index(idx) => s = format!("{}[{}]", s, format_place_str(idx)),
            crate::mir::Projection::Payload(idx) => s = format!("{}.payload_{}", s, idx),
        }
    }
    s
}

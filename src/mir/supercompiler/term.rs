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
}

impl TermInterner {
    pub fn new() -> Self {
        TermInterner {
            terms: Vec::new(),
            lookup: HashMap::new(),
            sizes: Vec::new(),
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

    pub fn intern_var(&mut self, place: Place, ty: Type) -> SymTermId {
        self.intern(SymTerm::Var(place, ty))
    }

    pub fn intern_binary(
        &mut self,
        op: BinaryOp,
        left: SymTermId,
        right: SymTermId,
        ty: Type,
    ) -> SymTermId {
        let left_term = self.get(left).clone();
        let right_term = self.get(right).clone();

        // 1. Constant folding
        if let (Some(l_lit), Some(r_lit)) = (left_term.to_literal(), right_term.to_literal()) {
            if let Some(folded) = fold_const_binary(op, &l_lit, &r_lit) {
                return self.intern_const(folded);
            }
        }

        // 2. Algebraic simplifications
        match op {
            BinaryOp::Add => {
                if is_zero(&right_term) {
                    return left;
                }
                if is_zero(&left_term) {
                    return right;
                }
            }
            BinaryOp::Sub => {
                if is_zero(&right_term) {
                    return left;
                }
                if left == right {
                    return self.intern_int(0);
                }
            }
            BinaryOp::Mul => {
                if is_zero(&right_term) || is_zero(&left_term) {
                    return self.intern_int(0);
                }
                if is_one(&right_term) {
                    return left;
                }
                if is_one(&left_term) {
                    return right;
                }
            }
            BinaryOp::Div => {
                if is_one(&right_term) {
                    return left;
                }
                if left == right && !is_zero(&right_term) {
                    return self.intern_int(1);
                }
            }
            BinaryOp::BitAnd => {
                if left == right {
                    return left;
                }
                if is_zero(&left_term) || is_zero(&right_term) {
                    return self.intern_int(0);
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
            }
            BinaryOp::BitXor => {
                if left == right {
                    return self.intern_int(0);
                }
                if is_zero(&right_term) {
                    return left;
                }
                if is_zero(&left_term) {
                    return right;
                }
            }
            BinaryOp::Eq if left == right => {
                return self.intern_bool(true);
            }
            BinaryOp::Ne if left == right => {
                return self.intern_bool(false);
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
        self.intern(SymTerm::Select(cond, then_term, else_term, ty))
    }

    pub fn intern_phi(
        &mut self,
        incoming: Vec<(BasicBlockId, SymTermId)>,
        ty: Type,
    ) -> SymTermId {
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

    pub fn intern_call(
        &mut self,
        callee: String,
        args: Vec<SymTermId>,
        ty: Type,
    ) -> SymTermId {
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
        }
    }

    fn intern(&mut self, term: SymTerm) -> SymTermId {
        if let Some(&id) = self.lookup.get(&term) {
            return id;
        }

        let id = SymTermId(self.terms.len());
        let size = match &term {
            SymTerm::ConstInt(_, _)
            | SymTerm::ConstFloat(_, _)
            | SymTerm::ConstBool(_)
            | SymTerm::ConstStr(_)
            | SymTerm::Var(_, _) => 1,
            SymTerm::Binary(_, l, r, _) => 1 + self.sizes[l.0] + self.sizes[r.0],
            SymTerm::Unary(_, inner, _) => 1 + self.sizes[inner.0],
            SymTerm::Constructor(_, _, fields, _) => {
                1 + fields.iter().map(|f| self.sizes[f.0]).sum::<usize>()
            }
            SymTerm::Call(_, args, _) => {
                1 + args.iter().map(|a| self.sizes[a.0]).sum::<usize>()
            }
            SymTerm::Select(c, t, e, _) => {
                1 + self.sizes[c.0] + self.sizes[t.0] + self.sizes[e.0]
            }
            SymTerm::Phi(incoming, _) => {
                1 + incoming.iter().map(|(_, t)| self.sizes[t.0]).sum::<usize>()
            }
            SymTerm::Ref(inner, _)
            | SymTerm::Deref(inner, _)
            | SymTerm::Discriminant(inner, _) => 1 + self.sizes[inner.0],
        };

        self.lookup.insert(term.clone(), id);
        self.terms.push(term);
        self.sizes.push(size);
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
            SymTerm::Discriminant(inner, _) => format!("discriminant({})", self.format_term(*inner)),
        }
    }
}

fn is_zero(term: &SymTerm) -> bool {
    match term {
        SymTerm::ConstInt(0, _) => true,
        SymTerm::ConstFloat(bits, _) => f64::from_bits(*bits) == 0.0,
        _ => false,
    }
}

fn is_one(term: &SymTerm) -> bool {
    match term {
        SymTerm::ConstInt(1, _) => true,
        SymTerm::ConstFloat(bits, _) => f64::from_bits(*bits) == 1.0,
        _ => false,
    }
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
                BinaryOp::Shl => li.wrapping_shl(*ri as u32),
                BinaryOp::Shr => li.wrapping_shr(*ri as u32),
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
            _ => None,
        },
        _ => None,
    }
}

fn fold_const_unary(op: UnaryOp, lit: &TypedLiteral) -> Option<TypedLiteral> {
    match (op, lit) {
        (UnaryOp::Neg, TypedLiteral::Int(i, ty)) => Some(TypedLiteral::Int(-i, ty.clone())),
        (UnaryOp::Neg, TypedLiteral::Float(f, ty)) => Some(TypedLiteral::Float(-f, ty.clone())),
        (UnaryOp::Not, TypedLiteral::Bool(b)) => Some(TypedLiteral::Bool(!b)),
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

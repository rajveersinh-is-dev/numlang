use crate::ast::{BinaryOp, Expr, Function, Literal, MatchPattern, Program, Stmt, UnaryOp};
use crate::span::Span;
use crate::typecheck::symtab::{FunctionSig, ScopeEnvironment, Symbol};
use crate::typecheck::typed_ast::{
    TypedBlock, TypedEnumDef, TypedEnumVariant, TypedExpr, TypedFunction, TypedLiteral,
    TypedMatchArm, TypedMatchPattern, TypedParam, TypedProgram, TypedStmt, TypedStructDef,
};
use crate::typecheck::types::Type;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum TypeError {
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: Type,
        found: Type,
        span: Span,
    },

    #[error("Undeclared variable '{name}'")]
    UndeclaredVariable {
        name: String,
        span: Span,
    },

    #[error("Undeclared function '{name}'")]
    UndeclaredFunction {
        name: String,
        span: Span,
    },

    #[error("Cannot mutate immutable variable '{name}'")]
    CannotMutateImmutable {
        name: String,
        span: Span,
    },

    #[error("Identifier '{name}' is already declared in this scope")]
    DuplicateDeclaration {
        name: String,
        span: Span,
    },

    #[error("Condition must evaluate to bool, found {found}")]
    InvalidConditionType {
        found: Type,
        span: Span,
    },

    #[error("Invalid binary operands for '{op:?}': left is {left}, right is {right}")]
    InvalidBinaryOperands {
        op: BinaryOp,
        left: Type,
        right: Type,
        span: Span,
    },

    #[error("Invalid unary operand for '{op:?}': found {found}")]
    InvalidUnaryOperand {
        op: UnaryOp,
        found: Type,
        span: Span,
    },

    #[error("Function '{name}' expected {expected} arguments, but received {found}")]
    ArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },

    #[error("Unknown type '{name}'")]
    UnknownType {
        name: String,
        span: Span,
    },

    #[error("Function return type mismatch: expected {expected}, found {found}")]
    InvalidReturn {
        expected: Type,
        found: Type,
        span: Span,
    },

    #[error("Cannot index non-array type '{found}'")]
    CannotIndexNonArray {
        found: Type,
        span: Span,
    },

    #[error("Array index must be an integer, found '{found}'")]
    InvalidIndexType {
        found: Type,
        span: Span,
    },

    #[error("Array literal cannot be empty")]
    EmptyArrayLiteral {
        span: Span,
    },

    #[error("Array index {index} out of bounds for array of length {len}")]
    IndexOutOfBounds {
        index: i64,
        len: usize,
        span: Span,
    },

    #[error("Array element type mismatch: expected {expected}, found {found}")]
    ArrayElementMismatch {
        expected: Type,
        found: Type,
        span: Span,
    },

    #[error("'break' may only be used inside a loop")]
    BreakOutsideLoop { span: Span },

    #[error("'continue' may only be used inside a loop")]
    ContinueOutsideLoop { span: Span },

    #[error("Struct '{name}' has no field '{field}'")]
    NoSuchField {
        name: String,
        field: String,
        span: Span,
    },

    #[error("Missing field '{field}' in struct '{name}' initialization")]
    MissingField {
        name: String,
        field: String,
        span: Span,
    },

    #[error("Cannot access field on non-struct type '{found}'")]
    CannotAccessFieldNonStruct {
        found: Type,
        span: Span,
    },

    #[error("Non-exhaustive match: missing wildcard '_' or uncovered cases")]
    NonExhaustiveMatch { span: Span },

    #[error("Match expression must have at least one arm")]
    EmptyMatch { span: Span },

    #[error("Enum '{enum_name}' has no variant '{variant_name}'")]
    NoSuchVariant {
        enum_name: String,
        variant_name: String,
        span: Span,
    },

    #[error("Cannot match variant pattern on non-enum type '{found}'")]
    CannotMatchNonEnum {
        found: Type,
        span: Span,
    },

    #[error("Variant '{enum_name}::{variant_name}' expected {expected} payload arguments, found {found}")]
    PayloadArityMismatch {
        enum_name: String,
        variant_name: String,
        expected: usize,
        found: usize,
        span: Span,
    },

    #[error("Type '{ty}' is not callable")]
    NotCallable {
        ty: Type,
        span: Span,
    },
}

impl TypeError {
    pub fn span(&self) -> Span {
        match self {
            TypeError::TypeMismatch { span, .. } => *span,
            TypeError::UndeclaredVariable { span, .. } => *span,
            TypeError::UndeclaredFunction { span, .. } => *span,
            TypeError::CannotMutateImmutable { span, .. } => *span,
            TypeError::DuplicateDeclaration { span, .. } => *span,
            TypeError::InvalidConditionType { span, .. } => *span,
            TypeError::InvalidBinaryOperands { span, .. } => *span,
            TypeError::InvalidUnaryOperand { span, .. } => *span,
            TypeError::ArityMismatch { span, .. } => *span,
            TypeError::UnknownType { span, .. } => *span,
            TypeError::InvalidReturn { span, .. } => *span,
            TypeError::CannotIndexNonArray { span, .. } => *span,
            TypeError::InvalidIndexType { span, .. } => *span,
            TypeError::EmptyArrayLiteral { span, .. } => *span,
            TypeError::IndexOutOfBounds { span, .. } => *span,
            TypeError::ArrayElementMismatch { span, .. } => *span,
            TypeError::BreakOutsideLoop { span } => *span,
            TypeError::ContinueOutsideLoop { span } => *span,
            TypeError::NoSuchField { span, .. } => *span,
            TypeError::MissingField { span, .. } => *span,
            TypeError::CannotAccessFieldNonStruct { span, .. } => *span,
            TypeError::NonExhaustiveMatch { span } => *span,
            TypeError::EmptyMatch { span } => *span,
            TypeError::NoSuchVariant { span, .. } => *span,
            TypeError::CannotMatchNonEnum { span, .. } => *span,
            TypeError::PayloadArityMismatch { span, .. } => *span,
            TypeError::NotCallable { span, .. } => *span,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            TypeError::TypeMismatch { .. } => "E001",
            TypeError::UndeclaredVariable { .. } => "E002",
            TypeError::UndeclaredFunction { .. } => "E003",
            TypeError::CannotMutateImmutable { .. } => "E004",
            TypeError::DuplicateDeclaration { .. } => "E005",
            TypeError::InvalidConditionType { .. } => "E006",
            TypeError::InvalidBinaryOperands { .. } => "E007",
            TypeError::InvalidUnaryOperand { .. } => "E008",
            TypeError::ArityMismatch { .. } => "E009",
            TypeError::UnknownType { .. } => "E010",
            TypeError::InvalidReturn { .. } => "E011",
            TypeError::CannotIndexNonArray { .. } => "E012",
            TypeError::InvalidIndexType { .. } => "E013",
            TypeError::EmptyArrayLiteral { .. } => "E014",
            TypeError::IndexOutOfBounds { .. } => "E015",
            TypeError::ArrayElementMismatch { .. } => "E016",
            TypeError::BreakOutsideLoop { .. } => "E017",
            TypeError::ContinueOutsideLoop { .. } => "E018",
            TypeError::NoSuchField { .. }
            | TypeError::MissingField { .. }
            | TypeError::CannotAccessFieldNonStruct { .. } => "E019",
            TypeError::NonExhaustiveMatch { .. }
            | TypeError::EmptyMatch { .. } => "E020",
            TypeError::NoSuchVariant { .. }
            | TypeError::CannotMatchNonEnum { .. }
            | TypeError::PayloadArityMismatch { .. } => "E021",
            TypeError::NotCallable { .. } => "E022",
        }
    }
}

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub field_indices: HashMap<String, usize>,
    pub field_offsets: HashMap<String, u32>,
    pub total_size: u32,
    pub align: u32,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<EnumVariantInfo>,
    pub variant_indices: HashMap<String, usize>,
    pub max_payload_size: u32,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    pub name: String,
    pub tag: usize,
    pub payload: Vec<Type>,
    pub span: Span,
}

pub struct TypeChecker {
    env: ScopeEnvironment,
    current_fn_return_ty: Type,
    active_loop_bounds: HashMap<String, i64>,
    loop_depth: usize,
    pub struct_infos: HashMap<String, StructInfo>,
    pub enum_infos: HashMap<String, EnumInfo>,
    pub variant_to_enum: HashMap<String, Vec<String>>,
    pub current_type_params: Vec<String>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: ScopeEnvironment::new(),
            current_fn_return_ty: Type::Void,
            active_loop_bounds: HashMap::new(),
            loop_depth: 0,
            struct_infos: HashMap::new(),
            enum_infos: HashMap::new(),
            variant_to_enum: HashMap::new(),
            current_type_params: Vec::new(),
        }
    }

    fn resolve_parsed_type_in_struct(
        &self,
        ty: Type,
        known_structs: &[crate::ast::StructDef],
        known_enums: &[crate::ast::EnumDef],
        span: Span,
    ) -> Result<Type, TypeError> {
        match ty {
            Type::Struct(sname) => {
                if self.current_type_params.contains(&sname) {
                    Ok(Type::Param(sname))
                } else if known_enums.iter().any(|en| en.name == sname) || self.enum_infos.contains_key(&sname) {
                    Ok(Type::Enum(sname))
                } else if known_structs.iter().any(|st| st.name == sname) || self.struct_infos.contains_key(&sname) {
                    Ok(Type::Struct(sname))
                } else {
                    Err(TypeError::UnknownType { name: sname, span })
                }
            }
            Type::Array(elem, len) => {
                let resolved_elem = self.resolve_parsed_type_in_struct(*elem, known_structs, known_enums, span)?;
                Ok(Type::Array(Box::new(resolved_elem), len))
            }
            Type::Box(inner) => {
                let resolved_inner = self.resolve_parsed_type_in_struct(*inner, known_structs, known_enums, span)?;
                Ok(Type::Box(Box::new(resolved_inner)))
            }
            Type::Ptr(inner) => {
                let resolved_inner = self.resolve_parsed_type_in_struct(*inner, known_structs, known_enums, span)?;
                Ok(Type::Ptr(Box::new(resolved_inner)))
            }
            Type::Fn(args, ret) => {
                let mut resolved_args = Vec::new();
                for a in args {
                    resolved_args.push(self.resolve_parsed_type_in_struct(a, known_structs, known_enums, span)?);
                }
                let resolved_ret = self.resolve_parsed_type_in_struct(*ret, known_structs, known_enums, span)?;
                Ok(Type::Fn(resolved_args, Box::new(resolved_ret)))
            }
            other => Ok(other),
        }
    }

    pub fn resolve_type_in_struct(
        &self,
        ty_str: &str,
        known_structs: &[crate::ast::StructDef],
        known_enums: &[crate::ast::EnumDef],
        span: Span,
    ) -> Result<Type, TypeError> {
        let ty = Type::from_name(ty_str).ok_or_else(|| TypeError::UnknownType {
            name: ty_str.to_string(),
            span,
        })?;
        self.resolve_parsed_type_in_struct(ty, known_structs, known_enums, span)
    }

    fn resolve_parsed_type(&self, ty: Type, span: Span) -> Result<Type, TypeError> {
        match ty {
            Type::Struct(sname) => {
                if self.current_type_params.contains(&sname) {
                    Ok(Type::Param(sname))
                } else if self.enum_infos.contains_key(&sname) {
                    Ok(Type::Enum(sname))
                } else if self.struct_infos.contains_key(&sname) {
                    Ok(Type::Struct(sname))
                } else {
                    Err(TypeError::UnknownType { name: sname, span })
                }
            }
            Type::Array(elem, len) => {
                let resolved_elem = self.resolve_parsed_type(*elem, span)?;
                Ok(Type::Array(Box::new(resolved_elem), len))
            }
            Type::Box(inner) => {
                let resolved_inner = self.resolve_parsed_type(*inner, span)?;
                Ok(Type::Box(Box::new(resolved_inner)))
            }
            Type::Ptr(inner) => {
                let resolved_inner = self.resolve_parsed_type(*inner, span)?;
                Ok(Type::Ptr(Box::new(resolved_inner)))
            }
            Type::Fn(args, ret) => {
                let mut resolved_args = Vec::new();
                for a in args {
                    resolved_args.push(self.resolve_parsed_type(a, span)?);
                }
                let resolved_ret = self.resolve_parsed_type(*ret, span)?;
                Ok(Type::Fn(resolved_args, Box::new(resolved_ret)))
            }
            other => Ok(other),
        }
    }

    pub fn resolve_type(&self, ty_str: &str, span: Span) -> Result<Type, TypeError> {
        let ty = Type::from_name(ty_str).ok_or_else(|| TypeError::UnknownType {
            name: ty_str.to_string(),
            span,
        })?;
        self.resolve_parsed_type(ty, span)
    }

    pub fn check_program(&mut self, program: &Program) -> Result<TypedProgram, TypeError> {
        // Pass 0a: Validate unique names for structs and enums
        for s in &program.structs {
            if self.struct_infos.contains_key(&s.name) || self.enum_infos.contains_key(&s.name) {
                return Err(TypeError::DuplicateDeclaration {
                    name: s.name.clone(),
                    span: s.span,
                });
            }
        }
        for e in &program.enums {
            if self.struct_infos.contains_key(&e.name) || self.enum_infos.contains_key(&e.name) {
                return Err(TypeError::DuplicateDeclaration {
                    name: e.name.clone(),
                    span: e.span,
                });
            }
            // Pre-register enum name
            self.enum_infos.insert(
                e.name.clone(),
                EnumInfo {
                    name: e.name.clone(),
                    variants: Vec::new(),
                    variant_indices: HashMap::new(),
                    max_payload_size: 0,
                    span: e.span,
                },
            );
        }

        // Pass 0b: Collect and validate all struct definitions
        let mut typed_structs = Vec::new();
        for s in &program.structs {
            let mut fields = Vec::new();
            let mut field_indices = HashMap::new();
            let mut field_offsets = HashMap::new();
            let mut current_offset: u32 = 0;
            let mut max_align: u32 = 1;

            for (idx, (f_name, f_ty_str)) in s.fields.iter().enumerate() {
                if field_indices.contains_key(f_name) {
                    return Err(TypeError::DuplicateDeclaration {
                        name: f_name.clone(),
                        span: s.span,
                    });
                }
                field_indices.insert(f_name.clone(), idx);

                let f_ty = self.resolve_type_in_struct(f_ty_str, &program.structs, &program.enums, s.span)?;

                let f_size = match &f_ty {
                    Type::Struct(dep) => {
                        self.struct_infos.get(dep).map_or(8, |i| i.total_size)
                    }
                    Type::Enum(dep) => {
                        self.enum_infos.get(dep).map_or(8, |i| 8 + i.max_payload_size)
                    }
                    Type::Array(elem, len) => {
                        let elem_size = match &**elem {
                            Type::Struct(dep) => {
                                self.struct_infos.get(dep).map_or(8, |i| i.total_size as usize)
                            }
                            Type::Enum(dep) => {
                                self.enum_infos.get(dep).map_or(8, |i| (8 + i.max_payload_size) as usize)
                            }
                            _ => elem.size_bytes(),
                        };
                        (elem_size * len) as u32
                    }
                    _ => f_ty.size_bytes() as u32,
                };
                let align = f_size.clamp(1, 8);
                max_align = max_align.max(align);
                current_offset = (current_offset + align - 1) & !(align - 1);
                field_offsets.insert(f_name.clone(), current_offset);
                current_offset += f_size;
                fields.push((f_name.clone(), f_ty));
            }

            let total_size = (current_offset + max_align - 1) & !(max_align - 1);
            let total_size = total_size.max(1);

            let info = StructInfo {
                name: s.name.clone(),
                fields: fields.clone(),
                field_indices,
                field_offsets,
                total_size,
                align: max_align,
                span: s.span,
            };
            self.struct_infos.insert(s.name.clone(), info);
            typed_structs.push(TypedStructDef {
                name: s.name.clone(),
                fields,
                span: s.span,
            });
        }

        // Pass 0c: Collect and validate all enum definitions
        let mut typed_enums = Vec::new();
        for e in &program.enums {
            let mut variants = Vec::new();
            let mut variant_indices = HashMap::new();
            let mut max_payload_size: u32 = 0;

            for (tag, v) in e.variants.iter().enumerate() {
                if variant_indices.contains_key(&v.name) {
                    return Err(TypeError::DuplicateDeclaration {
                        name: v.name.clone(),
                        span: v.span,
                    });
                }
                variant_indices.insert(v.name.clone(), tag);

                let mut payload_types = Vec::new();
                let mut variant_payload_size: u32 = 0;
                for p_str in &v.payload {
                    let p_ty = self.resolve_type(p_str, v.span)?;
                    let p_size = match &p_ty {
                        Type::Struct(sname) => {
                            self.struct_infos.get(sname).map_or(8, |i| i.total_size)
                        }
                        Type::Enum(ename) => {
                            self.enum_infos.get(ename).map_or(8, |i| 8 + i.max_payload_size)
                        }
                        _ => p_ty.size_bytes() as u32,
                    };
                    variant_payload_size += p_size;
                    payload_types.push(p_ty);
                }
                max_payload_size = max_payload_size.max(variant_payload_size);

                self.variant_to_enum
                    .entry(v.name.clone())
                    .or_default()
                    .push(e.name.clone());

                variants.push(EnumVariantInfo {
                    name: v.name.clone(),
                    tag,
                    payload: payload_types,
                    span: v.span,
                });
            }

            let info = EnumInfo {
                name: e.name.clone(),
                variants: variants.clone(),
                variant_indices,
                max_payload_size,
                span: e.span,
            };
            self.enum_infos.insert(e.name.clone(), info);

            typed_enums.push(TypedEnumDef {
                name: e.name.clone(),
                variants: variants
                    .into_iter()
                    .map(|vi| TypedEnumVariant {
                        name: vi.name,
                        tag: vi.tag,
                        payload: vi.payload,
                        span: vi.span,
                    })
                    .collect(),
                span: e.span,
            });
        }

        // Pass 1: Collect and validate all function signatures
        for func in &program.functions {
            self.current_type_params = func.type_params.clone();
            let return_ty = match &func.return_ty {
                Some(s) => self.resolve_type(s, func.span)?,
                None => Type::Void,
            };

            let mut param_names = Vec::new();
            let mut param_types = Vec::new();
            for param in &func.params {
                let pty = self.resolve_type(&param.ty, param.span)?;
                param_names.push(param.name.clone());
                param_types.push(pty);
            }
            self.current_type_params.clear();

            let sig = FunctionSig {
                name: func.name.clone(),
                type_params: func.type_params.clone(),
                param_names,
                param_types,
                return_ty,
                span: func.span,
            };

            if self.env.define_function(sig).is_err() {
                return Err(TypeError::DuplicateDeclaration {
                    name: func.name.clone(),
                    span: func.span,
                });
            }
        }

        // Pass 2: Type check each function body
        let mut typed_functions = Vec::new();
        for func in &program.functions {
            typed_functions.push(self.check_function(func)?);
        }

        Ok(TypedProgram {
            functions: typed_functions,
            structs: typed_structs,
            enums: typed_enums,
        })
    }

    fn check_function(&mut self, func: &Function) -> Result<TypedFunction, TypeError> {
        let sig = self
            .env
            .lookup_function(&func.name)
            .cloned()
            .expect("Function must exist in scope");

        self.current_type_params = func.type_params.clone();
        self.current_fn_return_ty = sig.return_ty.clone();
        self.env.enter_scope();

        let mut typed_params = Vec::new();
        for (i, param) in func.params.iter().enumerate() {
            let pty = sig.param_types[i].clone();
            let sym = Symbol {
                name: param.name.clone(),
                ty: pty.clone(),
                is_mutable: false,
                span: param.span,
            };
            if self.env.define_variable(sym).is_err() {
                return Err(TypeError::DuplicateDeclaration {
                    name: param.name.clone(),
                    span: param.span,
                });
            }
            typed_params.push(TypedParam {
                name: param.name.clone(),
                ty: pty,
                span: param.span,
            });
        }

        let mut typed_stmts = Vec::new();
        for stmt in &func.body.stmts {
            typed_stmts.push(self.check_stmt(stmt)?);
        }

        self.env.exit_scope();
        self.current_type_params.clear();

        Ok(TypedFunction {
            name: func.name.clone(),
            type_params: func.type_params.clone(),
            params: typed_params,
            return_ty: sig.return_ty,
            body: TypedBlock {
                stmts: typed_stmts,
                span: func.body.span,
            },
            span: func.span,
        })
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<TypedStmt, TypeError> {
        match stmt {
            Stmt::Let {
                name,
                is_mutable,
                ty,
                value,
                span,
            } => {
                let declared_ty = if let Some(t_str) = ty {
                    Some(self.resolve_type(t_str, *span)?)
                } else {
                    None
                };

                let typed_value = self.check_expr(value, declared_ty.clone())?;
                let final_ty = match declared_ty {
                    Some(dt) => {
                        if !typed_value.ty().is_compatible_with(&dt) {
                            return Err(TypeError::TypeMismatch {
                                expected: dt,
                                found: typed_value.ty(),
                                span: typed_value.span(),
                            });
                        }
                        dt
                    }
                    None => {
                        let inf = typed_value.ty();
                        if inf == Type::Void {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: Type::Void,
                                span: typed_value.span(),
                            });
                        }
                        inf
                    }
                };

                let sym = Symbol {
                    name: name.clone(),
                    ty: final_ty.clone(),
                    is_mutable: *is_mutable,
                    span: *span,
                };

                if self.env.define_variable(sym).is_err() {
                    return Err(TypeError::DuplicateDeclaration {
                        name: name.clone(),
                        span: *span,
                    });
                }

                Ok(TypedStmt::Let {
                    name: name.clone(),
                    is_mutable: *is_mutable,
                    ty: final_ty,
                    value: typed_value,
                    span: *span,
                })
            }

            Stmt::Assign { name, value, span } => {
                let sym = match self.env.lookup_variable(name) {
                    Some(s) => s.clone(),
                    None => {
                        return Err(TypeError::UndeclaredVariable {
                            name: name.clone(),
                            span: *span,
                        });
                    }
                };

                if !sym.is_mutable {
                    return Err(TypeError::CannotMutateImmutable {
                        name: name.clone(),
                        span: *span,
                    });
                }

                let typed_value = self.check_expr(value, Some(sym.ty.clone()))?;
                if !typed_value.ty().is_compatible_with(&sym.ty) {
                    return Err(TypeError::TypeMismatch {
                        expected: sym.ty,
                        found: typed_value.ty(),
                        span: typed_value.span(),
                    });
                }

                Ok(TypedStmt::Assign {
                    name: name.clone(),
                    value: typed_value,
                    span: *span,
                })
            }

            Stmt::IndexAssign {
                target,
                index,
                value,
                span,
            } => {
                let sym = match self.env.lookup_variable(target) {
                    Some(s) => s.clone(),
                    None => {
                        return Err(TypeError::UndeclaredVariable {
                            name: target.clone(),
                            span: *span,
                        });
                    }
                };

                if !sym.is_mutable {
                    return Err(TypeError::CannotMutateImmutable {
                        name: target.clone(),
                        span: *span,
                    });
                }

                let (elem_ty, len) = match &sym.ty {
                    Type::Array(elem, len) => ((**elem).clone(), *len),
                    _ => {
                        return Err(TypeError::CannotIndexNonArray {
                            found: sym.ty.clone(),
                            span: *span,
                        });
                    }
                };

                let typed_index = self.check_expr(index, Some(Type::I64))?;
                if !typed_index.ty().is_integer() {
                    return Err(TypeError::InvalidIndexType {
                        found: typed_index.ty(),
                        span: typed_index.span(),
                    });
                }

                let mut is_safe = false;
                if let TypedExpr::Literal {
                    lit: TypedLiteral::Int(n, _),
                    span: idx_span,
                    ..
                } = &typed_index
                {
                    if *n < 0 || (*n as usize) >= len {
                        return Err(TypeError::IndexOutOfBounds {
                            index: *n,
                            len,
                            span: *idx_span,
                        });
                    }
                    is_safe = true;
                } else if let TypedExpr::Ident { name: idx_var, .. } = &typed_index {
                    if let Some(&bound) = self.active_loop_bounds.get(idx_var) {
                        if bound <= len as i64 {
                            is_safe = true;
                        }
                    }
                }

                let typed_value = self.check_expr(value, Some(elem_ty.clone()))?;
                if typed_value.ty() != elem_ty {
                    return Err(TypeError::TypeMismatch {
                        expected: elem_ty,
                        found: typed_value.ty(),
                        span: typed_value.span(),
                    });
                }

                Ok(TypedStmt::IndexAssign {
                    target: target.clone(),
                    index: typed_index,
                    value: typed_value,
                    is_safe,
                    span: *span,
                })
            }

            Stmt::FieldAssign {
                target,
                field,
                value,
                span,
            } => {
                let sym = match self.env.lookup_variable(target) {
                    Some(s) => s.clone(),
                    None => {
                        return Err(TypeError::UndeclaredVariable {
                            name: target.clone(),
                            span: *span,
                        });
                    }
                };

                if !sym.is_mutable {
                    return Err(TypeError::CannotMutateImmutable {
                        name: target.clone(),
                        span: *span,
                    });
                }

                let sname = match &sym.ty {
                    Type::Struct(s) => s.clone(),
                    _ => {
                        return Err(TypeError::CannotAccessFieldNonStruct {
                            found: sym.ty.clone(),
                            span: *span,
                        });
                    }
                };

                let info = self.struct_infos.get(&sname).cloned().ok_or_else(|| TypeError::UnknownType {
                    name: sname.clone(),
                    span: *span,
                })?;

                let (_, f_ty) = info.fields.iter().find(|(n, _)| n == field).ok_or_else(|| TypeError::NoSuchField {
                    name: sname.clone(),
                    field: field.clone(),
                    span: *span,
                })?;

                let typed_val = self.check_expr(value, Some(f_ty.clone()))?;
                if typed_val.ty() != *f_ty {
                    return Err(TypeError::TypeMismatch {
                        expected: f_ty.clone(),
                        found: typed_val.ty(),
                        span: typed_val.span(),
                    });
                }

                Ok(TypedStmt::FieldAssign {
                    target: target.clone(),
                    field: field.clone(),
                    value: typed_val,
                    span: *span,
                })
            }

            Stmt::Return(opt_expr, span) => match opt_expr {
                Some(expr) => {
                    let typed_expr =
                        self.check_expr(expr, Some(self.current_fn_return_ty.clone()))?;
                    if !typed_expr.ty().is_compatible_with(&self.current_fn_return_ty) {
                        return Err(TypeError::InvalidReturn {
                            expected: self.current_fn_return_ty.clone(),
                            found: typed_expr.ty(),
                            span: *span,
                        });
                    }
                    Ok(TypedStmt::Return(Some(typed_expr), *span))
                }
                None => {
                    if self.current_fn_return_ty != Type::Void {
                        return Err(TypeError::InvalidReturn {
                            expected: self.current_fn_return_ty.clone(),
                            found: Type::Void,
                            span: *span,
                        });
                    }
                    Ok(TypedStmt::Return(None, *span))
                }
            },

            Stmt::Expr(expr) => {
                let typed_expr = self.check_expr(expr, None)?;
                Ok(TypedStmt::Expr(typed_expr))
            }

            Stmt::Break(span) => {
                if self.loop_depth == 0 {
                    Err(TypeError::BreakOutsideLoop { span: *span })
                } else {
                    Ok(TypedStmt::Break(*span))
                }
            }

            Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    Err(TypeError::ContinueOutsideLoop { span: *span })
                } else {
                    Ok(TypedStmt::Continue(*span))
                }
            }

            Stmt::Loop { body, span } => {
                let true_cond = TypedExpr::Literal {
                    lit: TypedLiteral::Bool(true),
                    ty: Type::Bool,
                    span: *span,
                };
                self.env.enter_scope();
                self.loop_depth += 1;
                let mut body_stmts = Vec::new();
                for s in &body.stmts {
                    body_stmts.push(self.check_stmt(s)?);
                }
                self.loop_depth -= 1;
                self.env.exit_scope();

                Ok(TypedStmt::While {
                    condition: true_cond,
                    body: TypedBlock {
                        stmts: body_stmts,
                        span: body.span,
                    },
                    span: *span,
                })
            }

            Stmt::For {
                var,
                lo,
                hi,
                inclusive,
                body,
                span,
            } => {
                let typed_lo = self.check_expr(lo, None)?;
                let lo_ty = typed_lo.ty();
                if !lo_ty.is_integer() {
                    return Err(TypeError::TypeMismatch {
                        expected: Type::I64,
                        found: lo_ty,
                        span: typed_lo.span(),
                    });
                }
                let typed_hi = self.check_expr(hi, Some(lo_ty.clone()))?;
                if typed_hi.ty() != lo_ty {
                    return Err(TypeError::TypeMismatch {
                        expected: lo_ty,
                        found: typed_hi.ty(),
                        span: typed_hi.span(),
                    });
                }

                if let TypedExpr::Literal {
                    lit: TypedLiteral::Int(n, _),
                    ..
                } = &typed_hi
                {
                    let bound = if *inclusive { *n + 1 } else { *n };
                    self.active_loop_bounds.insert(var.clone(), bound);
                }

                self.env.enter_scope();
                self.loop_depth += 1;
                let sym = Symbol {
                    name: var.clone(),
                    ty: lo_ty,
                    is_mutable: false,
                    span: *span,
                };
                if self.env.define_variable(sym).is_err() {
                    return Err(TypeError::DuplicateDeclaration {
                        name: var.clone(),
                        span: *span,
                    });
                }
                let mut body_stmts = Vec::new();
                for s in &body.stmts {
                    body_stmts.push(self.check_stmt(s)?);
                }
                self.loop_depth -= 1;
                self.env.exit_scope();
                self.active_loop_bounds.remove(var);

                Ok(TypedStmt::For {
                    var: var.clone(),
                    lo: typed_lo,
                    hi: typed_hi,
                    inclusive: *inclusive,
                    body: TypedBlock {
                        stmts: body_stmts,
                        span: body.span,
                    },
                    span: *span,
                })
            }

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let typed_cond = self.check_expr(condition, Some(Type::Bool))?;
                if typed_cond.ty() != Type::Bool {
                    return Err(TypeError::InvalidConditionType {
                        found: typed_cond.ty(),
                        span: typed_cond.span(),
                    });
                }

                self.env.enter_scope();
                let mut then_stmts = Vec::new();
                for s in &then_branch.stmts {
                    then_stmts.push(self.check_stmt(s)?);
                }
                self.env.exit_scope();

                let typed_else = if let Some(eb) = else_branch {
                    self.env.enter_scope();
                    let mut else_stmts = Vec::new();
                    for s in &eb.stmts {
                        else_stmts.push(self.check_stmt(s)?);
                    }
                    self.env.exit_scope();
                    Some(TypedBlock {
                        stmts: else_stmts,
                        span: eb.span,
                    })
                } else {
                    None
                };

                Ok(TypedStmt::If {
                    condition: typed_cond,
                    then_branch: TypedBlock {
                        stmts: then_stmts,
                        span: then_branch.span,
                    },
                    else_branch: typed_else,
                    span: *span,
                })
            }

            Stmt::While {
                condition,
                body,
                span,
            } => {
                let typed_cond = self.check_expr(condition, Some(Type::Bool))?;
                if typed_cond.ty() != Type::Bool {
                    return Err(TypeError::InvalidConditionType {
                        found: typed_cond.ty(),
                        span: typed_cond.span(),
                    });
                }

                let loop_bound = match condition {
                    Expr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                        if let (Expr::Ident(name, _), Expr::Literal(Literal::Int(n), _)) = (&**left, &**right) {
                            Some((name.clone(), *n))
                        } else {
                            None
                        }
                    }
                    Expr::Binary { op: BinaryOp::Le, left, right, .. } => {
                        if let (Expr::Ident(name, _), Expr::Literal(Literal::Int(n), _)) = (&**left, &**right) {
                            Some((name.clone(), *n + 1))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some((ref var_name, bound)) = loop_bound {
                    self.active_loop_bounds.insert(var_name.clone(), bound);
                }

                self.env.enter_scope();
                self.loop_depth += 1;
                let mut body_stmts = Vec::new();
                for s in &body.stmts {
                    body_stmts.push(self.check_stmt(s)?);
                }
                self.loop_depth -= 1;
                self.env.exit_scope();

                if let Some((ref var_name, _)) = loop_bound {
                    self.active_loop_bounds.remove(var_name);
                }

                Ok(TypedStmt::While {
                    condition: typed_cond,
                    body: TypedBlock {
                        stmts: body_stmts,
                        span: body.span,
                    },
                    span: *span,
                })
            }
        }
    }

    pub fn check_expr(
        &mut self,
        expr: &Expr,
        expected_hint: Option<Type>,
    ) -> Result<TypedExpr, TypeError> {
        match expr {
            Expr::Literal(lit, span) => match lit {
                Literal::Int(n) => {
                    let ty = match expected_hint {
                        Some(Type::I8) => Type::I8,
                        Some(Type::I16) => Type::I16,
                        Some(Type::I32) => Type::I32,
                        Some(Type::U8) => Type::U8,
                        Some(Type::U16) => Type::U16,
                        Some(Type::U32) => Type::U32,
                        Some(Type::U64) => Type::U64,
                        Some(Type::Usize) => Type::Usize,
                        _ => Type::I64,
                    };
                    Ok(TypedExpr::Literal {
                        lit: TypedLiteral::Int(*n, ty.clone()),
                        ty,
                        span: *span,
                    })
                }
                Literal::TypedInt(n, ref suffix) => {
                    let ty = Type::from_name(suffix).ok_or_else(|| TypeError::UnknownType {
                        name: suffix.clone(),
                        span: *span,
                    })?;
                    Ok(TypedExpr::Literal {
                        lit: TypedLiteral::Int(*n, ty.clone()),
                        ty,
                        span: *span,
                    })
                }
                Literal::Float(f) => {
                    let ty = match expected_hint {
                        Some(Type::F32) => Type::F32,
                        _ => Type::F64,
                    };
                    Ok(TypedExpr::Literal {
                        lit: TypedLiteral::Float(*f, ty.clone()),
                        ty,
                        span: *span,
                    })
                }
                Literal::Bool(b) => Ok(TypedExpr::Literal {
                    lit: TypedLiteral::Bool(*b),
                    ty: Type::Bool,
                    span: *span,
                }),
                Literal::Str(ref s) => Ok(TypedExpr::Literal {
                    lit: TypedLiteral::Str(s.clone()),
                    ty: Type::Str,
                    span: *span,
                }),
            },

            Expr::Ident(name, span) => {
                if let Some(sym) = self.env.lookup_variable(name) {
                    return Ok(TypedExpr::Ident {
                        name: name.clone(),
                        ty: sym.ty.clone(),
                        span: *span,
                    });
                }

                if let Some(candidates) = self.variant_to_enum.get(name) {
                    let en = if let Some(Type::Enum(ref expected_en)) = expected_hint {
                        if candidates.contains(expected_en) {
                            expected_en.clone()
                        } else {
                            candidates[0].clone()
                        }
                    } else {
                        candidates[0].clone()
                    };

                    if let Some(enum_info) = self.enum_infos.get(&en) {
                        if let Some(&var_idx) = enum_info.variant_indices.get(name) {
                            let var_info = &enum_info.variants[var_idx];
                            if var_info.payload.is_empty() {
                                return Ok(TypedExpr::EnumConstructor {
                                    enum_name: en.clone(),
                                    variant_name: name.clone(),
                                    tag: var_info.tag,
                                    args: Vec::new(),
                                    ty: Type::Enum(en),
                                    span: *span,
                                });
                            }
                        }
                    }
                }

                if let Some(fn_sig) = self.env.lookup_function(name) {
                    let fn_ty = Type::Fn(
                        fn_sig.param_types.clone(),
                        Box::new(fn_sig.return_ty.clone()),
                    );
                    return Ok(TypedExpr::Ident {
                        name: name.clone(),
                        ty: fn_ty,
                        span: *span,
                    });
                }

                Err(TypeError::UndeclaredVariable {
                    name: name.clone(),
                    span: *span,
                })
            }

            Expr::Group(inner, _) => self.check_expr(inner, expected_hint),

            Expr::Unary { op, expr, span } => {
                let typed_inner = self.check_expr(expr, expected_hint)?;
                match op {
                    UnaryOp::Not => {
                        if typed_inner.ty() != Type::Bool {
                            return Err(TypeError::InvalidUnaryOperand {
                                op: *op,
                                found: typed_inner.ty(),
                                span: *span,
                            });
                        }
                        Ok(TypedExpr::Unary {
                            op: *op,
                            expr: Box::new(typed_inner),
                            ty: Type::Bool,
                            span: *span,
                        })
                    }
                    UnaryOp::Neg => {
                        if let Type::Array(ref elem, _) = typed_inner.ty() {
                            if !elem.is_numeric() {
                                return Err(TypeError::InvalidUnaryOperand {
                                    op: *op,
                                    found: typed_inner.ty(),
                                    span: *span,
                                });
                            }
                            let ty = typed_inner.ty();
                            let neg_one = if elem.is_float() {
                                TypedExpr::Literal {
                                    lit: TypedLiteral::Float(-1.0, (**elem).clone()),
                                    ty: (**elem).clone(),
                                    span: *span,
                                }
                            } else {
                                TypedExpr::Literal {
                                    lit: TypedLiteral::Int(-1, (**elem).clone()),
                                    ty: (**elem).clone(),
                                    span: *span,
                                }
                            };
                            return Ok(TypedExpr::Call {
                                callee: "vec_scale".to_string(),
                                args: vec![typed_inner, neg_one],
                                ty,
                                span: *span,
                            });
                        }
                        if !typed_inner.ty().is_numeric() {
                            return Err(TypeError::InvalidUnaryOperand {
                                op: *op,
                                found: typed_inner.ty(),
                                span: *span,
                            });
                        }
                        let ty = typed_inner.ty();
                        Ok(TypedExpr::Unary {
                            op: *op,
                            expr: Box::new(typed_inner),
                            ty,
                            span: *span,
                        })
                    }
                }
            }

            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let typed_left = self.check_expr(left, expected_hint.clone())?;
                let right_hint = match typed_left.ty() {
                    Type::Array(ref elem, _) => Some((**elem).clone()),
                    ref t => Some(t.clone()),
                };
                let typed_right = self.check_expr(right, right_hint)?;

                let lty = typed_left.ty();
                let rty = typed_right.ty();

                // 1. Array-Array elementwise operations
                if let (Type::Array(ref elem_l, len_l), Type::Array(ref elem_r, len_r)) = (&lty, &rty) {
                    if elem_l != elem_r || len_l != len_r || !elem_l.is_numeric() {
                        return Err(TypeError::InvalidBinaryOperands {
                            op: *op,
                            left: lty,
                            right: rty,
                            span: *span,
                        });
                    }
                    match op {
                        BinaryOp::Add => {
                            return Ok(TypedExpr::Call {
                                callee: "vec_add".to_string(),
                                args: vec![typed_left, typed_right],
                                ty: lty,
                                span: *span,
                            });
                        }
                        BinaryOp::Sub => {
                            return Ok(TypedExpr::Call {
                                callee: "vec_sub".to_string(),
                                args: vec![typed_left, typed_right],
                                ty: lty,
                                span: *span,
                            });
                        }
                        BinaryOp::Mul => {
                            return Ok(TypedExpr::Call {
                                callee: "vec_mul".to_string(),
                                args: vec![typed_left, typed_right],
                                ty: lty,
                                span: *span,
                            });
                        }
                        _ => {
                            return Err(TypeError::InvalidBinaryOperands {
                                op: *op,
                                left: lty,
                                right: rty,
                                span: *span,
                            });
                        }
                    }
                }

                // 2. Array * Scalar, Array / Scalar
                if let Type::Array(ref elem_l, _) = lty {
                    if **elem_l == rty && rty.is_numeric() {
                        match op {
                            BinaryOp::Mul => {
                                return Ok(TypedExpr::Call {
                                    callee: "vec_scale".to_string(),
                                    args: vec![typed_left, typed_right],
                                    ty: lty,
                                    span: *span,
                                });
                            }
                            BinaryOp::Div => {
                                return Ok(TypedExpr::Call {
                                    callee: "vec_div_scalar".to_string(),
                                    args: vec![typed_left, typed_right],
                                    ty: lty,
                                    span: *span,
                                });
                            }
                            _ => {}
                        }
                    }
                }

                // 3. Scalar * Array
                if let Type::Array(ref elem_r, _) = rty {
                    if **elem_r == lty && lty.is_numeric()
                        && *op == BinaryOp::Mul {
                            return Ok(TypedExpr::Call {
                                callee: "vec_scale".to_string(),
                                args: vec![typed_right, typed_left],
                                ty: rty,
                                span: *span,
                            });
                        }
                }

                if lty != rty {
                    return Err(TypeError::InvalidBinaryOperands {
                        op: *op,
                        left: lty,
                        right: rty,
                        span: *span,
                    });
                }

                match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::Pow => {
                        if !lty.is_numeric() {
                            return Err(TypeError::InvalidBinaryOperands {
                                op: *op,
                                left: lty,
                                right: rty,
                                span: *span,
                            });
                        }
                        Ok(TypedExpr::Binary {
                            op: *op,
                            left: Box::new(typed_left),
                            right: Box::new(typed_right),
                            ty: lty,
                            span: *span,
                        })
                    }
                    BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Shl
                    | BinaryOp::Shr => {
                        if !lty.is_integer() {
                            return Err(TypeError::InvalidBinaryOperands {
                                op: *op,
                                left: lty,
                                right: rty,
                                span: *span,
                            });
                        }
                        Ok(TypedExpr::Binary {
                            op: *op,
                            left: Box::new(typed_left),
                            right: Box::new(typed_right),
                            ty: lty,
                            span: *span,
                        })
                    }
                    BinaryOp::Eq | BinaryOp::Ne => Ok(TypedExpr::Binary {
                        op: *op,
                        left: Box::new(typed_left),
                        right: Box::new(typed_right),
                        ty: Type::Bool,
                        span: *span,
                    }),
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        if !lty.is_numeric() {
                            return Err(TypeError::InvalidBinaryOperands {
                                op: *op,
                                left: lty,
                                right: rty,
                                span: *span,
                            });
                        }
                        Ok(TypedExpr::Binary {
                            op: *op,
                            left: Box::new(typed_left),
                            right: Box::new(typed_right),
                            ty: Type::Bool,
                            span: *span,
                        })
                    }
                }
            }

            Expr::ArrayLiteral { elements, span } => {
                if elements.is_empty() {
                    return Err(TypeError::EmptyArrayLiteral { span: *span });
                }
                let expected_elem_hint = match &expected_hint {
                    Some(Type::Array(elem, _)) => Some((**elem).clone()),
                    _ => None,
                };
                let first_typed = self.check_expr(&elements[0], expected_elem_hint)?;
                let elem_ty = first_typed.ty();
                let mut typed_elements = vec![first_typed];
                for el in &elements[1..] {
                    let typed_el = self.check_expr(el, Some(elem_ty.clone()))?;
                    if typed_el.ty() != elem_ty {
                        return Err(TypeError::ArrayElementMismatch {
                            expected: elem_ty.clone(),
                            found: typed_el.ty(),
                            span: typed_el.span(),
                        });
                    }
                    typed_elements.push(typed_el);
                }
                let len = typed_elements.len();
                let arr_ty = Type::Array(Box::new(elem_ty), len);
                Ok(TypedExpr::ArrayLiteral {
                    elements: typed_elements,
                    ty: arr_ty,
                    span: *span,
                })
            }

            Expr::Index { target, index, span } => {
                let typed_target = self.check_expr(target, None)?;
                let target_ty = typed_target.ty();
                let (elem_ty, len) = match &target_ty {
                    Type::Array(elem, len) => ((**elem).clone(), *len),
                    _ => {
                        return Err(TypeError::CannotIndexNonArray {
                            found: target_ty,
                            span: target.span(),
                        });
                    }
                };

                let typed_index = self.check_expr(index, Some(Type::I64))?;
                if !typed_index.ty().is_integer() {
                    return Err(TypeError::InvalidIndexType {
                        found: typed_index.ty(),
                        span: typed_index.span(),
                    });
                }

                let mut is_safe = false;
                if let TypedExpr::Literal {
                    lit: TypedLiteral::Int(n, _),
                    span: idx_span,
                    ..
                } = &typed_index
                {
                    if *n < 0 || (*n as usize) >= len {
                        return Err(TypeError::IndexOutOfBounds {
                            index: *n,
                            len,
                            span: *idx_span,
                        });
                    }
                    is_safe = true;
                } else if let TypedExpr::Ident { name: idx_var, .. } = &typed_index {
                    if let Some(&bound) = self.active_loop_bounds.get(idx_var) {
                        if bound <= len as i64 {
                            is_safe = true;
                        }
                    }
                }

                Ok(TypedExpr::Index {
                    target: Box::new(typed_target),
                    index: Box::new(typed_index),
                    is_safe,
                    ty: elem_ty,
                    span: *span,
                })
            }

            Expr::Call { callee, args, span } => {
                // Built-in intrinsics
                match callee.as_str() {
                    "print" | "println" => {
                        if args.len() != 1 {
                            if callee == "println" && args.is_empty() {
                                return Ok(TypedExpr::Call {
                                    callee: callee.clone(),
                                    args: vec![],
                                    ty: Type::Void,
                                    span: *span,
                                });
                            }
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], None)?;
                        let arg_ty = typed_arg.ty();
                        if !arg_ty.is_numeric() && arg_ty != Type::Bool && arg_ty != Type::Str {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: arg_ty,
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![typed_arg],
                            ty: Type::Void,
                            span: *span,
                        });
                    }
                    "sqrt" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "sqrt".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::F64))?;
                        if !typed_arg.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        let ty = typed_arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: "sqrt".to_string(),
                            args: vec![typed_arg],
                            ty,
                            span: *span,
                        });
                    }
                    "abs" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "abs".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], expected_hint)?;
                        if !typed_arg.ty().is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        let ty = typed_arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: "abs".to_string(),
                            args: vec![typed_arg],
                            ty,
                            span: *span,
                        });
                    }
                    "to_int" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "to_int".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::F64))?;
                        return Ok(TypedExpr::Call {
                            callee: "to_int".to_string(),
                            args: vec![typed_arg],
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    "to_float" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "to_float".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::I64))?;
                        return Ok(TypedExpr::Call {
                            callee: "to_float".to_string(),
                            args: vec![typed_arg],
                            ty: Type::F64,
                            span: *span,
                        });
                    }
                    "i64_to_f64" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "i64_to_f64".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::I64))?;
                        if typed_arg.ty() != Type::I64 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "i64_to_f64".to_string(),
                            args: vec![typed_arg],
                            ty: Type::F64,
                            span: *span,
                        });
                    }
                    "f64_to_i64" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "f64_to_i64".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::F64))?;
                        if typed_arg.ty() != Type::F64 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "f64_to_i64".to_string(),
                            args: vec![typed_arg],
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    "i64_to_f32" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "i64_to_f32".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::I64))?;
                        if typed_arg.ty() != Type::I64 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "i64_to_f32".to_string(),
                            args: vec![typed_arg],
                            ty: Type::F32,
                            span: *span,
                        });
                    }
                    "f32_to_f64" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "f32_to_f64".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::F32))?;
                        if typed_arg.ty() != Type::F32 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F32,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "f32_to_f64".to_string(),
                            args: vec![typed_arg],
                            ty: Type::F64,
                            span: *span,
                        });
                    }
                    "f64_to_f32" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "f64_to_f32".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::F64))?;
                        if typed_arg.ty() != Type::F64 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "f64_to_f32".to_string(),
                            args: vec![typed_arg],
                            ty: Type::F32,
                            span: *span,
                        });
                    }
                    "i64_to_i32" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "i64_to_i32".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::I64))?;
                        if typed_arg.ty() != Type::I64 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "i64_to_i32".to_string(),
                            args: vec![typed_arg],
                            ty: Type::I32,
                            span: *span,
                        });
                    }
                    "i32_to_i64" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "i32_to_i64".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::I32))?;
                        if typed_arg.ty() != Type::I32 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I32,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "i32_to_i64".to_string(),
                            args: vec![typed_arg],
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    "isqrt" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "isqrt".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::I64))?;
                        if !typed_arg.ty().is_integer() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        let ty = typed_arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: "isqrt".to_string(),
                            args: vec![typed_arg],
                            ty,
                            span: *span,
                        });
                    }
                    "tzcnt" | "ctz" | "clz" | "popcnt" | "bswap" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], expected_hint)?;
                        if !typed_arg.ty().is_integer() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        let ty = typed_arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![typed_arg],
                            ty,
                            span: *span,
                        });
                    }
                    "rotl" | "rotr" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_val = self.check_expr(&args[0], expected_hint)?;
                        if !typed_val.ty().is_integer() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: typed_val.ty(),
                                span: typed_val.span(),
                            });
                        }
                        let typed_shift = self.check_expr(&args[1], Some(Type::I64))?;
                        if !typed_shift.ty().is_integer() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: typed_shift.ty(),
                                span: typed_shift.span(),
                            });
                        }
                        let ty = typed_val.ty();
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![typed_val, typed_shift],
                            ty,
                            span: *span,
                        });
                    }
                    "to_i64" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "to_i64".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], None)?;
                        if !typed_arg.ty().is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "to_i64".to_string(),
                            args: vec![typed_arg],
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    "dot" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "dot".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: a.ty(),
                                    span: a.span(),
                                });
                            }
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: b.ty(),
                                    span: b.span(),
                                });
                            }
                        };
                        if elem_ty_a != elem_ty_b || len_a != len_b {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), len_a),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        if !elem_ty_a.is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: elem_ty_a,
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "dot".to_string(),
                            args: vec![a, b],
                            ty: elem_ty_a,
                            span: *span,
                        });
                    }
                    "vec_add" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "vec_add".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: a.ty(),
                                    span: a.span(),
                                });
                            }
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: b.ty(),
                                    span: b.span(),
                                });
                            }
                        };
                        if elem_ty_a != elem_ty_b || len_a != len_b {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), len_a),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty_a), len_a);
                        return Ok(TypedExpr::Call {
                            callee: "vec_add".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "sum" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "sum".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let elem_ty = match a.ty() {
                            Type::Array(elem, _) => *elem,
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: a.ty(),
                                    span: a.span(),
                                });
                            }
                        };
                        if !elem_ty.is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: elem_ty,
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "sum".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "min" | "max" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], expected_hint.clone())?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        if a.ty() != b.ty() || !a.ty().is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: a.ty(),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let ty = a.ty();
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![a, b],
                            ty,
                            span: *span,
                        });
                    }
                    "clamp" => {
                        if args.len() != 3 {
                            return Err(TypeError::ArityMismatch {
                                name: "clamp".to_string(),
                                expected: 3,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let val = self.check_expr(&args[0], expected_hint.clone())?;
                        let lo = self.check_expr(&args[1], Some(val.ty()))?;
                        let hi = self.check_expr(&args[2], Some(val.ty()))?;
                        if val.ty() != lo.ty() || val.ty() != hi.ty() || !val.ty().is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: val.ty(),
                                found: lo.ty(),
                                span: lo.span(),
                            });
                        }
                        let ty = val.ty();
                        return Ok(TypedExpr::Call {
                            callee: "clamp".to_string(),
                            args: vec![val, lo, hi],
                            ty,
                            span: *span,
                        });
                    }
                    "floor" | "ceil" | "round" | "trunc" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let arg = self.check_expr(&args[0], Some(Type::F64))?;
                        if !arg.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: arg.ty(),
                                span: arg.span(),
                            });
                        }
                        let ty = arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![arg],
                            ty,
                            span: *span,
                        });
                    }
                    "fma" => {
                        if args.len() != 3 {
                            return Err(TypeError::ArityMismatch {
                                name: "fma".to_string(),
                                expected: 3,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], expected_hint.clone())?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let c = self.check_expr(&args[2], Some(a.ty()))?;
                        if a.ty() != b.ty() || a.ty() != c.ty() || !a.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let ty = a.ty();
                        return Ok(TypedExpr::Call {
                            callee: "fma".to_string(),
                            args: vec![a, b, c],
                            ty,
                            span: *span,
                        });
                    }
                    "hypot" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "hypot".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], expected_hint.clone())?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        if a.ty() != b.ty() || !a.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let ty = a.ty();
                        return Ok(TypedExpr::Call {
                            callee: "hypot".to_string(),
                            args: vec![a, b],
                            ty,
                            span: *span,
                        });
                    }
                    "lerp" => {
                        if args.len() != 3 {
                            return Err(TypeError::ArityMismatch {
                                name: "lerp".to_string(),
                                expected: 3,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], expected_hint.clone())?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let t = self.check_expr(&args[2], Some(a.ty()))?;
                        if a.ty() != b.ty() || a.ty() != t.ty() || !a.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let ty = a.ty();
                        return Ok(TypedExpr::Call {
                            callee: "lerp".to_string(),
                            args: vec![a, b, t],
                            ty,
                            span: *span,
                        });
                    }
                    "signum" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "signum".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let arg = self.check_expr(&args[0], expected_hint)?;
                        if !arg.ty().is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: arg.ty(),
                                span: arg.span(),
                            });
                        }
                        let ty = arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: "signum".to_string(),
                            args: vec![arg],
                            ty,
                            span: *span,
                        });
                    }
                    "gcd" | "lcm" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], expected_hint.clone())?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        if a.ty() != b.ty() || !a.ty().is_integer() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let ty = a.ty();
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![a, b],
                            ty,
                            span: *span,
                        });
                    }
                    "vec_sub" | "vec_mul" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if elem_ty_a != elem_ty_b || len_a != len_b {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), len_a),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty_a), len_a);
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "vec_scale" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "vec_scale".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let v = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match v.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: v.ty(), span: v.span() }),
                        };
                        let s = self.check_expr(&args[1], Some(elem_ty.clone()))?;
                        if s.ty() != elem_ty {
                            return Err(TypeError::TypeMismatch {
                                expected: elem_ty.clone(),
                                found: s.ty(),
                                span: s.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), len);
                        return Ok(TypedExpr::Call {
                            callee: "vec_scale".to_string(),
                            args: vec![v, s],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "vec_div_scalar" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "vec_div_scalar".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let v = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match v.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: v.ty(), span: v.span() }),
                        };
                        let s = self.check_expr(&args[1], Some(elem_ty.clone()))?;
                        if s.ty() != elem_ty {
                            return Err(TypeError::TypeMismatch {
                                expected: elem_ty.clone(),
                                found: s.ty(),
                                span: s.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), len);
                        return Ok(TypedExpr::Call {
                            callee: "vec_div_scalar".to_string(),
                            args: vec![v, s],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "vec_norm" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "vec_norm".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let v = self.check_expr(&args[0], None)?;
                        let (elem_ty, _) = match v.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: v.ty(), span: v.span() }),
                        };
                        if !elem_ty.is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: elem_ty,
                                span: v.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "vec_norm".to_string(),
                            args: vec![v],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "vec_cross3" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "vec_cross3".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if elem_ty_a != elem_ty_b || len_a != 3 || len_b != 3 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), 3),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty_a), 3);
                        return Ok(TypedExpr::Call {
                            callee: "vec_cross3".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_mul4" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_mul4".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if elem_ty_a != elem_ty_b || len_a != 16 || len_b != 16 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), 16),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty_a), 16);
                        return Ok(TypedExpr::Call {
                            callee: "mat_mul4".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_transpose2" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_transpose2".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 4 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 4),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), 4);
                        return Ok(TypedExpr::Call {
                            callee: "mat_transpose2".to_string(),
                            args: vec![a],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_transpose3" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_transpose3".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 9 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 9),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), 9);
                        return Ok(TypedExpr::Call {
                            callee: "mat_transpose3".to_string(),
                            args: vec![a],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_transpose4" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_transpose4".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 16 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 16),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), 16);
                        return Ok(TypedExpr::Call {
                            callee: "mat_transpose4".to_string(),
                            args: vec![a],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_trace2" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_trace2".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 4 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 4),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "mat_trace2".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "mat_trace3" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_trace3".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 9 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 9),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "mat_trace3".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "mat_trace4" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_trace4".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 16 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 16),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "mat_trace4".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "mat_det2" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_det2".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 4 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 4),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "mat_det2".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "mat_det3" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_det3".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 9 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 9),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "mat_det3".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "mat_det4" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_det4".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 16 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty.clone()), 16),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "mat_det4".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    "mat_inv2" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_inv2".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 4 || !elem_ty.is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 4),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), 4);
                        return Ok(TypedExpr::Call {
                            callee: "mat_inv2".to_string(),
                            args: vec![a],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_inv3" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_inv3".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 9 || !elem_ty.is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 9),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), 9);
                        return Ok(TypedExpr::Call {
                            callee: "mat_inv3".to_string(),
                            args: vec![a],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_inv4" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_inv4".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let (elem_ty, len) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        if len != 16 || !elem_ty.is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 16),
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty), 16);
                        return Ok(TypedExpr::Call {
                            callee: "mat_inv4".to_string(),
                            args: vec![a],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_solve2" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_solve2".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], None)?;
                        let (elem_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if len_a != 4 || len_b != 2 || elem_a != elem_b || !elem_a.is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 2),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_a), 2);
                        return Ok(TypedExpr::Call {
                            callee: "mat_solve2".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_solve3" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_solve3".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], None)?;
                        let (elem_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if len_a != 9 || len_b != 3 || elem_a != elem_b || !elem_a.is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 3),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_a), 3);
                        return Ok(TypedExpr::Call {
                            callee: "mat_solve3".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_solve4" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_solve4".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], None)?;
                        let (elem_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if len_a != 16 || len_b != 4 || elem_a != elem_b || !elem_a.is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 4),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_a), 4);
                        return Ok(TypedExpr::Call {
                            callee: "mat_solve4".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_mul2" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_mul2".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if elem_ty_a != elem_ty_b || len_a != 4 || len_b != 4 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), 4),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty_a), 4);
                        return Ok(TypedExpr::Call {
                            callee: "mat_mul2".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "mat_mul3" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "mat_mul3".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: a.ty(), span: a.span() }),
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => return Err(TypeError::CannotIndexNonArray { found: b.ty(), span: b.span() }),
                        };
                        if elem_ty_a != elem_ty_b || len_a != 9 || len_b != 9 {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), 9),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty_a), 9);
                        return Ok(TypedExpr::Call {
                            callee: "mat_mul3".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "sin" | "cos" | "tan" | "exp" | "ln" | "log2" | "log10" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], expected_hint)?;
                        if !a.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let ty = a.ty();
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![a],
                            ty,
                            span: *span,
                        });
                    }
                    "atan2" | "powf" | "pow" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], expected_hint.clone())?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        if a.ty() != b.ty() || !a.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: a.ty(),
                                span: a.span(),
                            });
                        }
                        let ty = a.ty();
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![a, b],
                            ty,
                            span: *span,
                        });
                    }
                    "c_make" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "c_make".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let re = self.check_expr(&args[0], Some(Type::F64))?;
                        let im = self.check_expr(&args[1], Some(Type::F64))?;
                        if !re.ty().is_float() || !im.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: re.ty(),
                                span: re.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "c_make".to_string(),
                            args: vec![re, im],
                            ty: Type::Array(Box::new(Type::F64), 2),
                            span: *span,
                        });
                    }
                    "c_re" | "c_im" | "c_abs" | "c_arg" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let z = self.check_expr(&args[0], None)?;
                        match z.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 2 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 2),
                                found: z.ty(),
                                span: z.span(),
                            }),
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![z],
                            ty: Type::F64,
                            span: *span,
                        });
                    }
                    "c_add" | "c_sub" | "c_mul" | "c_div" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        match a.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 2 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 2),
                                found: a.ty(),
                                span: a.span(),
                            }),
                        }
                        match b.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 2 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 2),
                                found: b.ty(),
                                span: b.span(),
                            }),
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![a, b],
                            ty: Type::Array(Box::new(Type::F64), 2),
                            span: *span,
                        });
                    }
                    "c_conj" | "c_exp" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let z = self.check_expr(&args[0], None)?;
                        match z.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 2 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 2),
                                found: z.ty(),
                                span: z.span(),
                            }),
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![z],
                            ty: Type::Array(Box::new(Type::F64), 2),
                            span: *span,
                        });
                    }
                    "fft8" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "fft8".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let re = self.check_expr(&args[0], None)?;
                        let im = self.check_expr(&args[1], None)?;
                        match re.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 8 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 8),
                                found: re.ty(),
                                span: re.span(),
                            }),
                        }
                        match im.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 8 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 8),
                                found: im.ty(),
                                span: im.span(),
                            }),
                        }
                        return Ok(TypedExpr::Call {
                            callee: "fft8".to_string(),
                            args: vec![re, im],
                            ty: Type::Array(Box::new(Type::F64), 16),
                            span: *span,
                        });
                    }
                    "fft8_re" | "fft8_im" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let re = self.check_expr(&args[0], None)?;
                        let im = self.check_expr(&args[1], None)?;
                        match re.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 8 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 8),
                                found: re.ty(),
                                span: re.span(),
                            }),
                        }
                        match im.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 8 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 8),
                                found: im.ty(),
                                span: im.span(),
                            }),
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![re, im],
                            ty: Type::Array(Box::new(Type::F64), 8),
                            span: *span,
                        });
                    }
                    "fft16_re" | "fft16_im" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let re = self.check_expr(&args[0], None)?;
                        let im = self.check_expr(&args[1], None)?;
                        match re.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 16 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 16),
                                found: re.ty(),
                                span: re.span(),
                            }),
                        }
                        match im.ty() {
                            Type::Array(ref elem, len) if **elem == Type::F64 && len == 16 => {}
                            _ => return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(Type::F64), 16),
                                found: im.ty(),
                                span: im.span(),
                            }),
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: vec![re, im],
                            ty: Type::Array(Box::new(Type::F64), 16),
                            span: *span,
                        });
                    }
                    "__coupled_a" | "__coupled_b" => {
                        if args.len() != 9 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 9,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let mut typed_args = Vec::with_capacity(9);
                        for arg in args {
                            let typed_arg = self.check_expr(arg, Some(Type::I64))?;
                            if typed_arg.ty() != Type::I64 {
                                return Err(TypeError::TypeMismatch {
                                    expected: Type::I64,
                                    found: typed_arg.ty(),
                                    span: typed_arg.span(),
                                });
                            }
                            typed_args.push(typed_arg);
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: typed_args,
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    "__order3_recurrence" => {
                        if args.len() != 7 {
                            return Err(TypeError::ArityMismatch {
                                name: callee.clone(),
                                expected: 7,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let mut typed_args = Vec::with_capacity(7);
                        for arg in args {
                            let typed_arg = self.check_expr(arg, Some(Type::I64))?;
                            if typed_arg.ty() != Type::I64 {
                                return Err(TypeError::TypeMismatch {
                                    expected: Type::I64,
                                    found: typed_arg.ty(),
                                    span: typed_arg.span(),
                                });
                            }
                            typed_args.push(typed_arg);
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.clone(),
                            args: typed_args,
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    callee if callee.starts_with("__nway_recurrence_") => {
                        let mut typed_args = Vec::with_capacity(args.len());
                        for arg in args {
                            let typed_arg = self.check_expr(arg, Some(Type::I64))?;
                            if typed_arg.ty() != Type::I64 {
                                return Err(TypeError::TypeMismatch {
                                    expected: Type::I64,
                                    found: typed_arg.ty(),
                                    span: typed_arg.span(),
                                });
                            }
                            typed_args.push(typed_arg);
                        }
                        return Ok(TypedExpr::Call {
                            callee: callee.to_string(),
                            args: typed_args,
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    _ => {}
                }

                // Check if callee is a local variable holding a closure or function pointer
                if let Some(sym) = self.env.lookup_variable(callee).cloned() {
                    let (param_types, ret_ty): (Vec<Type>, Type) = match &sym.ty {
                        Type::Fn(param_types, ret_ty) => (param_types.clone(), (**ret_ty).clone()),
                        Type::Closure(c) => (c.params.clone(), (*c.ret).clone()),
                        other => {
                            return Err(TypeError::NotCallable {
                                ty: other.clone(),
                                span: *span,
                            });
                        }
                    };

                    if args.len() != param_types.len() {
                        return Err(TypeError::ArityMismatch {
                            name: callee.clone(),
                            expected: param_types.len(),
                            found: args.len(),
                            span: *span,
                        });
                    }
                    let mut typed_args = Vec::new();
                    for (arg, param_ty) in args.iter().zip(&param_types) {
                        let typed_arg = self.check_expr(arg, Some(param_ty.clone()))?;
                        if !typed_arg.ty().is_compatible_with(param_ty) {
                            return Err(TypeError::TypeMismatch {
                                expected: param_ty.clone(),
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        typed_args.push(typed_arg);
                    }
                    let callee_expr = Box::new(TypedExpr::Ident {
                        name: callee.clone(),
                        ty: sym.ty.clone(),
                        span: *span,
                    });
                    return Ok(TypedExpr::CallIndirect {
                        callee: callee_expr,
                        args: typed_args,
                        ty: ret_ty,
                        span: *span,
                    });
                }

                let sig = match self.env.lookup_function(callee).cloned() {
                    Some(s) => s,
                    None => {
                        if let Some(candidates) = self.variant_to_enum.get(callee) {
                            let en = if let Some(Type::Enum(ref expected_en)) = expected_hint {
                                if candidates.contains(expected_en) {
                                    expected_en.clone()
                                } else {
                                    candidates[0].clone()
                                }
                            } else {
                                candidates[0].clone()
                            };
                            return self.check_expr(
                                &Expr::EnumConstructor {
                                    enum_name: Some(en),
                                    variant_name: callee.clone(),
                                    args: args.clone(),
                                    span: *span,
                                },
                                expected_hint,
                            );
                        }
                        return Err(TypeError::UndeclaredFunction {
                            name: callee.clone(),
                            span: *span,
                        });
                    }
                };

                if args.len() != sig.param_types.len() {
                    return Err(TypeError::ArityMismatch {
                        name: callee.clone(),
                        expected: sig.param_types.len(),
                        found: args.len(),
                        span: *span,
                    });
                }

                if !sig.type_params.is_empty() {
                    let mut typed_args = Vec::new();
                    let mut subst = HashMap::new();
                    for (arg, param_ty) in args.iter().zip(&sig.param_types) {
                        let typed_arg = self.check_expr(arg, None)?;
                        crate::opt::monomorphize::unify_types(param_ty, &typed_arg.ty(), &mut subst, *span)?;
                        typed_args.push(typed_arg);
                    }
                    let return_ty = crate::opt::monomorphize::substitute_type(&sig.return_ty, &subst);
                    return Ok(TypedExpr::Call {
                        callee: callee.clone(),
                        args: typed_args,
                        ty: return_ty,
                        span: *span,
                    });
                }

                let mut typed_args = Vec::new();
                for (arg, param_ty) in args.iter().zip(&sig.param_types) {
                    let typed_arg = self.check_expr(arg, Some(param_ty.clone()))?;
                    if !typed_arg.ty().is_compatible_with(param_ty) {
                        return Err(TypeError::TypeMismatch {
                            expected: param_ty.clone(),
                            found: typed_arg.ty(),
                            span: typed_arg.span(),
                        });
                    }
                    typed_args.push(typed_arg);
                }

                Ok(TypedExpr::Call {
                    callee: callee.clone(),
                    args: typed_args,
                    ty: sig.return_ty,
                    span: *span,
                })
            }

            Expr::StructLiteral { name, fields, span } => {
                let info = match self.struct_infos.get(name) {
                    Some(inf) => inf.clone(),
                    None => {
                        return Err(TypeError::UnknownType {
                            name: name.clone(),
                            span: *span,
                        });
                    }
                };

                let mut provided_fields = HashMap::new();
                for (f_name, f_expr) in fields {
                    if provided_fields.contains_key(f_name) {
                        return Err(TypeError::DuplicateDeclaration {
                            name: f_name.clone(),
                            span: f_expr.span(),
                        });
                    }
                    provided_fields.insert(f_name.clone(), f_expr);
                }

                for (decl_name, _) in &info.fields {
                    if !provided_fields.contains_key(decl_name) {
                        return Err(TypeError::MissingField {
                            name: name.clone(),
                            field: decl_name.clone(),
                            span: *span,
                        });
                    }
                }

                for (prov_name, prov_expr) in &provided_fields {
                    if !info.field_indices.contains_key(prov_name) {
                        return Err(TypeError::NoSuchField {
                            name: name.clone(),
                            field: prov_name.clone(),
                            span: prov_expr.span(),
                        });
                    }
                }

                let mut typed_fields = Vec::new();
                for (decl_name, decl_ty) in &info.fields {
                    let f_expr = provided_fields.get(decl_name).unwrap();
                    let typed_expr = self.check_expr(f_expr, Some(decl_ty.clone()))?;
                    if typed_expr.ty() != *decl_ty {
                        return Err(TypeError::TypeMismatch {
                            expected: decl_ty.clone(),
                            found: typed_expr.ty(),
                            span: typed_expr.span(),
                        });
                    }
                    typed_fields.push((decl_name.clone(), typed_expr));
                }

                Ok(TypedExpr::StructLiteral {
                    name: name.clone(),
                    fields: typed_fields,
                    ty: Type::Struct(name.clone()),
                    span: *span,
                })
            }

            Expr::FieldAccess { target, field, span } => {
                let typed_target = self.check_expr(target, None)?;
                let sname = match typed_target.ty() {
                    Type::Struct(s) => s,
                    other => {
                        return Err(TypeError::CannotAccessFieldNonStruct {
                            found: other,
                            span: *span,
                        });
                    }
                };

                let info = match self.struct_infos.get(&sname) {
                    Some(inf) => inf.clone(),
                    None => {
                        return Err(TypeError::UnknownType {
                            name: sname.clone(),
                            span: *span,
                        });
                    }
                };

                let (_, f_ty) = info.fields.iter().find(|(n, _)| n == field).ok_or_else(|| TypeError::NoSuchField {
                    name: sname.clone(),
                    field: field.clone(),
                    span: *span,
                })?;

                Ok(TypedExpr::FieldAccess {
                    target: Box::new(typed_target),
                    field: field.clone(),
                    ty: f_ty.clone(),
                    span: *span,
                })
            }

            Expr::Match { scrutinee, arms, span } => {
                if arms.is_empty() {
                    return Err(TypeError::EmptyMatch { span: *span });
                }
                let typed_scrutinee = self.check_expr(scrutinee, None)?;
                let scrutinee_ty = typed_scrutinee.ty();

                if !scrutinee_ty.is_integer() && scrutinee_ty != Type::Bool && !scrutinee_ty.is_enum() {
                    return Err(TypeError::TypeMismatch {
                        expected: Type::I64,
                        found: scrutinee_ty,
                        span: typed_scrutinee.span(),
                    });
                }

                let mut typed_arms = Vec::new();
                let mut common_body_ty: Option<Type> = None;
                let mut has_wildcard = false;
                let mut seen_bool_true = false;
                let mut seen_bool_false = false;
                let mut seen_variants = std::collections::HashSet::new();

                for arm in arms {
                    let mut typed_patterns = Vec::new();
                    let mut arm_bindings: Vec<(String, Type)> = Vec::new();
                    for pat in &arm.patterns {
                        match pat {
                            MatchPattern::Wildcard => {
                                has_wildcard = true;
                                typed_patterns.push(TypedMatchPattern::Wildcard);
                            }
                            MatchPattern::Variant {
                                enum_name,
                                variant_name,
                                bindings,
                                span: pat_span,
                            } => {
                                if !scrutinee_ty.is_enum() {
                                    return Err(TypeError::CannotMatchNonEnum {
                                        found: scrutinee_ty.clone(),
                                        span: *pat_span,
                                    });
                                }
                                let s_enum_name = match &scrutinee_ty {
                                    Type::Enum(n) => n.clone(),
                                    _ => unreachable!(),
                                };
                                if let Some(ref specified_en) = enum_name {
                                    if specified_en != &s_enum_name {
                                        return Err(TypeError::TypeMismatch {
                                            expected: scrutinee_ty.clone(),
                                            found: Type::Enum(specified_en.clone()),
                                            span: *pat_span,
                                        });
                                    }
                                }
                                let enum_info = self.enum_infos.get(&s_enum_name).cloned().ok_or_else(|| TypeError::UnknownType {
                                    name: s_enum_name.clone(),
                                    span: *pat_span,
                                })?;
                                let &var_idx = enum_info.variant_indices.get(variant_name).ok_or_else(|| TypeError::NoSuchVariant {
                                    enum_name: s_enum_name.clone(),
                                    variant_name: variant_name.clone(),
                                    span: *pat_span,
                                })?;
                                let var_info = &enum_info.variants[var_idx];
                                if bindings.len() != var_info.payload.len() {
                                    return Err(TypeError::PayloadArityMismatch {
                                        enum_name: s_enum_name.clone(),
                                        variant_name: variant_name.clone(),
                                        expected: var_info.payload.len(),
                                        found: bindings.len(),
                                        span: *pat_span,
                                    });
                                }
                                seen_variants.insert(var_info.tag);
                                let mut typed_bindings = Vec::new();
                                for (b_name, p_ty) in bindings.iter().zip(&var_info.payload) {
                                    typed_bindings.push((b_name.clone(), p_ty.clone()));
                                }
                                if arm_bindings.is_empty() {
                                    arm_bindings = typed_bindings.clone();
                                }
                                typed_patterns.push(TypedMatchPattern::Variant {
                                    enum_name: s_enum_name,
                                    variant_name: variant_name.clone(),
                                    tag: var_info.tag,
                                    bindings: typed_bindings,
                                    span: *pat_span,
                                });
                            }
                            MatchPattern::Literal(lit) => {
                                match lit {
                                    Literal::Int(n) => {
                                        if !scrutinee_ty.is_integer() {
                                            return Err(TypeError::TypeMismatch {
                                                expected: scrutinee_ty.clone(),
                                                found: Type::I64,
                                                span: arm.span,
                                            });
                                        }
                                        typed_patterns.push(TypedMatchPattern::Literal(TypedLiteral::Int(*n, scrutinee_ty.clone())));
                                    }
                                    Literal::TypedInt(n, s) => {
                                        let lit_ty = Type::from_name(s).unwrap_or(Type::I64);
                                        if lit_ty != scrutinee_ty {
                                            return Err(TypeError::TypeMismatch {
                                                expected: scrutinee_ty.clone(),
                                                found: lit_ty,
                                                span: arm.span,
                                            });
                                        }
                                        typed_patterns.push(TypedMatchPattern::Literal(TypedLiteral::Int(*n, scrutinee_ty.clone())));
                                    }
                                    Literal::Bool(b) => {
                                        if scrutinee_ty != Type::Bool {
                                            return Err(TypeError::TypeMismatch {
                                                expected: scrutinee_ty.clone(),
                                                found: Type::Bool,
                                                span: arm.span,
                                            });
                                        }
                                        if *b {
                                            seen_bool_true = true;
                                        } else {
                                            seen_bool_false = true;
                                        }
                                        typed_patterns.push(TypedMatchPattern::Literal(TypedLiteral::Bool(*b)));
                                    }
                                    _ => {
                                        return Err(TypeError::TypeMismatch {
                                            expected: scrutinee_ty.clone(),
                                            found: Type::Str,
                                            span: arm.span,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    self.env.enter_scope();
                    for (b_name, p_ty) in &arm_bindings {
                        if b_name != "_" {
                            let sym = Symbol {
                                name: b_name.clone(),
                                ty: p_ty.clone(),
                                is_mutable: false,
                                span: arm.span,
                            };
                            if self.env.define_variable(sym).is_err() {
                                return Err(TypeError::DuplicateDeclaration {
                                    name: b_name.clone(),
                                    span: arm.span,
                                });
                            }
                        }
                    }
                    let typed_body = self.check_expr(&arm.body, expected_hint.clone())?;
                    self.env.exit_scope();

                    let body_ty = typed_body.ty();

                    if let Some(ref expected) = common_body_ty {
                        if body_ty != *expected {
                            return Err(TypeError::TypeMismatch {
                                expected: expected.clone(),
                                found: body_ty,
                                span: typed_body.span(),
                            });
                        }
                    } else {
                        common_body_ty = Some(body_ty.clone());
                    }

                    typed_arms.push(TypedMatchArm {
                        patterns: typed_patterns,
                        body: typed_body,
                        span: arm.span,
                    });
                }

                let is_exhaustive = has_wildcard
                    || (scrutinee_ty == Type::Bool && seen_bool_true && seen_bool_false)
                    || match &scrutinee_ty {
                        Type::Enum(en) => {
                            self.enum_infos.get(en).is_some_and(|info| seen_variants.len() == info.variants.len())
                        }
                        _ => false,
                    };
                if !is_exhaustive {
                    return Err(TypeError::NonExhaustiveMatch { span: *span });
                }

                let ret_ty = common_body_ty.unwrap_or(Type::Void);
                Ok(TypedExpr::Match {
                    scrutinee: Box::new(typed_scrutinee),
                    arms: typed_arms,
                    ty: ret_ty,
                    span: *span,
                })
            }

            Expr::EnumConstructor {
                enum_name,
                variant_name,
                args,
                span,
            } => {
                let en = if let Some(ref name) = enum_name {
                    name.clone()
                } else if let Some(candidates) = self.variant_to_enum.get(variant_name) {
                    if let Some(Type::Enum(ref expected_en)) = expected_hint {
                        if candidates.contains(expected_en) {
                            expected_en.clone()
                        } else if candidates.len() == 1 {
                            candidates[0].clone()
                        } else {
                            return Err(TypeError::NoSuchVariant {
                                enum_name: expected_en.clone(),
                                variant_name: variant_name.clone(),
                                span: *span,
                            });
                        }
                    } else if candidates.len() == 1 {
                        candidates[0].clone()
                    } else {
                        return Err(TypeError::UndeclaredVariable {
                            name: variant_name.clone(),
                            span: *span,
                        });
                    }
                } else {
                    return Err(TypeError::UndeclaredVariable {
                        name: variant_name.clone(),
                        span: *span,
                    });
                };

                let enum_info = self.enum_infos.get(&en).cloned().ok_or_else(|| TypeError::UnknownType {
                    name: en.clone(),
                    span: *span,
                })?;

                let &var_idx = enum_info.variant_indices.get(variant_name).ok_or_else(|| TypeError::NoSuchVariant {
                    enum_name: en.clone(),
                    variant_name: variant_name.clone(),
                    span: *span,
                })?;

                let var_info = &enum_info.variants[var_idx];
                if args.len() != var_info.payload.len() {
                    return Err(TypeError::PayloadArityMismatch {
                        enum_name: en.clone(),
                        variant_name: variant_name.clone(),
                        expected: var_info.payload.len(),
                        found: args.len(),
                        span: *span,
                    });
                }

                let mut typed_args = Vec::new();
                for (arg_expr, expected_ty) in args.iter().zip(&var_info.payload) {
                    let typed_arg = self.check_expr(arg_expr, Some(expected_ty.clone()))?;
                    if typed_arg.ty() != *expected_ty {
                        return Err(TypeError::TypeMismatch {
                            expected: expected_ty.clone(),
                            found: typed_arg.ty(),
                            span: typed_arg.span(),
                        });
                    }
                    typed_args.push(typed_arg);
                }

                Ok(TypedExpr::EnumConstructor {
                    enum_name: en.clone(),
                    variant_name: variant_name.clone(),
                    tag: var_info.tag,
                    args: typed_args,
                    ty: Type::Enum(en),
                    span: *span,
                })
            }

            Expr::Lambda {
                params,
                param_tys,
                body,
                span,
            } => {
                // If expected_hint gives us (param_types, ret_hint)
                let (hint_param_tys, hint_ret_ty): (Option<Vec<Type>>, Option<Type>) = match &expected_hint {
                    Some(Type::Fn(pts, ret)) => (Some(pts.clone()), Some((**ret).clone())),
                    Some(Type::Closure(c)) => (Some(c.params.clone()), Some((*c.ret).clone())),
                    _ => (None, None),
                };

                let mut resolved_params = Vec::new();
                for (i, p_name) in params.iter().enumerate() {
                    let ty = if let Some(Some(ref ty_str)) = param_tys.get(i) {
                        self.resolve_type(ty_str, *span)?
                    } else if let Some(ref h_pts) = hint_param_tys {
                        h_pts.get(i).cloned().unwrap_or(Type::I64)
                    } else {
                        Type::I64
                    };
                    resolved_params.push((p_name.clone(), ty));
                }

                // Determine captured variables from outer scope
                let mut captured_names = std::collections::HashSet::new();
                let mut free_vars = Vec::new();
                collect_free_variables(body, &mut free_vars);
                let param_set: std::collections::HashSet<_> = params.iter().cloned().collect();
                let mut captured = Vec::new();
                for fv in free_vars {
                    if !param_set.contains(&fv) && captured_names.insert(fv.clone()) {
                        if let Some(sym) = self.env.lookup_variable(&fv) {
                            captured.push((fv, sym.ty.clone()));
                        }
                    }
                }

                self.env.enter_scope();
                for (p_name, p_ty) in &resolved_params {
                    let sym = Symbol {
                        name: p_name.clone(),
                        ty: p_ty.clone(),
                        is_mutable: false,
                        span: *span,
                    };
                    if self.env.define_variable(sym).is_err() {
                        self.env.exit_scope();
                        return Err(TypeError::DuplicateDeclaration {
                            name: p_name.clone(),
                            span: *span,
                        });
                    }
                }

                let typed_body = self.check_expr(body, hint_ret_ty)?;
                self.env.exit_scope();

                let body_ty = typed_body.ty();
                let p_types: Vec<Type> = resolved_params.iter().map(|(_, t)| t.clone()).collect();
                let closure_ty = Type::Closure(Box::new(crate::typecheck::types::ClosureType {
                    params: p_types,
                    ret: Box::new(body_ty),
                    captured: captured.clone(),
                }));

                Ok(TypedExpr::Lambda {
                    params: resolved_params,
                    body: Box::new(typed_body),
                    captured,
                    ty: closure_ty,
                    span: *span,
                })
            }

            Expr::Box { inner, span } => {
                let inner_hint = match &expected_hint {
                    Some(Type::Box(inner_t)) => Some((**inner_t).clone()),
                    _ => None,
                };
                let typed_inner = self.check_expr(inner, inner_hint)?;
                let inner_ty = typed_inner.ty();
                Ok(TypedExpr::Box {
                    inner: Box::new(typed_inner),
                    ty: Type::Box(Box::new(inner_ty)),
                    span: *span,
                })
            }

            Expr::Deref { inner, span } => {
                let inner_hint = expected_hint.map(|h| Type::Box(Box::new(h)));
                let typed_inner = self.check_expr(inner, inner_hint)?;
                let elem_ty = match typed_inner.ty() {
                    Type::Box(t) => *t,
                    other => {
                        return Err(TypeError::TypeMismatch {
                            expected: Type::Box(Box::new(Type::I64)),
                            found: other,
                            span: *span,
                        });
                    }
                };
                Ok(TypedExpr::Deref {
                    inner: Box::new(typed_inner),
                    ty: elem_ty,
                    span: *span,
                })
            }
        }
    }
}

fn collect_free_variables(expr: &Expr, free: &mut Vec<String>) {
    match expr {
        Expr::Ident(name, _) => {
            free.push(name.clone());
        }
        Expr::Unary { expr, .. } => {
            collect_free_variables(expr, free);
        }
        Expr::Binary { left, right, .. } => {
            collect_free_variables(left, free);
            collect_free_variables(right, free);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_free_variables(arg, free);
            }
        }
        Expr::ArrayLiteral { elements, .. } => {
            for el in elements {
                collect_free_variables(el, free);
            }
        }
        Expr::Index { target, index, .. } => {
            collect_free_variables(target, free);
            collect_free_variables(index, free);
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, expr) in fields {
                collect_free_variables(expr, free);
            }
        }
        Expr::FieldAccess { target, .. } => {
            collect_free_variables(target, free);
        }
        Expr::Match { scrutinee, arms, .. } => {
            collect_free_variables(scrutinee, free);
            for arm in arms {
                collect_free_variables(&arm.body, free);
            }
        }
        Expr::EnumConstructor { args, .. } => {
            for arg in args {
                collect_free_variables(arg, free);
            }
        }
        Expr::Lambda { params, body, .. } => {
            let mut inner_free = Vec::new();
            collect_free_variables(body, &mut inner_free);
            let param_set: std::collections::HashSet<_> = params.iter().collect();
            for f in inner_free {
                if !param_set.contains(&f) {
                    free.push(f);
                }
            }
        }
        Expr::Box { inner, .. } | Expr::Deref { inner, .. } => {
            collect_free_variables(inner, free);
        }
        Expr::Group(inner, _) => {
            collect_free_variables(inner, free);
        }
        Expr::Literal(..) => {}
    }
}

pub fn typecheck(program: &Program) -> Result<TypedProgram, TypeError> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)
}

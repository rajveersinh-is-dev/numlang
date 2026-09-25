use crate::ast::{BinaryOp, UnaryOp};
use crate::mir::{BasicBlockId, Place, Projection, Terminator};
use crate::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedMatchPattern, TypedProgram, TypedStmt};
use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq)]
pub enum Rvalue {
    Use(Place),
    BinaryOp(BinaryOp, Place, Place),
    UnaryOp(UnaryOp, Place),
    Constant(TypedLiteral),
    Call(String, Vec<Place>),
    Array(Vec<Place>),
    Struct(String, Vec<(String, Place)>),
    EnumVariant {
        enum_name: String,
        variant_name: String,
        tag: usize,
        fields: Vec<Place>,
    },
    Discriminant(Place),
    Phi(Vec<(BasicBlockId, Place)>),
    FnPtr(String),
    ClosureAlloc {
        fn_name: String,
        captured: Vec<Place>,
    },
    Alloc(Place),
    Load(Place),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Assign(Place, Rvalue),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirBasicBlock {
    pub id: BasicBlockId,
    pub arguments: Vec<(String, Type)>,
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirLocalDecl {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_ty: Type,
    pub locals: Vec<MirLocalDecl>,
    pub blocks: Vec<MirBasicBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
    pub structs: Vec<crate::typecheck::typed_ast::TypedStructDef>,
    pub enums: Vec<crate::typecheck::typed_ast::TypedEnumDef>,
}

pub struct MirBuilder {
    blocks: Vec<MirBasicBlock>,
    locals: Vec<MirLocalDecl>,
    current_block: Option<BasicBlockId>,
    next_block_id: usize,
    next_temp_id: usize,
    loop_stack: Vec<(BasicBlockId, BasicBlockId)>,
}

impl MirBuilder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        MirBuilder {
            blocks: Vec::new(),
            locals: Vec::new(),
            current_block: None,
            next_block_id: 0,
            next_temp_id: 0,
            loop_stack: Vec::new(),
        }
    }

    fn new_block(&mut self) -> BasicBlockId {
        let id = BasicBlockId(self.next_block_id);
        self.next_block_id += 1;
        self.blocks.push(MirBasicBlock {
            id: id.clone(),
            arguments: Vec::new(),
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    fn new_temp(&mut self, ty: Type) -> Place {
        let name = format!("_t{}", self.next_temp_id);
        self.next_temp_id += 1;
        self.locals.push(MirLocalDecl {
            name: name.clone(),
            ty,
            mutable: true,
        });
        Place { local: name, projections: vec![] }
    }

    fn lower_expr(&mut self, expr: &TypedExpr, target_place: Option<Place>) -> Place {
        let block_id = self.current_block.clone().expect("must be in a block");
        let ty = expr.ty();

        let place = target_place.unwrap_or_else(|| self.new_temp(ty.clone()));

        match expr {
            TypedExpr::Literal { lit, .. } => {
                self.blocks[block_id.0].statements.push(Statement::Assign(place.clone(), Rvalue::Constant(lit.clone())));
                place
            }
            TypedExpr::Ident { name, .. } => {
                let src_place = Place { local: name.clone(), projections: vec![] };
                self.blocks[block_id.0].statements.push(Statement::Assign(place.clone(), Rvalue::Use(src_place)));
                place
            }
            TypedExpr::Unary { op, expr, .. } => {
                let inner = self.lower_expr(expr, None);
                self.blocks[self.current_block.clone().unwrap().0].statements.push(Statement::Assign(place.clone(), Rvalue::UnaryOp(*op, inner)));
                place
            }
            TypedExpr::Binary { op, left, right, .. } => {
                let l = self.lower_expr(left, None);
                let r = self.lower_expr(right, None);
                self.blocks[self.current_block.clone().unwrap().0].statements.push(Statement::Assign(place.clone(), Rvalue::BinaryOp(*op, l, r)));
                place
            }
            TypedExpr::Call { callee, args, .. } => {
                let mut arg_places = Vec::new();
                for arg in args {
                    arg_places.push(self.lower_expr(arg, None));
                }
                self.blocks[self.current_block.clone().unwrap().0].statements.push(Statement::Assign(place.clone(), Rvalue::Call(callee.clone(), arg_places)));
                place
            }
            TypedExpr::ArrayLiteral { elements, .. } => {
                let mut elem_places = Vec::new();
                for elem in elements {
                    elem_places.push(self.lower_expr(elem, None));
                }
                self.blocks[self.current_block.clone().unwrap().0].statements.push(Statement::Assign(place.clone(), Rvalue::Array(elem_places)));
                place
            }
            TypedExpr::Index { target, index, .. } => {
                let tgt = self.lower_expr(target, None);
                let idx = self.lower_expr(index, None);
                let mut src_place = tgt;
                src_place.projections.push(Projection::Index(Box::new(idx)));
                self.blocks[self.current_block.clone().unwrap().0].statements.push(Statement::Assign(place.clone(), Rvalue::Use(src_place)));
                place
            }
            TypedExpr::StructLiteral { name, fields, .. } => {
                let mut field_places = Vec::new();
                for (f_name, f_expr) in fields {
                    field_places.push((f_name.clone(), self.lower_expr(f_expr, None)));
                }
                self.blocks[self.current_block.clone().unwrap().0].statements.push(Statement::Assign(place.clone(), Rvalue::Struct(name.clone(), field_places)));
                place
            }
            TypedExpr::FieldAccess { target, field, .. } => {
                let mut tgt = self.lower_expr(target, None);
                tgt.projections.push(Projection::Field(field.clone()));
                self.blocks[self.current_block.clone().unwrap().0].statements.push(Statement::Assign(place.clone(), Rvalue::Use(tgt)));
                place
            }
            TypedExpr::Match {
                scrutinee,
                arms,
                ..
            } => {
                let scrut_place = self.lower_expr(scrutinee, None);
                let res_place = place;
                let merge_block = self.new_block();

                let discr_place = if matches!(scrutinee.ty(), Type::Enum(_)) {
                    let d = self.new_temp(Type::I64);
                    let curr = self.current_block.clone().unwrap().0;
                    self.blocks[curr]
                        .statements
                        .push(Statement::Assign(d.clone(), Rvalue::Discriminant(scrut_place.clone())));
                    d
                } else {
                    scrut_place.clone()
                };

                let current_entry = self.current_block.clone().unwrap();
                let mut targets = Vec::new();
                let mut default_target = None;

                for arm in arms {
                    let arm_block = self.new_block();
                    self.current_block = Some(arm_block.clone());

                    for pat in &arm.patterns {
                        match pat {
                            TypedMatchPattern::Variant {
                                tag,
                                bindings,
                                ..
                            } => {
                                targets.push((*tag as i64, arm_block.clone()));
                                for (i, (b_name, b_ty)) in bindings.iter().enumerate() {
                                    if b_name != "_" {
                                        self.locals.push(MirLocalDecl {
                                            name: b_name.clone(),
                                            ty: b_ty.clone(),
                                            mutable: true,
                                        });
                                        let b_place =
                                            Place { local: b_name.clone(), projections: vec![] };
                                        let mut src = scrut_place.clone();
                                        src.projections.push(Projection::Payload(i));
                                        self.blocks[arm_block.0]
                                            .statements
                                            .push(Statement::Assign(b_place, Rvalue::Use(src)));
                                    }
                                }
                            }
                            TypedMatchPattern::Literal(lit) => {
                                let case_val = match lit {
                                    TypedLiteral::Int(v, _) => *v,
                                    TypedLiteral::Bool(true) => 1,
                                    TypedLiteral::Bool(false) => 0,
                                    _ => 0,
                                };
                                targets.push((case_val, arm_block.clone()));
                            }
                            TypedMatchPattern::Wildcard => {
                                default_target = Some(arm_block.clone());
                            }
                        }
                    }

                    self.lower_expr(&arm.body, Some(res_place.clone()));
                    if let Some(curr) = self.current_block.clone() {
                        if self.blocks[curr.0].terminator == Terminator::Unreachable {
                            self.blocks[curr.0].terminator =
                                Terminator::Branch { target: merge_block.clone() };
                        }
                    }
                }

                let default_bb = default_target.unwrap_or_else(|| {
                    let dead = self.new_block();
                    self.blocks[dead.0].terminator = Terminator::Unreachable;
                    dead
                });

                self.blocks[current_entry.0].terminator = Terminator::Switch {
                    value: discr_place,
                    targets,
                    default: default_bb,
                };

                self.current_block = Some(merge_block);
                res_place
            }
            TypedExpr::EnumConstructor {
                enum_name,
                variant_name,
                tag,
                args,
                ..
            } => {
                let mut field_places = Vec::new();
                for arg in args {
                    field_places.push(self.lower_expr(arg, None));
                }
                self.blocks[self.current_block.clone().unwrap().0]
                    .statements
                    .push(Statement::Assign(
                        place.clone(),
                        Rvalue::EnumVariant {
                            enum_name: enum_name.clone(),
                            variant_name: variant_name.clone(),
                            tag: *tag,
                            fields: field_places,
                        },
                    ));
                place
            }
            TypedExpr::Lambda { captured, .. } => {
                let mut captured_places = Vec::new();
                for (cap_name, _) in captured {
                    captured_places.push(Place { local: cap_name.clone(), projections: vec![] });
                }
                self.blocks[self.current_block.clone().unwrap().0]
                    .statements
                    .push(Statement::Assign(
                        place.clone(),
                        Rvalue::ClosureAlloc {
                            fn_name: "closure_stub".to_string(),
                            captured: captured_places,
                        },
                    ));
                place
            }
            TypedExpr::CallIndirect { callee, args, .. } => {
                let callee_place = self.lower_expr(callee, None);
                let mut arg_places = Vec::new();
                for arg in args {
                    arg_places.push(self.lower_expr(arg, None));
                }
                let next_bb = self.new_block();
                let curr_bb = self.current_block.clone().unwrap();
                self.blocks[curr_bb.0].terminator = Terminator::IndirectCall {
                    callee: callee_place,
                    args: arg_places,
                    dest: place.clone(),
                    next: next_bb.clone(),
                };
                self.current_block = Some(next_bb);
                place
            }
            TypedExpr::Box { inner, .. } => {
                let inner_place = self.lower_expr(inner, None);
                self.blocks[self.current_block.clone().unwrap().0]
                    .statements
                    .push(Statement::Assign(
                        place.clone(),
                        Rvalue::Alloc(inner_place),
                    ));
                place
            }
            TypedExpr::Deref { inner, .. } => {
                let inner_place = self.lower_expr(inner, None);
                self.blocks[self.current_block.clone().unwrap().0]
                    .statements
                    .push(Statement::Assign(
                        place.clone(),
                        Rvalue::Load(inner_place),
                    ));
                place
            }
        }
    }

    fn lower_stmt(&mut self, stmt: &TypedStmt) {
        match stmt {
            TypedStmt::Let { name, is_mutable, ty, value, .. } => {
                self.locals.push(MirLocalDecl {
                    name: name.clone(),
                    ty: ty.clone(),
                    mutable: *is_mutable,
                });
                let dest = Place { local: name.clone(), projections: vec![] };
                self.lower_expr(value, Some(dest));
            }
            TypedStmt::Assign { name, value, .. } => {
                let dest = Place { local: name.clone(), projections: vec![] };
                self.lower_expr(value, Some(dest));
            }
            TypedStmt::IndexAssign { target, index, value, .. } => {
                let idx = self.lower_expr(index, None);
                let val = self.lower_expr(value, None);
                let mut dest = Place { local: target.clone(), projections: vec![] };
                dest.projections.push(Projection::Index(Box::new(idx)));
                let current = self.current_block.clone().unwrap().0;
                self.blocks[current].statements.push(Statement::Assign(dest, Rvalue::Use(val)));
            }
            TypedStmt::FieldAssign { target, field, value, .. } => {
                let val = self.lower_expr(value, None);
                let mut dest = Place { local: target.clone(), projections: vec![] };
                dest.projections.push(Projection::Field(field.clone()));
                let current = self.current_block.clone().unwrap().0;
                self.blocks[current].statements.push(Statement::Assign(dest, Rvalue::Use(val)));
            }
            TypedStmt::Return(expr_opt, _) => {
                let ret_val = expr_opt.as_ref().map(|e| self.lower_expr(e, None));
                let current = self.current_block.clone().unwrap().0;
                self.blocks[current].terminator = Terminator::Return { value: ret_val };
            }
            TypedStmt::Expr(expr) => {
                self.lower_expr(expr, None);
            }
            TypedStmt::If { condition, then_branch, else_branch, .. } => {
                let cond_place = self.lower_expr(condition, None);
                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();

                let current = self.current_block.clone().unwrap().0;
                let else_target = if else_branch.is_some() { else_block.clone() } else { merge_block.clone() };

                self.blocks[current].terminator = Terminator::BranchIf {
                    condition: cond_place,
                    then_target: then_block.clone(),
                    else_target,
                };

                self.current_block = Some(then_block);
                self.lower_block(then_branch);
                if self.blocks[self.current_block.clone().unwrap().0].terminator == Terminator::Unreachable {
                    self.blocks[self.current_block.clone().unwrap().0].terminator = Terminator::Branch { target: merge_block.clone() };
                }

                if let Some(eb) = else_branch {
                    self.current_block = Some(else_block);
                    self.lower_block(eb);
                    if self.blocks[self.current_block.clone().unwrap().0].terminator == Terminator::Unreachable {
                        self.blocks[self.current_block.clone().unwrap().0].terminator = Terminator::Branch { target: merge_block.clone() };
                    }
                }

                self.current_block = Some(merge_block);
            }
            TypedStmt::While { condition, body, .. } => {
                let cond_block = self.new_block();
                let body_block = self.new_block();
                let merge_block = self.new_block();

                let current = self.current_block.clone().unwrap().0;
                self.blocks[current].terminator = Terminator::Branch { target: cond_block.clone() };

                self.current_block = Some(cond_block.clone());
                let cond_place = self.lower_expr(condition, None);
                let current_cond = self.current_block.clone().unwrap().0;
                self.blocks[current_cond].terminator = Terminator::BranchIf {
                    condition: cond_place,
                    then_target: body_block.clone(),
                    else_target: merge_block.clone(),
                };

                self.loop_stack.push((cond_block.clone(), merge_block.clone()));
                self.current_block = Some(body_block);
                self.lower_block(body);
                if self.blocks[self.current_block.clone().unwrap().0].terminator == Terminator::Unreachable {
                    self.blocks[self.current_block.clone().unwrap().0].terminator = Terminator::Branch { target: cond_block };
                }
                self.loop_stack.pop();

                self.current_block = Some(merge_block);
            }
            TypedStmt::Break(_) => {
                if let Some((_, merge_target)) = self.loop_stack.last() {
                    let current = self.current_block.clone().unwrap().0;
                    self.blocks[current].terminator = Terminator::Branch { target: merge_target.clone() };
                    let unreachable_block = self.new_block();
                    self.current_block = Some(unreachable_block);
                }
            }
            TypedStmt::Continue(_) => {
                if let Some((cond_target, _)) = self.loop_stack.last() {
                    let current = self.current_block.clone().unwrap().0;
                    self.blocks[current].terminator = Terminator::Branch { target: cond_target.clone() };
                    let unreachable_block = self.new_block();
                    self.current_block = Some(unreachable_block);
                }
            }
            TypedStmt::For { .. } => {
                // Desugared to While loop before lowering
            }
        }
    }

    fn lower_block(&mut self, block: &TypedBlock) {
        for stmt in &block.stmts {
            self.lower_stmt(stmt);
        }
    }

    pub fn build(mut self, func: &TypedFunction) -> MirFunction {
        for p in &func.params {
            self.locals.push(MirLocalDecl {
                name: p.name.clone(),
                ty: p.ty.clone(),
                mutable: false,
            });
        }
        self.locals.push(MirLocalDecl {
            name: "_ret".to_string(),
            ty: func.return_ty.clone(),
            mutable: true,
        });

        let entry = self.new_block();
        self.current_block = Some(entry);
        self.lower_block(&func.body);

        if let Some(curr) = self.current_block.clone() {
            if self.blocks[curr.0].terminator == Terminator::Unreachable {
                self.blocks[curr.0].terminator = Terminator::Return { value: None };
            }
        }

        MirFunction {
            name: func.name.clone(),
            params: func.params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
            return_ty: func.return_ty.clone(),
            locals: self.locals,
            blocks: self.blocks,
        }
    }
}

pub fn lower_program(program: &TypedProgram) -> MirProgram {
    let mut mir_funcs = Vec::new();
    for func in &program.functions {
        let builder = MirBuilder::new();
        mir_funcs.push(builder.build(func));
    }
    MirProgram {
        functions: mir_funcs,
        structs: program.structs.clone(),
        enums: program.enums.clone(),
    }
}

use std::collections::HashMap;
use crate::ast::{BinaryOp, UnaryOp};
use crate::ir::{BasicBlock, BlockId, Instruction, IrFunction, IrOp, IrParam, IrProgram, Operand, ValueId};
use crate::typecheck::{
    TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt,
};

pub struct IrLowerer {
    next_value: usize,
    next_block: usize,
    blocks: Vec<BasicBlock>,
    current_block: BlockId,
    scopes: Vec<HashMap<String, ValueId>>,
}

impl Default for IrLowerer {
    fn default() -> Self {
        Self::new()
    }
}

impl IrLowerer {
    pub fn new() -> Self {
        Self {
            next_value: 0,
            next_block: 0,
            blocks: Vec::new(),
            current_block: BlockId(0),
            scopes: vec![HashMap::new()],
        }
    }

    fn new_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value);
        self.next_value += 1;
        id
    }

    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        self.blocks.push(BasicBlock {
            id,
            instructions: Vec::new(),
        });
        id
    }

    fn switch_to_block(&mut self, block: BlockId) {
        self.current_block = block;
    }

    fn emit(&mut self, inst: Instruction) {
        if let Some(bb) = self.blocks.iter_mut().find(|b| b.id == self.current_block) {
            bb.instructions.push(inst);
        }
    }

    fn current_block_terminated(&self) -> bool {
        if let Some(bb) = self.blocks.iter().find(|b| b.id == self.current_block) {
            if let Some(last) = bb.instructions.last() {
                return matches!(
                    last,
                    Instruction::Return { .. } | Instruction::Branch { .. } | Instruction::BranchIf { .. }
                );
            }
        }
        false
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn set_variable(&mut self, name: &str, val: ValueId) {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), val);
                return;
            }
        }
        if let Some(cur) = self.scopes.last_mut() {
            cur.insert(name.to_string(), val);
        }
    }

    fn get_variable(&self, name: &str) -> Option<ValueId> {
        for scope in self.scopes.iter().rev() {
            if let Some(&v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    pub fn lower_program(&mut self, program: &TypedProgram) -> IrProgram {
        let mut functions = Vec::new();
        for func in &program.functions {
            functions.push(self.lower_function(func));
        }
        IrProgram { functions }
    }

    pub fn lower_function(&mut self, func: &TypedFunction) -> IrFunction {
        self.next_value = 0;
        self.next_block = 0;
        self.blocks.clear();
        self.scopes = vec![HashMap::new()];

        let entry_bb = self.new_block();
        self.switch_to_block(entry_bb);

        let mut params = Vec::new();
        for param in &func.params {
            let val = self.new_value();
            self.scopes[0].insert(param.name.clone(), val);
            params.push(IrParam {
                name: param.name.clone(),
                val,
                ty: param.ty,
            });
        }

        for stmt in &func.body.stmts {
            self.lower_stmt(stmt);
        }

        // Implicit return void if not terminated
        if !self.current_block_terminated() {
            self.emit(Instruction::Return { val: None });
        }

        IrFunction {
            name: func.name.clone(),
            params,
            return_ty: func.return_ty,
            blocks: self.blocks.clone(),
        }
    }

    fn lower_stmt(&mut self, stmt: &TypedStmt) {
        match stmt {
            TypedStmt::Let {
                name,
                ty,
                value,
                ..
            } => {
                let op = self.lower_expr(value);
                let dest = self.new_value();
                self.emit(Instruction::Assign {
                    dest,
                    operand: op,
                    ty: *ty,
                });
                self.scopes.last_mut().unwrap().insert(name.clone(), dest);
            }

            TypedStmt::Assign { name, value, .. } => {
                let op = self.lower_expr(value);
                let dest = self.new_value();
                self.emit(Instruction::Assign {
                    dest,
                    operand: op,
                    ty: value.ty(),
                });
                self.set_variable(name, dest);
            }

            TypedStmt::Return(opt_expr, ..) => {
                let val = opt_expr.as_ref().map(|e| self.lower_expr(e));
                self.emit(Instruction::Return { val });
            }

            TypedStmt::Expr(expr) => {
                self.lower_expr(expr);
            }

            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_op = self.lower_expr(condition);
                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let merge_bb = self.new_block();

                let alt_target = if else_branch.is_some() {
                    else_bb
                } else {
                    merge_bb
                };

                self.emit(Instruction::BranchIf {
                    cond: cond_op,
                    then_block: then_bb,
                    else_block: alt_target,
                });

                // Then branch
                self.switch_to_block(then_bb);
                self.enter_scope();
                for s in &then_branch.stmts {
                    self.lower_stmt(s);
                }
                self.exit_scope();
                if !self.current_block_terminated() {
                    self.emit(Instruction::Branch { target: merge_bb });
                }

                // Else branch
                if let Some(eb) = else_branch {
                    self.switch_to_block(else_bb);
                    self.enter_scope();
                    for s in &eb.stmts {
                        self.lower_stmt(s);
                    }
                    self.exit_scope();
                    if !self.current_block_terminated() {
                        self.emit(Instruction::Branch { target: merge_bb });
                    }
                }

                self.switch_to_block(merge_bb);
            }

            TypedStmt::While {
                condition,
                body,
                ..
            } => {
                let cond_bb = self.new_block();
                let body_bb = self.new_block();
                let exit_bb = self.new_block();

                self.emit(Instruction::Branch { target: cond_bb });

                // Condition block
                self.switch_to_block(cond_bb);
                let cond_op = self.lower_expr(condition);
                self.emit(Instruction::BranchIf {
                    cond: cond_op,
                    then_block: body_bb,
                    else_block: exit_bb,
                });

                // Body block
                self.switch_to_block(body_bb);
                self.enter_scope();
                for s in &body.stmts {
                    self.lower_stmt(s);
                }
                self.exit_scope();
                if !self.current_block_terminated() {
                    self.emit(Instruction::Branch { target: cond_bb });
                }

                self.switch_to_block(exit_bb);
            }
        }
    }

    fn lower_expr(&mut self, expr: &TypedExpr) -> Operand {
        match expr {
            TypedExpr::Literal { lit, ty, .. } => match lit {
                TypedLiteral::Int(n, _) => Operand::IntConst(*n, *ty),
                TypedLiteral::Float(f, _) => Operand::FloatConst(*f, *ty),
                TypedLiteral::Bool(b) => Operand::BoolConst(*b),
            },

            TypedExpr::Ident { name, .. } => {
                let val = self
                    .get_variable(name)
                    .expect("Variable must be found in scope");
                Operand::Value(val)
            }

            TypedExpr::Unary { op, expr, ty, .. } => {
                let inner_op = self.lower_expr(expr);
                let dest = self.new_value();
                let ir_op = match op {
                    UnaryOp::Neg => IrOp::Neg,
                    UnaryOp::Not => IrOp::Not,
                };
                self.emit(Instruction::Unary {
                    dest,
                    op: ir_op,
                    ty: *ty,
                    operand: inner_op,
                });
                Operand::Value(dest)
            }

            TypedExpr::Binary {
                op,
                left,
                right,
                ty,
                ..
            } => {
                let l_op = self.lower_expr(left);
                let r_op = self.lower_expr(right);
                let dest = self.new_value();
                let ir_op = match op {
                    BinaryOp::Add => IrOp::Add,
                    BinaryOp::Sub => IrOp::Sub,
                    BinaryOp::Mul => IrOp::Mul,
                    BinaryOp::Div => IrOp::Div,
                    BinaryOp::Mod => IrOp::Mod,
                    BinaryOp::Pow => IrOp::Pow,
                    BinaryOp::Eq => IrOp::Eq,
                    BinaryOp::Ne => IrOp::Ne,
                    BinaryOp::Lt => IrOp::Lt,
                    BinaryOp::Le => IrOp::Le,
                    BinaryOp::Gt => IrOp::Gt,
                    BinaryOp::Ge => IrOp::Ge,
                };
                self.emit(Instruction::Binary {
                    dest,
                    op: ir_op,
                    ty: *ty,
                    left: l_op,
                    right: r_op,
                });
                Operand::Value(dest)
            }

            TypedExpr::Call {
                callee,
                args,
                ty,
                ..
            } => {
                let mut lowered_args = Vec::new();
                for a in args {
                    lowered_args.push(self.lower_expr(a));
                }

                if *ty == crate::typecheck::Type::Void {
                    self.emit(Instruction::Call {
                        dest: None,
                        callee: callee.clone(),
                        args: lowered_args,
                        return_ty: *ty,
                    });
                    Operand::BoolConst(false)
                } else {
                    let dest = self.new_value();
                    self.emit(Instruction::Call {
                        dest: Some(dest),
                        callee: callee.clone(),
                        args: lowered_args,
                        return_ty: *ty,
                    });
                    Operand::Value(dest)
                }
            }
        }
    }
}

pub fn lower_to_ir(program: &TypedProgram) -> IrProgram {
    let mut lowerer = IrLowerer::new();
    lowerer.lower_program(program)
}

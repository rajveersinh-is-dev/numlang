use std::collections::HashMap;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_native;
use cranelift_object::{ObjectBuilder, ObjectModule};
use thiserror::Error;

use crate::ast::{BinaryOp, UnaryOp};
use crate::typecheck::{
    Type, TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt,
};

#[derive(Debug, Clone, Error)]
pub enum CodegenError {
    #[error("Cranelift codegen error: {0}")]
    BackendError(String),
}

fn type_to_clif(ty: Type) -> types::Type {
    match ty {
        Type::I32 => types::I32,
        Type::I64 => types::I64,
        Type::F32 => types::F32,
        Type::F64 => types::F64,
        Type::Bool => types::I8,
        Type::Void => types::I32,
    }
}

pub struct CraneliftCompiler {
    module: ObjectModule,
    func_ids: HashMap<String, FuncId>,
    exit_process_id: FuncId,
}

impl CraneliftCompiler {
    pub fn new() -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("use_colocated_libcalls", "false")
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        flag_builder
            .set("is_pic", "false")
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        flag_builder
            .set("opt_level", "speed")
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let isa_builder =
            cranelift_native::builder().map_err(|e| CodegenError::BackendError(e.to_string()))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let builder = ObjectBuilder::new(
            isa,
            "numlang_program",
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut module = ObjectModule::new(builder);

        // Declare ExitProcess from kernel32.lib
        let mut exit_sig = module.make_signature();
        exit_sig.params.push(AbiParam::new(types::I32));
        let exit_process_id = module
            .declare_function("ExitProcess", Linkage::Import, &exit_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        Ok(Self {
            module,
            func_ids: HashMap::new(),
            exit_process_id,
        })
    }

    pub fn compile_program(mut self, program: &TypedProgram) -> Result<Vec<u8>, CodegenError> {
        // Step 1: Declare all user functions
        for func in &program.functions {
            let mut sig = self.module.make_signature();
            for param in &func.params {
                sig.params.push(AbiParam::new(type_to_clif(param.ty)));
            }
            if func.return_ty != Type::Void {
                sig.returns.push(AbiParam::new(type_to_clif(func.return_ty)));
            }

            let func_id = self
                .module
                .declare_function(&func.name, Linkage::Export, &sig)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?;
            self.func_ids.insert(func.name.clone(), func_id);
        }

        // Step 2: Define each function body
        let mut fn_builder_ctx = FunctionBuilderContext::new();
        let mut ctx = self.module.make_context();

        for func in &program.functions {
            self.compile_function(func, &mut ctx, &mut fn_builder_ctx)?;
        }

        // Step 3: If main function exists, synthesize Windows mainCRTStartup entry point
        if let Some(&main_id) = self.func_ids.get("main") {
            self.synthesize_entry_point(main_id, program, &mut ctx, &mut fn_builder_ctx)?;
        }

        // Step 4: Emit native COFF object file
        let product = self.module.finish();
        let obj_bytes = product
            .emit()
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        Ok(obj_bytes)
    }

    fn synthesize_entry_point(
        &mut self,
        main_id: FuncId,
        program: &TypedProgram,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let main_func = program.functions.iter().find(|f| f.name == "main").unwrap();

        let startup_sig = self.module.make_signature();
        let startup_id = self
            .module
            .declare_function("mainCRTStartup", Linkage::Export, &startup_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        ctx.func.signature = startup_sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        builder.ensure_inserted_block();

        let local_main = self.module.declare_func_in_func(main_id, &mut builder.func);
        let call_inst = builder.ins().call(local_main, &[]);

        let exit_code = if main_func.return_ty != Type::Void {
            let res = builder.inst_results(call_inst)[0];
            match main_func.return_ty {
                Type::I64 => builder.ins().ireduce(types::I32, res),
                Type::I32 => res,
                _ => builder.ins().iconst(types::I32, 0),
            }
        } else {
            builder.ins().iconst(types::I32, 0)
        };

        let local_exit = self
            .module
            .declare_func_in_func(self.exit_process_id, &mut builder.func);
        builder.ins().call(local_exit, &[exit_code]);
        builder.ins().return_(&[]);

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(startup_id, ctx)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        self.module.clear_context(ctx);

        Ok(())
    }

    fn compile_function(
        &mut self,
        func: &TypedFunction,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let func_id = *self.func_ids.get(&func.name).unwrap();

        let mut sig = self.module.make_signature();
        for param in &func.params {
            sig.params.push(AbiParam::new(type_to_clif(param.ty)));
        }
        if func.return_ty != Type::Void {
            sig.returns.push(AbiParam::new(type_to_clif(func.return_ty)));
        }

        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        builder.ensure_inserted_block();

        let mut variables: HashMap<String, Variable> = HashMap::new();

        for (i, param) in func.params.iter().enumerate() {
            let clif_ty = type_to_clif(param.ty);
            let var = builder.declare_var(clif_ty);
            let val = builder.block_params(entry_block)[i];
            builder.def_var(var, val);
            variables.insert(param.name.clone(), var);
        }

        let mut state = FunctionTranslationState {
            module: &mut self.module,
            func_ids: &self.func_ids,
            variables,
        };

        let terminated = state.translate_block(&func.body, &mut builder)?;

        if !terminated {
            builder.ins().return_(&[]);
        }

        let config = self.module.target_config();
        builder.finalize(config);

        if let Err(e) = self.module.define_function(func_id, ctx) {
            eprintln!("VERIFIER ERROR for function {}:\n{:#?}\nIR:\n{}", func.name, e, ctx.func);
            return Err(CodegenError::BackendError(format!("{:#?}", e)));
        }
        self.module.clear_context(ctx);

        Ok(())
    }
}

struct FunctionTranslationState<'a> {
    module: &'a mut ObjectModule,
    func_ids: &'a HashMap<String, FuncId>,
    variables: HashMap<String, Variable>,
}

impl<'a> FunctionTranslationState<'a> {
    fn translate_block(
        &mut self,
        block: &TypedBlock,
        builder: &mut FunctionBuilder,
    ) -> Result<bool, CodegenError> {
        let mut terminated = false;
        for stmt in &block.stmts {
            if terminated {
                break;
            }
            if self.translate_stmt(stmt, builder)? {
                terminated = true;
            }
        }
        Ok(terminated)
    }

    fn translate_stmt(
        &mut self,
        stmt: &TypedStmt,
        builder: &mut FunctionBuilder,
    ) -> Result<bool, CodegenError> {
        match stmt {
            TypedStmt::Let {
                name, ty, value, ..
            } => {
                let val = self.translate_expr(value, builder)?;
                let clif_ty = type_to_clif(*ty);
                let var = builder.declare_var(clif_ty);
                builder.def_var(var, val);
                self.variables.insert(name.clone(), var);
                Ok(false)
            }

            TypedStmt::Assign { name, value, .. } => {
                let val = self.translate_expr(value, builder)?;
                let var = *self
                    .variables
                    .get(name)
                    .expect("Variable must exist for assignment");
                builder.def_var(var, val);
                Ok(false)
            }

            TypedStmt::Return(opt_expr, ..) => {
                if let Some(expr) = opt_expr {
                    let val = self.translate_expr(expr, builder)?;
                    builder.ins().return_(&[val]);
                } else {
                    builder.ins().return_(&[]);
                }
                Ok(true)
            }

            TypedStmt::Expr(expr) => {
                let _ = self.translate_expr(expr, builder)?;
                Ok(false)
            }

            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_val = self.translate_expr(condition, builder)?;

                let then_block = builder.create_block();
                let else_block = builder.create_block();
                let merge_block = builder.create_block();

                builder.ins().brif(
                    cond_val,
                    then_block,
                    &[],
                    else_block,
                    &[],
                );

                // Then block
                builder.switch_to_block(then_block);
                builder.seal_block(then_block);
                let then_term = self.translate_block(then_branch, builder)?;
                if !then_term {
                    builder.ins().jump(merge_block, &[]);
                }

                // Else block
                let else_term = if let Some(eb) = else_branch {
                    builder.switch_to_block(else_block);
                    builder.seal_block(else_block);
                    let et = self.translate_block(eb, builder)?;
                    if !et {
                        builder.ins().jump(merge_block, &[]);
                    }
                    et
                } else {
                    builder.switch_to_block(else_block);
                    builder.seal_block(else_block);
                    builder.ins().jump(merge_block, &[]);
                    false
                };

                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);

                if then_term && else_term {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }

            TypedStmt::While {
                condition,
                body,
                ..
            } => {
                let header_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();

                builder.ins().jump(header_block, &[]);
                builder.switch_to_block(header_block);

                let cond_val = self.translate_expr(condition, builder)?;
                builder
                    .ins()
                    .brif(cond_val, body_block, &[], exit_block, &[]);

                builder.switch_to_block(body_block);
                builder.seal_block(body_block);
                let body_term = self.translate_block(body, builder)?;
                if !body_term {
                    builder.ins().jump(header_block, &[]);
                }

                builder.seal_block(header_block);
                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);
                Ok(false)
            }
        }
    }

    fn translate_expr(
        &mut self,
        expr: &TypedExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<Value, CodegenError> {
        match expr {
            TypedExpr::Literal { lit, ty, .. } => match lit {
                TypedLiteral::Int(n, _) => {
                    let clif_ty = type_to_clif(*ty);
                    Ok(builder.ins().iconst(clif_ty, *n))
                }
                TypedLiteral::Float(f, _) => match ty {
                    Type::F32 => Ok(builder.ins().f32const(*f as f32)),
                    _ => Ok(builder.ins().f64const(*f)),
                },
                TypedLiteral::Bool(b) => {
                    let v = if *b { 1 } else { 0 };
                    Ok(builder.ins().iconst(types::I8, v))
                }
            },

            TypedExpr::Ident { name, .. } => {
                let var = *self
                    .variables
                    .get(name)
                    .expect("Variable must be found in scope");
                Ok(builder.use_var(var))
            }

            TypedExpr::Unary {
                op, expr, ty, ..
            } => {
                let inner = self.translate_expr(expr, builder)?;
                match op {
                    UnaryOp::Neg => {
                        if ty.is_float() {
                            Ok(builder.ins().fneg(inner))
                        } else {
                            Ok(builder.ins().ineg(inner))
                        }
                    }
                    UnaryOp::Not => {
                        let zero = builder.ins().iconst(types::I8, 0);
                        let cmp = builder.ins().icmp(IntCC::Equal, inner, zero);
                        Ok(cmp)
                    }
                }
            }

            TypedExpr::Binary {
                op,
                left,
                right,
                ..
            } => {
                let l = self.translate_expr(left, builder)?;
                let r = self.translate_expr(right, builder)?;
                let operand_ty = left.ty();

                match op {
                    BinaryOp::Add => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fadd(l, r))
                        } else {
                            Ok(builder.ins().iadd(l, r))
                        }
                    }
                    BinaryOp::Sub => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fsub(l, r))
                        } else {
                            Ok(builder.ins().isub(l, r))
                        }
                    }
                    BinaryOp::Mul => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fmul(l, r))
                        } else {
                            Ok(builder.ins().imul(l, r))
                        }
                    }
                    BinaryOp::Div => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fdiv(l, r))
                        } else {
                            Ok(builder.ins().sdiv(l, r))
                        }
                    }
                    BinaryOp::Mod => {
                        if operand_ty.is_integer() {
                            Ok(builder.ins().srem(l, r))
                        } else {
                            Ok(l)
                        }
                    }
                    BinaryOp::Pow => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fmul(l, r))
                        } else {
                            Ok(builder.ins().imul(l, r))
                        }
                    }
                    BinaryOp::Eq => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::Equal, l, r)
                        } else {
                            builder.ins().icmp(IntCC::Equal, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Ne => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::NotEqual, l, r)
                        } else {
                            builder.ins().icmp(IntCC::NotEqual, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Lt => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::LessThan, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedLessThan, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Le => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Gt => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::GreaterThan, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedGreaterThan, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Ge => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r)
                        };
                        Ok(cmp)
                    }
                }
            }

            TypedExpr::Call { callee, args, .. } => {
                let func_id = *self
                    .func_ids
                    .get(callee)
                    .expect("Callee must be declared in module");
                let local_func = self.module.declare_func_in_func(func_id, &mut builder.func);

                let mut arg_vals = Vec::new();
                for a in args {
                    arg_vals.push(self.translate_expr(a, builder)?);
                }

                let call_inst = builder.ins().call(local_func, &arg_vals);
                let results = builder.inst_results(call_inst);
                if results.is_empty() {
                    Ok(builder.ins().iconst(types::I32, 0))
                } else {
                    Ok(results[0])
                }
            }
        }
    }
}

pub fn compile_to_obj(program: &TypedProgram) -> Result<Vec<u8>, CodegenError> {
    let compiler = CraneliftCompiler::new()?;
    compiler.compile_program(program)
}

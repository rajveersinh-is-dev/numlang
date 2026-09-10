use std::collections::HashMap;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, InstBuilder, MemFlagsData, StackSlot, StackSlotData, StackSlotKind, TrapCode,
    Value,
};
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
        Type::Array(_, _) => types::I64, // Pointer to array
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

        let mut isa_builder =
            cranelift_native::builder().map_err(|e| CodegenError::BackendError(e.to_string()))?;

        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx2") {
                let _ = isa_builder.enable("has_avx2");
            }
            if std::is_x86_feature_detected!("fma") {
                let _ = isa_builder.enable("has_fma");
            }
            if std::is_x86_feature_detected!("sse4.2") {
                let _ = isa_builder.enable("has_sse42");
            }
            if std::is_x86_feature_detected!("bmi1") {
                let _ = isa_builder.enable("has_bmi1");
            }
            if std::is_x86_feature_detected!("bmi2") {
                let _ = isa_builder.enable("has_bmi2");
            }
        }

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
                sig.params.push(AbiParam::new(type_to_clif(param.ty.clone())));
            }
            if func.return_ty != Type::Void {
                sig.returns.push(AbiParam::new(type_to_clif(func.return_ty.clone())));
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

        // Step 3: Emit entry point (mainCRTStartup) if main exists
        if let Some(&main_id) = self.func_ids.get("main") {
            self.compile_entry_point(main_id, &mut ctx, &mut fn_builder_ctx)?;
        }

        // Step 4: Emit final object file
        let product = self.module.finish();
        let obj_bytes = product
            .emit()
            .map_err(|e| CodegenError::BackendError(format!("Failed to emit object: {}", e)))?;

        Ok(obj_bytes)
    }

    fn compile_entry_point(
        &mut self,
        main_id: FuncId,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let entry_sig = self.module.make_signature();
        let entry_id = self
            .module
            .declare_function("mainCRTStartup", Linkage::Export, &entry_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        ctx.func.signature = entry_sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        builder.ensure_inserted_block();

        let local_main = self.module.declare_func_in_func(main_id, &mut builder.func);
        let call_inst = builder.ins().call(local_main, &[]);
        let results = builder.inst_results(call_inst);

        let exit_code = if results.is_empty() {
            builder.ins().iconst(types::I32, 0)
        } else {
            let ret_val = results[0];
            let ret_ty = builder.func.dfg.value_type(ret_val);
            if ret_ty == types::I64 {
                builder.ins().ireduce(types::I32, ret_val)
            } else if ret_ty == types::I32 {
                ret_val
            } else {
                builder.ins().iconst(types::I32, 0)
            }
        };

        let local_exit = self
            .module
            .declare_func_in_func(self.exit_process_id, &mut builder.func);
        builder.ins().call(local_exit, &[exit_code]);
        builder.ins().trap(TrapCode::user(1).unwrap());

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(entry_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in entry: {:#?}", e)))?;
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
            sig.params.push(AbiParam::new(type_to_clif(param.ty.clone())));
        }
        if func.return_ty != Type::Void {
            sig.returns.push(AbiParam::new(type_to_clif(func.return_ty.clone())));
        }

        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        builder.ensure_inserted_block();

        let mut variables: HashMap<String, Storage> = HashMap::new();

        for (i, param) in func.params.iter().enumerate() {
            let clif_ty = type_to_clif(param.ty.clone());
            let var = builder.declare_var(clif_ty);
            let val = builder.block_params(entry_block)[i];
            builder.def_var(var, val);
            variables.insert(param.name.clone(), Storage::Scalar(var));
        }

        let mut state = FunctionTranslationState {
            module: &mut self.module,
            func_ids: &self.func_ids,
            exit_process_id: self.exit_process_id,
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

#[derive(Clone, Copy)]
enum Storage {
    Scalar(Variable),
    Array {
        slot: StackSlot,
        len: usize,
    },
}

struct FunctionTranslationState<'a> {
    module: &'a mut ObjectModule,
    func_ids: &'a HashMap<String, FuncId>,
    exit_process_id: FuncId,
    variables: HashMap<String, Storage>,
}

impl<'a> FunctionTranslationState<'a> {
    fn get_small_constant_loop_info<'b>(condition: &'b TypedExpr) -> Option<(&'b str, usize)> {
        match condition {
            TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(n, _), .. }) = (&**left, &**right) {
                    if *n > 0 && *n <= 16 {
                        return Some((name.as_str(), *n as usize));
                    }
                }
                None
            }
            TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => {
                if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(n, _), .. }) = (&**left, &**right) {
                    let limit = *n + 1;
                    if limit > 0 && limit <= 16 {
                        return Some((name.as_str(), limit as usize));
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn is_var_initialized_to_zero(stmt: &TypedStmt, var_name: &str) -> bool {
        match stmt {
            TypedStmt::Let { name, value, .. } | TypedStmt::Assign { name, value, .. } => {
                if name == var_name {
                    if let TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. } = value {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn is_simple_induction_body(body: &TypedBlock, var_name: &str) -> bool {
        let mut has_increment = false;
        for s in &body.stmts {
            match s {
                TypedStmt::While { .. } | TypedStmt::Return(..) => return false,
                TypedStmt::Assign { name, value, .. } if name == var_name => {
                    match value {
                        TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } => {
                            let is_plus_one = match (&**left, &**right) {
                                (TypedExpr::Ident { name: l, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) => l == var_name,
                                (TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }, TypedExpr::Ident { name: r, .. }) => r == var_name,
                                _ => false,
                            };
                            if is_plus_one && !has_increment {
                                has_increment = true;
                            } else {
                                return false;
                            }
                        }
                        _ => return false,
                    }
                }
                _ => {}
            }
        }
        has_increment
    }

    fn translate_block(
        &mut self,
        block: &TypedBlock,
        builder: &mut FunctionBuilder,
    ) -> Result<bool, CodegenError> {
        let mut terminated = false;
        let num_stmts = block.stmts.len();
        let mut i = 0;
        while i < num_stmts {
            if terminated {
                break;
            }
            let stmt = &block.stmts[i];

            // Full loop unrolling for small fixed iteration loops
            if let TypedStmt::While { condition, body, .. } = stmt {
                if i > 0 {
                    if let Some((var_name, limit)) = Self::get_small_constant_loop_info(condition) {
                        if Self::is_var_initialized_to_zero(&block.stmts[i - 1], var_name)
                            && Self::is_simple_induction_body(body, var_name)
                        {
                            for _ in 0..limit {
                                if self.translate_block(body, builder)? {
                                    terminated = true;
                                    break;
                                }
                            }
                            i += 1;
                            continue;
                        }
                    }
                }
            }

            if self.translate_stmt(stmt, builder)? {
                terminated = true;
            }
            i += 1;
        }
        Ok(terminated)
    }

    fn emit_bounds_check(
        &mut self,
        idx_val: Value,
        len: usize,
        builder: &mut FunctionBuilder,
    ) {
        let is_oob = builder
            .ins()
            .icmp_imm_u(IntCC::UnsignedGreaterThanOrEqual, idx_val, len as i64);

        let ok_block = builder.create_block();
        let panic_block = builder.create_block();

        builder
            .ins()
            .brif(is_oob, panic_block, &[], ok_block, &[]);

        builder.switch_to_block(panic_block);
        builder.seal_block(panic_block);

        let exit_code = builder.ins().iconst(types::I32, 101);
        let exit_func = self
            .module
            .declare_func_in_func(self.exit_process_id, &mut builder.func);
        builder.ins().call(exit_func, &[exit_code]);
        builder.ins().trap(TrapCode::user(2).unwrap());

        builder.switch_to_block(ok_block);
        builder.seal_block(ok_block);
    }

    fn resolve_array_arg(
        &mut self,
        arg: &TypedExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<(StackSlot, usize, Type), CodegenError> {
        match arg {
            TypedExpr::Ident { name, ty, .. } => {
                if let Some(Storage::Array { slot, len }) = self.variables.get(name) {
                    let elem = ty.element_type().unwrap().clone();
                    Ok((*slot, *len, elem))
                } else {
                    panic!("Argument is not an array variable");
                }
            }
            TypedExpr::ArrayLiteral { elements, ty, .. } => {
                let elem = ty.element_type().unwrap().clone();
                let elem_size = elem.size_bytes() as u32;
                let len = elements.len();
                let total_bytes = (elem_size * (len as u32)).max(1);
                let slot_data =
                    StackSlotData::new(StackSlotKind::ExplicitSlot, total_bytes, elem_size.min(8) as u8);
                let slot = builder.create_sized_stack_slot(slot_data);

                for (i, el) in elements.iter().enumerate() {
                    let el_val = self.translate_expr(el, builder)?;
                    let offset = (i as i32) * (elem_size as i32);
                    let addr = builder.ins().stack_addr(types::I64, slot, offset);
                    builder.ins().store(MemFlagsData::trusted(), el_val, addr, 0);
                }
                Ok((slot, len, elem))
            }
            _ => panic!("Expected array variable or literal"),
        }
    }

    fn translate_vec_add_into_slot(
        &mut self,
        args: &[TypedExpr],
        dst_slot: StackSlot,
        elem_ty: &Type,
        len: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let (slot_a, _, _) = self.resolve_array_arg(&args[0], builder)?;
        let (slot_b, _, _) = self.resolve_array_arg(&args[1], builder)?;
        let elem_size = elem_ty.size_bytes() as i32;
        let clif_ty = type_to_clif(elem_ty.clone());

        let mut i = 0;
        while i + 8 <= len {
            for k in 0..8 {
                let offset = ((i + k) as i32) * elem_size;
                let addr_a = builder.ins().stack_addr(types::I64, slot_a, offset);
                let val_a = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr_a, 0);
                let addr_b = builder.ins().stack_addr(types::I64, slot_b, offset);
                let val_b = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr_b, 0);

                let sum = if elem_ty.is_float() {
                    builder.ins().fadd(val_a, val_b)
                } else {
                    builder.ins().iadd(val_a, val_b)
                };

                let addr_dst = builder.ins().stack_addr(types::I64, dst_slot, offset);
                builder.ins().store(MemFlagsData::trusted(), sum, addr_dst, 0);
            }
            i += 8;
        }

        while i + 4 <= len {
            for k in 0..4 {
                let offset = ((i + k) as i32) * elem_size;
                let addr_a = builder.ins().stack_addr(types::I64, slot_a, offset);
                let val_a = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr_a, 0);
                let addr_b = builder.ins().stack_addr(types::I64, slot_b, offset);
                let val_b = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr_b, 0);

                let sum = if elem_ty.is_float() {
                    builder.ins().fadd(val_a, val_b)
                } else {
                    builder.ins().iadd(val_a, val_b)
                };

                let addr_dst = builder.ins().stack_addr(types::I64, dst_slot, offset);
                builder.ins().store(MemFlagsData::trusted(), sum, addr_dst, 0);
            }
            i += 4;
        }

        while i < len {
            let offset = (i as i32) * elem_size;
            let addr_a = builder.ins().stack_addr(types::I64, slot_a, offset);
            let val_a = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr_a, 0);
            let addr_b = builder.ins().stack_addr(types::I64, slot_b, offset);
            let val_b = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr_b, 0);

            let sum = if elem_ty.is_float() {
                builder.ins().fadd(val_a, val_b)
            } else {
                builder.ins().iadd(val_a, val_b)
            };

            let addr_dst = builder.ins().stack_addr(types::I64, dst_slot, offset);
            builder.ins().store(MemFlagsData::trusted(), sum, addr_dst, 0);
            i += 1;
        }
        Ok(())
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
                if let Type::Array(elem, len) = ty {
                    let elem_size = elem.size_bytes() as u32;
                    let total_bytes = (elem_size * (*len as u32)).max(1);
                    let slot_data =
                        StackSlotData::new(StackSlotKind::ExplicitSlot, total_bytes, elem_size.min(8) as u8);
                    let slot = builder.create_sized_stack_slot(slot_data);

                    match value {
                        TypedExpr::ArrayLiteral { elements, .. } => {
                            for (i, el) in elements.iter().enumerate() {
                                let el_val = self.translate_expr(el, builder)?;
                                let offset = (i as i32) * (elem_size as i32);
                                let addr = builder.ins().stack_addr(types::I64, slot, offset);
                                builder.ins().store(MemFlagsData::trusted(), el_val, addr, 0);
                            }
                        }
                        TypedExpr::Ident { name: src_name, .. } => {
                            if let Some(Storage::Array { slot: src_slot, .. }) =
                                self.variables.get(src_name).copied()
                            {
                                for i in 0..*len {
                                    let offset = (i as i32) * (elem_size as i32);
                                    let clif_ty = type_to_clif((**elem).clone());
                                    let src_addr =
                                        builder.ins().stack_addr(types::I64, src_slot, offset);
                                    let el_val = builder.ins().load(
                                        clif_ty,
                                        MemFlagsData::trusted(),
                                        src_addr,
                                        0,
                                    );
                                    let dst_addr =
                                        builder.ins().stack_addr(types::I64, slot, offset);
                                    builder
                                        .ins()
                                        .store(MemFlagsData::trusted(), el_val, dst_addr, 0);
                                }
                            }
                        }
                        TypedExpr::Call { callee, args, .. } if callee == "vec_add" => {
                            self.translate_vec_add_into_slot(args, slot, elem, *len, builder)?;
                        }
                        _ => {}
                    }

                    self.variables
                        .insert(name.clone(), Storage::Array { slot, len: *len });
                    Ok(false)
                } else {
                    let val = self.translate_expr(value, builder)?;
                    let clif_ty = type_to_clif(ty.clone());
                    let var = builder.declare_var(clif_ty);
                    builder.def_var(var, val);
                    self.variables.insert(name.clone(), Storage::Scalar(var));
                    Ok(false)
                }
            }

            TypedStmt::Assign { name, value, .. } => {
                let storage = *self
                    .variables
                    .get(name)
                    .expect("Variable must exist for assignment");
                match storage {
                    Storage::Scalar(var) => {
                        let val = self.translate_expr(value, builder)?;
                        builder.def_var(var, val);
                    }
                    Storage::Array { slot, len } => {
                        if let TypedExpr::Ident { name: src_name, ty, .. } = value {
                            let elem = ty.element_type().unwrap().clone();
                            let elem_size = elem.size_bytes() as i32;
                            if let Some(Storage::Array { slot: src_slot, .. }) =
                                self.variables.get(src_name).copied()
                            {
                                for i in 0..len {
                                    let offset = (i as i32) * elem_size;
                                    let clif_ty = type_to_clif(elem.clone());
                                    let src_addr =
                                        builder.ins().stack_addr(types::I64, src_slot, offset);
                                    let el_val = builder.ins().load(
                                        clif_ty,
                                        MemFlagsData::trusted(),
                                        src_addr,
                                        0,
                                    );
                                    let dst_addr =
                                        builder.ins().stack_addr(types::I64, slot, offset);
                                    builder
                                        .ins()
                                        .store(MemFlagsData::trusted(), el_val, dst_addr, 0);
                                }
                            }
                        } else if let TypedExpr::Call { callee, args, ty, .. } = value {
                            if callee == "vec_add" {
                                let elem = ty.element_type().unwrap().clone();
                                self.translate_vec_add_into_slot(args, slot, &elem, len, builder)?;
                            }
                        }
                    }
                }
                Ok(false)
            }

            TypedStmt::IndexAssign {
                target,
                index,
                value,
                is_safe,
                ..
            } => {
                let (slot, len) = match self.variables.get(target) {
                    Some(Storage::Array { slot, len }) => (*slot, *len),
                    _ => panic!("Target must be an array variable"),
                };

                let elem_ty = value.ty();
                let elem_size = elem_ty.size_bytes();

                let mut idx_val = self.translate_expr(index, builder)?;
                if index.ty() == Type::I32 {
                    idx_val = builder.ins().uextend(types::I64, idx_val);
                }

                if !*is_safe {
                    self.emit_bounds_check(idx_val, len, builder);
                }

                let val = self.translate_expr(value, builder)?;
                let offset = builder.ins().imul_imm_s(idx_val, elem_size as i64);
                let base_addr = builder.ins().stack_addr(types::I64, slot, 0);
                let elem_addr = builder.ins().iadd(base_addr, offset);

                builder.ins().store(MemFlagsData::trusted(), val, elem_addr, 0);
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

                // Then branch
                builder.switch_to_block(then_block);
                builder.seal_block(then_block);
                let then_term = self.translate_block(then_branch, builder)?;
                if !then_term {
                    builder.ins().jump(merge_block, &[]);
                }

                // Else branch
                builder.switch_to_block(else_block);
                builder.seal_block(else_block);
                let else_term = if let Some(eb) = else_branch {
                    self.translate_block(eb, builder)?
                } else {
                    false
                };
                if !else_term {
                    builder.ins().jump(merge_block, &[]);
                }

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
                let induction_info = match condition {
                    TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                        if let TypedExpr::Ident { name, .. } = &**left {
                            Some((name.clone(), &**right, false))
                        } else {
                            None
                        }
                    }
                    TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => {
                        if let TypedExpr::Ident { name, .. } = &**left {
                            Some((name.clone(), &**right, true))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some((ref var_name, limit_expr, is_le)) = induction_info {
                    if let Some(Storage::Scalar(var)) = self.variables.get(var_name).copied() {
                        if Self::is_simple_induction_body(body, var_name) {
                            let unroll_head_block = builder.create_block();
                            let unroll_body_block = builder.create_block();
                            let cleanup_head_block = builder.create_block();
                            let cleanup_body_block = builder.create_block();
                            let exit_block = builder.create_block();

                            builder.ins().jump(unroll_head_block, &[]);

                            // Unrolled loop header: test if at least 4 iterations remain
                            builder.switch_to_block(unroll_head_block);
                            let cur_val = builder.use_var(var);
                            let var_ty = builder.func.dfg.value_type(cur_val);
                            let cur_plus_3 = builder.ins().iadd_imm_s(cur_val, 3);
                            let mut limit_val = self.translate_expr(limit_expr, builder)?;
                            let limit_clif_ty = builder.func.dfg.value_type(limit_val);
                            if var_ty == types::I64 && limit_clif_ty == types::I32 {
                                limit_val = builder.ins().sextend(types::I64, limit_val);
                            } else if var_ty == types::I32 && limit_clif_ty == types::I64 {
                                limit_val = builder.ins().ireduce(types::I32, limit_val);
                            }

                            let can_unroll = if is_le {
                                builder.ins().icmp(IntCC::SignedLessThanOrEqual, cur_plus_3, limit_val)
                            } else {
                                builder.ins().icmp(IntCC::SignedLessThan, cur_plus_3, limit_val)
                            };
                            builder.ins().brif(can_unroll, unroll_body_block, &[], cleanup_head_block, &[]);

                            // Unrolled body (4 iterations straight-line)
                            builder.switch_to_block(unroll_body_block);
                            builder.seal_block(unroll_body_block);
                            for _ in 0..4 {
                                self.translate_block(body, builder)?;
                            }
                            builder.ins().jump(unroll_head_block, &[]);
                            builder.seal_block(unroll_head_block);

                            // Cleanup loop header
                            builder.switch_to_block(cleanup_head_block);
                            let cur_val_cleanup = builder.use_var(var);
                            let mut limit_val_cleanup = self.translate_expr(limit_expr, builder)?;
                            let limit_clif_ty_cleanup = builder.func.dfg.value_type(limit_val_cleanup);
                            if var_ty == types::I64 && limit_clif_ty_cleanup == types::I32 {
                                limit_val_cleanup = builder.ins().sextend(types::I64, limit_val_cleanup);
                            } else if var_ty == types::I32 && limit_clif_ty_cleanup == types::I64 {
                                limit_val_cleanup = builder.ins().ireduce(types::I32, limit_val_cleanup);
                            }

                            let has_more = if is_le {
                                builder.ins().icmp(IntCC::SignedLessThanOrEqual, cur_val_cleanup, limit_val_cleanup)
                            } else {
                                builder.ins().icmp(IntCC::SignedLessThan, cur_val_cleanup, limit_val_cleanup)
                            };
                            builder.ins().brif(has_more, cleanup_body_block, &[], exit_block, &[]);

                            // Cleanup body (1 iteration)
                            builder.switch_to_block(cleanup_body_block);
                            builder.seal_block(cleanup_body_block);
                            self.translate_block(body, builder)?;
                            builder.ins().jump(cleanup_head_block, &[]);
                            builder.seal_block(cleanup_head_block);

                            // Exit block
                            builder.switch_to_block(exit_block);
                            builder.seal_block(exit_block);
                            return Ok(false);
                        }
                    }
                }

                // Fallback to standard while loop
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
                    let clif_ty = type_to_clif(ty.clone());
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
                let storage = *self
                    .variables
                    .get(name)
                    .expect("Variable must be found in scope");
                match storage {
                    Storage::Scalar(var) => Ok(builder.use_var(var)),
                    Storage::Array { slot, .. } => {
                        Ok(builder.ins().stack_addr(types::I64, slot, 0))
                    }
                }
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

            TypedExpr::ArrayLiteral { elements, ty, .. } => {
                let elem_ty = ty.element_type().unwrap();
                let elem_size = elem_ty.size_bytes() as u32;
                let len = elements.len();
                let total_bytes = (elem_size * (len as u32)).max(1);
                let slot_data =
                    StackSlotData::new(StackSlotKind::ExplicitSlot, total_bytes, elem_size.min(8) as u8);
                let slot = builder.create_sized_stack_slot(slot_data);

                for (i, el) in elements.iter().enumerate() {
                    let el_val = self.translate_expr(el, builder)?;
                    let offset = (i as i32) * (elem_size as i32);
                    let addr = builder.ins().stack_addr(types::I64, slot, offset);
                    builder.ins().store(MemFlagsData::trusted(), el_val, addr, 0);
                }

                Ok(builder.ins().stack_addr(types::I64, slot, 0))
            }

            TypedExpr::Index {
                target,
                index,
                is_safe,
                ty,
                ..
            } => {
                let (slot, len) = match target.as_ref() {
                    TypedExpr::Ident { name, .. } => match self.variables.get(name) {
                        Some(Storage::Array { slot, len }) => (*slot, *len),
                        _ => panic!("Index target must be an array variable"),
                    },
                    _ => panic!("Indexing supported on array variables"),
                };

                let elem_size = ty.size_bytes();
                let mut idx_val = self.translate_expr(index, builder)?;
                if index.ty() == Type::I32 {
                    idx_val = builder.ins().uextend(types::I64, idx_val);
                }

                if !*is_safe {
                    self.emit_bounds_check(idx_val, len, builder);
                }

                let offset = builder.ins().imul_imm_s(idx_val, elem_size as i64);
                let base_addr = builder.ins().stack_addr(types::I64, slot, 0);
                let elem_addr = builder.ins().iadd(base_addr, offset);

                let clif_ty = type_to_clif(ty.clone());
                Ok(builder.ins().load(clif_ty, MemFlagsData::trusted(), elem_addr, 0))
            }

            TypedExpr::Call { callee, args, .. } => {
                match callee.as_str() {
                    "sqrt" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().sqrt(arg));
                    }
                    "abs" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        let arg_ty = args[0].ty();
                        if arg_ty.is_float() {
                            return Ok(builder.ins().fabs(arg));
                        } else {
                            let clif_ty = type_to_clif(arg_ty);
                            let zero = builder.ins().iconst(clif_ty, 0);
                            let is_neg = builder.ins().icmp(IntCC::SignedLessThan, arg, zero);
                            let neg = builder.ins().ineg(arg);
                            return Ok(builder.ins().select(is_neg, neg, arg));
                        }
                    }
                    "to_i64" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        let arg_ty = args[0].ty();
                        if arg_ty.is_float() {
                            return Ok(builder.ins().fcvt_to_sint(types::I64, arg));
                        } else {
                            let clif_ty = type_to_clif(arg_ty);
                            if clif_ty == types::I32 {
                                return Ok(builder.ins().sextend(types::I64, arg));
                            } else {
                                return Ok(arg);
                            }
                        }
                    }
                    "dot" => {
                        let (slot_a, len_a, elem_ty) = self.resolve_array_arg(&args[0], builder)?;
                        let (slot_b, _, _) = self.resolve_array_arg(&args[1], builder)?;
                        let elem_size = elem_ty.size_bytes() as i32;
                        let clif_ty = type_to_clif(elem_ty.clone());

                        let zero = if elem_ty.is_float() {
                            if elem_ty == Type::F32 {
                                builder.ins().f32const(0.0)
                            } else {
                                builder.ins().f64const(0.0)
                            }
                        } else {
                            builder.ins().iconst(clif_ty, 0)
                        };

                        let mut acc0 = zero;
                        let mut acc1 = zero;
                        let mut acc2 = zero;
                        let mut acc3 = zero;
                        let mut acc4 = zero;
                        let mut acc5 = zero;
                        let mut acc6 = zero;
                        let mut acc7 = zero;

                        let mut i = 0;
                        while i + 8 <= len_a {
                            let off0 = (i as i32) * elem_size;
                            let a0 = builder.ins().stack_addr(types::I64, slot_a, off0);
                            let v_a0 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a0, 0);
                            let b0 = builder.ins().stack_addr(types::I64, slot_b, off0);
                            let v_b0 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b0, 0);

                            let off1 = ((i + 1) as i32) * elem_size;
                            let a1 = builder.ins().stack_addr(types::I64, slot_a, off1);
                            let v_a1 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a1, 0);
                            let b1 = builder.ins().stack_addr(types::I64, slot_b, off1);
                            let v_b1 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b1, 0);

                            let off2 = ((i + 2) as i32) * elem_size;
                            let a2 = builder.ins().stack_addr(types::I64, slot_a, off2);
                            let v_a2 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a2, 0);
                            let b2 = builder.ins().stack_addr(types::I64, slot_b, off2);
                            let v_b2 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b2, 0);

                            let off3 = ((i + 3) as i32) * elem_size;
                            let a3 = builder.ins().stack_addr(types::I64, slot_a, off3);
                            let v_a3 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a3, 0);
                            let b3 = builder.ins().stack_addr(types::I64, slot_b, off3);
                            let v_b3 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b3, 0);

                            let off4 = ((i + 4) as i32) * elem_size;
                            let a4 = builder.ins().stack_addr(types::I64, slot_a, off4);
                            let v_a4 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a4, 0);
                            let b4 = builder.ins().stack_addr(types::I64, slot_b, off4);
                            let v_b4 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b4, 0);

                            let off5 = ((i + 5) as i32) * elem_size;
                            let a5 = builder.ins().stack_addr(types::I64, slot_a, off5);
                            let v_a5 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a5, 0);
                            let b5 = builder.ins().stack_addr(types::I64, slot_b, off5);
                            let v_b5 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b5, 0);

                            let off6 = ((i + 6) as i32) * elem_size;
                            let a6 = builder.ins().stack_addr(types::I64, slot_a, off6);
                            let v_a6 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a6, 0);
                            let b6 = builder.ins().stack_addr(types::I64, slot_b, off6);
                            let v_b6 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b6, 0);

                            let off7 = ((i + 7) as i32) * elem_size;
                            let a7 = builder.ins().stack_addr(types::I64, slot_a, off7);
                            let v_a7 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a7, 0);
                            let b7 = builder.ins().stack_addr(types::I64, slot_b, off7);
                            let v_b7 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b7, 0);

                            if elem_ty.is_float() {
                                acc0 = builder.ins().fma(v_a0, v_b0, acc0);
                                acc1 = builder.ins().fma(v_a1, v_b1, acc1);
                                acc2 = builder.ins().fma(v_a2, v_b2, acc2);
                                acc3 = builder.ins().fma(v_a3, v_b3, acc3);
                                acc4 = builder.ins().fma(v_a4, v_b4, acc4);
                                acc5 = builder.ins().fma(v_a5, v_b5, acc5);
                                acc6 = builder.ins().fma(v_a6, v_b6, acc6);
                                acc7 = builder.ins().fma(v_a7, v_b7, acc7);
                            } else {
                                let p0 = builder.ins().imul(v_a0, v_b0); acc0 = builder.ins().iadd(acc0, p0);
                                let p1 = builder.ins().imul(v_a1, v_b1); acc1 = builder.ins().iadd(acc1, p1);
                                let p2 = builder.ins().imul(v_a2, v_b2); acc2 = builder.ins().iadd(acc2, p2);
                                let p3 = builder.ins().imul(v_a3, v_b3); acc3 = builder.ins().iadd(acc3, p3);
                                let p4 = builder.ins().imul(v_a4, v_b4); acc4 = builder.ins().iadd(acc4, p4);
                                let p5 = builder.ins().imul(v_a5, v_b5); acc5 = builder.ins().iadd(acc5, p5);
                                let p6 = builder.ins().imul(v_a6, v_b6); acc6 = builder.ins().iadd(acc6, p6);
                                let p7 = builder.ins().imul(v_a7, v_b7); acc7 = builder.ins().iadd(acc7, p7);
                            }
                            i += 8;
                        }

                        while i + 4 <= len_a {
                            let off0 = (i as i32) * elem_size;
                            let a0 = builder.ins().stack_addr(types::I64, slot_a, off0);
                            let v_a0 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a0, 0);
                            let b0 = builder.ins().stack_addr(types::I64, slot_b, off0);
                            let v_b0 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b0, 0);

                            let off1 = ((i + 1) as i32) * elem_size;
                            let a1 = builder.ins().stack_addr(types::I64, slot_a, off1);
                            let v_a1 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a1, 0);
                            let b1 = builder.ins().stack_addr(types::I64, slot_b, off1);
                            let v_b1 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b1, 0);

                            let off2 = ((i + 2) as i32) * elem_size;
                            let a2 = builder.ins().stack_addr(types::I64, slot_a, off2);
                            let v_a2 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a2, 0);
                            let b2 = builder.ins().stack_addr(types::I64, slot_b, off2);
                            let v_b2 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b2, 0);

                            let off3 = ((i + 3) as i32) * elem_size;
                            let a3 = builder.ins().stack_addr(types::I64, slot_a, off3);
                            let v_a3 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a3, 0);
                            let b3 = builder.ins().stack_addr(types::I64, slot_b, off3);
                            let v_b3 = builder.ins().load(clif_ty, MemFlagsData::trusted(), b3, 0);

                            if elem_ty.is_float() {
                                acc0 = builder.ins().fma(v_a0, v_b0, acc0);
                                acc1 = builder.ins().fma(v_a1, v_b1, acc1);
                                acc2 = builder.ins().fma(v_a2, v_b2, acc2);
                                acc3 = builder.ins().fma(v_a3, v_b3, acc3);
                            } else {
                                let p0 = builder.ins().imul(v_a0, v_b0);
                                acc0 = builder.ins().iadd(acc0, p0);
                                let p1 = builder.ins().imul(v_a1, v_b1);
                                acc1 = builder.ins().iadd(acc1, p1);
                                let p2 = builder.ins().imul(v_a2, v_b2);
                                acc2 = builder.ins().iadd(acc2, p2);
                                let p3 = builder.ins().imul(v_a3, v_b3);
                                acc3 = builder.ins().iadd(acc3, p3);
                            }
                            i += 4;
                        }

                        while i < len_a {
                            let off = (i as i32) * elem_size;
                            let a = builder.ins().stack_addr(types::I64, slot_a, off);
                            let v_a = builder.ins().load(clif_ty, MemFlagsData::trusted(), a, 0);
                            let b = builder.ins().stack_addr(types::I64, slot_b, off);
                            let v_b = builder.ins().load(clif_ty, MemFlagsData::trusted(), b, 0);
                            if elem_ty.is_float() {
                                acc0 = builder.ins().fma(v_a, v_b, acc0);
                            } else {
                                let p = builder.ins().imul(v_a, v_b);
                                acc0 = builder.ins().iadd(acc0, p);
                            }
                            i += 1;
                        }

                        let total = if elem_ty.is_float() {
                            let s01 = builder.ins().fadd(acc0, acc1);
                            let s23 = builder.ins().fadd(acc2, acc3);
                            let s45 = builder.ins().fadd(acc4, acc5);
                            let s67 = builder.ins().fadd(acc6, acc7);
                            let s03 = builder.ins().fadd(s01, s23);
                            let s47 = builder.ins().fadd(s45, s67);
                            builder.ins().fadd(s03, s47)
                        } else {
                            let s01 = builder.ins().iadd(acc0, acc1);
                            let s23 = builder.ins().iadd(acc2, acc3);
                            let s45 = builder.ins().iadd(acc4, acc5);
                            let s67 = builder.ins().iadd(acc6, acc7);
                            let s03 = builder.ins().iadd(s01, s23);
                            let s47 = builder.ins().iadd(s45, s67);
                            builder.ins().iadd(s03, s47)
                        };
                        return Ok(total);
                    }
                    "sum" => {
                        let (slot_a, len_a, elem_ty) = self.resolve_array_arg(&args[0], builder)?;
                        let elem_size = elem_ty.size_bytes() as i32;
                        let clif_ty = type_to_clif(elem_ty.clone());

                        let zero = if elem_ty.is_float() {
                            if elem_ty == Type::F32 {
                                builder.ins().f32const(0.0)
                            } else {
                                builder.ins().f64const(0.0)
                            }
                        } else {
                            builder.ins().iconst(clif_ty, 0)
                        };

                        let mut acc0 = zero;
                        let mut acc1 = zero;
                        let mut acc2 = zero;
                        let mut acc3 = zero;
                        let mut acc4 = zero;
                        let mut acc5 = zero;
                        let mut acc6 = zero;
                        let mut acc7 = zero;

                        let mut i = 0;
                        while i + 8 <= len_a {
                            let off0 = (i as i32) * elem_size;
                            let a0 = builder.ins().stack_addr(types::I64, slot_a, off0);
                            let v_a0 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a0, 0);

                            let off1 = ((i + 1) as i32) * elem_size;
                            let a1 = builder.ins().stack_addr(types::I64, slot_a, off1);
                            let v_a1 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a1, 0);

                            let off2 = ((i + 2) as i32) * elem_size;
                            let a2 = builder.ins().stack_addr(types::I64, slot_a, off2);
                            let v_a2 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a2, 0);

                            let off3 = ((i + 3) as i32) * elem_size;
                            let a3 = builder.ins().stack_addr(types::I64, slot_a, off3);
                            let v_a3 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a3, 0);

                            let off4 = ((i + 4) as i32) * elem_size;
                            let a4 = builder.ins().stack_addr(types::I64, slot_a, off4);
                            let v_a4 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a4, 0);

                            let off5 = ((i + 5) as i32) * elem_size;
                            let a5 = builder.ins().stack_addr(types::I64, slot_a, off5);
                            let v_a5 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a5, 0);

                            let off6 = ((i + 6) as i32) * elem_size;
                            let a6 = builder.ins().stack_addr(types::I64, slot_a, off6);
                            let v_a6 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a6, 0);

                            let off7 = ((i + 7) as i32) * elem_size;
                            let a7 = builder.ins().stack_addr(types::I64, slot_a, off7);
                            let v_a7 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a7, 0);

                            if elem_ty.is_float() {
                                acc0 = builder.ins().fadd(acc0, v_a0);
                                acc1 = builder.ins().fadd(acc1, v_a1);
                                acc2 = builder.ins().fadd(acc2, v_a2);
                                acc3 = builder.ins().fadd(acc3, v_a3);
                                acc4 = builder.ins().fadd(acc4, v_a4);
                                acc5 = builder.ins().fadd(acc5, v_a5);
                                acc6 = builder.ins().fadd(acc6, v_a6);
                                acc7 = builder.ins().fadd(acc7, v_a7);
                            } else {
                                acc0 = builder.ins().iadd(acc0, v_a0);
                                acc1 = builder.ins().iadd(acc1, v_a1);
                                acc2 = builder.ins().iadd(acc2, v_a2);
                                acc3 = builder.ins().iadd(acc3, v_a3);
                                acc4 = builder.ins().iadd(acc4, v_a4);
                                acc5 = builder.ins().iadd(acc5, v_a5);
                                acc6 = builder.ins().iadd(acc6, v_a6);
                                acc7 = builder.ins().iadd(acc7, v_a7);
                            }
                            i += 8;
                        }

                        while i + 4 <= len_a {
                            let off0 = (i as i32) * elem_size;
                            let a0 = builder.ins().stack_addr(types::I64, slot_a, off0);
                            let v_a0 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a0, 0);

                            let off1 = ((i + 1) as i32) * elem_size;
                            let a1 = builder.ins().stack_addr(types::I64, slot_a, off1);
                            let v_a1 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a1, 0);

                            let off2 = ((i + 2) as i32) * elem_size;
                            let a2 = builder.ins().stack_addr(types::I64, slot_a, off2);
                            let v_a2 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a2, 0);

                            let off3 = ((i + 3) as i32) * elem_size;
                            let a3 = builder.ins().stack_addr(types::I64, slot_a, off3);
                            let v_a3 = builder.ins().load(clif_ty, MemFlagsData::trusted(), a3, 0);

                            if elem_ty.is_float() {
                                acc0 = builder.ins().fadd(acc0, v_a0);
                                acc1 = builder.ins().fadd(acc1, v_a1);
                                acc2 = builder.ins().fadd(acc2, v_a2);
                                acc3 = builder.ins().fadd(acc3, v_a3);
                            } else {
                                acc0 = builder.ins().iadd(acc0, v_a0);
                                acc1 = builder.ins().iadd(acc1, v_a1);
                                acc2 = builder.ins().iadd(acc2, v_a2);
                                acc3 = builder.ins().iadd(acc3, v_a3);
                            }
                            i += 4;
                        }

                        while i < len_a {
                            let off = (i as i32) * elem_size;
                            let a = builder.ins().stack_addr(types::I64, slot_a, off);
                            let v_a = builder.ins().load(clif_ty, MemFlagsData::trusted(), a, 0);
                            if elem_ty.is_float() {
                                acc0 = builder.ins().fadd(acc0, v_a);
                            } else {
                                acc0 = builder.ins().iadd(acc0, v_a);
                            }
                            i += 1;
                        }

                        let total = if elem_ty.is_float() {
                            let s01 = builder.ins().fadd(acc0, acc1);
                            let s23 = builder.ins().fadd(acc2, acc3);
                            let s45 = builder.ins().fadd(acc4, acc5);
                            let s67 = builder.ins().fadd(acc6, acc7);
                            let s03 = builder.ins().fadd(s01, s23);
                            let s47 = builder.ins().fadd(s45, s67);
                            builder.ins().fadd(s03, s47)
                        } else {
                            let s01 = builder.ins().iadd(acc0, acc1);
                            let s23 = builder.ins().iadd(acc2, acc3);
                            let s45 = builder.ins().iadd(acc4, acc5);
                            let s67 = builder.ins().iadd(acc6, acc7);
                            let s03 = builder.ins().iadd(s01, s23);
                            let s47 = builder.ins().iadd(s45, s67);
                            builder.ins().iadd(s03, s47)
                        };
                        return Ok(total);
                    }
                    _ => {}
                }

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

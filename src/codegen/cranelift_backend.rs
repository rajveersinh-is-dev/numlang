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

fn compute_magic_s64(d: i64) -> (i64, u8, bool) {
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

fn get_constant_int(expr: &TypedExpr) -> Option<i64> {
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

fn is_safe_for_select(expr: &TypedExpr) -> bool {
    match expr {
        TypedExpr::Literal { .. } | TypedExpr::Ident { .. } => true,
        TypedExpr::Unary { expr, .. } => is_safe_for_select(expr),
        TypedExpr::Binary { op, left, right, .. } => {
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
        _ => false,
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

        // Step 3: Emit entry point (mainCRTStartup) if main exists and benchmarking mode is disabled
        if std::env::var("NUMLANG_BENCH").is_err() {
            if let Some(&main_id) = self.func_ids.get("main") {
                self.compile_entry_point(main_id, &mut ctx, &mut fn_builder_ctx)?;
            }
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

#[derive(Clone)]
enum Storage {
    Scalar(Variable),
    Array {
        slot: StackSlot,
        len: usize,
    },
    PromotedArray {
        vars: Vec<Variable>,
        len: usize,
        elem_ty: Type,
    },
}

#[derive(Clone)]
enum ResolvedArray {
    Promoted {
        vars: Vec<Variable>,
        len: usize,
        elem_ty: Type,
    },
    Slot {
        slot: StackSlot,
        len: usize,
        elem_ty: Type,
    },
}

impl ResolvedArray {
    fn len(&self) -> usize {
        match self {
            ResolvedArray::Promoted { len, .. } => *len,
            ResolvedArray::Slot { len, .. } => *len,
        }
    }

    fn elem_ty(&self) -> Type {
        match self {
            ResolvedArray::Promoted { elem_ty, .. } => elem_ty.clone(),
            ResolvedArray::Slot { elem_ty, .. } => elem_ty.clone(),
        }
    }
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

    fn var_mutations_in_block(block: &TypedBlock, var_name: &str) -> usize {
        let mut count = 0;
        for s in &block.stmts {
            match s {
                TypedStmt::Assign { name, .. } if name == var_name => count += 1,
                TypedStmt::If { then_branch, else_branch, .. } => {
                    count += Self::var_mutations_in_block(then_branch, var_name);
                    if let Some(eb) = else_branch {
                        count += Self::var_mutations_in_block(eb, var_name);
                    }
                }
                TypedStmt::While { body, .. } => {
                    count += Self::var_mutations_in_block(body, var_name);
                }
                _ => {}
            }
        }
        count
    }

    fn is_simple_induction_body(body: &TypedBlock, var_name: &str) -> bool {
        if Self::var_mutations_in_block(body, var_name) != 1 {
            return false;
        }
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

    fn emit_fast_signed_div(
        &mut self,
        l: Value,
        r: Value,
        d: i64,
        operand_ty: &Type,
        builder: &mut FunctionBuilder,
    ) -> Result<Value, CodegenError> {
        if d == 0 {
            return Ok(builder.ins().sdiv(l, r));
        }
        if d == 1 {
            return Ok(l);
        }
        if d == -1 {
            return Ok(builder.ins().ineg(l));
        }

        let is_i32 = *operand_ty == Type::I32;
        let n = if is_i32 {
            builder.ins().sextend(types::I64, l)
        } else {
            l
        };

        let ad = d.unsigned_abs();
        let q64 = if ad.is_power_of_two() {
            let k = ad.trailing_zeros();
            let q_pos = if k == 1 {
                let sign = builder.ins().ushr_imm_s(n, 63);
                let biased = builder.ins().iadd(n, sign);
                builder.ins().sshr_imm_s(biased, 1)
            } else {
                let sign = builder.ins().sshr_imm_s(n, 63);
                let bias = builder.ins().ushr_imm_s(sign, (64 - k) as i64);
                let biased = builder.ins().iadd(n, bias);
                builder.ins().sshr_imm_s(biased, k as i64)
            };
            if d < 0 {
                builder.ins().ineg(q_pos)
            } else {
                q_pos
            }
        } else {
            let (m, shift, add_ind) = compute_magic_s64(d.abs());
            let m_val = builder.ins().iconst(types::I64, m);
            let mut hi = builder.ins().smulhi(n, m_val);
            if add_ind {
                hi = builder.ins().iadd(hi, n);
            }
            let shifted = if shift > 0 {
                builder.ins().sshr_imm_s(hi, shift as i64)
            } else {
                hi
            };
            let sign = builder.ins().ushr_imm_s(n, 63);
            let q_pos = builder.ins().iadd(shifted, sign);
            if d < 0 {
                builder.ins().ineg(q_pos)
            } else {
                q_pos
            }
        };

        if is_i32 {
            Ok(builder.ins().ireduce(types::I32, q64))
        } else {
            Ok(q64)
        }
    }

    fn emit_fast_signed_rem(
        &mut self,
        l: Value,
        r: Value,
        d: i64,
        operand_ty: &Type,
        builder: &mut FunctionBuilder,
    ) -> Result<Value, CodegenError> {
        if d == 0 {
            return Ok(builder.ins().srem(l, r));
        }
        if d == 1 || d == -1 {
            let clif_ty = type_to_clif(operand_ty.clone());
            return Ok(builder.ins().iconst(clif_ty, 0));
        }

        let is_i32 = *operand_ty == Type::I32;
        let n = if is_i32 {
            builder.ins().sextend(types::I64, l)
        } else {
            l
        };

        let ad = d.unsigned_abs();
        let q64 = if ad.is_power_of_two() {
            let k = ad.trailing_zeros();
            let q_pos = if k == 1 {
                let sign = builder.ins().ushr_imm_s(n, 63);
                let biased = builder.ins().iadd(n, sign);
                builder.ins().sshr_imm_s(biased, 1)
            } else {
                let sign = builder.ins().sshr_imm_s(n, 63);
                let bias = builder.ins().ushr_imm_s(sign, (64 - k) as i64);
                let biased = builder.ins().iadd(n, bias);
                builder.ins().sshr_imm_s(biased, k as i64)
            };
            if d < 0 {
                builder.ins().ineg(q_pos)
            } else {
                q_pos
            }
        } else {
            let (m, shift, add_ind) = compute_magic_s64(d.abs());
            let m_val = builder.ins().iconst(types::I64, m);
            let mut hi = builder.ins().smulhi(n, m_val);
            if add_ind {
                hi = builder.ins().iadd(hi, n);
            }
            let shifted = if shift > 0 {
                builder.ins().sshr_imm_s(hi, shift as i64)
            } else {
                hi
            };
            let sign = builder.ins().ushr_imm_s(n, 63);
            let q_pos = builder.ins().iadd(shifted, sign);
            if d < 0 {
                builder.ins().ineg(q_pos)
            } else {
                q_pos
            }
        };

        // rem = n - q * d
        let q_times_d = if ad.is_power_of_two() {
            let k = ad.trailing_zeros();
            let shifted = builder.ins().ishl_imm_s(q64, k as i64);
            if d < 0 {
                builder.ins().ineg(shifted)
            } else {
                shifted
            }
        } else {
            let d_val = builder.ins().iconst(types::I64, d);
            builder.ins().imul(q64, d_val)
        };
        let rem64 = builder.ins().isub(n, q_times_d);

        if is_i32 {
            Ok(builder.ins().ireduce(types::I32, rem64))
        } else {
            Ok(rem64)
        }
    }

    fn resolve_array(
        &mut self,
        arg: &TypedExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<ResolvedArray, CodegenError> {
        match arg {
            TypedExpr::Ident { name, ty, .. } => {
                if let Some(storage) = self.variables.get(name).cloned() {
                    match storage {
                        Storage::PromotedArray { vars, len, elem_ty } => {
                            Ok(ResolvedArray::Promoted { vars, len, elem_ty })
                        }
                        Storage::Array { slot, len } => {
                            let elem_ty = ty.element_type().unwrap().clone();
                            Ok(ResolvedArray::Slot { slot, len, elem_ty })
                        }
                        _ => panic!("Argument '{}' is not an array variable", name),
                    }
                } else {
                    panic!("Variable '{}' not found", name);
                }
            }
            TypedExpr::ArrayLiteral { elements, ty, .. } => {
                let elem = ty.element_type().unwrap().clone();
                let len = elements.len();
                if len <= 16 {
                    let clif_ty = type_to_clif(elem.clone());
                    let mut vars = Vec::with_capacity(len);
                    for el in elements {
                        let var = builder.declare_var(clif_ty);
                        let val = self.translate_expr(el, builder)?;
                        builder.def_var(var, val);
                        vars.push(var);
                    }
                    Ok(ResolvedArray::Promoted {
                        vars,
                        len,
                        elem_ty: elem,
                    })
                } else {
                    let elem_size = elem.size_bytes() as u32;
                    let total_bytes = (elem_size * (len as u32)).max(1);
                    let slot_data = StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        total_bytes,
                        elem_size.min(8) as u8,
                    );
                    let slot = builder.create_sized_stack_slot(slot_data);

                    for (i, el) in elements.iter().enumerate() {
                        let el_val = self.translate_expr(el, builder)?;
                        let offset = (i as i32) * (elem_size as i32);
                        let addr = builder.ins().stack_addr(types::I64, slot, offset);
                        builder.ins().store(MemFlagsData::trusted(), el_val, addr, 0);
                    }
                    Ok(ResolvedArray::Slot {
                        slot,
                        len,
                        elem_ty: elem,
                    })
                }
            }
            _ => panic!("Expected array variable or literal"),
        }
    }

    fn get_array_element(
        &mut self,
        arr: &ResolvedArray,
        idx: usize,
        builder: &mut FunctionBuilder,
    ) -> Value {
        match arr {
            ResolvedArray::Promoted { vars, .. } => builder.use_var(vars[idx]),
            ResolvedArray::Slot { slot, elem_ty, .. } => {
                let elem_size = elem_ty.size_bytes() as i32;
                let clif_ty = type_to_clif(elem_ty.clone());
                let offset = (idx as i32) * elem_size;
                let addr = builder.ins().stack_addr(types::I64, *slot, offset);
                builder.ins().load(clif_ty, MemFlagsData::trusted(), addr, 0)
            }
        }
    }

    fn translate_vec_add_into_vars(
        &mut self,
        args: &[TypedExpr],
        dst_vars: &[Variable],
        elem_ty: &Type,
        len: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let arr_a = self.resolve_array(&args[0], builder)?;
        let arr_b = self.resolve_array(&args[1], builder)?;
        for i in 0..len {
            let val_a = self.get_array_element(&arr_a, i, builder);
            let val_b = self.get_array_element(&arr_b, i, builder);
            let sum = if elem_ty.is_float() {
                builder.ins().fadd(val_a, val_b)
            } else {
                builder.ins().iadd(val_a, val_b)
            };
            builder.def_var(dst_vars[i], sum);
        }
        Ok(())
    }

    fn translate_vec_add_into_slot(
        &mut self,
        args: &[TypedExpr],
        dst_slot: StackSlot,
        elem_ty: &Type,
        len: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let arr_a = self.resolve_array(&args[0], builder)?;
        let arr_b = self.resolve_array(&args[1], builder)?;
        let elem_size = elem_ty.size_bytes() as i32;

        for i in 0..len {
            let offset = (i as i32) * elem_size;
            let val_a = self.get_array_element(&arr_a, i, builder);
            let val_b = self.get_array_element(&arr_b, i, builder);
            let sum = if elem_ty.is_float() {
                builder.ins().fadd(val_a, val_b)
            } else {
                builder.ins().iadd(val_a, val_b)
            };
            let addr_dst = builder.ins().stack_addr(types::I64, dst_slot, offset);
            builder.ins().store(MemFlagsData::trusted(), sum, addr_dst, 0);
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
                    if *len <= 16 {
                        let clif_ty = type_to_clif((**elem).clone());
                        let mut vars = Vec::with_capacity(*len);
                        for _ in 0..*len {
                            vars.push(builder.declare_var(clif_ty));
                        }

                        match value {
                            TypedExpr::ArrayLiteral { elements, .. } => {
                                for (i, el) in elements.iter().enumerate() {
                                    let el_val = self.translate_expr(el, builder)?;
                                    builder.def_var(vars[i], el_val);
                                }
                            }
                            TypedExpr::Ident { name: src_name, .. } => {
                                if let Some(src_storage) = self.variables.get(src_name).cloned() {
                                    match src_storage {
                                        Storage::PromotedArray { vars: src_vars, .. } => {
                                            for i in 0..*len {
                                                let val = builder.use_var(src_vars[i]);
                                                builder.def_var(vars[i], val);
                                            }
                                        }
                                        Storage::Array { slot: src_slot, .. } => {
                                            let elem_size = elem.size_bytes() as i32;
                                            for i in 0..*len {
                                                let offset = (i as i32) * elem_size;
                                                let addr = builder.ins().stack_addr(types::I64, src_slot, offset);
                                                let val = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr, 0);
                                                builder.def_var(vars[i], val);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            TypedExpr::Call { callee, args, .. } if callee == "vec_add" => {
                                self.translate_vec_add_into_vars(args, &vars, elem, *len, builder)?;
                            }
                            _ => {
                                let zero = if elem.is_float() {
                                    if **elem == Type::F32 { builder.ins().f32const(0.0) } else { builder.ins().f64const(0.0) }
                                } else {
                                    builder.ins().iconst(clif_ty, 0)
                                };
                                for v in &vars {
                                    builder.def_var(*v, zero);
                                }
                            }
                        }

                        self.variables.insert(
                            name.clone(),
                            Storage::PromotedArray {
                                vars,
                                len: *len,
                                elem_ty: (**elem).clone(),
                            },
                        );
                        return Ok(false);
                    }

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
                            if let Some(src_storage) = self.variables.get(src_name).cloned() {
                                match src_storage {
                                    Storage::PromotedArray { vars: src_vars, .. } => {
                                        for i in 0..*len {
                                            let offset = (i as i32) * (elem_size as i32);
                                            let el_val = builder.use_var(src_vars[i]);
                                            let dst_addr =
                                                builder.ins().stack_addr(types::I64, slot, offset);
                                            builder.ins().store(MemFlagsData::trusted(), el_val, dst_addr, 0);
                                        }
                                    }
                                    Storage::Array { slot: src_slot, .. } => {
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
                                    _ => {}
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
                let storage = self
                    .variables
                    .get(name)
                    .cloned()
                    .expect("Variable must exist for assignment");
                match storage {
                    Storage::Scalar(var) => {
                        let val = self.translate_expr(value, builder)?;
                        builder.def_var(var, val);
                    }
                    Storage::PromotedArray { vars, len, elem_ty } => {
                        if let TypedExpr::Ident { name: src_name, .. } = value {
                            if let Some(src_storage) = self.variables.get(src_name).cloned() {
                                match src_storage {
                                    Storage::PromotedArray { vars: src_vars, .. } => {
                                        for i in 0..len {
                                            let val = builder.use_var(src_vars[i]);
                                            builder.def_var(vars[i], val);
                                        }
                                    }
                                    Storage::Array { slot: src_slot, .. } => {
                                        let elem_size = elem_ty.size_bytes() as i32;
                                        let clif_ty = type_to_clif(elem_ty.clone());
                                        for i in 0..len {
                                            let offset = (i as i32) * elem_size;
                                            let addr = builder.ins().stack_addr(types::I64, src_slot, offset);
                                            let val = builder.ins().load(clif_ty, MemFlagsData::trusted(), addr, 0);
                                            builder.def_var(vars[i], val);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        } else if let TypedExpr::Call { callee, args, .. } = value {
                            if callee == "vec_add" {
                                self.translate_vec_add_into_vars(args, &vars, &elem_ty, len, builder)?;
                            }
                        }
                    }
                    Storage::Array { slot, len } => {
                        if let TypedExpr::Ident { name: src_name, ty, .. } = value {
                            let elem = ty.element_type().unwrap().clone();
                            let elem_size = elem.size_bytes() as i32;
                            if let Some(src_storage) = self.variables.get(src_name).cloned() {
                                match src_storage {
                                    Storage::PromotedArray { vars: src_vars, .. } => {
                                        for i in 0..len {
                                            let offset = (i as i32) * elem_size;
                                            let el_val = builder.use_var(src_vars[i]);
                                            let dst_addr =
                                                builder.ins().stack_addr(types::I64, slot, offset);
                                            builder.ins().store(MemFlagsData::trusted(), el_val, dst_addr, 0);
                                        }
                                    }
                                    Storage::Array { slot: src_slot, .. } => {
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
                                    _ => {}
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
                let storage = self
                    .variables
                    .get(target)
                    .cloned()
                    .expect("Target must be an array variable");
                match storage {
                    Storage::PromotedArray { vars, len, .. } => {
                        if let TypedExpr::Literal {
                            lit: TypedLiteral::Int(idx_const, _),
                            ..
                        } = index
                        {
                            let c = *idx_const as usize;
                            if c < len {
                                let val = self.translate_expr(value, builder)?;
                                builder.def_var(vars[c], val);
                                return Ok(false);
                            }
                        }

                        let mut idx_val = self.translate_expr(index, builder)?;
                        if index.ty() == Type::I32 {
                            idx_val = builder.ins().uextend(types::I64, idx_val);
                        }
                        if !*is_safe {
                            self.emit_bounds_check(idx_val, len, builder);
                        }
                        let new_val = self.translate_expr(value, builder)?;
                        for k in 0..len {
                            let k_val = builder.ins().iconst(types::I64, k as i64);
                            let is_match = builder.ins().icmp(IntCC::Equal, idx_val, k_val);
                            let old_val = builder.use_var(vars[k]);
                            let updated = builder.ins().select(is_match, new_val, old_val);
                            builder.def_var(vars[k], updated);
                        }
                        Ok(false)
                    }
                    Storage::Array { slot, len } => {
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
                    _ => panic!("Target must be an array variable"),
                }
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
                // Multi-variable branchless predication: if both branches assign to the same list of scalar variables with safe expressions,
                // emit branchless `select` (cmov on x86_64) for each variable without any control-flow branching.
                if let Some(TypedBlock { stmts: else_stmts, .. }) = else_branch {
                    if then_branch.stmts.len() == else_stmts.len() && !then_branch.stmts.is_empty() {
                        let mut can_select = true;
                        let mut pairs = Vec::new();
                        for (s_then, s_else) in then_branch.stmts.iter().zip(else_stmts.iter()) {
                            if let (
                                TypedStmt::Assign { name: name_then, value: val_then, .. },
                                TypedStmt::Assign { name: name_else, value: val_else, .. },
                            ) = (s_then, s_else) {
                                if name_then == name_else && is_safe_for_select(val_then) && is_safe_for_select(val_else) {
                                    if let Some(Storage::Scalar(var)) = self.variables.get(name_then).cloned() {
                                        pairs.push((var, val_then, val_else));
                                        continue;
                                    }
                                }
                            }
                            can_select = false;
                            break;
                        }

                        if can_select && !pairs.is_empty() {
                            let cond_val = self.translate_expr(condition, builder)?;
                            for (var, val_then, val_else) in pairs {
                                let then_val = self.translate_expr(val_then, builder)?;
                                let else_val = self.translate_expr(val_else, builder)?;
                                let selected = builder.ins().select(cond_val, then_val, else_val);
                                builder.def_var(var, selected);
                            }
                            return Ok(false);
                        }
                    }
                }

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
                    if let Some(Storage::Scalar(var)) = self.variables.get(var_name).cloned() {
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

                // Rotated while loop: single conditional branch at the bottom of the body
                let body_block = builder.create_block();
                let exit_block = builder.create_block();

                let cond_init = self.translate_expr(condition, builder)?;
                builder
                    .ins()
                    .brif(cond_init, body_block, &[], exit_block, &[]);

                builder.switch_to_block(body_block);
                let body_term = self.translate_block(body, builder)?;
                if !body_term {
                    let cond_repeat = self.translate_expr(condition, builder)?;
                    builder
                        .ins()
                        .brif(cond_repeat, body_block, &[], exit_block, &[]);
                }
                builder.seal_block(body_block);

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
                let storage = self
                    .variables
                    .get(name)
                    .cloned()
                    .expect("Variable must be found in scope");
                match storage {
                    Storage::Scalar(var) => Ok(builder.use_var(var)),
                    Storage::Array { slot, .. } => {
                        Ok(builder.ins().stack_addr(types::I64, slot, 0))
                    }
                    Storage::PromotedArray { .. } => {
                        Ok(builder.ins().iconst(types::I64, 0))
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
                // Power-of-2 divisibility optimization: (x % 2^k) == 0  or  (x % 2^k) != 0
                if (*op == BinaryOp::Eq || *op == BinaryOp::Ne) && left.ty().is_integer() {
                    let check_pattern = |a: &TypedExpr, b: &TypedExpr| -> Option<(TypedExpr, i64)> {
                        if let (
                            TypedExpr::Binary { op: BinaryOp::Mod, left: x, right: d_expr, .. },
                            TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. },
                        ) = (a, b) {
                            if let Some(d) = get_constant_int(d_expr) {
                                if d > 0 && (d as u64).is_power_of_two() {
                                    return Some(((&**x).clone(), d));
                                }
                            }
                        }
                        None
                    };

                    if let Some((x_expr, d)) = check_pattern(left, right).or_else(|| check_pattern(right, left)) {
                        let x_val = self.translate_expr(&x_expr, builder)?;
                        let mask = (d - 1) as i64;
                        let masked = builder.ins().band_imm_s(x_val, mask);
                        let zero = builder.ins().iconst(type_to_clif(x_expr.ty()), 0);
                        let cc = if *op == BinaryOp::Eq {
                            IntCC::Equal
                        } else {
                            IntCC::NotEqual
                        };
                        return Ok(builder.ins().icmp(cc, masked, zero));
                    }
                }

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
                        } else if let Some(d) = get_constant_int(right) {
                            self.emit_fast_signed_div(l, r, d, &operand_ty, builder)
                        } else {
                            Ok(builder.ins().sdiv(l, r))
                        }
                    }
                    BinaryOp::Mod => {
                        if operand_ty.is_integer() {
                            if let Some(d) = get_constant_int(right) {
                                self.emit_fast_signed_rem(l, r, d, &operand_ty, builder)
                            } else {
                                Ok(builder.ins().srem(l, r))
                            }
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
                match target.as_ref() {
                    TypedExpr::Ident { name, .. } => {
                        let storage = self
                            .variables
                            .get(name)
                            .cloned()
                            .expect("Target array must exist");
                        match storage {
                            Storage::PromotedArray { vars, len, .. } => {
                                if let TypedExpr::Literal {
                                    lit: TypedLiteral::Int(idx_const, _),
                                    ..
                                } = index.as_ref()
                                {
                                    let c = *idx_const as usize;
                                    if c < len {
                                        return Ok(builder.use_var(vars[c]));
                                    }
                                }

                                let mut idx_val = self.translate_expr(index, builder)?;
                                if index.ty() == Type::I32 {
                                    idx_val = builder.ins().uextend(types::I64, idx_val);
                                }
                                if !*is_safe {
                                    self.emit_bounds_check(idx_val, len, builder);
                                }
                                let mut res = builder.use_var(vars[0]);
                                for k in 1..len {
                                    let k_val = builder.ins().iconst(types::I64, k as i64);
                                    let is_match = builder.ins().icmp(IntCC::Equal, idx_val, k_val);
                                    let val_k = builder.use_var(vars[k]);
                                    res = builder.ins().select(is_match, val_k, res);
                                }
                                Ok(res)
                            }
                            Storage::Array { slot, len } => {
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
                            _ => panic!("Index target must be an array variable"),
                        }
                    }
                    _ => panic!("Indexing supported on array variables"),
                }
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
                        let arr_a = self.resolve_array(&args[0], builder)?;
                        let arr_b = self.resolve_array(&args[1], builder)?;
                        let len_a = arr_a.len();
                        let elem_ty = arr_a.elem_ty();
                        let clif_ty = type_to_clif(elem_ty.clone());

                        if len_a == 4 {
                            let v_a0 = self.get_array_element(&arr_a, 0, builder);
                            let v_b0 = self.get_array_element(&arr_b, 0, builder);
                            let v_a1 = self.get_array_element(&arr_a, 1, builder);
                            let v_b1 = self.get_array_element(&arr_b, 1, builder);
                            let v_a2 = self.get_array_element(&arr_a, 2, builder);
                            let v_b2 = self.get_array_element(&arr_b, 2, builder);
                            let v_a3 = self.get_array_element(&arr_a, 3, builder);
                            let v_b3 = self.get_array_element(&arr_b, 3, builder);

                            if elem_ty.is_float() {
                                let p0 = builder.ins().fmul(v_a0, v_b0);
                                let p1 = builder.ins().fmul(v_a1, v_b1);
                                let p2 = builder.ins().fmul(v_a2, v_b2);
                                let p3 = builder.ins().fmul(v_a3, v_b3);
                                let s01 = builder.ins().fadd(p0, p1);
                                let s23 = builder.ins().fadd(p2, p3);
                                return Ok(builder.ins().fadd(s01, s23));
                            } else {
                                let p0 = builder.ins().imul(v_a0, v_b0);
                                let p1 = builder.ins().imul(v_a1, v_b1);
                                let p2 = builder.ins().imul(v_a2, v_b2);
                                let p3 = builder.ins().imul(v_a3, v_b3);
                                let s01 = builder.ins().iadd(p0, p1);
                                let s23 = builder.ins().iadd(p2, p3);
                                return Ok(builder.ins().iadd(s01, s23));
                            }
                        }

                        if len_a == 8 {
                            let v_a0 = self.get_array_element(&arr_a, 0, builder);
                            let v_b0 = self.get_array_element(&arr_b, 0, builder);
                            let v_a1 = self.get_array_element(&arr_a, 1, builder);
                            let v_b1 = self.get_array_element(&arr_b, 1, builder);
                            let v_a2 = self.get_array_element(&arr_a, 2, builder);
                            let v_b2 = self.get_array_element(&arr_b, 2, builder);
                            let v_a3 = self.get_array_element(&arr_a, 3, builder);
                            let v_b3 = self.get_array_element(&arr_b, 3, builder);
                            let v_a4 = self.get_array_element(&arr_a, 4, builder);
                            let v_b4 = self.get_array_element(&arr_b, 4, builder);
                            let v_a5 = self.get_array_element(&arr_a, 5, builder);
                            let v_b5 = self.get_array_element(&arr_b, 5, builder);
                            let v_a6 = self.get_array_element(&arr_a, 6, builder);
                            let v_b6 = self.get_array_element(&arr_b, 6, builder);
                            let v_a7 = self.get_array_element(&arr_a, 7, builder);
                            let v_b7 = self.get_array_element(&arr_b, 7, builder);

                            if elem_ty.is_float() {
                                let p0 = builder.ins().fmul(v_a0, v_b0);
                                let p1 = builder.ins().fmul(v_a1, v_b1);
                                let p2 = builder.ins().fmul(v_a2, v_b2);
                                let p3 = builder.ins().fmul(v_a3, v_b3);
                                let p4 = builder.ins().fmul(v_a4, v_b4);
                                let p5 = builder.ins().fmul(v_a5, v_b5);
                                let p6 = builder.ins().fmul(v_a6, v_b6);
                                let p7 = builder.ins().fmul(v_a7, v_b7);
                                let s01 = builder.ins().fadd(p0, p1);
                                let s23 = builder.ins().fadd(p2, p3);
                                let s45 = builder.ins().fadd(p4, p5);
                                let s67 = builder.ins().fadd(p6, p7);
                                let s03 = builder.ins().fadd(s01, s23);
                                let s47 = builder.ins().fadd(s45, s67);
                                return Ok(builder.ins().fadd(s03, s47));
                            } else {
                                let p0 = builder.ins().imul(v_a0, v_b0);
                                let p1 = builder.ins().imul(v_a1, v_b1);
                                let p2 = builder.ins().imul(v_a2, v_b2);
                                let p3 = builder.ins().imul(v_a3, v_b3);
                                let p4 = builder.ins().imul(v_a4, v_b4);
                                let p5 = builder.ins().imul(v_a5, v_b5);
                                let p6 = builder.ins().imul(v_a6, v_b6);
                                let p7 = builder.ins().imul(v_a7, v_b7);
                                let s01 = builder.ins().iadd(p0, p1);
                                let s23 = builder.ins().iadd(p2, p3);
                                let s45 = builder.ins().iadd(p4, p5);
                                let s67 = builder.ins().iadd(p6, p7);
                                let s03 = builder.ins().iadd(s01, s23);
                                let s47 = builder.ins().iadd(s45, s67);
                                return Ok(builder.ins().iadd(s03, s47));
                            }
                        }

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
                            let v_a0 = self.get_array_element(&arr_a, i, builder);
                            let v_b0 = self.get_array_element(&arr_b, i, builder);
                            let v_a1 = self.get_array_element(&arr_a, i + 1, builder);
                            let v_b1 = self.get_array_element(&arr_b, i + 1, builder);
                            let v_a2 = self.get_array_element(&arr_a, i + 2, builder);
                            let v_b2 = self.get_array_element(&arr_b, i + 2, builder);
                            let v_a3 = self.get_array_element(&arr_a, i + 3, builder);
                            let v_b3 = self.get_array_element(&arr_b, i + 3, builder);
                            let v_a4 = self.get_array_element(&arr_a, i + 4, builder);
                            let v_b4 = self.get_array_element(&arr_b, i + 4, builder);
                            let v_a5 = self.get_array_element(&arr_a, i + 5, builder);
                            let v_b5 = self.get_array_element(&arr_b, i + 5, builder);
                            let v_a6 = self.get_array_element(&arr_a, i + 6, builder);
                            let v_b6 = self.get_array_element(&arr_b, i + 6, builder);
                            let v_a7 = self.get_array_element(&arr_a, i + 7, builder);
                            let v_b7 = self.get_array_element(&arr_b, i + 7, builder);

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
                            let v_a0 = self.get_array_element(&arr_a, i, builder);
                            let v_b0 = self.get_array_element(&arr_b, i, builder);
                            let v_a1 = self.get_array_element(&arr_a, i + 1, builder);
                            let v_b1 = self.get_array_element(&arr_b, i + 1, builder);
                            let v_a2 = self.get_array_element(&arr_a, i + 2, builder);
                            let v_b2 = self.get_array_element(&arr_b, i + 2, builder);
                            let v_a3 = self.get_array_element(&arr_a, i + 3, builder);
                            let v_b3 = self.get_array_element(&arr_b, i + 3, builder);

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
                            let v_a = self.get_array_element(&arr_a, i, builder);
                            let v_b = self.get_array_element(&arr_b, i, builder);
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
                        let arr_a = self.resolve_array(&args[0], builder)?;
                        let len_a = arr_a.len();
                        let elem_ty = arr_a.elem_ty();
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
                            let v_a0 = self.get_array_element(&arr_a, i, builder);
                            let v_a1 = self.get_array_element(&arr_a, i + 1, builder);
                            let v_a2 = self.get_array_element(&arr_a, i + 2, builder);
                            let v_a3 = self.get_array_element(&arr_a, i + 3, builder);
                            let v_a4 = self.get_array_element(&arr_a, i + 4, builder);
                            let v_a5 = self.get_array_element(&arr_a, i + 5, builder);
                            let v_a6 = self.get_array_element(&arr_a, i + 6, builder);
                            let v_a7 = self.get_array_element(&arr_a, i + 7, builder);

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
                            let v_a0 = self.get_array_element(&arr_a, i, builder);
                            let v_a1 = self.get_array_element(&arr_a, i + 1, builder);
                            let v_a2 = self.get_array_element(&arr_a, i + 2, builder);
                            let v_a3 = self.get_array_element(&arr_a, i + 3, builder);

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
                            let v_a = self.get_array_element(&arr_a, i, builder);
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
    let mut optimized = program.clone();
    crate::opt::optimize_program(&mut optimized);
    let compiler = CraneliftCompiler::new()?;
    compiler.compile_program(&optimized)
}

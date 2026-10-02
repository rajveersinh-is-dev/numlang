//! SSA Mid-level IR (MIR) lowering to Cranelift IR.

use std::collections::HashMap;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, InstBuilder, MemFlagsData, StackSlot, StackSlotData, StackSlotKind, TrapCode, Value,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{Linkage, Module};

use crate::ast::{BinaryOp, UnaryOp};
use crate::typecheck::{Type, TypedLiteral};
use super::abi::*;
use super::ast_stmt::emit_copy_bytes_raw;
use super::CraneliftCompiler;

impl CraneliftCompiler {
    pub fn compile_mir_function(
        &mut self,
        func: &crate::mir::lower::MirFunction,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let func_id = *self
            .func_ids
            .get(&func.name)
            .ok_or_else(|| CodegenError::BackendError(format!("Function {} not declared", func.name)))?;

        let mut sig = self.module.make_signature();
        let is_sret = matches!(&func.return_ty, Type::Struct(_) | Type::Enum(_));
        if is_sret {
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        } else if func.return_ty != Type::Void {
            sig.returns.push(AbiParam::new(type_to_clif(func.return_ty.clone())));
        }
        for (_, p_ty) in &func.params {
            if let Type::Struct(_) | Type::Enum(_) = p_ty {
                sig.params.push(AbiParam::new(types::I64));
            } else {
                sig.params.push(AbiParam::new(type_to_clif(p_ty.clone())));
            }
        }

        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        if func.blocks.is_empty() {
            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.seal_block(entry);
            builder.ins().return_(&[]);
            let config = self.module.target_config();
            builder.finalize(config);
            self.module
                .define_function(func_id, ctx)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?;
            self.module.clear_context(ctx);
            return Ok(());
        }

        // Map basic blocks
        let clif_entry = builder.create_block();
        let mut block_map: HashMap<crate::mir::BasicBlockId, cranelift_codegen::ir::Block> =
            HashMap::new();
        for b in &func.blocks {
            let clif_b = builder.create_block();
            block_map.insert(b.id.clone(), clif_b);
        }

        // Declare variables for locals and parameters
        let aliases = crate::mir::supercompiler::fusion::build_alias_map(func);
        let mut array_slots: HashMap<String, (StackSlot, usize, Type)> = HashMap::new();
        for local in &func.locals {
            if let Type::Array(ref elem_ty, len) = local.ty {
                let elem_size = elem_ty.size_bytes().max(1);
                let slot_size = ((len * elem_size) as u32).max(8);
                let slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    slot_size,
                    3,
                ));
                array_slots.insert(local.name.clone(), (slot, len, (**elem_ty).clone()));
            }
        }
        for local in &func.locals {
            let real_name = crate::mir::supercompiler::fusion::resolve_alias(&local.name, &aliases);
            if let Some(slot_info) = array_slots.get(real_name).cloned() {
                array_slots.insert(local.name.clone(), slot_info);
            }
        }
        for (k, v) in &aliases {
            if let Some(slot_info) = array_slots.get(v).cloned() {
                array_slots.insert(k.clone(), slot_info);
            }
        }

        let mut var_map: HashMap<String, (Variable, types::Type)> = HashMap::new();
        for local in &func.locals {
            let clif_ty = type_to_clif(local.ty.clone());
            let var = builder.declare_var(clif_ty);
            var_map.insert(local.name.clone(), (var, clif_ty));
        }
        for (p_name, p_ty) in &func.params {
            if !var_map.contains_key(p_name) {
                let clif_ty = type_to_clif(p_ty.clone());
                let var = builder.declare_var(clif_ty);
                var_map.insert(p_name.clone(), (var, clif_ty));
            }
        }

        // Pre-declare any temporaries referenced in statements/terminators
        for b in &func.blocks {
            for stmt in &b.statements {
                let crate::mir::lower::Statement::Assign(place, rval) = stmt;
                if !var_map.contains_key(&place.local) {
                    let var = builder.declare_var(types::I64);
                    var_map.insert(place.local.clone(), (var, types::I64));
                }
                match rval {
                    crate::mir::lower::Rvalue::Use(p) => {
                        if !var_map.contains_key(&p.local) {
                            let var = builder.declare_var(types::I64);
                            var_map.insert(p.local.clone(), (var, types::I64));
                        }
                    }
                    crate::mir::lower::Rvalue::BinaryOp(_, l, r) => {
                        if !var_map.contains_key(&l.local) {
                            let var = builder.declare_var(types::I64);
                            var_map.insert(l.local.clone(), (var, types::I64));
                        }
                        if !var_map.contains_key(&r.local) {
                            let var = builder.declare_var(types::I64);
                            var_map.insert(r.local.clone(), (var, types::I64));
                        }
                    }
                    crate::mir::lower::Rvalue::UnaryOp(_, p) => {
                        if !var_map.contains_key(&p.local) {
                            let var = builder.declare_var(types::I64);
                            var_map.insert(p.local.clone(), (var, types::I64));
                        }
                    }
                    crate::mir::lower::Rvalue::Call(_, args) => {
                        for p in args {
                            if !var_map.contains_key(&p.local) {
                                let var = builder.declare_var(types::I64);
                                var_map.insert(p.local.clone(), (var, types::I64));
                            }
                        }
                    }
                    crate::mir::lower::Rvalue::Discriminant(p) => {
                        if !var_map.contains_key(&p.local) {
                            let var = builder.declare_var(types::I64);
                            var_map.insert(p.local.clone(), (var, types::I64));
                        }
                    }
                    crate::mir::lower::Rvalue::EnumVariant { fields, .. } => {
                        for p in fields {
                            if !var_map.contains_key(&p.local) {
                                let var = builder.declare_var(types::I64);
                                var_map.insert(p.local.clone(), (var, types::I64));
                            }
                        }
                    }
                    crate::mir::lower::Rvalue::Alloc(p) | crate::mir::lower::Rvalue::Load(p)
                        if !var_map.contains_key(&p.local) =>
                    {
                        let var = builder.declare_var(types::I64);
                        var_map.insert(p.local.clone(), (var, types::I64));
                    }
                    _ => {}
                }
            }
            match &b.terminator {
                crate::mir::Terminator::Return { value: Some(p) }
                    if !var_map.contains_key(&p.local) =>
                {
                    let var = builder.declare_var(types::I64);
                    var_map.insert(p.local.clone(), (var, types::I64));
                }
                crate::mir::Terminator::BranchIf { condition, .. }
                    if !var_map.contains_key(&condition.local) =>
                {
                    let var = builder.declare_var(types::I8);
                    var_map.insert(condition.local.clone(), (var, types::I8));
                }
                crate::mir::Terminator::Switch { value, .. }
                    if !var_map.contains_key(&value.local) =>
                {
                    let var = builder.declare_var(types::I64);
                    var_map.insert(value.local.clone(), (var, types::I64));
                }
                _ => {}
            }
        }

        let mut local_types: HashMap<String, Type> = HashMap::new();
        for local in &func.locals {
            local_types.insert(local.name.clone(), local.ty.clone());
        }
        for (p_name, p_ty) in &func.params {
            local_types.insert(p_name.clone(), p_ty.clone());
        }

        builder.append_block_params_for_function_params(clif_entry);
        builder.switch_to_block(clif_entry);

        let mut current_sret_ptr: Option<Value> = None;
        let param_offset = if is_sret {
            let sret_val = builder.block_params(clif_entry)[0];
            current_sret_ptr = Some(sret_val);
            1
        } else {
            0
        };

        for (i, (param_name, _)) in func.params.iter().enumerate() {
            let val = builder.block_params(clif_entry)[i + param_offset];
            if let Some(&(var, _)) = var_map.get(param_name) {
                builder.def_var(var, val);
            }
        }

        let first_mir_block = block_map[&func.blocks[0].id];
        builder.ins().jump(first_mir_block, &[]);

        // Emit blocks
        for b in &func.blocks {
            let clif_b = block_map[&b.id];
            builder.switch_to_block(clif_b);

            for stmt in &b.statements {
                let crate::mir::lower::Statement::Assign(place, rval) = stmt;
                let val = match rval {
                    crate::mir::lower::Rvalue::Constant(lit) => match lit {
                        TypedLiteral::Int(v, ty) => {
                            builder.ins().iconst(type_to_clif(ty.clone()), *v)
                        }
                        TypedLiteral::Float(f, ty) => {
                            if *ty == Type::F32 {
                                builder.ins().f32const(*f as f32)
                            } else {
                                builder.ins().f64const(*f)
                            }
                        }
                        TypedLiteral::Bool(bv) => {
                            builder.ins().iconst(types::I8, if *bv { 1 } else { 0 })
                        }
                        TypedLiteral::Str(_) => builder.ins().iconst(types::I64, 0),
                    },
                    crate::mir::lower::Rvalue::Use(p) => {
                        get_place_value(&mut builder, &var_map, &array_slots, &aliases, p)
                    }
                    crate::mir::lower::Rvalue::BinaryOp(op, l, r) => {
                        let mut lv = get_place_value(&mut builder, &var_map, &array_slots, &aliases, l);
                        let mut rv = get_place_value(&mut builder, &var_map, &array_slots, &aliases, r);
                        let l_ty = builder.func.dfg.value_type(lv);
                        let r_ty = builder.func.dfg.value_type(rv);
                        if l_ty.is_int() && r_ty.is_int() && l_ty != r_ty {
                            if l_ty.bits() > r_ty.bits() {
                                rv = builder.ins().uextend(l_ty, rv);
                            } else {
                                lv = builder.ins().uextend(r_ty, lv);
                            }
                        }
                        if l_ty.is_float() {
                            match op {
                                BinaryOp::Add => builder.ins().fadd(lv, rv),
                                BinaryOp::Sub => builder.ins().fsub(lv, rv),
                                BinaryOp::Mul => builder.ins().fmul(lv, rv),
                                BinaryOp::Div => builder.ins().fdiv(lv, rv),
                                BinaryOp::Eq => builder.ins().fcmp(FloatCC::Equal, lv, rv),
                                BinaryOp::Ne => builder.ins().fcmp(FloatCC::NotEqual, lv, rv),
                                BinaryOp::Lt => builder.ins().fcmp(FloatCC::LessThan, lv, rv),
                                BinaryOp::Le => builder.ins().fcmp(FloatCC::LessThanOrEqual, lv, rv),
                                BinaryOp::Gt => builder.ins().fcmp(FloatCC::GreaterThan, lv, rv),
                                BinaryOp::Ge => builder.ins().fcmp(FloatCC::GreaterThanOrEqual, lv, rv),
                                _ => builder.ins().fadd(lv, rv),
                            }
                        } else {
                            match op {
                                BinaryOp::Add => builder.ins().iadd(lv, rv),
                                BinaryOp::Sub => builder.ins().isub(lv, rv),
                                BinaryOp::Mul => builder.ins().imul(lv, rv),
                                BinaryOp::Div => builder.ins().sdiv(lv, rv),
                                BinaryOp::Mod => builder.ins().srem(lv, rv),
                                BinaryOp::Eq => builder.ins().icmp(IntCC::Equal, lv, rv),
                                BinaryOp::Ne => builder.ins().icmp(IntCC::NotEqual, lv, rv),
                                BinaryOp::Lt => {
                                    builder.ins().icmp(IntCC::SignedLessThan, lv, rv)
                                }
                                BinaryOp::Le => {
                                    builder.ins().icmp(IntCC::SignedLessThanOrEqual, lv, rv)
                                }
                                BinaryOp::Gt => {
                                    builder.ins().icmp(IntCC::SignedGreaterThan, lv, rv)
                                }
                                BinaryOp::Ge => {
                                    builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, lv, rv)
                                }
                                BinaryOp::BitAnd => builder.ins().band(lv, rv),
                                BinaryOp::BitOr => builder.ins().bor(lv, rv),
                                BinaryOp::BitXor => builder.ins().bxor(lv, rv),
                                BinaryOp::Shl => builder.ins().ishl(lv, rv),
                                BinaryOp::Shr => builder.ins().sshr(lv, rv),
                                BinaryOp::Pow => builder.ins().imul(lv, rv),
                            }
                        }
                    }
                    crate::mir::lower::Rvalue::UnaryOp(op, p) => {
                        let v = get_place_value(&mut builder, &var_map, &array_slots, &aliases, p);
                        let v_ty = builder.func.dfg.value_type(v);
                        match op {
                            UnaryOp::Neg => {
                                if v_ty.is_float() {
                                    builder.ins().fneg(v)
                                } else {
                                    builder.ins().ineg(v)
                                }
                            }
                            UnaryOp::Not => builder.ins().bnot(v),
                        }
                    }
                    crate::mir::lower::Rvalue::Call(callee, args) => {
                        if callee == "__nl_loop_reset" || callee == "__nl_arena_reset" {
                            let loop_reset_func = self.module.declare_func_in_func(self.loop_reset_id, builder.func);
                            builder.ins().call(loop_reset_func, &[]);
                            builder.ins().iconst(types::I64, 0)
                        } else if callee == "__nl_arena_alloc" && !args.is_empty() {
                            let size_arg = get_place_value(&mut builder, &var_map, &array_slots, &aliases, &args[0]);
                            let malloc_func = self.module.declare_func_in_func(self.malloc_id, builder.func);
                            let call_inst = builder.ins().call(malloc_func, &[size_arg]);
                            builder.inst_results(call_inst)[0]
                        } else if callee == "__numlang_fib" && !args.is_empty() {
                            let n_arg = get_place_value(&mut builder, &var_map, &array_slots, &aliases, &args[0]);
                            let n_val = if builder.func.dfg.value_type(n_arg) != types::I64 {
                                builder.ins().uextend(types::I64, n_arg)
                            } else {
                                n_arg
                            };

                            let fib_loop = builder.create_block();
                            let fib_body = builder.create_block();
                            let fib_done = builder.create_block();

                            let a_var = builder.declare_var(types::I64);
                            let b_var = builder.declare_var(types::I64);
                            let i_var = builder.declare_var(types::I64);

                            let zero = builder.ins().iconst(types::I64, 0);
                            let one = builder.ins().iconst(types::I64, 1);

                            builder.def_var(a_var, zero);
                            builder.def_var(b_var, one);
                            builder.def_var(i_var, zero);

                            builder.ins().jump(fib_loop, &[]);
                            builder.switch_to_block(fib_loop);

                            let cur_i = builder.use_var(i_var);
                            let cond = builder.ins().icmp(IntCC::SignedLessThan, cur_i, n_val);
                            builder.ins().brif(cond, fib_body, &[], fib_done, &[]);

                            builder.switch_to_block(fib_body);
                            let cur_a = builder.use_var(a_var);
                            let cur_b = builder.use_var(b_var);
                            let next_b = builder.ins().iadd(cur_a, cur_b);
                            let next_i = builder.ins().iadd(cur_i, one);
                            builder.def_var(a_var, cur_b);
                            builder.def_var(b_var, next_b);
                            builder.def_var(i_var, next_i);
                            builder.ins().jump(fib_loop, &[]);

                            builder.switch_to_block(fib_done);
                            builder.use_var(a_var)
                        } else if (callee == "__coupled_a" || callee == "__coupled_b") && args.len() >= 9 {
                            let is_b = callee == "__coupled_b";
                            let mut arg_vals = Vec::with_capacity(9);
                            for arg_op in args.iter().take(9) {
                                let arg_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, arg_op);
                                let val_i64 = if builder.func.dfg.value_type(arg_val) != types::I64 {
                                    builder.ins().uextend(types::I64, arg_val)
                                } else {
                                    arg_val
                                };
                                arg_vals.push(val_i64);
                            }
                            let p = arg_vals[0];
                            let q = arg_vals[1];
                            let ca = arg_vals[2];
                            let r = arg_vals[3];
                            let s = arg_vals[4];
                            let cb = arg_vals[5];
                            let a0 = arg_vals[6];
                            let b0 = arg_vals[7];
                            let n_val = arg_vals[8];

                            let loop_head = builder.create_block();
                            let loop_body = builder.create_block();
                            let loop_done = builder.create_block();

                            let a_var = builder.declare_var(types::I64);
                            let b_var = builder.declare_var(types::I64);
                            let i_var = builder.declare_var(types::I64);

                            let zero = builder.ins().iconst(types::I64, 0);
                            let one = builder.ins().iconst(types::I64, 1);

                            builder.def_var(a_var, a0);
                            builder.def_var(b_var, b0);
                            builder.def_var(i_var, zero);

                            builder.ins().jump(loop_head, &[]);
                            builder.switch_to_block(loop_head);

                            let cur_i = builder.use_var(i_var);
                            let cond = builder.ins().icmp(IntCC::SignedLessThan, cur_i, n_val);
                            builder.ins().brif(cond, loop_body, &[], loop_done, &[]);

                            builder.switch_to_block(loop_body);
                            let cur_a = builder.use_var(a_var);
                            let cur_b = builder.use_var(b_var);

                            let pa = builder.ins().imul(p, cur_a);
                            let qb = builder.ins().imul(q, cur_b);
                            let next_a_tmp = builder.ins().iadd(pa, qb);
                            let next_a = builder.ins().iadd(next_a_tmp, ca);

                            let ra = builder.ins().imul(r, cur_a);
                            let sb = builder.ins().imul(s, cur_b);
                            let next_b_tmp = builder.ins().iadd(ra, sb);
                            let next_b = builder.ins().iadd(next_b_tmp, cb);

                            let next_i = builder.ins().iadd(cur_i, one);
                            builder.def_var(a_var, next_a);
                            builder.def_var(b_var, next_b);
                            builder.def_var(i_var, next_i);
                            builder.ins().jump(loop_head, &[]);

                            builder.switch_to_block(loop_done);
                            if is_b {
                                builder.use_var(b_var)
                            } else {
                                builder.use_var(a_var)
                            }
                        } else if callee == "__order3_recurrence" && args.len() >= 7 {
                            let mut arg_vals = Vec::with_capacity(7);
                            for arg_op in args.iter().take(7) {
                                let arg_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, arg_op);
                                let val_i64 = if builder.func.dfg.value_type(arg_val) != types::I64 {
                                    builder.ins().uextend(types::I64, arg_val)
                                } else {
                                    arg_val
                                };
                                arg_vals.push(val_i64);
                            }
                            let c1 = arg_vals[0];
                            let c2 = arg_vals[1];
                            let c3 = arg_vals[2];
                            let s0 = arg_vals[3];
                            let s1 = arg_vals[4];
                            let s2 = arg_vals[5];
                            let n_val = arg_vals[6];

                            let loop_head = builder.create_block();
                            let loop_body = builder.create_block();
                            let loop_done = builder.create_block();

                            let v0_var = builder.declare_var(types::I64);
                            let v1_var = builder.declare_var(types::I64);
                            let v2_var = builder.declare_var(types::I64);
                            let i_var = builder.declare_var(types::I64);

                            let zero = builder.ins().iconst(types::I64, 0);
                            let one = builder.ins().iconst(types::I64, 1);

                            builder.def_var(v0_var, s0);
                            builder.def_var(v1_var, s1);
                            builder.def_var(v2_var, s2);
                            builder.def_var(i_var, zero);

                            builder.ins().jump(loop_head, &[]);
                            builder.switch_to_block(loop_head);

                            let cur_i = builder.use_var(i_var);
                            let cond = builder.ins().icmp(IntCC::SignedLessThan, cur_i, n_val);
                            builder.ins().brif(cond, loop_body, &[], loop_done, &[]);

                            builder.switch_to_block(loop_body);
                            let cur_v0 = builder.use_var(v0_var);
                            let cur_v1 = builder.use_var(v1_var);
                            let cur_v2 = builder.use_var(v2_var);

                            let c1_v2 = builder.ins().imul(c1, cur_v2);
                            let c2_v1 = builder.ins().imul(c2, cur_v1);
                            let c3_v0 = builder.ins().imul(c3, cur_v0);

                            let sum1 = builder.ins().iadd(c1_v2, c2_v1);
                            let next_v2 = builder.ins().iadd(sum1, c3_v0);

                            let next_i = builder.ins().iadd(cur_i, one);
                            builder.def_var(v0_var, cur_v1);
                            builder.def_var(v1_var, cur_v2);
                            builder.def_var(v2_var, next_v2);
                            builder.def_var(i_var, next_i);
                            builder.ins().jump(loop_head, &[]);

                            builder.switch_to_block(loop_done);
                            builder.use_var(v0_var)
                        } else if callee.starts_with("__nway_recurrence_") && !args.is_empty() {
                            let target_idx = callee
                                .strip_prefix("__nway_recurrence_")
                                .and_then(|s| s.parse::<usize>().ok())
                                .unwrap_or(0);
                            let mut arg_vals = Vec::with_capacity(args.len());
                            for arg_op in args {
                                let arg_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, arg_op);
                                let val_i64 = if builder.func.dfg.value_type(arg_val) != types::I64 {
                                    builder.ins().uextend(types::I64, arg_val)
                                } else {
                                    arg_val
                                };
                                arg_vals.push(val_i64);
                            }
                            let n = (1..=8).find(|&k| k * k + 2 * k + 2 == arg_vals.len()).unwrap_or(3);
                            let n_val = arg_vals[1 + n * n + 2 * n];

                            let mut v_vars = Vec::with_capacity(n);
                            for i in 0..n {
                                let var = builder.declare_var(types::I64);
                                builder.def_var(var, arg_vals[1 + n * n + n + i]);
                                v_vars.push(var);
                            }
                            let i_var = builder.declare_var(types::I64);
                            let zero = builder.ins().iconst(types::I64, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            builder.def_var(i_var, zero);

                            let loop_head = builder.create_block();
                            let loop_body = builder.create_block();
                            let loop_done = builder.create_block();

                            builder.ins().jump(loop_head, &[]);
                            builder.switch_to_block(loop_head);

                            let cur_i = builder.use_var(i_var);
                            let cond = builder.ins().icmp(IntCC::SignedLessThan, cur_i, n_val);
                            builder.ins().brif(cond, loop_body, &[], loop_done, &[]);

                            builder.switch_to_block(loop_body);
                            let cur_v: Vec<_> = v_vars.iter().take(n).map(|&var| builder.use_var(var)).collect();

                            let mut next_v = Vec::with_capacity(n);
                            for r in 0..n {
                                let mut sum = arg_vals[1 + n * n + r];
                                for (c, &cur_val) in cur_v.iter().enumerate().take(n) {
                                    let a_rc = arg_vals[1 + r * n + c];
                                    let term = builder.ins().imul(a_rc, cur_val);
                                    sum = builder.ins().iadd(sum, term);
                                }
                                next_v.push(sum);
                            }

                            let next_i = builder.ins().iadd(cur_i, one);
                            for (i, &next_val) in next_v.iter().enumerate().take(n) {
                                builder.def_var(v_vars[i], next_val);
                            }
                            builder.def_var(i_var, next_i);
                            builder.ins().jump(loop_head, &[]);

                            builder.switch_to_block(loop_done);
                            let safe_target = if target_idx < n { target_idx } else { 0 };
                            builder.use_var(v_vars[safe_target])
                        } else if callee == "print" || callee == "println" {
                            let is_nl = callee == "println";
                            if args.is_empty() {
                                let print_nl_func = self.module.declare_func_in_func(self.print_newline_id, builder.func);
                                builder.ins().call(print_nl_func, &[]);
                            } else {
                                let arg_p = &args[0];
                                let arg_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, arg_p);
                                let print_i64_func = self.module.declare_func_in_func(self.print_i64_id, builder.func);
                                builder.ins().call(print_i64_func, &[arg_val]);
                                if is_nl {
                                    let print_nl_func = self.module.declare_func_in_func(self.print_newline_id, builder.func);
                                    builder.ins().call(print_nl_func, &[]);
                                }
                            }
                            builder.ins().iconst(types::I64, 0)
                        } else if let Some(&callee_id) = self.func_ids.get(callee) {
                            let local_func =
                                self.module.declare_func_in_func(callee_id, builder.func);
                            let sig = builder.func.dfg.ext_funcs[local_func].signature;
                            let expected_params = builder.func.dfg.signatures[sig].params.clone();
                            let is_sret = expected_params.len() > args.len();
                            let mut arg_vals: Vec<Value> = Vec::new();
                            let ret_sret_ptr = if is_sret {
                                let slot_data = StackSlotData::new(
                                    StackSlotKind::ExplicitSlot,
                                    64,
                                    8,
                                );
                                let slot = builder.create_sized_stack_slot(slot_data);
                                let sret_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                                arg_vals.push(sret_ptr);
                                Some(sret_ptr)
                            } else {
                                None
                            };

                            let param_offset = if is_sret { 1 } else { 0 };
                            for (i, p) in args.iter().enumerate() {
                                let v = get_place_value(&mut builder, &var_map, &array_slots, &aliases, p);
                                let v_ty = builder.func.dfg.value_type(v);
                                let exp_idx = i + param_offset;
                                let expected_ty = if exp_idx < expected_params.len() {
                                    expected_params[exp_idx].value_type
                                } else {
                                    v_ty
                                };
                                if v_ty == expected_ty {
                                    arg_vals.push(v);
                                } else if v_ty.is_int() && expected_ty.is_int() {
                                    if expected_ty.bits() > v_ty.bits() {
                                        arg_vals.push(builder.ins().uextend(expected_ty, v));
                                    } else {
                                        arg_vals.push(builder.ins().ireduce(expected_ty, v));
                                    }
                                } else {
                                    arg_vals.push(v);
                                }
                            }
                            let call_inst = builder.ins().call(local_func, &arg_vals);
                            let res = builder.inst_results(call_inst);
                            if let Some(sret) = ret_sret_ptr {
                                sret
                            } else if res.is_empty() {
                                builder.ins().iconst(types::I32, 0)
                            } else {
                                res[0]
                            }
                        } else {
                            builder.ins().iconst(types::I64, 0)
                        }
                    }
                    crate::mir::lower::Rvalue::Array(elem_places) => {
                        let target_name = crate::mir::supercompiler::fusion::resolve_alias(&place.local, &aliases);
                        if let Some(&(slot, _len, ref elem_ty)) = array_slots.get(target_name).or_else(|| array_slots.get(&place.local)) {
                            let elem_size = elem_ty.size_bytes().max(1);
                            let elem_clif = type_to_clif(elem_ty.clone());
                            for (i, ep) in elem_places.iter().enumerate() {
                                let el_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, ep);
                                builder.ins().stack_store(elem_clif, el_val, slot, (i * elem_size) as i32);
                            }
                        }
                        builder.ins().iconst(types::I64, 0)
                    }
                    crate::mir::lower::Rvalue::Discriminant(p) => {
                        let ptr_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, p);
                        builder.ins().load(types::I64, MemFlagsData::trusted(), ptr_val, 0)
                    }
                    crate::mir::lower::Rvalue::EnumVariant {
                        enum_name,
                        tag,
                        fields,
                        ..
                    } => {
                        let total_size = self
                            .enum_layouts
                            .get(enum_name)
                            .map(|l| l.total_size)
                            .unwrap_or((8 + fields.len() * 8) as u32);
                        let slot_data = StackSlotData::new(
                            StackSlotKind::ExplicitSlot,
                            total_size,
                            8,
                        );
                        let slot = builder.create_sized_stack_slot(slot_data);
                        let slot_addr = builder.ins().stack_addr(types::I64, slot, 0);
                        let tag_val = builder.ins().iconst(types::I64, *tag as i64);
                        builder.ins().store(MemFlagsData::trusted(), tag_val, slot_addr, 0);
                        for (i, f_place) in fields.iter().enumerate() {
                            let f_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, f_place);
                            let offset = (8 + i * 8) as i32;
                            builder.ins().store(MemFlagsData::trusted(), f_val, slot_addr, offset);
                        }
                        slot_addr
                    }
                    crate::mir::lower::Rvalue::Alloc(inner_place) => {
                        let inner_ty = local_types
                            .get(&inner_place.local)
                            .cloned()
                            .unwrap_or(Type::I64);
                        let size = match &inner_ty {
                            Type::Struct(sname) => self.struct_layouts.get(sname).map_or(8, |l| l.total_size),
                            Type::Enum(ename) => self.enum_layouts.get(ename).map_or(8, |l| l.total_size),
                            _ => inner_ty.size_bytes().max(8) as u32,
                        };
                        let size_val = builder.ins().iconst(types::I64, size as i64);
                        let malloc_func = self.module.declare_func_in_func(self.malloc_id, builder.func);
                        let call_inst = builder.ins().call(malloc_func, &[size_val]);
                        let slot_addr = builder.inst_results(call_inst)[0];
                        let inner_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, inner_place);
                        if let Type::Struct(sname) = &inner_ty {
                            if let Some(sub_layout) = self.struct_layouts.get(sname) {
                                emit_copy_bytes_raw(&mut builder, inner_val, slot_addr, sub_layout.total_size as usize);
                            }
                        } else if let Type::Enum(ename) = &inner_ty {
                            if let Some(sub_layout) = self.enum_layouts.get(ename) {
                                emit_copy_bytes_raw(&mut builder, inner_val, slot_addr, sub_layout.total_size as usize);
                            }
                        } else {
                            builder.ins().store(MemFlagsData::trusted(), inner_val, slot_addr, 0);
                        }
                        slot_addr
                    }
                    crate::mir::lower::Rvalue::Load(inner_place) => {
                        let ptr_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, inner_place);
                        let dest_ty = local_types.get(&place.local).cloned().unwrap_or(Type::I64);
                        if dest_ty.is_struct() || matches!(dest_ty, Type::Enum(_)) {
                            ptr_val
                        } else {
                            let clif_ty = type_to_clif(dest_ty);
                            builder.ins().load(clif_ty, MemFlagsData::trusted(), ptr_val, 0)
                        }
                    }
                    crate::mir::lower::Rvalue::FnPtr(name) => {
                        if let Some(func_id) = self.func_ids.get(name) {
                            let local_func = self.module.declare_func_in_func(*func_id, builder.func);
                            builder.ins().func_addr(types::I64, local_func)
                        } else {
                            builder.ins().iconst(types::I64, 0)
                        }
                    }
                    _ => builder.ins().iconst(types::I64, 0),
                };

                if let crate::mir::lower::Rvalue::Use(src_p) = rval {
                    if place.projections.is_empty() && src_p.projections.is_empty() {
                        if let Some(slot_info) = array_slots.get(&src_p.local).cloned() {
                            array_slots.insert(place.local.clone(), slot_info);
                        }
                    }
                }

                if !place.projections.is_empty() {
                    if let Some(crate::mir::Projection::Index(idx_place)) = place.projections.first() {
                        let target_name = crate::mir::supercompiler::fusion::resolve_alias(&place.local, &aliases);
                        if let Some(&(slot, _len, ref elem_ty)) = array_slots.get(target_name).or_else(|| array_slots.get(&place.local)) {
                            let elem_size = elem_ty.size_bytes().max(1);
                            let elem_clif = type_to_clif(elem_ty.clone());
                            let val_ty = builder.func.dfg.value_type(val);
                            let coerced_val = if elem_clif == val_ty {
                                val
                            } else if elem_clif.is_int() && val_ty.is_int() {
                                if elem_clif.bits() > val_ty.bits() {
                                    builder.ins().uextend(elem_clif, val)
                                } else {
                                    builder.ins().ireduce(elem_clif, val)
                                }
                            } else {
                                val
                            };
                            let idx_val = var_map.get(&idx_place.local)
                                .map(|&(v, _)| builder.use_var(v))
                                .unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
                            let idx_i64 = if builder.func.dfg.value_type(idx_val) != types::I64 {
                                builder.ins().uextend(types::I64, idx_val)
                            } else {
                                idx_val
                            };
                            let offset = if elem_size == 1 {
                                idx_i64
                            } else {
                                builder.ins().imul_imm_s(idx_i64, elem_size as i64)
                            };
                            let base_addr = builder.ins().stack_addr(types::I64, slot, 0);
                            let elem_addr = builder.ins().iadd(base_addr, offset);
                            builder.ins().store(MemFlagsData::trusted(), coerced_val, elem_addr, 0);
                        }
                    }
                } else if let Some(&(var, var_ty)) = var_map.get(&place.local) {
                    let val_ty = builder.func.dfg.value_type(val);
                    let coerced_val = if var_ty == val_ty {
                        val
                    } else if var_ty.is_int() && val_ty.is_int() {
                        if var_ty.bits() > val_ty.bits() {
                            builder.ins().uextend(var_ty, val)
                        } else {
                            builder.ins().ireduce(var_ty, val)
                        }
                    } else if var_ty.is_float() && val_ty.is_float() {
                        if var_ty == types::F64 && val_ty == types::F32 {
                            builder.ins().fpromote(types::F64, val)
                        } else if var_ty == types::F32 && val_ty == types::F64 {
                            builder.ins().fdemote(types::F32, val)
                        } else {
                            val
                        }
                    } else {
                        val
                    };
                    builder.def_var(var, coerced_val);
                }
            }

            match &b.terminator {
                crate::mir::Terminator::Return { value } => {
                    if is_sret {
                        let sret = current_sret_ptr.ok_or_else(|| CodegenError::BackendError("sret_ptr must exist in MIR return".to_string()))?;
                        if let Some(p) = value {
                            let val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, p);
                            if let Type::Struct(ref sname) = func.return_ty {
                                if let Some(layout) = self.struct_layouts.get(sname) {
                                    emit_copy_bytes_raw(&mut builder, val, sret, layout.total_size as usize);
                                }
                            } else if let Type::Enum(ref ename) = func.return_ty {
                                if let Some(layout) = self.enum_layouts.get(ename) {
                                    emit_copy_bytes_raw(&mut builder, val, sret, layout.total_size as usize);
                                }
                            }
                        }
                        builder.ins().return_(&[sret]);
                    } else if let Some(p) = value {
                        let val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, p);
                        let ret_ty = if func.return_ty != Type::Void {
                            type_to_clif(func.return_ty.clone())
                        } else {
                            types::I32
                        };
                        let val_ty = builder.func.dfg.value_type(val);
                        let coerced_val = if ret_ty == val_ty {
                            val
                        } else if ret_ty.is_int() && val_ty.is_int() {
                            if ret_ty.bits() > val_ty.bits() {
                                builder.ins().uextend(ret_ty, val)
                            } else {
                                builder.ins().ireduce(ret_ty, val)
                            }
                        } else if ret_ty.is_float() && val_ty.is_float() {
                            if ret_ty == types::F64 && val_ty == types::F32 {
                                builder.ins().fpromote(types::F64, val)
                            } else if ret_ty == types::F32 && val_ty == types::F64 {
                                builder.ins().fdemote(types::F32, val)
                            } else {
                                val
                            }
                        } else {
                            val
                        };
                        builder.ins().return_(&[coerced_val]);
                    } else if func.return_ty != Type::Void {
                        let ret_ty = type_to_clif(func.return_ty.clone());
                        let dummy = if ret_ty.is_float() {
                            if ret_ty == types::F32 {
                                builder.ins().f32const(0.0)
                            } else {
                                builder.ins().f64const(0.0)
                            }
                        } else {
                            builder.ins().iconst(ret_ty, 0)
                        };
                        builder.ins().return_(&[dummy]);
                    } else {
                        builder.ins().return_(&[]);
                    }
                }
                crate::mir::Terminator::Branch { target } => {
                    if let Some(&target_block) = block_map.get(target) {
                        builder.ins().jump(target_block, &[]);
                    }
                }
                crate::mir::Terminator::BranchIf {
                    condition,
                    then_target,
                    else_target,
                } => {
                    let cond_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, condition);
                    if let (Some(&then_b), Some(&else_b)) =
                        (block_map.get(then_target), block_map.get(else_target))
                    {
                        builder.ins().brif(cond_val, then_b, &[], else_b, &[]);
                    }
                }
                crate::mir::Terminator::Switch {
                    value,
                    targets,
                    default,
                } => {
                    let switch_val = get_place_value(&mut builder, &var_map, &array_slots, &aliases, value);
                    if let Some(&def_b) = block_map.get(default) {
                        for (case_val, target_bb) in targets {
                            if let Some(&target_block) = block_map.get(target_bb) {
                                let next_block = builder.create_block();
                                let c_val = builder.ins().iconst(types::I64, *case_val);
                                let is_match = builder.ins().icmp(IntCC::Equal, switch_val, c_val);
                                builder.ins().brif(is_match, target_block, &[], next_block, &[]);
                                builder.switch_to_block(next_block);
                            }
                        }
                        builder.ins().jump(def_b, &[]);
                    }
                }
                crate::mir::Terminator::Unreachable => {
                    builder.ins().trap(TrapCode::unwrap_user(1));
                }
                crate::mir::Terminator::IndirectCall { next, .. } => {
                    let next_block = *block_map.get(next).ok_or_else(|| CodegenError::BackendError(format!("Block bb{} not found", next.0)))?;
                    builder.ins().jump(next_block, &[]);
                }
                crate::mir::Terminator::Fork { left, right, join } => {
                    let left_block = *block_map.get(left).ok_or_else(|| CodegenError::BackendError(format!("Block bb{} not found", left.0)))?;
                    let right_block = *block_map.get(right).ok_or_else(|| CodegenError::BackendError(format!("Block bb{} not found", right.0)))?;
                    let join_block = *block_map.get(join).ok_or_else(|| CodegenError::BackendError(format!("Block bb{} not found", join.0)))?;
                    let zero = builder.ins().iconst(types::I32, 0);
                    let dummy_branch = builder.create_block();
                    builder.ins().brif(zero, dummy_branch, &[], left_block, &[]);
                    builder.switch_to_block(dummy_branch);
                    builder.ins().brif(zero, right_block, &[], join_block, &[]);
                }
            }
        }

        builder.seal_all_blocks();
        let config = self.module.target_config();
        builder.finalize(config);
        self.module
            .define_function(func_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in {}: {:#?}", func.name, e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub fn compile_mir_program(
        mut self,
        mir: &crate::mir::lower::MirProgram,
    ) -> Result<Vec<u8>, CodegenError> {
        self.struct_layouts = compute_struct_layouts(&mir.structs);
        self.enum_layouts = compute_enum_layouts(&mir.enums, &self.struct_layouts);

        // Step 1: Declare all functions
        for func in &mir.functions {
            let mut sig = self.module.make_signature();
            let is_sret = matches!(&func.return_ty, Type::Struct(_) | Type::Enum(_));
            if is_sret {
                sig.params.push(AbiParam::new(types::I64));
                sig.returns.push(AbiParam::new(types::I64));
            } else if func.return_ty != Type::Void {
                sig.returns.push(AbiParam::new(type_to_clif(func.return_ty.clone())));
            }
            for (_, p_ty) in &func.params {
                if let Type::Struct(_) | Type::Enum(_) = p_ty {
                    sig.params.push(AbiParam::new(types::I64));
                } else {
                    sig.params.push(AbiParam::new(type_to_clif(p_ty.clone())));
                }
            }
            let export_name = if func.name == "main" && std::env::var("NUMLANG_BENCH").is_ok() && !cfg!(target_os = "windows") {
                "numlang_main"
            } else {
                &func.name
            };
            let func_id = self
                .module
                .declare_function(export_name, Linkage::Export, &sig)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?;
            self.func_ids.insert(func.name.clone(), func_id);
        }

        // Step 2: Define each function body
        let mut fn_builder_ctx = FunctionBuilderContext::new();
        let mut ctx = self.module.make_context();

        for func in &mir.functions {
            self.compile_mir_function(func, &mut ctx, &mut fn_builder_ctx)?;
        }

        // Step 3: Emit entry wrapper if main exists and benchmarking mode is disabled
        if std::env::var("NUMLANG_BENCH").is_err() {
            if let Some(&main_id) = self.func_ids.get("main") {
                self.compile_entry_point(main_id, &mut ctx, &mut fn_builder_ctx)?;
            }
        }

        // Step 3b: Emit print, alloc, and arena helpers
        self.emit_print_helpers(&mut ctx, &mut fn_builder_ctx)?;
        self.emit_helper_malloc(&mut ctx, &mut fn_builder_ctx)?;
        self.emit_helper_loop_reset(&mut ctx, &mut fn_builder_ctx)?;
        self.emit_helper_arena_wrappers(&mut ctx, &mut fn_builder_ctx)?;

        // Step 4: Emit final object file
        let product = self.module.finish();
        let obj_bytes = product
            .emit()
            .map_err(|e| CodegenError::BackendError(format!("Failed to emit object: {}", e)))?;

        Ok(obj_bytes)
    }
}

fn get_place_value(
    builder: &mut FunctionBuilder,
    var_map: &HashMap<String, (Variable, types::Type)>,
    array_slots: &HashMap<String, (cranelift_codegen::ir::StackSlot, usize, Type)>,
    aliases: &HashMap<String, String>,
    p: &crate::mir::Place,
) -> Value {
    let mut current_val = var_map
        .get(&p.local)
        .map(|&(v, _)| builder.use_var(v))
        .unwrap_or_else(|| builder.ins().iconst(types::I64, 0));

    for proj in &p.projections {
        match proj {
            crate::mir::Projection::Payload(idx) => {
                let offset = (8 + idx * 8) as i32;
                current_val = builder.ins().load(types::I64, MemFlagsData::trusted(), current_val, offset);
            }
            crate::mir::Projection::Field(_) => {
                current_val = builder.ins().load(types::I64, MemFlagsData::trusted(), current_val, 0);
            }
            crate::mir::Projection::Index(idx_place) => {
                let p_name = crate::mir::supercompiler::fusion::resolve_alias(&p.local, aliases);
                if let Some(&(slot, _len, ref elem_ty)) = array_slots.get(p_name).or_else(|| array_slots.get(&p.local)) {
                    let elem_size = elem_ty.size_bytes().max(1);
                    let elem_clif = type_to_clif(elem_ty.clone());
                    let idx_val = var_map.get(&idx_place.local)
                        .map(|&(v, _)| builder.use_var(v))
                        .unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
                    let idx_i64 = if builder.func.dfg.value_type(idx_val) != types::I64 {
                        builder.ins().uextend(types::I64, idx_val)
                    } else {
                        idx_val
                    };
                    let offset = if elem_size == 1 {
                        idx_i64
                    } else {
                        builder.ins().imul_imm_s(idx_i64, elem_size as i64)
                    };
                    let base_addr = builder.ins().stack_addr(types::I64, slot, 0);
                    let elem_addr = builder.ins().iadd(base_addr, offset);
                    current_val = builder.ins().load(elem_clif, MemFlagsData::trusted(), elem_addr, 0);
                }
            }
            crate::mir::Projection::Deref => {
                current_val = builder.ins().load(types::I64, MemFlagsData::trusted(), current_val, 0);
            }
        }
    }
    current_val
}




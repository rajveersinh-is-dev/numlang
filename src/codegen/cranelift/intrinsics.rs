//! Built-in runtime intrinsics, mathematical routines, and syscall wrappers.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};

use cranelift_codegen::ir::{
    types, AbiParam, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};
use crate::typecheck::Type;
use super::abi::*;
use super::ast_stmt::{emit_copy_bytes_raw, FunctionTranslationState};
use super::CraneliftCompiler;

impl CraneliftCompiler {
    pub(crate) fn emit_helper_malloc(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        #[cfg(target_os = "windows")]
        let os_alloc_id = {
            let mut sig_local_alloc = self.module.make_signature();
            sig_local_alloc.params.push(AbiParam::new(types::I32));
            sig_local_alloc.params.push(AbiParam::new(types::I64));
            sig_local_alloc.returns.push(AbiParam::new(types::I64));
            self.module
                .declare_function("LocalAlloc", Linkage::Import, &sig_local_alloc)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?
        };

        #[cfg(not(target_os = "windows"))]
        let os_alloc_id = {
            let mut sig_malloc = self.module.make_signature();
            sig_malloc.params.push(AbiParam::new(types::I64));
            sig_malloc.returns.push(AbiParam::new(types::I64));
            self.module
                .declare_function("malloc", Linkage::Import, &sig_malloc)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?
        };

        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        ctx.func.signature = sig;

        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        let fast_path = builder.create_block();
        let slow_path = builder.create_block();

        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);

        let raw_size = builder.block_params(entry)[0];
        // Align raw_size to 8 bytes: (raw_size + 7) & ~7
        let c7 = builder.ins().iconst(types::I64, 7);
        let cm8 = builder.ins().iconst(types::I64, -8);
        let raw_plus_7 = builder.ins().iadd(raw_size, c7);
        let aligned_size = builder.ins().band(raw_plus_7, cm8);

        let gv_cur = self.module.declare_data_in_func(self.arena_cur_id, builder.func);
        let addr_cur = builder.ins().symbol_value(types::I64, gv_cur);
        let cur_val = builder.ins().load(types::I64, MemFlagsData::trusted(), addr_cur, 0);

        let gv_end = self.module.declare_data_in_func(self.arena_end_id, builder.func);
        let addr_end = builder.ins().symbol_value(types::I64, gv_end);
        let end_val = builder.ins().load(types::I64, MemFlagsData::trusted(), addr_end, 0);

        let next_cur = builder.ins().iadd(cur_val, aligned_size);

        // Check: cur_val != 0 && next_cur <= end_val
        let zero64 = builder.ins().iconst(types::I64, 0);
        let not_null = builder.ins().icmp(IntCC::NotEqual, cur_val, zero64);
        let fits = builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, next_cur, end_val);
        let can_bump = builder.ins().band(not_null, fits);

        builder.ins().brif(can_bump, fast_path, &[], slow_path, &[]);
        builder.seal_block(entry);

        // Fast path: store new cur, return cur_val
        builder.switch_to_block(fast_path);
        builder.seal_block(fast_path);
        builder.ins().store(MemFlagsData::trusted(), next_cur, addr_cur, 0);
        builder.ins().return_(&[cur_val]);

        // Slow path: allocate 2MB chunk (or 2 * aligned_size if larger)
        builder.switch_to_block(slow_path);
        builder.seal_block(slow_path);

        let min_chunk_size = builder.ins().iconst(types::I64, 2 * 1024 * 1024); // 2 MB
        let doubled_size = builder.ins().imul_imm_s(aligned_size, 2);
        let is_huge = builder.ins().icmp(IntCC::UnsignedGreaterThan, doubled_size, min_chunk_size);
        let alloc_size = builder.ins().select(is_huge, doubled_size, min_chunk_size);

        let new_chunk_ptr = {
            #[cfg(target_os = "windows")]
            {
                let flags_val = builder.ins().iconst(types::I32, 0x0040); // LPTR = LMEM_FIXED | LMEM_ZEROINIT
                let local_alloc = self.module.declare_func_in_func(os_alloc_id, builder.func);
                let call = builder.ins().call(local_alloc, &[flags_val, alloc_size]);
                builder.inst_results(call)[0]
            }
            #[cfg(not(target_os = "windows"))]
            {
                let malloc_func = self.module.declare_func_in_func(os_alloc_id, builder.func);
                let call = builder.ins().call(malloc_func, &[alloc_size]);
                builder.inst_results(call)[0]
            }
        };

        let gv_start_s = self.module.declare_data_in_func(self.arena_start_id, builder.func);
        let addr_start_s = builder.ins().symbol_value(types::I64, gv_start_s);
        let gv_cur_s = self.module.declare_data_in_func(self.arena_cur_id, builder.func);
        let addr_cur_s = builder.ins().symbol_value(types::I64, gv_cur_s);
        let gv_end_s = self.module.declare_data_in_func(self.arena_end_id, builder.func);
        let addr_end_s = builder.ins().symbol_value(types::I64, gv_end_s);

        let new_end = builder.ins().iadd(new_chunk_ptr, alloc_size);
        let new_cur = builder.ins().iadd(new_chunk_ptr, aligned_size);

        builder.ins().store(MemFlagsData::trusted(), new_chunk_ptr, addr_start_s, 0);
        builder.ins().store(MemFlagsData::trusted(), new_end, addr_end_s, 0);
        builder.ins().store(MemFlagsData::trusted(), new_cur, addr_cur_s, 0);

        builder.ins().return_(&[new_chunk_ptr]);

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.malloc_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_malloc: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub(crate) fn emit_helper_loop_reset(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let sig = self.module.make_signature();
        ctx.func.signature = sig;

        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let gv_cur = self.module.declare_data_in_func(self.arena_cur_id, builder.func);
        let addr_cur = builder.ins().symbol_value(types::I64, gv_cur);

        let gv_start = self.module.declare_data_in_func(self.arena_start_id, builder.func);
        let addr_start = builder.ins().symbol_value(types::I64, gv_start);

        let start_val = builder.ins().load(types::I64, MemFlagsData::trusted(), addr_start, 0);
        builder.ins().store(MemFlagsData::trusted(), start_val, addr_cur, 0);

        builder.ins().return_(&[]);

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.loop_reset_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_loop_reset: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub(crate) fn emit_helper_arena_wrappers(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        // __nl_arena_alloc(size) -> call __nl_malloc(size)
        let mut sig_malloc = self.module.make_signature();
        sig_malloc.params.push(AbiParam::new(types::I64));
        sig_malloc.returns.push(AbiParam::new(types::I64));
        ctx.func.signature = sig_malloc;

        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let sz = builder.block_params(entry)[0];
        let malloc_fn = self.module.declare_func_in_func(self.malloc_id, builder.func);
        let call = builder.ins().call(malloc_fn, &[sz]);
        let res = builder.inst_results(call)[0];
        builder.ins().return_(&[res]);

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.arena_alloc_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_arena_alloc: {:#?}", e)))?;
        self.module.clear_context(ctx);

        // __nl_arena_reset() -> call __nl_loop_reset()
        let sig_void = self.module.make_signature();
        ctx.func.signature = sig_void;

        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let reset_fn = self.module.declare_func_in_func(self.loop_reset_id, builder.func);
        builder.ins().call(reset_fn, &[]);
        builder.ins().return_(&[]);

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.arena_reset_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_arena_reset: {:#?}", e)))?;
        self.module.clear_context(ctx);

        Ok(())
    }

    pub(crate) fn emit_print_helpers(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        self.emit_helper_print_str(ctx, fn_builder_ctx)?;
        self.emit_helper_print_newline(ctx, fn_builder_ctx)?;
        self.emit_helper_print_u64(ctx, fn_builder_ctx)?;
        self.emit_helper_print_i64(ctx, fn_builder_ctx)?;
        self.emit_helper_print_bool(ctx, fn_builder_ctx)?;
        self.emit_helper_print_f64(ctx, fn_builder_ctx)?;
        Ok(())
    }

    pub(crate) fn emit_helper_print_str(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I32));
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        let write_block = builder.create_block();
        let ret_block = builder.create_block();

        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let ptr = builder.block_params(entry_block)[0];
        let len = builder.block_params(entry_block)[1];

        let zero32 = builder.ins().iconst(types::I32, 0);
        let is_positive = builder.ins().icmp(IntCC::SignedGreaterThan, len, zero32);
        builder.ins().brif(is_positive, write_block, &[], ret_block, &[]);

        builder.switch_to_block(write_block);
        builder.seal_block(write_block);

        #[cfg(target_os = "windows")]
        {
            let written_slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
            let written_addr = builder.ins().stack_addr(types::I64, written_slot, 0);

            let std_out_handle = builder.ins().iconst(types::I32, -11); // STD_OUTPUT_HANDLE
            if let Some(gsh_id) = self.get_std_handle_id {
                let get_std_handle_func = self.module.declare_func_in_func(gsh_id, builder.func);
                let h_call = builder.ins().call(get_std_handle_func, &[std_out_handle]);
                let h_stdout = builder.inst_results(h_call)[0];

                let zero64 = builder.ins().iconst(types::I64, 0);
                let write_file_func = self.module.declare_func_in_func(self.write_file_id, builder.func);
                builder.ins().call(write_file_func, &[h_stdout, ptr, len, written_addr, zero64]);
            }
            builder.ins().jump(ret_block, &[]);
        }

        #[cfg(not(target_os = "windows"))]
        {
            let fd_stdout = builder.ins().iconst(types::I32, 1);
            let len64 = builder.ins().uextend(types::I64, len);
            let write_func = self.module.declare_func_in_func(self.write_file_id, builder.func);
            builder.ins().call(write_func, &[fd_stdout, ptr, len64]);
            builder.ins().jump(ret_block, &[]);
        }

        builder.switch_to_block(ret_block);
        builder.seal_block(ret_block);
        builder.ins().return_(&[]);

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.print_str_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_print_str: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub(crate) fn emit_helper_print_newline(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        ctx.func.signature = self.module.make_signature();
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let nl = builder.ins().iconst(types::I8, 10);
        let addr = builder.ins().stack_addr(types::I64, slot, 0);
        builder.ins().store(MemFlagsData::trusted(), nl, addr, 0);
        let len = builder.ins().iconst(types::I32, 1);

        let print_str_func = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func, &[addr, len]);

        builder.ins().return_(&[]);
        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.print_newline_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_print_newline: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub(crate) fn emit_helper_print_bool(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(types::I8));
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        let true_block = builder.create_block();
        let false_block = builder.create_block();
        let merge_block = builder.create_block();

        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let val = builder.block_params(entry_block)[0];
        let zero8 = builder.ins().iconst(types::I8, 0);
        let is_true = builder.ins().icmp(IntCC::NotEqual, val, zero8);
        builder.ins().brif(is_true, true_block, &[], false_block, &[]);

        builder.switch_to_block(true_block);
        builder.seal_block(true_block);
        let slot_t = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let val_t = builder.ins().iconst(types::I32, 0x65757274); // "true"
        let addr_t = builder.ins().stack_addr(types::I64, slot_t, 0);
        builder.ins().store(MemFlagsData::trusted(), val_t, addr_t, 0);
        let len_t = builder.ins().iconst(types::I32, 4);
        let print_str_func_t = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_t, &[addr_t, len_t]);
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(false_block);
        builder.seal_block(false_block);
        let slot_f = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let val_f = builder.ins().iconst(types::I64, 0x65736c6166); // "false"
        let addr_f = builder.ins().stack_addr(types::I64, slot_f, 0);
        builder.ins().store(MemFlagsData::trusted(), val_f, addr_f, 0);
        let len_f = builder.ins().iconst(types::I32, 5);
        let print_str_func_f = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_f, &[addr_f, len_f]);
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        builder.ins().return_(&[]);
        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.print_bool_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_print_bool: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub(crate) fn emit_helper_print_u64(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        let zero_block = builder.create_block();
        let non_zero_block = builder.create_block();
        let loop_header = builder.create_block();
        let loop_body = builder.create_block();
        let done_block = builder.create_block();
        let ret_block = builder.create_block();

        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let val = builder.block_params(entry_block)[0];
        let slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 32, 8));
        let slot_base = builder.ins().stack_addr(types::I64, slot, 0);

        let zero64 = builder.ins().iconst(types::I64, 0);
        let is_zero = builder.ins().icmp(IntCC::Equal, val, zero64);
        builder.ins().brif(is_zero, zero_block, &[], non_zero_block, &[]);

        // zero block
        builder.switch_to_block(zero_block);
        builder.seal_block(zero_block);
        let char_zero = builder.ins().iconst(types::I8, 48); // '0'
        let addr_31 = builder.ins().iadd_imm_s(slot_base, 31);
        builder.ins().store(MemFlagsData::trusted(), char_zero, addr_31, 0);
        let one32 = builder.ins().iconst(types::I32, 1);
        let print_str_func_z = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_z, &[addr_31, one32]);
        builder.ins().jump(ret_block, &[]);

        // non-zero block
        builder.switch_to_block(non_zero_block);
        builder.seal_block(non_zero_block);
        let u_var = builder.declare_var(types::I64);
        let idx_var = builder.declare_var(types::I32);
        builder.def_var(u_var, val);
        let init_idx = builder.ins().iconst(types::I32, 32);
        builder.def_var(idx_var, init_idx);
        builder.ins().jump(loop_header, &[]);

        // loop header
        builder.switch_to_block(loop_header);
        let cur_u = builder.use_var(u_var);
        let u_is_zero = builder.ins().icmp(IntCC::Equal, cur_u, zero64);
        builder.ins().brif(u_is_zero, done_block, &[], loop_body, &[]);

        // loop body
        builder.switch_to_block(loop_body);
        builder.seal_block(loop_body);
        let ten = builder.ins().iconst(types::I64, 10);
        let rem = builder.ins().urem(cur_u, ten);
        let rem8 = builder.ins().ireduce(types::I8, rem);
        let char_val = builder.ins().iadd_imm_s(rem8, 48);

        let cur_idx = builder.use_var(idx_var);
        let next_idx = builder.ins().iadd_imm_s(cur_idx, -1);
        builder.def_var(idx_var, next_idx);
        let next_idx64 = builder.ins().uextend(types::I64, next_idx);
        let char_addr = builder.ins().iadd(slot_base, next_idx64);
        builder.ins().store(MemFlagsData::trusted(), char_val, char_addr, 0);

        let next_u = builder.ins().udiv(cur_u, ten);
        builder.def_var(u_var, next_u);
        builder.ins().jump(loop_header, &[]);
        builder.seal_block(loop_header);

        // done block
        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        let final_idx = builder.use_var(idx_var);
        let final_idx64 = builder.ins().uextend(types::I64, final_idx);
        let start_addr = builder.ins().iadd(slot_base, final_idx64);
        let thirty_two = builder.ins().iconst(types::I32, 32);
        let len = builder.ins().isub(thirty_two, final_idx);
        let print_str_func_nz = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_nz, &[start_addr, len]);
        builder.ins().jump(ret_block, &[]);

        // ret block
        builder.switch_to_block(ret_block);
        builder.seal_block(ret_block);
        builder.ins().return_(&[]);
        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.print_u64_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_print_u64: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub(crate) fn emit_helper_print_i64(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        let neg_block = builder.create_block();
        let merge_block = builder.create_block();

        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let val = builder.block_params(entry_block)[0];
        let zero64 = builder.ins().iconst(types::I64, 0);
        let is_neg = builder.ins().icmp(IntCC::SignedLessThan, val, zero64);
        let neg_val = builder.ins().ineg(val);
        let u_val = builder.ins().select(is_neg, neg_val, val);
        builder.ins().brif(is_neg, neg_block, &[], merge_block, &[]);

        // neg block: print '-'
        builder.switch_to_block(neg_block);
        builder.seal_block(neg_block);
        let slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let minus = builder.ins().iconst(types::I8, 45); // '-'
        let addr = builder.ins().stack_addr(types::I64, slot, 0);
        builder.ins().store(MemFlagsData::trusted(), minus, addr, 0);
        let len1 = builder.ins().iconst(types::I32, 1);
        let print_str_func = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func, &[addr, len1]);
        builder.ins().jump(merge_block, &[]);

        // merge block: call __nl_print_u64 with positive / negated val
        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        let print_u64_func = self.module.declare_func_in_func(self.print_u64_id, builder.func);
        builder.ins().call(print_u64_func, &[u_val]);
        builder.ins().return_(&[]);

        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.print_i64_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_print_i64: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    pub(crate) fn emit_helper_print_f64(
        &mut self,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(types::F64));
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        let nan_block = builder.create_block();
        let not_nan_block = builder.create_block();
        let neg_block = builder.create_block();
        let check_inf_block = builder.create_block();
        let inf_block = builder.create_block();
        let norm_block = builder.create_block();
        let loop_trim = builder.create_block();
        let loop_trim_body = builder.create_block();
        let print_frac_block = builder.create_block();
        let ret_block = builder.create_block();

        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let val = builder.block_params(entry_block)[0];
        let len1 = builder.ins().iconst(types::I32, 1);

        // 1. Check NaN
        let is_nan = builder.ins().fcmp(FloatCC::NotEqual, val, val);
        builder.ins().brif(is_nan, nan_block, &[], not_nan_block, &[]);

        // nan block
        builder.switch_to_block(nan_block);
        builder.seal_block(nan_block);
        let slot_nan = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let val_nan = builder.ins().iconst(types::I32, 0x4e614e); // "NaN"
        let addr_nan = builder.ins().stack_addr(types::I64, slot_nan, 0);
        builder.ins().store(MemFlagsData::trusted(), val_nan, addr_nan, 0);
        let len_nan = builder.ins().iconst(types::I32, 3);
        let print_str_func = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func, &[addr_nan, len_nan]);
        builder.ins().jump(ret_block, &[]);

        // not_nan block: check sign
        builder.switch_to_block(not_nan_block);
        builder.seal_block(not_nan_block);
        let zero_f = builder.ins().f64const(0.0);
        let is_neg = builder.ins().fcmp(FloatCC::LessThan, val, zero_f);
        let fabs_val = builder.ins().fabs(val);
        builder.ins().brif(is_neg, neg_block, &[], check_inf_block, &[]);

        // neg block
        builder.switch_to_block(neg_block);
        builder.seal_block(neg_block);
        let slot_m = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let dash = builder.ins().iconst(types::I8, 45); // '-'
        let addr_m = builder.ins().stack_addr(types::I64, slot_m, 0);
        builder.ins().store(MemFlagsData::trusted(), dash, addr_m, 0);
        let print_str_func_m = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_m, &[addr_m, len1]);
        builder.ins().jump(check_inf_block, &[]);

        // check_inf_block
        builder.switch_to_block(check_inf_block);
        builder.seal_block(check_inf_block);
        let inf = builder.ins().f64const(f64::INFINITY);
        let is_inf = builder.ins().fcmp(FloatCC::Equal, fabs_val, inf);
        builder.ins().brif(is_inf, inf_block, &[], norm_block, &[]);

        // inf block
        builder.switch_to_block(inf_block);
        builder.seal_block(inf_block);
        let slot_inf = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let val_inf = builder.ins().iconst(types::I32, 0x666e69); // "inf"
        let addr_inf = builder.ins().stack_addr(types::I64, slot_inf, 0);
        builder.ins().store(MemFlagsData::trusted(), val_inf, addr_inf, 0);
        let len_inf = builder.ins().iconst(types::I32, 3);
        let print_str_func_inf = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_inf, &[addr_inf, len_inf]);
        builder.ins().jump(ret_block, &[]);

        // norm_block
        builder.switch_to_block(norm_block);
        builder.seal_block(norm_block);
        let int_part = builder.ins().fcvt_to_sint(types::I64, fabs_val);
        let print_u64_func = self.module.declare_func_in_func(self.print_u64_id, builder.func);
        builder.ins().call(print_u64_func, &[int_part]);

        // print '.'
        let slot_dot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let dot = builder.ins().iconst(types::I8, 46); // '.'
        let addr_dot = builder.ins().stack_addr(types::I64, slot_dot, 0);
        builder.ins().store(MemFlagsData::trusted(), dot, addr_dot, 0);
        let print_str_func_dot = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_dot, &[addr_dot, len1]);

        let int_part_f = builder.ins().fcvt_from_sint(types::F64, int_part);
        let frac = builder.ins().fsub(fabs_val, int_part_f);
        let scale = builder.ins().f64const(1000000.0);
        let scaled = builder.ins().fmul(frac, scale);
        let half = builder.ins().f64const(0.5);
        let scaled_rnd = builder.ins().fadd(scaled, half);
        let scaled_raw = builder.ins().fcvt_to_sint(types::I64, scaled_rnd);
        let zero64 = builder.ins().iconst(types::I64, 0);
        let max_frac = builder.ins().iconst(types::I64, 999999);
        let c_lo = builder.ins().icmp(IntCC::SignedLessThan, scaled_raw, zero64);
        let s1 = builder.ins().select(c_lo, zero64, scaled_raw);
        let c_hi = builder.ins().icmp(IntCC::SignedGreaterThan, s1, max_frac);
        let scaled_int = builder.ins().select(c_hi, max_frac, s1);

        let frac_slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
        let frac_base = builder.ins().stack_addr(types::I64, frac_slot, 0);
        let divs = [100000i64, 10000, 1000, 100, 10, 1];
        let ten = builder.ins().iconst(types::I64, 10);
        for (i, d) in divs.iter().enumerate() {
            let div_val = builder.ins().iconst(types::I64, *d);
            let q = builder.ins().sdiv(scaled_int, div_val);
            let rem = builder.ins().srem(q, ten);
            let rem8 = builder.ins().ireduce(types::I8, rem);
            let d_char = builder.ins().iadd_imm_s(rem8, 48);
            let char_addr = builder.ins().iadd_imm_s(frac_base, i as i64);
            builder.ins().store(MemFlagsData::trusted(), d_char, char_addr, 0);
        }

        let len_var = builder.declare_var(types::I32);
        let init_len = builder.ins().iconst(types::I32, 6);
        builder.def_var(len_var, init_len);
        builder.ins().jump(loop_trim, &[]);

        // loop_trim: while len > 1 { if buf[len - 1] == '0' { len -= 1 } else { break } }
        builder.switch_to_block(loop_trim);
        let cur_len = builder.use_var(len_var);
        let one32 = builder.ins().iconst(types::I32, 1);
        let can_trim = builder.ins().icmp(IntCC::SignedGreaterThan, cur_len, one32);
        builder.ins().brif(can_trim, loop_trim_body, &[], print_frac_block, &[]);

        // loop_trim_body
        builder.switch_to_block(loop_trim_body);
        builder.seal_block(loop_trim_body);
        let last_idx = builder.ins().iadd_imm_s(cur_len, -1);
        let last_idx64 = builder.ins().uextend(types::I64, last_idx);
        let last_addr = builder.ins().iadd(frac_base, last_idx64);
        let last_char = builder.ins().load(types::I8, MemFlagsData::trusted(), last_addr, 0);
        let zero_char = builder.ins().iconst(types::I8, 48);
        let is_zero_char = builder.ins().icmp(IntCC::Equal, last_char, zero_char);
        let decr_len = builder.ins().iadd_imm_s(cur_len, -1);
        let next_len = builder.ins().select(is_zero_char, decr_len, cur_len);
        builder.def_var(len_var, next_len);
        builder.ins().brif(is_zero_char, loop_trim, &[], print_frac_block, &[]);
        builder.seal_block(loop_trim);

        // print_frac_block
        builder.switch_to_block(print_frac_block);
        builder.seal_block(print_frac_block);
        let final_len = builder.use_var(len_var);
        let print_str_func_f = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func_f, &[frac_base, final_len]);
        builder.ins().jump(ret_block, &[]);

        // ret_block
        builder.switch_to_block(ret_block);
        builder.seal_block(ret_block);
        builder.ins().return_(&[]);
        let config = self.module.target_config();
        builder.finalize(config);

        self.module
            .define_function(self.print_f64_id, ctx)
            .map_err(|e| CodegenError::BackendError(format!("Verifier error in __nl_print_f64: {:#?}", e)))?;
        self.module.clear_context(ctx);
        Ok(())
    }


}

impl<'a> FunctionTranslationState<'a> {
    pub(crate) fn emit_copy_bytes(builder: &mut FunctionBuilder, src_ptr: Value, dst_ptr: Value, total_bytes: usize) {
        emit_copy_bytes_raw(builder, src_ptr, dst_ptr, total_bytes);
    }

    pub(crate) fn emit_zero_bytes(builder: &mut FunctionBuilder, dst_ptr: Value, total_bytes: usize) {
        let mut offset = 0;
        let zero64 = builder.ins().iconst(types::I64, 0);
        while offset + 8 <= total_bytes {
            builder.ins().store(MemFlagsData::trusted(), zero64, dst_ptr, offset as i32);
            offset += 8;
        }
        if offset + 4 <= total_bytes {
            let zero32 = builder.ins().iconst(types::I32, 0);
            builder.ins().store(MemFlagsData::trusted(), zero32, dst_ptr, offset as i32);
            offset += 4;
        }
        if offset + 2 <= total_bytes {
            let zero16 = builder.ins().iconst(types::I16, 0);
            builder.ins().store(MemFlagsData::trusted(), zero16, dst_ptr, offset as i32);
            offset += 2;
        }
        if offset < total_bytes {
            let zero8 = builder.ins().iconst(types::I8, 0);
            builder.ins().store(MemFlagsData::trusted(), zero8, dst_ptr, offset as i32);
        }
    }

    pub(crate) fn emit_bytes_write(&mut self, bytes: &[u8], builder: &mut FunctionBuilder) -> Result<(), CodegenError> {
        if bytes.is_empty() {
            return Ok(());
        }
        let slot_size = bytes.len().div_ceil(8) * 8;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            slot_size as u32,
            8,
        ));
        for (i, chunk) in bytes.chunks(8).enumerate() {
            let mut val_bytes = [0u8; 8];
            val_bytes[..chunk.len()].copy_from_slice(chunk);
            let val_u64 = u64::from_le_bytes(val_bytes);
            let val = builder.ins().iconst(types::I64, val_u64 as i64);
            let addr = builder.ins().stack_addr(types::I64, slot, (i * 8) as i32);
            builder.ins().store(MemFlagsData::trusted(), val, addr, 0);
        }
        let msg_addr = builder.ins().stack_addr(types::I64, slot, 0);
        let msg_len = builder.ins().iconst(types::I32, bytes.len() as i64);
        let print_str_func = self.module.declare_func_in_func(self.print_str_id, builder.func);
        builder.ins().call(print_str_func, &[msg_addr, msg_len]);
        Ok(())
    }

    pub(crate) fn get_iconst(&mut self, ty: types::Type, n: i64, builder: &mut FunctionBuilder) -> Value {
        let key = (ty, n as u64);
        if let Some(&val) = self.const_pool.get(&key) {
            val
        } else {
            builder.ins().iconst(ty, n)
        }
    }

    pub(crate) fn get_f64const(&mut self, f: f64, builder: &mut FunctionBuilder) -> Value {
        let key = f.to_bits();
        if let Some(&val) = self.f64_pool.get(&key) {
            val
        } else {
            builder.ins().f64const(f)
        }
    }

    pub(crate) fn get_f32const(&mut self, f: f32, builder: &mut FunctionBuilder) -> Value {
        let key = f.to_bits();
        if let Some(&val) = self.f32_pool.get(&key) {
            val
        } else {
            builder.ins().f32const(f)
        }
    }

    pub(crate) fn emit_int_pow(&mut self, base: Value, exp: Value, ty: &Type, builder: &mut FunctionBuilder) -> Value {
        let clif_ty = type_to_clif(ty.clone());
        Self::emit_int_pow_raw(base, exp, clif_ty, builder)
    }

pub(crate) fn emit_int_pow_raw(base: Value, exp: Value, clif_ty: types::Type, builder: &mut FunctionBuilder) -> Value {
    let var_res = builder.declare_var(clif_ty);
    let var_b = builder.declare_var(clif_ty);
    let var_e = builder.declare_var(clif_ty);

    let one = builder.ins().iconst(clif_ty, 1);
    let zero = builder.ins().iconst(clif_ty, 0);
    builder.def_var(var_res, one);
    builder.def_var(var_b, base);
    builder.def_var(var_e, exp);

    let loop_header = builder.create_block();
    let loop_body = builder.create_block();
    let loop_exit = builder.create_block();

    builder.ins().jump(loop_header, &[]);
    builder.switch_to_block(loop_header);
    let cur_e = builder.use_var(var_e);
    let cond = builder.ins().icmp(IntCC::SignedGreaterThan, cur_e, zero);
    builder.ins().brif(cond, loop_body, &[], loop_exit, &[]);

    builder.switch_to_block(loop_body);
    builder.seal_block(loop_body);
    let is_odd = builder.ins().band_imm_s(cur_e, 1);
    let is_odd_cond = builder.ins().icmp(IntCC::NotEqual, is_odd, zero);
    let cur_res = builder.use_var(var_res);
    let cur_b = builder.use_var(var_b);
    let mult = builder.ins().imul(cur_res, cur_b);
    let next_res = builder.ins().select(is_odd_cond, mult, cur_res);
    builder.def_var(var_res, next_res);

    let next_b = builder.ins().imul(cur_b, cur_b);
    builder.def_var(var_b, next_b);
    let next_e = builder.ins().sshr_imm_s(cur_e, 1);
    builder.def_var(var_e, next_e);
    builder.ins().jump(loop_header, &[]);

    builder.seal_block(loop_header);
    builder.switch_to_block(loop_exit);
    builder.seal_block(loop_exit);

    builder.use_var(var_res)
}

    pub(crate) fn emit_fast_int_mul(
        &mut self,
        l: Value,
        r: Value,
        c: Option<i64>,
        clif_ty: types::Type,
        builder: &mut FunctionBuilder,
    ) -> Value {
        if let Some(k) = c {
            match k {
                0 => self.get_iconst(clif_ty, 0, builder),
                1 => l,
                -1 => builder.ins().ineg(l),
                2 => builder.ins().iadd(l, l),
                3 => {
                    let two_l = builder.ins().ishl_imm_s(l, 1);
                    builder.ins().iadd(two_l, l)
                }
                4 => builder.ins().ishl_imm_s(l, 2),
                5 => {
                    let four_l = builder.ins().ishl_imm_s(l, 2);
                    builder.ins().iadd(four_l, l)
                }
                6 => {
                    let three_l = {
                        let two_l = builder.ins().ishl_imm_s(l, 1);
                        builder.ins().iadd(two_l, l)
                    };
                    builder.ins().ishl_imm_s(three_l, 1)
                }
                7 => {
                    let eight_l = builder.ins().ishl_imm_s(l, 3);
                    builder.ins().isub(eight_l, l)
                }
                8 => builder.ins().ishl_imm_s(l, 3),
                9 => {
                    let eight_l = builder.ins().ishl_imm_s(l, 3);
                    builder.ins().iadd(eight_l, l)
                }
                10 => {
                    let five_l = {
                        let four_l = builder.ins().ishl_imm_s(l, 2);
                        builder.ins().iadd(four_l, l)
                    };
                    builder.ins().ishl_imm_s(five_l, 1)
                }
                11 => {
                    let two_l = builder.ins().ishl_imm_s(l, 1);
                    let three_l = builder.ins().iadd(two_l, l);
                    let eight_l = builder.ins().ishl_imm_s(l, 3);
                    builder.ins().iadd(three_l, eight_l)
                }
                13 => {
                    let two_l = builder.ins().ishl_imm_s(l, 1);
                    let three_l = builder.ins().iadd(two_l, l);
                    let twelve_l = builder.ins().ishl_imm_s(three_l, 2);
                    builder.ins().iadd(twelve_l, l)
                }
                19 => {
                    let two_l = builder.ins().ishl_imm_s(l, 1);
                    let three_l = builder.ins().iadd(two_l, l);
                    let sixteen_l = builder.ins().ishl_imm_s(l, 4);
                    builder.ins().iadd(three_l, sixteen_l)
                }
                23 => {
                    let two_l = builder.ins().ishl_imm_s(l, 1);
                    let three_l = builder.ins().iadd(two_l, l);
                    let tfour_l = builder.ins().ishl_imm_s(three_l, 3);
                    builder.ins().isub(tfour_l, l)
                }
                29 => {
                    let two_l = builder.ins().ishl_imm_s(l, 1);
                    let three_l = builder.ins().iadd(two_l, l);
                    let thirtytwo_l = builder.ins().ishl_imm_s(l, 5);
                    builder.ins().isub(thirtytwo_l, three_l)
                }
                _ if k > 0 && (k as u64).is_power_of_two() => {
                    let shift = (k as u64).trailing_zeros();
                    builder.ins().ishl_imm_s(l, shift as i64)
                }
                _ if k > 1 && ((k as u64) + 1).is_power_of_two() => {
                    let shift = ((k as u64) + 1).trailing_zeros();
                    let shifted = builder.ins().ishl_imm_s(l, shift as i64);
                    builder.ins().isub(shifted, l)
                }
                _ if k > 1 && ((k as u64) - 1).is_power_of_two() => {
                    let shift = ((k as u64) - 1).trailing_zeros();
                    let shifted = builder.ins().ishl_imm_s(l, shift as i64);
                    builder.ins().iadd(shifted, l)
                }
                _ if k > 0 && k % 3 == 0 && ((k / 3) as u64).is_power_of_two() => {
                    let two_l = builder.ins().ishl_imm_s(l, 1);
                    let three_l = builder.ins().iadd(two_l, l);
                    let shift = ((k / 3) as u64).trailing_zeros();
                    if shift > 0 {
                        builder.ins().ishl_imm_s(three_l, shift as i64)
                    } else {
                        three_l
                    }
                }
                _ if k > 0 && k % 5 == 0 && ((k / 5) as u64).is_power_of_two() => {
                    let four_l = builder.ins().ishl_imm_s(l, 2);
                    let five_l = builder.ins().iadd(four_l, l);
                    let shift = ((k / 5) as u64).trailing_zeros();
                    if shift > 0 {
                        builder.ins().ishl_imm_s(five_l, shift as i64)
                    } else {
                        five_l
                    }
                }
                _ if k > 0 && k % 9 == 0 && ((k / 9) as u64).is_power_of_two() => {
                    let eight_l = builder.ins().ishl_imm_s(l, 3);
                    let nine_l = builder.ins().iadd(eight_l, l);
                    let shift = ((k / 9) as u64).trailing_zeros();
                    if shift > 0 {
                        builder.ins().ishl_imm_s(nine_l, shift as i64)
                    } else {
                        nine_l
                    }
                }
                _ => builder.ins().imul(l, r),
            }
        } else {
            builder.ins().imul(l, r)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_fast_signed_div(
        &mut self,
        l: Value,
        r: Value,
        d: i64,
        operand_ty: &Type,
        is_nonneg: bool,
        is_u32: bool,
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
        let q64 = if is_nonneg && d > 0 {
            if ad.is_power_of_two() {
                let k = ad.trailing_zeros();
                if k == 0 {
                    n
                } else {
                    builder.ins().ushr_imm_s(n, k as i64)
                }
            } else if let Some((m, s)) = if is_u32 { compute_magic_u32_fast(ad) } else { None } {
                let m_val = self.get_iconst(types::I64, m as i64, builder);
                let prod = builder.ins().imul(n, m_val);
                builder.ins().ushr_imm_s(prod, s as i64)
            } else if let Some((m, s)) = compute_magic_u64_nonneg(ad) {
                let m_val = self.get_iconst(types::I64, m as i64, builder);
                let hi = builder.ins().umulhi(n, m_val);
                if s > 0 {
                    builder.ins().ushr_imm_s(hi, s as i64)
                } else {
                    hi
                }
            } else {
                let (m, shift, add_ind) = compute_magic_s64(d.abs());
                let m_val = self.get_iconst(types::I64, m, builder);
                let mut hi = builder.ins().smulhi(n, m_val);
                if add_ind {
                    hi = builder.ins().iadd(hi, n);
                }
                if shift > 0 {
                    builder.ins().sshr_imm_s(hi, shift as i64)
                } else {
                    hi
                }
            }
        } else if ad.is_power_of_two() {
            let k = ad.trailing_zeros();
            let sign = builder.ins().sshr_imm_s(n, 63);
            let bias = builder.ins().band_imm_s(sign, (ad - 1) as i64);
            let biased = builder.ins().iadd(n, bias);
            let q_pos = builder.ins().sshr_imm_s(biased, k as i64);
            if d < 0 {
                builder.ins().ineg(q_pos)
            } else {
                q_pos
            }
        } else {
            let (m, shift, add_ind) = compute_magic_s64(d.abs());
            let m_val = self.get_iconst(types::I64, m, builder);
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_fast_signed_rem(
        &mut self,
        l: Value,
        r: Value,
        d: i64,
        operand_ty: &Type,
        is_nonneg: bool,
        is_u32: bool,
        builder: &mut FunctionBuilder,
    ) -> Result<Value, CodegenError> {
        if d == 0 {
            return Ok(builder.ins().srem(l, r));
        }
        if d == 1 || d == -1 {
            let clif_ty = type_to_clif(operand_ty.clone());
            return Ok(self.get_iconst(clif_ty, 0, builder));
        }

        let is_i32 = *operand_ty == Type::I32;
        let n = if is_i32 {
            builder.ins().sextend(types::I64, l)
        } else {
            l
        };

        let ad = d.unsigned_abs();
        let rem64 = if is_nonneg && d > 0 {
            if ad == 4294967296 {
                let r32 = builder.ins().ireduce(types::I32, n);
                builder.ins().uextend(types::I64, r32)
            } else if ad.is_power_of_two() {
                builder.ins().band_imm_s(n, d - 1)
            } else if let Some((m, s)) = if is_u32 { compute_magic_u32_fast(ad) } else { None } {
                let m_val = self.get_iconst(types::I64, m as i64, builder);
                let prod = builder.ins().imul(n, m_val);
                let q = builder.ins().ushr_imm_s(prod, s as i64);
                let d_val = self.get_iconst(types::I64, d, builder);
                let q_times_d = self.emit_fast_int_mul(q, d_val, Some(d), types::I64, builder);
                builder.ins().isub(n, q_times_d)
            } else if let Some((m, s)) = compute_magic_u64_nonneg(ad) {
                let m_val = self.get_iconst(types::I64, m as i64, builder);
                let hi = builder.ins().umulhi(n, m_val);
                let q = if s > 0 {
                    builder.ins().ushr_imm_s(hi, s as i64)
                } else {
                    hi
                };
                let d_val = self.get_iconst(types::I64, d, builder);
                let q_times_d = self.emit_fast_int_mul(q, d_val, Some(d), types::I64, builder);
                builder.ins().isub(n, q_times_d)
            } else {
                let (m, shift, add_ind) = compute_magic_s64(d.abs());
                let m_val = self.get_iconst(types::I64, m, builder);
                let mut hi = builder.ins().smulhi(n, m_val);
                if add_ind {
                    hi = builder.ins().iadd(hi, n);
                }
                let q = if shift > 0 {
                    builder.ins().sshr_imm_s(hi, shift as i64)
                } else {
                    hi
                };
                let d_val = self.get_iconst(types::I64, d, builder);
                let q_times_d = self.emit_fast_int_mul(q, d_val, Some(d), types::I64, builder);
                builder.ins().isub(n, q_times_d)
            }
        } else if ad.is_power_of_two() {
            let sign = builder.ins().sshr_imm_s(n, 63);
            let bias = builder.ins().band_imm_s(sign, (ad - 1) as i64);
            let biased = builder.ins().iadd(n, bias);
            let masked = builder.ins().band_imm_s(biased, -(ad as i64));
            let rem_pos = builder.ins().isub(n, masked);
            if d < 0 {
                builder.ins().ineg(rem_pos)
            } else {
                rem_pos
            }
        } else {
            let (m, shift, add_ind) = compute_magic_s64(d.abs());
            let m_val = self.get_iconst(types::I64, m, builder);
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
            let q64 = if d < 0 {
                builder.ins().ineg(q_pos)
            } else {
                q_pos
            };
            let d_val = self.get_iconst(types::I64, d, builder);
            let q_times_d = self.emit_fast_int_mul(q64, d_val, Some(d), types::I64, builder);
            builder.ins().isub(n, q_times_d)
        };

        if is_i32 {
            Ok(builder.ins().ireduce(types::I32, rem64))
        } else {
            Ok(rem64)
        }
    }


    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_det3_val(
        builder: &mut FunctionBuilder,
        elem_ty: &Type,
        m00: Value, m01: Value, m02: Value,
        m10: Value, m11: Value, m12: Value,
        m20: Value, m21: Value, m22: Value,
    ) -> Value {
        if elem_ty.is_float() {
            let p0 = builder.ins().fmul(m11, m22);
            let p1 = builder.ins().fmul(m12, m21);
            let sub0 = builder.ins().fsub(p0, p1);
            let d0 = builder.ins().fmul(m00, sub0);

            let p2 = builder.ins().fmul(m10, m22);
            let p3 = builder.ins().fmul(m12, m20);
            let sub1 = builder.ins().fsub(p2, p3);
            let d1 = builder.ins().fmul(m01, sub1);

            let p4 = builder.ins().fmul(m10, m21);
            let p5 = builder.ins().fmul(m11, m20);
            let sub2 = builder.ins().fsub(p4, p5);
            let d2 = builder.ins().fmul(m02, sub2);

            let sub_d = builder.ins().fsub(d0, d1);
            builder.ins().fadd(sub_d, d2)
        } else {
            let p0 = builder.ins().imul(m11, m22);
            let p1 = builder.ins().imul(m12, m21);
            let sub0 = builder.ins().isub(p0, p1);
            let d0 = builder.ins().imul(m00, sub0);

            let p2 = builder.ins().imul(m10, m22);
            let p3 = builder.ins().imul(m12, m20);
            let sub1 = builder.ins().isub(p2, p3);
            let d1 = builder.ins().imul(m01, sub1);

            let p4 = builder.ins().imul(m10, m21);
            let p5 = builder.ins().imul(m11, m20);
            let sub2 = builder.ins().isub(p4, p5);
            let d2 = builder.ins().imul(m02, sub2);

            let sub_d = builder.ins().isub(d0, d1);
            builder.ins().iadd(sub_d, d2)
        }
    }

    pub(crate) fn emit_gcd(
        builder: &mut FunctionBuilder,
        clif_ty: types::Type,
        a_val: Value,
        b_val: Value,
    ) -> Value {
        let zero = builder.ins().iconst(clif_ty, 0);
        let neg_a = builder.ins().ineg(a_val);
        let cond_a = builder.ins().icmp(IntCC::SignedLessThan, a_val, zero);
        let abs_a = builder.ins().select(cond_a, neg_a, a_val);

        let neg_b = builder.ins().ineg(b_val);
        let cond_b = builder.ins().icmp(IntCC::SignedLessThan, b_val, zero);
        let abs_b = builder.ins().select(cond_b, neg_b, b_val);

        let var_u = builder.declare_var(clif_ty);
        let var_v = builder.declare_var(clif_ty);
        builder.def_var(var_u, abs_a);
        builder.def_var(var_v, abs_b);

        let header_block = builder.create_block();
        let body_block = builder.create_block();
        let exit_block = builder.create_block();

        builder.ins().jump(header_block, &[]);

        builder.switch_to_block(header_block);
        let cur_v = builder.use_var(var_v);
        let is_zero = builder.ins().icmp(IntCC::Equal, cur_v, zero);
        builder.ins().brif(is_zero, exit_block, &[], body_block, &[]);

        builder.switch_to_block(body_block);
        let cur_u = builder.use_var(var_u);
        let rem = builder.ins().urem(cur_u, cur_v);
        builder.def_var(var_u, cur_v);
        builder.def_var(var_v, rem);
        builder.ins().jump(header_block, &[]);

        builder.seal_block(header_block);
        builder.seal_block(body_block);

        builder.switch_to_block(exit_block);
        builder.seal_block(exit_block);
        builder.use_var(var_u)
    }

    pub(crate) fn emit_atan2(builder: &mut FunctionBuilder, y: Value, x: Value) -> Value {
        let zero = builder.ins().f64const(0.0);
        let pi = builder.ins().f64const(std::f64::consts::PI);
        let half_pi = builder.ins().f64const(std::f64::consts::FRAC_PI_2);

        let abs_y = builder.ins().fabs(y);
        let abs_x = builder.ins().fabs(x);

        let x_greater = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, abs_x, abs_y);
        let num = builder.ins().select(x_greater, abs_y, abs_x);
        let den = builder.ins().select(x_greater, abs_x, abs_y);
        let t = builder.ins().fdiv(num, den);
        let t2 = builder.ins().fmul(t, t);

        let c11 = builder.ins().f64const(-0.01172120);
        let c9 = builder.ins().f64const(0.05265332);
        let c7 = builder.ins().f64const(-0.11643287);
        let c5 = builder.ins().f64const(0.19354346);
        let c3 = builder.ins().f64const(-0.33262347);
        let c1 = builder.ins().f64const(0.99997726);

        let p9 = builder.ins().fma(t2, c11, c9);
        let p7 = builder.ins().fma(t2, p9, c7);
        let p5 = builder.ins().fma(t2, p7, c5);
        let p3 = builder.ins().fma(t2, p5, c3);
        let poly = builder.ins().fma(t2, p3, c1);
        let at = builder.ins().fmul(t, poly);

        let pi_over_2_sub = builder.ins().fsub(half_pi, at);
        let mut angle = builder.ins().select(x_greater, at, pi_over_2_sub);

        let x_neg = builder.ins().fcmp(FloatCC::LessThan, x, zero);
        let pi_sub = builder.ins().fsub(pi, angle);
        angle = builder.ins().select(x_neg, pi_sub, angle);

        let y_neg = builder.ins().fcmp(FloatCC::LessThan, y, zero);
        let neg_angle = builder.ins().fneg(angle);
        builder.ins().select(y_neg, neg_angle, angle)
    }

    pub(crate) fn emit_fft(
        builder: &mut FunctionBuilder,
        re_vals: &[Value],
        im_vals: &[Value],
        n: usize,
        real_only: bool,
    ) -> (Vec<Value>, Vec<Value>) {
        assert!(n == 8 || n == 16);
        let bits = if n == 8 { 3 } else { 4 };
        let mut r = vec![re_vals[0]; n];
        let mut i = vec![im_vals[0]; n];
        for k in 0..n {
            let mut rev = 0;
            for b in 0..bits {
                if (k & (1 << b)) != 0 {
                    rev |= 1 << (bits - 1 - b);
                }
            }
            r[rev] = re_vals[k];
            i[rev] = im_vals[k];
        }

        let mut s = 1;
        while s < n {
            let len = s * 2;
            let is_last_stage = s * 2 == n;
            let mut group = 0;
            while group < n {
                for j in 0..s {
                    let angle = -2.0 * std::f64::consts::PI * (j as f64) / (len as f64);
                    let wr_f = angle.cos();
                    let wi_f = angle.sin();

                    let u_idx = group + j;
                    let v_idx = group + j + s;

                    let rv = r[v_idx];
                    let iv = i[v_idx];
                    let ru = r[u_idx];
                    let iu = i[u_idx];

                    let (tr, ti) = if (wr_f - 1.0).abs() < 1e-12 && wi_f.abs() < 1e-12 {
                        (rv, iv)
                    } else if wr_f.abs() < 1e-12 && (wi_f + 1.0).abs() < 1e-12 {
                        (iv, builder.ins().fneg(rv))
                    } else if (wr_f + 1.0).abs() < 1e-12 && wi_f.abs() < 1e-12 {
                        (builder.ins().fneg(rv), builder.ins().fneg(iv))
                    } else if wr_f.abs() < 1e-12 && (wi_f - 1.0).abs() < 1e-12 {
                        (builder.ins().fneg(iv), rv)
                    } else {
                        let wr = builder.ins().f64const(wr_f);
                        let wi = builder.ins().f64const(wi_f);
                        let tr0 = builder.ins().fmul(wr, rv);
                        let tr1 = builder.ins().fmul(wi, iv);
                        let tr = builder.ins().fsub(tr0, tr1);
                        let ti0 = builder.ins().fmul(wr, iv);
                        let ti = builder.ins().fma(wi, rv, ti0);
                        (tr, ti)
                    };

                    r[u_idx] = builder.ins().fadd(ru, tr);
                    r[v_idx] = builder.ins().fsub(ru, tr);
                    if !(real_only && is_last_stage) {
                        i[u_idx] = builder.ins().fadd(iu, ti);
                        i[v_idx] = builder.ins().fsub(iu, ti);
                    }
                }
                group += len;
            }
            s *= 2;
        }

        (r, i)
    }

    pub(crate) fn emit_sub_mul(builder: &mut FunctionBuilder, a: Value, b: Value, c: Value, d: Value) -> Value {
        let ab = builder.ins().fmul(a, b);
        let cd = builder.ins().fmul(c, d);
        builder.ins().fsub(ab, cd)
    }


}

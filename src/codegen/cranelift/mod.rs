//! Cranelift code generation backend for NumLang.

pub mod abi;
pub mod ast_expr;
pub mod ast_stmt;
pub mod deopt;
pub mod escape;
pub mod intrinsics;
pub mod mir_emit;

pub use abi::*;
use ast_stmt::{FunctionTranslationState, Storage};
pub use deopt::*;

use cranelift_codegen::ir::{
    types, AbiParam, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, TrapCode, Value,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_native;
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::HashMap;

use crate::codegen::backend_trait::BackendCompiler;
use crate::typecheck::{Type, TypedFunction, TypedProgram};

pub struct CraneliftCompiler {
    pub(crate) module: ObjectModule,
    pub(crate) func_ids: HashMap<String, FuncId>,
    pub struct_layouts: HashMap<String, StructLayout>,
    pub enum_layouts: HashMap<String, EnumLayout>,
    pub(crate) exit_process_id: FuncId,
    pub(crate) get_std_handle_id: Option<FuncId>,
    pub(crate) write_file_id: FuncId,
    pub(crate) print_str_id: FuncId,
    pub(crate) print_newline_id: FuncId,
    pub(crate) print_i64_id: FuncId,
    pub(crate) print_u64_id: FuncId,
    pub(crate) print_f64_id: FuncId,
    pub(crate) print_bool_id: FuncId,
    pub(crate) sin_id: FuncId,
    pub(crate) cos_id: FuncId,
    pub(crate) tan_id: FuncId,
    pub(crate) exp_id: FuncId,
    pub(crate) log_id: FuncId,
    pub(crate) log2_id: FuncId,
    pub(crate) log10_id: FuncId,
    pub(crate) pow_id: FuncId,
    pub(crate) malloc_id: FuncId,
    pub(crate) loop_reset_id: FuncId,
    pub(crate) arena_alloc_id: FuncId,
    pub(crate) arena_reset_id: FuncId,
    pub(crate) arena_cur_id: DataId,
    pub(crate) arena_end_id: DataId,
    pub(crate) arena_start_id: DataId,
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

        #[cfg(target_arch = "x86_64")]
        let isa_builder = {
            let mut isa_builder = isa_builder;
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
            isa_builder
        };

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let builder = ObjectBuilder::new(
            isa,
            "numlang_out",
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut module = ObjectModule::new(builder);

        #[cfg(target_os = "windows")]
        let exit_process_id = {
            let mut exit_sig = module.make_signature();
            exit_sig.params.push(AbiParam::new(types::I32));
            module
                .declare_function("ExitProcess", Linkage::Import, &exit_sig)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?
        };

        #[cfg(not(target_os = "windows"))]
        let exit_process_id = {
            let mut exit_sig = module.make_signature();
            exit_sig.params.push(AbiParam::new(types::I32));
            module
                .declare_function("exit", Linkage::Import, &exit_sig)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?
        };

        #[cfg(target_os = "windows")]
        let (get_std_handle_id, write_file_id) = {
            let mut gsh_sig = module.make_signature();
            gsh_sig.params.push(AbiParam::new(types::I32));
            gsh_sig.returns.push(AbiParam::new(types::I64));
            let gsh_id = module
                .declare_function("GetStdHandle", Linkage::Import, &gsh_sig)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?;

            let mut wf_sig = module.make_signature();
            wf_sig.params.push(AbiParam::new(types::I64));
            wf_sig.params.push(AbiParam::new(types::I64));
            wf_sig.params.push(AbiParam::new(types::I32));
            wf_sig.params.push(AbiParam::new(types::I64));
            wf_sig.params.push(AbiParam::new(types::I64));
            wf_sig.returns.push(AbiParam::new(types::I32));
            let wf_id = module
                .declare_function("WriteFile", Linkage::Import, &wf_sig)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?;
            (Some(gsh_id), wf_id)
        };

        #[cfg(not(target_os = "windows"))]
        let (get_std_handle_id, write_file_id) = {
            let mut write_sig = module.make_signature();
            write_sig.params.push(AbiParam::new(types::I32));
            write_sig.params.push(AbiParam::new(types::I64));
            write_sig.params.push(AbiParam::new(types::I64));
            write_sig.returns.push(AbiParam::new(types::I64));
            let write_id = module
                .declare_function("write", Linkage::Import, &write_sig)
                .map_err(|e| CodegenError::BackendError(e.to_string()))?;
            (None, write_id)
        };

        let mut print_str_sig = module.make_signature();
        print_str_sig.params.push(AbiParam::new(types::I64));
        print_str_sig.params.push(AbiParam::new(types::I32));
        let print_str_id = module
            .declare_function("__nl_print_str", Linkage::Local, &print_str_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let print_nl_sig = module.make_signature();
        let print_newline_id = module
            .declare_function("__nl_print_newline", Linkage::Local, &print_nl_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut print_i64_sig = module.make_signature();
        print_i64_sig.params.push(AbiParam::new(types::I64));
        let print_i64_id = module
            .declare_function("__nl_print_i64", Linkage::Local, &print_i64_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut print_u64_sig = module.make_signature();
        print_u64_sig.params.push(AbiParam::new(types::I64));
        let print_u64_id = module
            .declare_function("__nl_print_u64", Linkage::Local, &print_u64_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut print_f64_sig = module.make_signature();
        print_f64_sig.params.push(AbiParam::new(types::F64));
        let print_f64_id = module
            .declare_function("__nl_print_f64", Linkage::Local, &print_f64_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut print_bool_sig = module.make_signature();
        print_bool_sig.params.push(AbiParam::new(types::I8));
        let print_bool_id = module
            .declare_function("__nl_print_bool", Linkage::Local, &print_bool_sig)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut sig_1f = module.make_signature();
        sig_1f.params.push(AbiParam::new(types::F64));
        sig_1f.returns.push(AbiParam::new(types::F64));

        let sin_id = module
            .declare_function("sin", Linkage::Import, &sig_1f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        let cos_id = module
            .declare_function("cos", Linkage::Import, &sig_1f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        let tan_id = module
            .declare_function("tan", Linkage::Import, &sig_1f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        let exp_id = module
            .declare_function("exp", Linkage::Import, &sig_1f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        let log_id = module
            .declare_function("log", Linkage::Import, &sig_1f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        let log2_id = module
            .declare_function("log2", Linkage::Import, &sig_1f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        let log10_id = module
            .declare_function("log10", Linkage::Import, &sig_1f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut sig_2f = module.make_signature();
        sig_2f.params.push(AbiParam::new(types::F64));
        sig_2f.params.push(AbiParam::new(types::F64));
        sig_2f.returns.push(AbiParam::new(types::F64));

        let pow_id = module
            .declare_function("pow", Linkage::Import, &sig_2f)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let mut sig_malloc = module.make_signature();
        sig_malloc.params.push(AbiParam::new(types::I64));
        sig_malloc.returns.push(AbiParam::new(types::I64));

        let sig_void = module.make_signature();

        let mut data_desc = DataDescription::new();
        data_desc.define_zeroinit(8);

        let arena_cur_id = module
            .declare_data("__nl_arena_cur", Linkage::Local, true, false)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        module
            .define_data(arena_cur_id, &data_desc)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let arena_end_id = module
            .declare_data("__nl_arena_end", Linkage::Local, true, false)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        module
            .define_data(arena_end_id, &data_desc)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let arena_start_id = module
            .declare_data("__nl_arena_start", Linkage::Local, true, false)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;
        module
            .define_data(arena_start_id, &data_desc)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let malloc_id = module
            .declare_function("__nl_malloc", Linkage::Export, &sig_malloc)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let arena_alloc_id = module
            .declare_function("__nl_arena_alloc", Linkage::Export, &sig_malloc)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let loop_reset_id = module
            .declare_function("__nl_loop_reset", Linkage::Export, &sig_void)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        let arena_reset_id = module
            .declare_function("__nl_arena_reset", Linkage::Export, &sig_void)
            .map_err(|e| CodegenError::BackendError(e.to_string()))?;

        Ok(Self {
            module,
            func_ids: HashMap::new(),
            struct_layouts: HashMap::new(),
            enum_layouts: HashMap::new(),
            exit_process_id,
            get_std_handle_id,
            write_file_id,
            print_str_id,
            print_newline_id,
            print_i64_id,
            print_u64_id,
            print_f64_id,
            print_bool_id,
            sin_id,
            cos_id,
            tan_id,
            exp_id,
            log_id,
            log2_id,
            log10_id,
            pow_id,
            malloc_id,
            loop_reset_id,
            arena_alloc_id,
            arena_reset_id,
            arena_cur_id,
            arena_end_id,
            arena_start_id,
        })
    }

    pub fn compile_program(mut self, program: &TypedProgram) -> Result<Vec<u8>, CodegenError> {
        self.struct_layouts = compute_struct_layouts(&program.structs);
        self.enum_layouts = compute_enum_layouts(&program.enums, &self.struct_layouts);

        // Step 1: Declare all user functions
        for func in &program.functions {
            let mut sig = self.module.make_signature();
            if matches!(&func.return_ty, Type::Struct(_) | Type::Enum(_)) {
                sig.params.push(AbiParam::new(types::I64)); // hidden sret pointer
                sig.returns.push(AbiParam::new(types::I64));
            } else if func.return_ty != Type::Void {
                sig.returns
                    .push(AbiParam::new(type_to_clif(func.return_ty.clone())));
            }

            for param in &func.params {
                if let Type::Struct(sname) = &param.ty {
                    if let Some(layout) = self.struct_layouts.get(sname) {
                        let leaves = layout.get_leaf_fields(&self.struct_layouts);
                        for (_, leaf_ty) in leaves {
                            sig.params.push(AbiParam::new(type_to_clif(leaf_ty)));
                        }
                    }
                } else if let Type::Enum(_) = &param.ty {
                    sig.params.push(AbiParam::new(types::I64));
                } else {
                    sig.params
                        .push(AbiParam::new(type_to_clif(param.ty.clone())));
                }
            }

            let export_name = if func.name == "main"
                && std::env::var("NUMLANG_BENCH").is_ok()
                && !cfg!(target_os = "windows")
            {
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

        for func in &program.functions {
            self.compile_function(func, &mut ctx, &mut fn_builder_ctx)?;
        }

        // Step 3: Emit entry point (mainCRTStartup) if main exists and benchmarking mode is disabled
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

        let local_main = self.module.declare_func_in_func(main_id, builder.func);
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
            .declare_func_in_func(self.exit_process_id, builder.func);
        builder.ins().call(local_exit, &[exit_code]);
        builder.ins().trap(TrapCode::unwrap_user(1));

        let config = self.module.target_config();
        builder.finalize(config);

        self.module.define_function(entry_id, ctx).map_err(|e| {
            CodegenError::BackendError(format!("Verifier error in entry: {:#?}", e))
        })?;
        self.module.clear_context(ctx);

        Ok(())
    }

    fn compile_function(
        &mut self,
        func: &TypedFunction,
        ctx: &mut cranelift_codegen::Context,
        fn_builder_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), CodegenError> {
        let func_id = *self.func_ids.get(&func.name).ok_or_else(|| {
            CodegenError::BackendError(format!("Function '{}' not declared", func.name))
        })?;

        let mut sig = self.module.make_signature();
        let is_sret = matches!(&func.return_ty, Type::Struct(_) | Type::Enum(_));
        if is_sret {
            sig.params.push(AbiParam::new(types::I64)); // hidden sret pointer
            sig.returns.push(AbiParam::new(types::I64));
        } else if func.return_ty != Type::Void {
            sig.returns
                .push(AbiParam::new(type_to_clif(func.return_ty.clone())));
        }

        for param in &func.params {
            if let Type::Struct(sname) = &param.ty {
                if let Some(layout) = self.struct_layouts.get(sname.as_str()) {
                    let leaves = layout.get_leaf_fields(&self.struct_layouts);
                    for (_, leaf_ty) in leaves {
                        sig.params.push(AbiParam::new(type_to_clif(leaf_ty)));
                    }
                }
            } else if let Type::Enum(_) = &param.ty {
                sig.params.push(AbiParam::new(types::I64));
            } else {
                sig.params
                    .push(AbiParam::new(type_to_clif(param.ty.clone())));
            }
        }

        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);
        builder.ensure_inserted_block();

        let mut block_param_idx = 0;
        let block_params: Vec<Value> = builder.block_params(entry_block).to_vec();
        let current_sret_ptr = if is_sret {
            let sret = block_params[block_param_idx];
            block_param_idx += 1;
            Some(sret)
        } else {
            None
        };

        let mut variables: HashMap<String, Storage> = HashMap::new();

        for param in &func.params {
            if let Type::Struct(sname) = &param.ty {
                let layout = self.struct_layouts.get(sname.as_str()).ok_or_else(|| {
                    CodegenError::BackendError(format!("Struct layout for '{}' not found", sname))
                })?;
                let slot_data = StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    layout.total_size,
                    layout.align.min(8) as u8,
                );
                let slot = builder.create_sized_stack_slot(slot_data);
                let slot_addr = builder.ins().stack_addr(types::I64, slot, 0);

                let leaves = layout.get_leaf_fields(&self.struct_layouts);
                for (leaf_offset, _leaf_ty) in leaves {
                    let leaf_val = block_params[block_param_idx];
                    block_param_idx += 1;
                    builder.ins().store(
                        MemFlagsData::trusted(),
                        leaf_val,
                        slot_addr,
                        leaf_offset as i32,
                    );
                }
                variables.insert(
                    param.name.clone(),
                    Storage::Struct {
                        slot,
                        struct_name: sname.clone(),
                    },
                );
            } else if let Type::Enum(ename) = &param.ty {
                let layout = self
                    .enum_layouts
                    .get(ename.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        CodegenError::BackendError(format!("Enum layout for '{}' not found", ename))
                    })?;
                let slot_data = StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    layout.total_size,
                    layout.align.min(8) as u8,
                );
                let slot = builder.create_sized_stack_slot(slot_data);
                let slot_addr = builder.ins().stack_addr(types::I64, slot, 0);
                let incoming_ptr = block_params[block_param_idx];
                block_param_idx += 1;
                FunctionTranslationState::emit_copy_bytes(
                    &mut builder,
                    incoming_ptr,
                    slot_addr,
                    layout.total_size as usize,
                );
                variables.insert(
                    param.name.clone(),
                    Storage::Enum {
                        slot,
                        enum_name: ename.clone(),
                    },
                );
            } else {
                let clif_ty = type_to_clif(param.ty.clone());
                let var = builder.declare_var(clif_ty);
                let val = block_params[block_param_idx];
                block_param_idx += 1;
                builder.def_var(var, val);
                variables.insert(param.name.clone(), Storage::Scalar(var));
            }
        }

        let body_to_translate = crate::opt::recursion::try_lower_binary_recurrence_tree(func)
            .or_else(|| crate::opt::recursion::try_lower_tail_calls(func))
            .unwrap_or_else(|| func.body.clone());
        let dynamically_indexed_arrays = collect_dynamically_indexed_arrays(&body_to_translate);
        let known_non_negative_vars = collect_known_non_negative_vars(&body_to_translate);
        let known_u32_vars = collect_known_u32_vars(&body_to_translate, &known_non_negative_vars);
        let known_var_bounds =
            collect_known_var_upper_bounds(&body_to_translate, &known_non_negative_vars);
        let mut const_pool = HashMap::new();
        let mut f64_pool = HashMap::new();
        let mut f32_pool = HashMap::new();

        for &f in &[0.0f64, 1.0, 2.0, 0.5] {
            f64_pool.insert(f.to_bits(), builder.ins().f64const(f));
        }
        for &f in &[0.0f32, 1.0, 2.0, 0.5] {
            f32_pool.insert(f.to_bits(), builder.ins().f32const(f));
        }

        let mut divisors = Vec::new();
        collect_constant_divisors_block(&body_to_translate, &mut divisors);
        for d in divisors {
            const_pool
                .entry((types::I64, d as u64))
                .or_insert_with(|| builder.ins().iconst(types::I64, d));
            let ad = d.unsigned_abs();
            if let Some((m, _)) = compute_magic_u32_fast(ad) {
                const_pool
                    .entry((types::I64, m))
                    .or_insert_with(|| builder.ins().iconst(types::I64, m as i64));
            }
            if let Some((m, _)) = compute_magic_u64_nonneg(ad) {
                const_pool.entry((types::I64, m)).or_insert_with(|| {
                    let c1 = builder.ins().iconst(types::I64, (m.wrapping_sub(1)) as i64);
                    let c2 = builder.ins().iconst(types::I64, 1);
                    builder.ins().iadd(c1, c2)
                });
            }
            let (m, _, _) = compute_magic_s64(d.abs());
            const_pool.entry((types::I64, m as u64)).or_insert_with(|| {
                let m_u = m as u64;
                let c1 = builder
                    .ins()
                    .iconst(types::I64, (m_u.wrapping_sub(1)) as i64);
                let c2 = builder.ins().iconst(types::I64, 1);
                builder.ins().iadd(c1, c2)
            });
        }

        let mut state = FunctionTranslationState {
            module: &mut self.module,
            func_ids: &self.func_ids,
            struct_layouts: &self.struct_layouts,
            enum_layouts: &self.enum_layouts,
            current_sret_ptr,
            exit_process_id: self.exit_process_id,
            get_std_handle_id: self.get_std_handle_id,
            write_file_id: self.write_file_id,
            print_str_id: self.print_str_id,
            print_newline_id: self.print_newline_id,
            print_i64_id: self.print_i64_id,
            print_u64_id: self.print_u64_id,
            print_f64_id: self.print_f64_id,
            print_bool_id: self.print_bool_id,
            sin_id: self.sin_id,
            cos_id: self.cos_id,
            tan_id: self.tan_id,
            exp_id: self.exp_id,
            log_id: self.log_id,
            log2_id: self.log2_id,
            log10_id: self.log10_id,
            pow_id: self.pow_id,
            variables,
            loop_exit_blocks: Vec::new(),
            loop_continue_blocks: Vec::new(),
            dynamically_indexed_arrays,
            known_non_negative_vars,
            known_u32_vars,
            known_var_bounds,
            const_pool,
            f64_pool,
            f32_pool,
            array_load_cache: HashMap::new(),
            malloc_id: self.malloc_id,
            loop_reset_id: self.loop_reset_id,
        };

        let terminated = state.translate_block(&body_to_translate, &mut builder)?;

        if !terminated {
            builder.ins().return_(&[]);
        }

        let config = self.module.target_config();
        builder.finalize(config);
        if let Ok(dump_target) = std::env::var("DUMP_CLIF") {
            if dump_target.is_empty() || func.name == dump_target {
                eprintln!("=== CLIF IR for {} ===\n{}", func.name, ctx.func);
            }
        }

        if let Err(e) = self.module.define_function(func_id, ctx) {
            eprintln!(
                "VERIFIER ERROR for function {}:\n{:#?}\nIR:\n{}",
                func.name, e, ctx.func
            );
            return Err(CodegenError::BackendError(format!("{:#?}", e)));
        }
        self.module.clear_context(ctx);

        Ok(())
    }
}

pub fn compile_to_obj(program: &TypedProgram) -> Result<Vec<u8>, CodegenError> {
    compile_to_obj_with_opt(program, true)
}

pub fn compile_to_obj_with_opt(
    program: &TypedProgram,
    should_optimize: bool,
) -> Result<Vec<u8>, CodegenError> {
    let compiler = CraneliftCompiler::new()?;
    if should_optimize {
        let mut optimized = program.clone();
        crate::opt::optimize_program(&mut optimized);
        compiler.compile_program(&optimized)
    } else {
        let mut unopt = program.clone();
        crate::opt::monomorphize::monomorphize(&mut unopt);
        unopt.desugar_for_loops();
        compiler.compile_program(&unopt)
    }
}

pub fn compile_mir_to_obj(mir: &crate::mir::lower::MirProgram) -> Result<Vec<u8>, CodegenError> {
    let compiler = CraneliftCompiler::new()?;
    compiler.compile_mir_program(mir)
}

pub fn compile_supercompiled_to_obj(program: &TypedProgram) -> Result<Vec<u8>, CodegenError> {
    compile_supercompiled_to_obj_with_mode(
        program,
        crate::mir::supercompiler::SupercompileMode::Classic,
        "size",
    )
}

pub fn compile_supercompiled_to_obj_with_mode(
    program: &TypedProgram,
    mode: crate::mir::supercompiler::SupercompileMode,
    objective: &str,
) -> Result<Vec<u8>, CodegenError> {
    compile_supercompiled_to_obj_with_mode_options(program, mode, objective, false)
}

pub fn compile_supercompiled_to_obj_with_mode_options(
    program: &TypedProgram,
    mode: crate::mir::supercompiler::SupercompileMode,
    objective: &str,
    parallel_residualize: bool,
) -> Result<Vec<u8>, CodegenError> {
    compile_supercompiled_to_obj_with_cache(program, mode, objective, parallel_residualize, None)
}

pub fn compile_supercompiled_to_obj_with_cache(
    program: &TypedProgram,
    mode: crate::mir::supercompiler::SupercompileMode,
    objective: &str,
    parallel_residualize: bool,
    opt_cache: Option<&crate::mir::supercompiler::cache::SpecializationCache>,
) -> Result<Vec<u8>, CodegenError> {
    let mut typed = program.clone();
    crate::compiler::distill_and_optimize(
        &mut typed,
        &crate::compiler::CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..Default::default()
        },
    );
    let mut mir_program = crate::mir::lower::lower_program(&typed);
    crate::mir::supercompiler::supercompile_mir_program_with_cache(
        &mut mir_program,
        mode,
        objective,
        parallel_residualize,
        opt_cache,
    );
    compile_mir_to_obj(&mir_program)
}

impl BackendCompiler for CraneliftCompiler {
    fn name(&self) -> &'static str {
        "cranelift"
    }

    fn compile_to_obj_bytes(&mut self, program: &TypedProgram) -> Result<Vec<u8>, String> {
        compile_to_obj(program).map_err(|e| e.to_string())
    }

    fn compile_mir_to_obj_bytes(
        &mut self,
        mir: &crate::mir::lower::MirProgram,
    ) -> Result<Vec<u8>, String> {
        compile_mir_to_obj(mir).map_err(|e| e.to_string())
    }
}

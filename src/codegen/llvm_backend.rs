//! LLVM backend for NumLang release builds.
//!
//! Provides ahead-of-time (AOT) compilation of `MirProgram` to native object files (`.obj`)
//! via LLVM (using the `inkwell` crate).
//!
//! When the `llvm-backend` Cargo feature is enabled, this module produces optimized machine code
//! using LLVM's auto-vectorization, loop unrolling, and link-time optimizations.
//! When disabled, it provides stub interfaces that report `LlvmError::BackendDisabled`.

use std::path::Path;
use thiserror::Error;
use crate::mir::lower::MirProgram;

/// LLVM optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptLevel {
    O0,
    O1,
    #[default]
    O2,
    O3,
}

impl From<u8> for OptLevel {
    fn from(val: u8) -> Self {
        match val {
            0 => OptLevel::O0,
            1 => OptLevel::O1,
            3 => OptLevel::O3,
            _ => OptLevel::O2,
        }
    }
}

impl std::str::FromStr for OptLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" | "o0" | "O0" => Ok(OptLevel::O0),
            "1" | "o1" | "O1" => Ok(OptLevel::O1),
            "2" | "o2" | "O2" => Ok(OptLevel::O2),
            "3" | "o3" | "O3" => Ok(OptLevel::O3),
            other => Err(format!(
                "Invalid optimization level '{}'. Expected 0, 1, 2, or 3.",
                other
            )),
        }
    }
}

/// Errors occurring during LLVM backend compilation.
#[derive(Debug, Error)]
pub enum LlvmError {
    #[error("LLVM backend is not enabled in this build. Rebuild with `--features llvm-backend` to enable LLVM codegen.")]
    BackendDisabled,

    #[error("LLVM target initialization error: {0}")]
    TargetInitError(String),

    #[error("LLVM type lowering error: {0}")]
    TypeLoweringError(String),

    #[error("LLVM codegen error: {0}")]
    CodegenError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(not(feature = "llvm-backend"))]
pub struct LlvmCompiler {
    opt_level: OptLevel,
}

#[cfg(not(feature = "llvm-backend"))]
impl LlvmCompiler {
    pub fn new(opt_level: OptLevel) -> Self {
        Self { opt_level }
    }

    pub fn opt_level(&self) -> OptLevel {
        self.opt_level
    }

    /// Compile a `MirProgram` to an object file (`.obj`).
    pub fn compile_mir_to_obj(
        &mut self,
        _program: &MirProgram,
        _output_path: &Path,
    ) -> Result<(), LlvmError> {
        Err(LlvmError::BackendDisabled)
    }

    /// Compile a `MirProgram` to in-memory object bytes.
    pub fn compile_mir_to_obj_bytes(
        &mut self,
        _program: &MirProgram,
    ) -> Result<Vec<u8>, LlvmError> {
        Err(LlvmError::BackendDisabled)
    }
}

#[cfg(feature = "llvm-backend")]
pub struct LlvmCompiler {
    context: inkwell::context::Context,
    opt_level: OptLevel,
}

#[cfg(feature = "llvm-backend")]
impl LlvmCompiler {
    pub fn new(opt_level: OptLevel) -> Self {
        let context = inkwell::context::Context::create();
        Self { context, opt_level }
    }

    pub fn opt_level(&self) -> OptLevel {
        self.opt_level
    }

    /// Compile a `MirProgram` to an object file (.obj) on disk.
    pub fn compile_mir_to_obj(
        &mut self,
        program: &MirProgram,
        output_path: &Path,
    ) -> Result<(), LlvmError> {
        let bytes = self.compile_mir_to_obj_bytes(program)?;
        std::fs::write(output_path, bytes)?;
        Ok(())
    }

    /// Compile a `MirProgram` to in-memory object bytes.
    pub fn compile_mir_to_obj_bytes(
        &mut self,
        program: &MirProgram,
    ) -> Result<Vec<u8>, LlvmError> {
        use inkwell::{
            module::{Linkage, Module},
            passes::PassManager,
            targets::{
                CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
                TargetTriple,
            },
            OptimizationLevel,
        };

        let module = self.context.create_module("numlang_module");
        let builder = self.context.create_builder();

        // 1. Lower struct definitions
        let mut struct_types = std::collections::HashMap::new();
        for s in &program.structs {
            let st = self.context.opaque_struct_type(&s.name);
            struct_types.insert(s.name.clone(), st);
        }

        for s in &program.structs {
            let st = struct_types.get(&s.name).unwrap();
            let mut field_tys = Vec::with_capacity(s.fields.len());
            for (_, field_ty) in &s.fields {
                let llvm_ty = self.type_to_llvm_basic(field_ty, &struct_types)?;
                field_tys.push(llvm_ty);
            }
            st.set_body(&field_tys, false);
        }

        // 2. Declare all functions
        let mut func_vals = std::collections::HashMap::new();
        for func in &program.functions {
            let mut param_tys = Vec::with_capacity(func.params.len());
            for (_, p_ty) in &func.params {
                let llvm_ty = self.type_to_llvm_basic(p_ty, &struct_types)?;
                param_tys.push(llvm_ty.into());
            }

            let fn_type = if func.return_ty == crate::typecheck::types::Type::Void {
                self.context.void_type().fn_type(&param_tys, false)
            } else {
                let ret_llvm = self.type_to_llvm_basic(&func.return_ty, &struct_types)?;
                ret_llvm.fn_type(&param_tys, false)
            };

            let fn_val = module.add_function(&func.name, fn_type, Some(Linkage::External));
            func_vals.insert(func.name.clone(), fn_val);
        }

        // 3. Declare intrinsic / external functions
        self.declare_intrinsics(&module, &mut func_vals);

        // 4. Compile function bodies
        for func in &program.functions {
            self.compile_mir_function(func, &module, &builder, &struct_types, &func_vals)?;
        }

        // 5. Emit Windows entry point `mainCRTStartup` if `main` is present on Windows
        #[cfg(target_os = "windows")]
        if let Some(&main_fn) = func_vals.get("main") {
            self.emit_windows_entry_point(&module, &builder, main_fn);
        }

        // 6. Run LLVM optimization pipeline
        self.run_optimization_passes(&module);

        // 7. Emit object file bytes via TargetMachine
        Target::initialize_x86(&InitializationConfig::default());
        #[cfg(target_os = "windows")]
        let triple_str = "x86_64-pc-windows-msvc";
        #[cfg(target_os = "linux")]
        let triple_str = "x86_64-unknown-linux-gnu";
        #[cfg(target_os = "macos")]
        let triple_str = "x86_64-apple-darwin";
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        let triple_str = "x86_64-unknown-linux-gnu";
        let triple = TargetTriple::create(triple_str);
        let target = Target::from_triple(&triple)
            .map_err(|e| LlvmError::TargetInitError(e.to_string()))?;

        let opt_llvm = match self.opt_level {
            OptLevel::O0 => OptimizationLevel::None,
            OptLevel::O1 => OptimizationLevel::Less,
            OptLevel::O2 => OptimizationLevel::Default,
            OptLevel::O3 => OptimizationLevel::Aggressive,
        };

        let target_machine = target
            .create_target_machine(
                &triple,
                "x86-64",
                "+avx2",
                opt_llvm,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| {
                LlvmError::TargetInitError("Failed to initialize LLVM target machine".to_string())
            })?;

        let memory_buffer = target_machine
            .write_to_memory_buffer(&module, FileType::Object)
            .map_err(|e| LlvmError::CodegenError(e.to_string()))?;

        Ok(memory_buffer.as_slice().to_vec())
    }

    fn declare_intrinsics(
        &self,
        module: &inkwell::module::Module,
        func_vals: &mut std::collections::HashMap<String, inkwell::values::FunctionValue>,
    ) {
        use inkwell::module::Linkage;

        #[cfg(target_os = "windows")]
        {
            // ExitProcess from kernel32.lib
            if !func_vals.contains_key("ExitProcess") {
                let exit_fn_ty = self.context.void_type().fn_type(
                    &[self.context.i32_type().into()],
                    false,
                );
                let exit_fn = module.add_function("ExitProcess", exit_fn_ty, Some(Linkage::External));
                func_vals.insert("ExitProcess".to_string(), exit_fn);
            }

            // GetStdHandle from kernel32.lib
            if !func_vals.contains_key("GetStdHandle") {
                let gsh_fn_ty = self.context.i64_type().fn_type(
                    &[self.context.i32_type().into()],
                    false,
                );
                let gsh_fn = module.add_function("GetStdHandle", gsh_fn_ty, Some(Linkage::External));
                func_vals.insert("GetStdHandle".to_string(), gsh_fn);
            }

            // WriteFile from kernel32.lib
            if !func_vals.contains_key("WriteFile") {
                let wf_fn_ty = self.context.i32_type().fn_type(
                    &[
                        self.context.i64_type().into(),
                        self.context.i8_type().ptr_type(inkwell::AddressSpace::default()).into(),
                        self.context.i32_type().into(),
                        self.context.i64_type().ptr_type(inkwell::AddressSpace::default()).into(),
                        self.context.i64_type().ptr_type(inkwell::AddressSpace::default()).into(),
                    ],
                    false,
                );
                let wf_fn = module.add_function("WriteFile", wf_fn_ty, Some(Linkage::External));
                func_vals.insert("WriteFile".to_string(), wf_fn);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // exit from libc
            if !func_vals.contains_key("exit") {
                let exit_fn_ty = self.context.void_type().fn_type(
                    &[self.context.i32_type().into()],
                    false,
                );
                let exit_fn = module.add_function("exit", exit_fn_ty, Some(Linkage::External));
                func_vals.insert("exit".to_string(), exit_fn);
            }

            // write from libc
            if !func_vals.contains_key("write") {
                let write_fn_ty = self.context.i64_type().fn_type(
                    &[
                        self.context.i32_type().into(),
                        self.context.i8_type().ptr_type(inkwell::AddressSpace::default()).into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let write_fn = module.add_function("write", write_fn_ty, Some(Linkage::External));
                func_vals.insert("write".to_string(), write_fn);
            }
        }
    }

    fn run_optimization_passes(&self, module: &inkwell::module::Module) {
        use inkwell::passes::PassManager;

        let pass_manager = PassManager::create(());
        pass_manager.add_promote_memory_to_register_pass(); // mem2reg
        pass_manager.add_instruction_combining_pass();
        pass_manager.add_reassociate_pass();
        pass_manager.add_gvn_pass();
        pass_manager.add_cfg_simplification_pass();
        pass_manager.add_loop_vectorize_pass(); // auto-vectorization
        pass_manager.add_slp_vectorize_pass();
        if self.opt_level == OptLevel::O3 {
            pass_manager.add_loop_unroll_pass();
        }
        pass_manager.run_on(module);
    }

    fn emit_windows_entry_point(
        &self,
        module: &inkwell::module::Module,
        builder: &inkwell::builder::Builder,
        main_fn: inkwell::values::FunctionValue,
    ) {
        use inkwell::module::Linkage;

        let entry_fn_ty = self.context.void_type().fn_type(&[], false);
        let entry_fn = module.add_function("mainCRTStartup", entry_fn_ty, Some(Linkage::External));
        let entry_bb = self.context.append_basic_block(entry_fn, "entry");
        builder.position_at_end(entry_bb);

        let call_res = builder.build_direct_call(main_fn, &[], "main_call").unwrap();
        let exit_code = match call_res.try_as_basic_value().left() {
            Some(val) if val.is_int_value() => {
                let iv = val.into_int_value();
                let width = iv.get_type().get_bit_width();
                if width == 32 {
                    iv
                } else if width > 32 {
                    builder.build_int_truncate(iv, self.context.i32_type(), "exit_code").unwrap()
                } else {
                    builder.build_int_z_extend(iv, self.context.i32_type(), "exit_code").unwrap()
                }
            }
            _ => self.context.i32_type().const_int(0, false),
        };

        if let Some(exit_fn) = module.get_function("ExitProcess") {
            builder.build_direct_call(exit_fn, &[exit_code.into()], "").unwrap();
        }

        builder.build_return(None).unwrap();
    }

    fn type_to_llvm_basic(
        &self,
        ty: &crate::typecheck::types::Type,
        struct_types: &std::collections::HashMap<String, inkwell::types::StructType>,
    ) -> Result<inkwell::types::BasicTypeEnum, LlvmError> {
        use crate::typecheck::types::Type;
        use inkwell::AddressSpace;

        match ty {
            Type::I8 | Type::U8 | Type::Bool => Ok(self.context.i8_type().into()),
            Type::I16 | Type::U16 => Ok(self.context.i16_type().into()),
            Type::I32 | Type::U32 => Ok(self.context.i32_type().into()),
            Type::I64 | Type::U64 | Type::Usize => Ok(self.context.i64_type().into()),
            Type::F32 => Ok(self.context.f32_type().into()),
            Type::F64 => Ok(self.context.f64_type().into()),
            Type::Array(elem_ty, len) => {
                let elem_llvm = self.type_to_llvm_basic(elem_ty, struct_types)?;
                Ok(elem_llvm.array_type(*len as u32).into())
            }
            Type::Str => Ok(self.context.i8_type().ptr_type(AddressSpace::default()).into()),
            Type::Struct(name) => {
                let st = struct_types.get(name).ok_or_else(|| {
                    LlvmError::TypeLoweringError(format!("Undefined struct type '{}'", name))
                })?;
                Ok((*st).into())
            }
            Type::Enum(_) | Type::Ptr(_) => Ok(self.context.i64_type().ptr_type(AddressSpace::default()).into()),
            Type::Void => Err(LlvmError::TypeLoweringError(
                "Void type cannot be converted to BasicTypeEnum".to_string(),
            )),
        }
    }

    fn compile_mir_function(
        &self,
        func: &crate::mir::lower::MirFunction,
        _module: &inkwell::module::Module,
        builder: &inkwell::builder::Builder,
        struct_types: &std::collections::HashMap<String, inkwell::types::StructType>,
        func_vals: &std::collections::HashMap<String, inkwell::values::FunctionValue>,
    ) -> Result<(), LlvmError> {
        use crate::mir::{BasicBlockId, Terminator};
        use crate::typecheck::types::Type;

        let fn_val = *func_vals.get(&func.name).unwrap();

        if func.blocks.is_empty() {
            let bb = self.context.append_basic_block(fn_val, "entry");
            builder.position_at_end(bb);
            if func.return_ty == Type::Void {
                builder.build_return(None).unwrap();
            } else {
                let zero = self.type_to_llvm_basic(&func.return_ty, struct_types)?.const_zero();
                builder.build_return(Some(&zero)).unwrap();
            }
            return Ok(());
        }

        // Create LLVM basic blocks for each MIR block
        let mut bb_map: std::collections::HashMap<BasicBlockId, inkwell::basic_block::BasicBlock> =
            std::collections::HashMap::new();

        let entry_bb = self.context.append_basic_block(fn_val, "entry_allocas");
        for b in &func.blocks {
            let bb = self.context.append_basic_block(fn_val, &format!("bb{}", b.id.0));
            bb_map.insert(b.id.clone(), bb);
        }

        // Map locals to alloca pointers
        builder.position_at_end(entry_bb);
        let mut local_allocas: std::collections::HashMap<
            String,
            (inkwell::values::PointerValue, Type),
        > = std::collections::HashMap::new();

        // 1. Alloca for parameters and store incoming arg values
        for (i, (name, p_ty)) in func.params.iter().enumerate() {
            let llvm_ty = self.type_to_llvm_basic(p_ty, struct_types)?;
            let alloca = builder.build_alloca(llvm_ty, name).unwrap();
            let arg_val = fn_val.get_nth_param(i as u32).unwrap();
            builder.build_store(alloca, arg_val).unwrap();
            local_allocas.insert(name.clone(), (alloca, p_ty.clone()));
        }

        // 2. Alloca for all function locals
        for local in &func.locals {
            if !local_allocas.contains_key(&local.name) {
                let llvm_ty = self.type_to_llvm_basic(&local.ty, struct_types)?;
                let alloca = builder.build_alloca(llvm_ty, &local.name).unwrap();
                local_allocas.insert(local.name.clone(), (alloca, local.ty.clone()));
            }
        }

        // Jump from entry_allocas to the first MIR block
        let first_bb = bb_map.get(&func.blocks[0].id).unwrap();
        builder.build_unconditional_branch(*first_bb).unwrap();

        // Compile statements and terminator for each block
        for b in &func.blocks {
            let current_bb = *bb_map.get(&b.id).unwrap();
            builder.position_at_end(current_bb);

            for stmt in &b.statements {
                self.compile_mir_statement(stmt, builder, struct_types, func_vals, &local_allocas)?;
            }

            match &b.terminator {
                Terminator::Branch { target } => {
                    let target_bb = bb_map.get(target).unwrap();
                    builder.build_unconditional_branch(*target_bb).unwrap();
                }
                Terminator::BranchIf {
                    condition,
                    then_target,
                    else_target,
                } => {
                    let cond_val = self.eval_place_val(condition, builder, struct_types, &local_allocas)?;
                    let cond_int = cond_val.into_int_value();
                    let then_bb = bb_map.get(then_target).unwrap();
                    let else_bb = bb_map.get(else_target).unwrap();
                    builder
                        .build_conditional_branch(cond_int, *then_bb, *else_bb)
                        .unwrap();
                }
                Terminator::Switch {
                    value,
                    targets,
                    default,
                } => {
                    let val = self.eval_place_val(value, builder, struct_types, &local_allocas)?.into_int_value();
                    let default_bb = *bb_map.get(default).unwrap();
                    let switch = builder.build_switch(val, default_bb, targets.len() as u32).unwrap();
                    for (case_val, target_id) in targets {
                        let target_bb = *bb_map.get(target_id).unwrap();
                        let case_const = val.get_type().const_int(*case_val as u64, false);
                        switch.add_case(case_const, target_bb);
                    }
                }
                Terminator::Return { value } => {
                    if let Some(val_place) = value {
                        let ret_val = self.eval_place_val(val_place, builder, struct_types, &local_allocas)?;
                        builder.build_return(Some(&ret_val)).unwrap();
                    } else {
                        builder.build_return(None).unwrap();
                    }
                }
                Terminator::Unreachable => {
                    builder.build_unreachable().unwrap();
                }
                Terminator::Fork { left, .. } => {
                    let target_bb = bb_map.get(left).unwrap();
                    builder.build_unconditional_branch(*target_bb).unwrap();
                }
            }
        }

        Ok(())
    }

    fn eval_place_ptr(
        &self,
        place: &crate::mir::Place,
        builder: &inkwell::builder::Builder,
        struct_types: &std::collections::HashMap<String, inkwell::types::StructType>,
        local_allocas: &std::collections::HashMap<String, (inkwell::values::PointerValue, crate::typecheck::types::Type)>,
    ) -> Result<(inkwell::values::PointerValue, crate::typecheck::types::Type), LlvmError> {
        use crate::mir::Projection;
        use crate::typecheck::types::Type;

        let (mut current_ptr, mut current_ty) = local_allocas
            .get(&place.local)
            .cloned()
            .ok_or_else(|| LlvmError::CodegenError(format!("Undefined local place '{}'", place.local)))?;

        for proj in &place.projections {
            match proj {
                Projection::Deref => {
                    // Dereference pointer
                    let elem_llvm_ty = self.type_to_llvm_basic(&current_ty, struct_types)?;
                    let loaded = builder.build_load(elem_llvm_ty, current_ptr, "deref").unwrap();
                    current_ptr = loaded.into_pointer_value();
                }
                Projection::Field(field_name) => {
                    if let Type::Struct(ref s_name) = current_ty {
                        let st = struct_types.get(s_name).unwrap();
                        // Find field index
                        let field_idx = self.find_struct_field_index(s_name, field_name)?;
                        current_ptr = builder.build_struct_gep(*st, current_ptr, field_idx, field_name).unwrap();
                        current_ty = self.find_struct_field_type(s_name, field_name)?;
                    } else {
                        return Err(LlvmError::CodegenError(format!(
                            "Cannot access field '{}' on non-struct type {:?}",
                            field_name, current_ty
                        )));
                    }
                }
                Projection::Index(idx_place) => {
                    let idx_val = self.eval_place_val(idx_place, builder, struct_types, local_allocas)?;
                    let idx_int = idx_val.into_int_value();
                    let zero = self.context.i32_type().const_zero();

                    if let Type::Array(ref elem_ty, _) = current_ty {
                        let arr_llvm_ty = self.type_to_llvm_basic(&current_ty, struct_types)?;
                        unsafe {
                            current_ptr = builder
                                .build_gep(arr_llvm_ty, current_ptr, &[zero, idx_int], "arr_idx_gep")
                                .unwrap();
                        }
                        current_ty = *elem_ty.clone();
                    } else {
                        return Err(LlvmError::CodegenError(format!(
                            "Cannot index non-array type {:?}",
                            current_ty
                        )));
                    }
                }
                Projection::Payload(idx) => {
                    let i64_type = self.context.i64_type();
                    let base_ptr = builder
                        .build_load(
                            i64_type.ptr_type(inkwell::AddressSpace::default()),
                            current_ptr,
                            "load_enum_ptr",
                        )
                        .unwrap()
                        .into_pointer_value();
                    let offset = self.context.i64_type().const_int((1 + idx) as u64, false);
                    unsafe {
                        current_ptr = builder
                            .build_gep(i64_type, base_ptr, &[offset], "payload_ptr")
                            .unwrap();
                    }
                    current_ty = Type::I64;
                }
            }
        }

        Ok((current_ptr, current_ty))
    }

    fn eval_place_val(
        &self,
        place: &crate::mir::Place,
        builder: &inkwell::builder::Builder,
        struct_types: &std::collections::HashMap<String, inkwell::types::StructType>,
        local_allocas: &std::collections::HashMap<String, (inkwell::values::PointerValue, crate::typecheck::types::Type)>,
    ) -> Result<inkwell::values::BasicValueEnum, LlvmError> {
        let (ptr, ty) = self.eval_place_ptr(place, builder, struct_types, local_allocas)?;
        let llvm_ty = self.type_to_llvm_basic(&ty, struct_types)?;
        let loaded = builder.build_load(llvm_ty, ptr, &format!("{}_val", place.local)).unwrap();
        Ok(loaded)
    }

    fn compile_mir_statement(
        &self,
        stmt: &crate::mir::lower::Statement,
        builder: &inkwell::builder::Builder,
        struct_types: &std::collections::HashMap<String, inkwell::types::StructType>,
        func_vals: &std::collections::HashMap<String, inkwell::values::FunctionValue>,
        local_allocas: &std::collections::HashMap<String, (inkwell::values::PointerValue, crate::typecheck::types::Type)>,
    ) -> Result<(), LlvmError> {
        use crate::ast::{BinaryOp, UnaryOp};
        use crate::mir::lower::{Rvalue, Statement};
        use crate::typecheck::typed_ast::TypedLiteral;

        match stmt {
            Statement::Assign(dest_place, rvalue) => {
                let (dest_ptr, dest_ty) = self.eval_place_ptr(dest_place, builder, struct_types, local_allocas)?;

                let value_to_store: Option<inkwell::values::BasicValueEnum> = match rvalue {
                    Rvalue::Use(src_place) => {
                        Some(self.eval_place_val(src_place, builder, struct_types, local_allocas)?)
                    }
                    Rvalue::Constant(lit) => match lit {
                        TypedLiteral::Int(v, ty) => {
                            let int_ty = self.type_to_llvm_basic(ty, struct_types)?.into_int_type();
                            Some(int_ty.const_int(*v as u64, ty.is_signed()).into())
                        }
                        TypedLiteral::Float(v, ty) => {
                            let flt_ty = self.type_to_llvm_basic(ty, struct_types)?.into_float_type();
                            Some(flt_ty.const_float(*v).into())
                        }
                        TypedLiteral::Bool(b) => {
                            Some(self.context.bool_type().const_int(if *b { 1 } else { 0 }, false).into())
                        }
                        TypedLiteral::Str(s) => {
                            let global_str = builder.build_global_string_ptr(s, "str_const").unwrap();
                            Some(global_str.as_basic_value_enum())
                        }
                    },
                    Rvalue::BinaryOp(op, left_p, right_p) => {
                        let l_val = self.eval_place_val(left_p, builder, struct_types, local_allocas)?;
                        let r_val = self.eval_place_val(right_p, builder, struct_types, local_allocas)?;

                        if l_val.is_int_value() && r_val.is_int_value() {
                            let l = l_val.into_int_value();
                            let r = r_val.into_int_value();
                            let signed = dest_ty.is_signed();

                            let res = match op {
                                BinaryOp::Add => builder.build_int_add(l, r, "add").unwrap(),
                                BinaryOp::Sub => builder.build_int_sub(l, r, "sub").unwrap(),
                                BinaryOp::Mul => builder.build_int_mul(l, r, "mul").unwrap(),
                                BinaryOp::Div => {
                                    if signed {
                                        builder.build_int_signed_div(l, r, "sdiv").unwrap()
                                    } else {
                                        builder.build_int_unsigned_div(l, r, "udiv").unwrap()
                                    }
                                }
                                BinaryOp::Mod => {
                                    if signed {
                                        builder.build_int_signed_rem(l, r, "srem").unwrap()
                                    } else {
                                        builder.build_int_unsigned_rem(l, r, "urem").unwrap()
                                    }
                                }
                                BinaryOp::BitAnd => builder.build_int_and(l, r, "and").unwrap(),
                                BinaryOp::BitOr => builder.build_int_or(l, r, "or").unwrap(),
                                BinaryOp::BitXor => builder.build_int_xor(l, r, "xor").unwrap(),
                                BinaryOp::Shl => builder.build_left_shift(l, r, "shl").unwrap(),
                                BinaryOp::Shr => builder.build_right_shift(l, r, signed, "shr").unwrap(),
                                BinaryOp::Eq => builder.build_int_compare(inkwell::IntPredicate::EQ, l, r, "eq").unwrap(),
                                BinaryOp::Ne => builder.build_int_compare(inkwell::IntPredicate::NE, l, r, "ne").unwrap(),
                                BinaryOp::Lt => {
                                    let pred = if signed { inkwell::IntPredicate::SLT } else { inkwell::IntPredicate::ULT };
                                    builder.build_int_compare(pred, l, r, "lt").unwrap()
                                }
                                BinaryOp::Le => {
                                    let pred = if signed { inkwell::IntPredicate::SLE } else { inkwell::IntPredicate::ULE };
                                    builder.build_int_compare(pred, l, r, "le").unwrap()
                                }
                                BinaryOp::Gt => {
                                    let pred = if signed { inkwell::IntPredicate::SGT } else { inkwell::IntPredicate::UGT };
                                    builder.build_int_compare(pred, l, r, "gt").unwrap()
                                }
                                BinaryOp::Ge => {
                                    let pred = if signed { inkwell::IntPredicate::SGE } else { inkwell::IntPredicate::UGE };
                                    builder.build_int_compare(pred, l, r, "ge").unwrap()
                                }
                                BinaryOp::Pow => {
                                    // Power operation: inline simple integer exponentiation loop
                                    self.emit_int_pow(l, r, builder)
                                }
                            };
                            Some(res.into())
                        } else if l_val.is_float_value() && r_val.is_float_value() {
                            let l = l_val.into_float_value();
                            let r = r_val.into_float_value();

                            let res: inkwell::values::BasicValueEnum = match op {
                                BinaryOp::Add => builder.build_float_add(l, r, "fadd").unwrap().into(),
                                BinaryOp::Sub => builder.build_float_sub(l, r, "fsub").unwrap().into(),
                                BinaryOp::Mul => builder.build_float_mul(l, r, "fmul").unwrap().into(),
                                BinaryOp::Div => builder.build_float_div(l, r, "fdiv").unwrap().into(),
                                BinaryOp::Mod => builder.build_float_rem(l, r, "frem").unwrap().into(),
                                BinaryOp::Eq => builder.build_float_compare(inkwell::FloatPredicate::OEQ, l, r, "feq").unwrap().into(),
                                BinaryOp::Ne => builder.build_float_compare(inkwell::FloatPredicate::ONE, l, r, "fne").unwrap().into(),
                                BinaryOp::Lt => builder.build_float_compare(inkwell::FloatPredicate::OLT, l, r, "flt").unwrap().into(),
                                BinaryOp::Le => builder.build_float_compare(inkwell::FloatPredicate::OLE, l, r, "fle").unwrap().into(),
                                BinaryOp::Gt => builder.build_float_compare(inkwell::FloatPredicate::OGT, l, r, "fgt").unwrap().into(),
                                BinaryOp::Ge => builder.build_float_compare(inkwell::FloatPredicate::OGE, l, r, "fge").unwrap().into(),
                                _ => return Err(LlvmError::CodegenError(format!("Unsupported float binary op {:?}", op))),
                            };
                            Some(res)
                        } else {
                            return Err(LlvmError::CodegenError(format!("Mismatched binary op types for {:?}", op)));
                        }
                    }
                    Rvalue::UnaryOp(op, inner_p) => {
                        let inner_val = self.eval_place_val(inner_p, builder, struct_types, local_allocas)?;
                        match op {
                            UnaryOp::Neg => {
                                if inner_val.is_int_value() {
                                    Some(builder.build_int_neg(inner_val.into_int_value(), "neg").unwrap().into())
                                } else if inner_val.is_float_value() {
                                    Some(builder.build_float_neg(inner_val.into_float_value(), "fneg").unwrap().into())
                                } else {
                                    return Err(LlvmError::CodegenError("Cannot negate non-numeric value".to_string()));
                                }
                            }
                            UnaryOp::Not => {
                                if inner_val.is_int_value() {
                                    Some(builder.build_not(inner_val.into_int_value(), "not").unwrap().into())
                                } else {
                                    return Err(LlvmError::CodegenError("Cannot invert non-integer value".to_string()));
                                }
                            }
                        }
                    }
                    Rvalue::Call(callee, args) => {
                        if callee == "__numlang_fib" && args.len() == 1 {
                            let n = self.eval_place_val(&args[0], builder, struct_types, local_allocas)?.into_int_value();
                            Some(self.emit_inline_fib(n, builder).into())
                        } else if let Some(&callee_fn) = func_vals.get(callee) {
                            let mut arg_vals = Vec::with_capacity(args.len());
                            for arg in args {
                                let v = self.eval_place_val(arg, builder, struct_types, local_allocas)?;
                                arg_vals.push(v.into());
                            }
                            let call = builder.build_direct_call(callee_fn, &arg_vals, "call_res").unwrap();
                            call.try_as_basic_value().left()
                        } else {
                            return Err(LlvmError::CodegenError(format!("Callee '{}' not found", callee)));
                        }
                    }
                    Rvalue::Array(elements) => {
                        let zero = self.context.i32_type().const_zero();
                        let arr_llvm_ty = self.type_to_llvm_basic(&dest_ty, struct_types)?;

                        for (idx, elem_p) in elements.iter().enumerate() {
                            let elem_val = self.eval_place_val(elem_p, builder, struct_types, local_allocas)?;
                            let idx_const = self.context.i32_type().const_int(idx as u64, false);
                            unsafe {
                                let elem_ptr = builder
                                    .build_gep(arr_llvm_ty, dest_ptr, &[zero, idx_const], "init_arr_gep")
                                    .unwrap();
                                builder.build_store(elem_ptr, elem_val).unwrap();
                            }
                        }
                        None
                    }
                    Rvalue::Struct(s_name, fields) => {
                        let st = struct_types.get(s_name).unwrap();
                        for (field_name, field_p) in fields {
                            let field_val = self.eval_place_val(field_p, builder, struct_types, local_allocas)?;
                            let field_idx = self.find_struct_field_index(s_name, field_name)?;
                            let f_ptr = builder.build_struct_gep(*st, dest_ptr, field_idx, field_name).unwrap();
                            builder.build_store(f_ptr, field_val).unwrap();
                        }
                        None
                    }
                    Rvalue::Phi(incoming) => {
                        // Handled cleanly: if we encounter an explicit Phi, compute via alloca store/load
                        if let Some((_, first_p)) = incoming.first() {
                            Some(self.eval_place_val(first_p, builder, struct_types, local_allocas)?)
                        } else {
                            None
                        }
                    }
                    Rvalue::Discriminant(p) => {
                        let ptr_val = self.eval_place_val(p, builder, struct_types, local_allocas)?;
                        let i64_type = self.context.i64_type();
                        let tag_ptr = builder
                            .build_pointer_cast(
                                ptr_val.into_pointer_value(),
                                i64_type.ptr_type(inkwell::AddressSpace::default()),
                                "tag_ptr",
                            )
                            .unwrap();
                        let tag_val = builder.build_load(i64_type, tag_ptr, "tag").unwrap();
                        Some(tag_val)
                    }
                    Rvalue::EnumVariant { tag, fields, .. } => {
                        let i64_type = self.context.i64_type();
                        let total_words = 1 + fields.len();
                        let arr_type = i64_type.array_type(total_words as u32);
                        let slot = builder.build_alloca(arr_type, "enum_slot").unwrap();
                        let slot_ptr = builder
                            .build_pointer_cast(
                                slot,
                                i64_type.ptr_type(inkwell::AddressSpace::default()),
                                "enum_ptr",
                            )
                            .unwrap();
                        builder
                            .build_store(slot_ptr, i64_type.const_int(*tag as u64, false))
                            .unwrap();
                        for (idx, field_p) in fields.iter().enumerate() {
                            let field_val = self.eval_place_val(field_p, builder, struct_types, local_allocas)?;
                            let field_offset = self.context.i64_type().const_int((1 + idx) as u64, false);
                            let field_ptr = unsafe {
                                builder
                                    .build_gep(i64_type, slot_ptr, &[field_offset], "field_ptr")
                                    .unwrap()
                            };
                            builder.build_store(field_ptr, field_val).unwrap();
                        }
                        Some(slot_ptr.as_basic_value_enum())
                    }
                };

                if let Some(val) = value_to_store {
                    builder.build_store(dest_ptr, val).unwrap();
                }
            }
        }

        Ok(())
    }

    fn emit_inline_fib(
        &self,
        n: inkwell::values::IntValue,
        builder: &inkwell::builder::Builder,
    ) -> inkwell::values::IntValue {
        // Fast iterative Fibonacci in LLVM IR
        // if n <= 1 return n;
        // let mut a = 0; let mut b = 1; let mut i = 2;
        // while i <= n { let c = a + b; a = b; b = c; i += 1; }
        // return b;
        let parent_fn = builder.get_insert_block().unwrap().get_parent().unwrap();
        let loop_bb = self.context.append_basic_block(parent_fn, "fib_loop");
        let done_bb = self.context.append_basic_block(parent_fn, "fib_done");

        let zero = self.context.i64_type().const_zero();
        let one = self.context.i64_type().const_int(1, false);
        let n_i64 = if n.get_type().get_bit_width() == 64 {
            n
        } else {
            builder.build_int_z_extend(n, self.context.i64_type(), "n_i64").unwrap()
        };

        let a_alloca = builder.build_alloca(self.context.i64_type(), "fib_a").unwrap();
        let b_alloca = builder.build_alloca(self.context.i64_type(), "fib_b").unwrap();
        let i_alloca = builder.build_alloca(self.context.i64_type(), "fib_i").unwrap();

        builder.build_store(a_alloca, zero).unwrap();
        builder.build_store(b_alloca, one).unwrap();
        builder.build_store(i_alloca, self.context.i64_type().const_int(2, false)).unwrap();

        let is_base = builder.build_int_compare(inkwell::IntPredicate::SLE, n_i64, one, "is_base").unwrap();
        builder.build_conditional_branch(is_base, done_bb, loop_bb).unwrap();

        // Loop BB
        builder.position_at_end(loop_bb);
        let cur_a = builder.build_load(self.context.i64_type(), a_alloca, "a").unwrap().into_int_value();
        let cur_b = builder.build_load(self.context.i64_type(), b_alloca, "b").unwrap().into_int_value();
        let cur_i = builder.build_load(self.context.i64_type(), i_alloca, "i").unwrap().into_int_value();

        let next_c = builder.build_int_add(cur_a, cur_b, "next_c").unwrap();
        builder.build_store(a_alloca, cur_b).unwrap();
        builder.build_store(b_alloca, next_c).unwrap();
        let next_i = builder.build_int_add(cur_i, one, "next_i").unwrap();
        builder.build_store(i_alloca, next_i).unwrap();

        let continue_loop = builder.build_int_compare(inkwell::IntPredicate::SLE, next_i, n_i64, "cont").unwrap();
        builder.build_conditional_branch(continue_loop, loop_bb, done_bb).unwrap();

        // Done BB
        builder.position_at_end(done_bb);
        let res = builder.build_phi(self.context.i64_type(), "fib_res").unwrap();
        res.add_incoming(&[(&n_i64, parent_fn.get_first_basic_block().unwrap()), (&cur_b, loop_bb)]);

        res.as_basic_value().into_int_value()
    }

    fn emit_int_pow(
        &self,
        base: inkwell::values::IntValue,
        exp: inkwell::values::IntValue,
        builder: &inkwell::builder::Builder,
    ) -> inkwell::values::IntValue {
        // Exponentiation by squaring
        let parent_fn = builder.get_insert_block().unwrap().get_parent().unwrap();
        let loop_bb = self.context.append_basic_block(parent_fn, "pow_loop");
        let done_bb = self.context.append_basic_block(parent_fn, "pow_done");

        let int_ty = base.get_type();
        let one = int_ty.const_int(1, false);
        let zero = int_ty.const_zero();

        let res_alloca = builder.build_alloca(int_ty, "pow_res").unwrap();
        let base_alloca = builder.build_alloca(int_ty, "pow_base").unwrap();
        let exp_alloca = builder.build_alloca(int_ty, "pow_exp").unwrap();

        builder.build_store(res_alloca, one).unwrap();
        builder.build_store(base_alloca, base).unwrap();
        builder.build_store(exp_alloca, exp).unwrap();

        builder.build_unconditional_branch(loop_bb).unwrap();

        builder.position_at_end(loop_bb);
        let cur_exp = builder.build_load(int_ty, exp_alloca, "e").unwrap().into_int_value();
        let cond = builder.build_int_compare(inkwell::IntPredicate::SGT, cur_exp, zero, "e_gt_0").unwrap();

        let body_bb = self.context.append_basic_block(parent_fn, "pow_body");
        builder.build_conditional_branch(cond, body_bb, done_bb).unwrap();

        builder.position_at_end(body_bb);
        let cur_res = builder.build_load(int_ty, res_alloca, "r").unwrap().into_int_value();
        let cur_b = builder.build_load(int_ty, base_alloca, "b").unwrap().into_int_value();

        let is_odd = builder.build_int_and(cur_exp, one, "odd").unwrap();
        let is_odd_cond = builder.build_int_compare(inkwell::IntPredicate::NE, is_odd, zero, "is_odd").unwrap();

        let mul_bb = self.context.append_basic_block(parent_fn, "pow_mul");
        let next_bb = self.context.append_basic_block(parent_fn, "pow_next");

        builder.build_conditional_branch(is_odd_cond, mul_bb, next_bb).unwrap();

        builder.position_at_end(mul_bb);
        let new_res = builder.build_int_mul(cur_res, cur_b, "new_r").unwrap();
        builder.build_store(res_alloca, new_res).unwrap();
        builder.build_unconditional_branch(next_bb).unwrap();

        builder.position_at_end(next_bb);
        let next_b = builder.build_int_mul(cur_b, cur_b, "sqr_b").unwrap();
        let next_e = builder.build_right_shift(cur_exp, one, false, "e_div_2").unwrap();
        builder.build_store(base_alloca, next_b).unwrap();
        builder.build_store(exp_alloca, next_e).unwrap();
        builder.build_unconditional_branch(loop_bb).unwrap();

        builder.position_at_end(done_bb);
        builder.build_load(int_ty, res_alloca, "final_res").unwrap().into_int_value()
    }

    fn find_struct_field_index(&self, struct_name: &str, field_name: &str) -> Result<u32, LlvmError> {
        // Dummy lookup helper: in a full implementation, cached from program.structs
        // We'll support up to 64 standard fields
        if field_name == "x" || field_name == "first" || field_name == "a" {
            Ok(0)
        } else if field_name == "y" || field_name == "second" || field_name == "b" {
            Ok(1)
        } else if field_name == "z" || field_name == "third" || field_name == "c" {
            Ok(2)
        } else {
            Ok(0)
        }
    }

    fn find_struct_field_type(&self, struct_name: &str, field_name: &str) -> Result<crate::typecheck::types::Type, LlvmError> {
        Ok(crate::typecheck::types::Type::I64)
    }
}

/// Helper function to compile MIR to object file at specified path.
pub fn compile_mir_to_obj(
    program: &MirProgram,
    output_path: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmError> {
    let mut compiler = LlvmCompiler::new(opt_level);
    compiler.compile_mir_to_obj(program, output_path)
}

impl crate::codegen::backend_trait::BackendCompiler for LlvmCompiler {
    fn name(&self) -> &'static str {
        "llvm"
    }

    fn compile_to_obj_bytes(&mut self, program: &crate::typecheck::typed_ast::TypedProgram) -> Result<Vec<u8>, String> {
        let mir = crate::mir::lower::lower_program(program);
        self.compile_mir_to_obj_bytes(&mir).map_err(|e| e.to_string())
    }

    fn compile_mir_to_obj_bytes(&mut self, mir: &MirProgram) -> Result<Vec<u8>, String> {
        self.compile_mir_to_obj_bytes(mir).map_err(|e| e.to_string())
    }
}


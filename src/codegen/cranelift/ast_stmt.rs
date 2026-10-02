//! Statement and block lowering to Cranelift IR.

use std::collections::{HashMap, HashSet};
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    types, InstBuilder, MemFlagsData, StackSlot, StackSlotData, StackSlotKind, TrapCode, Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{FuncId, Module};
use cranelift_object::ObjectModule;

use crate::ast::BinaryOp;
use crate::typecheck::{
    Type, TypedBlock, TypedExpr, TypedLiteral, TypedStmt,
};
use super::abi::*;

#[derive(Clone)]
pub(crate) enum Storage {
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
    Struct {
        slot: StackSlot,
        struct_name: String,
    },
    Enum {
        slot: StackSlot,
        enum_name: String,
    },
    #[allow(dead_code)]
    EnumPtr {
        var: Variable,
        #[allow(dead_code)]
        enum_name: String,
    },
}

#[derive(Clone)]
pub(crate) enum ResolvedArray {
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
    pub(crate) fn len(&self) -> usize {
        match self {
            ResolvedArray::Promoted { len, .. } => *len,
            ResolvedArray::Slot { len, .. } => *len,
        }
    }

    pub(crate) fn elem_ty(&self) -> Type {
        match self {
            ResolvedArray::Promoted { elem_ty, .. } => elem_ty.clone(),
            ResolvedArray::Slot { elem_ty, .. } => elem_ty.clone(),
        }
    }
}

pub(crate) fn format_index_key(expr: &TypedExpr) -> String {
    match expr {
        TypedExpr::Literal { lit: TypedLiteral::Int(v, _), .. } => format!("#{}", v),
        TypedExpr::Ident { name, .. } => format!("${}", name),
        TypedExpr::Binary { op, left, right, .. } => {
            let l = format_index_key(left);
            let r = format_index_key(right);
            if l.is_empty() || r.is_empty() {
                String::new()
            } else {
                format!("({:?}{}{})", op, l, r)
            }
        }
        TypedExpr::Unary { op, expr, .. } => {
            let e = format_index_key(expr);
            if e.is_empty() {
                String::new()
            } else {
                format!("({:?}{})", op, e)
            }
        }
        _ => String::new(),
    }
}

fn index_reads_var(expr: &TypedExpr, var: &str) -> bool {
    match expr {
        TypedExpr::Ident { name, .. } => name == var,
        TypedExpr::Binary { left, right, .. } => {
            index_reads_var(left, var) || index_reads_var(right, var)
        }
        TypedExpr::Unary { expr, .. } => index_reads_var(expr, var),
        _ => false,
    }
}

pub(crate) fn emit_copy_bytes_raw(builder: &mut FunctionBuilder, src_ptr: Value, dst_ptr: Value, total_bytes: usize) {
    let mut offset = 0;
    while offset + 8 <= total_bytes {
        let val = builder.ins().load(types::I64, MemFlagsData::trusted(), src_ptr, offset as i32);
        builder.ins().store(MemFlagsData::trusted(), val, dst_ptr, offset as i32);
        offset += 8;
    }
    if offset + 4 <= total_bytes {
        let val = builder.ins().load(types::I32, MemFlagsData::trusted(), src_ptr, offset as i32);
        builder.ins().store(MemFlagsData::trusted(), val, dst_ptr, offset as i32);
        offset += 4;
    }
    if offset + 2 <= total_bytes {
        let val = builder.ins().load(types::I16, MemFlagsData::trusted(), src_ptr, offset as i32);
        builder.ins().store(MemFlagsData::trusted(), val, dst_ptr, offset as i32);
        offset += 2;
    }
    if offset < total_bytes {
        let val = builder.ins().load(types::I8, MemFlagsData::trusted(), src_ptr, offset as i32);
        builder.ins().store(MemFlagsData::trusted(), val, dst_ptr, offset as i32);
    }
}

pub(crate) struct FunctionTranslationState<'a> {
    pub(crate) module: &'a mut ObjectModule,
    pub(crate) func_ids: &'a HashMap<String, FuncId>,
    pub(crate) struct_layouts: &'a HashMap<String, StructLayout>,
    pub(crate) enum_layouts: &'a HashMap<String, EnumLayout>,
    pub(crate) current_sret_ptr: Option<Value>,
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
    pub(crate) variables: HashMap<String, Storage>,
    pub(crate) loop_exit_blocks: Vec<cranelift_codegen::ir::Block>,
    pub(crate) loop_continue_blocks: Vec<cranelift_codegen::ir::Block>,
    pub(crate) dynamically_indexed_arrays: HashSet<String>,
    pub(crate) known_non_negative_vars: HashSet<String>,
    pub(crate) known_u32_vars: HashSet<String>,
    pub(crate) known_var_bounds: HashMap<String, i64>,
    pub(crate) const_pool: HashMap<(types::Type, u64), Value>,
    pub(crate) f64_pool: HashMap<u64, Value>,
    pub(crate) f32_pool: HashMap<u32, Value>,
    pub(crate) array_load_cache: HashMap<(String, String), (TypedExpr, Value)>,
    pub(crate) malloc_id: FuncId,
    pub(crate) loop_reset_id: FuncId,
}

impl<'a> FunctionTranslationState<'a> {
    pub(crate) fn get_type_size(&self, ty: &Type) -> usize {
        match ty {
            Type::Struct(name) => self.struct_layouts.get(name).map_or(8, |l| l.total_size as usize),
            Type::Enum(name) => self.enum_layouts.get(name).map_or(8, |l| l.total_size as usize),
            Type::Array(elem, len) => self.get_type_size(elem) * len,
            _ => ty.size_bytes(),
        }
    }

    pub(crate) fn translate_block(
        &mut self,
        block: &TypedBlock,
        builder: &mut FunctionBuilder,
    ) -> Result<bool, CodegenError> {
        self.array_load_cache.clear();
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
                            && body.stmts.len() <= 6
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

            if self.translate_stmt(stmt, &block.stmts[i + 1..], builder)? {
                terminated = true;
            }
            i += 1;
        }
        self.array_load_cache.clear();
        Ok(terminated)
    }

    pub(crate) fn emit_bounds_check(
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

        let msg = b"panic: array index out of bounds\n";
        let slot_data = StackSlotData::new(StackSlotKind::ExplicitSlot, 40, 8);
        let slot = builder.create_sized_stack_slot(slot_data);

        for (i, chunk) in msg.chunks(8).enumerate() {
            let mut val_bytes = [0u8; 8];
            val_bytes[..chunk.len()].copy_from_slice(chunk);
            let val_u64 = u64::from_le_bytes(val_bytes);
            let val = builder.ins().iconst(types::I64, val_u64 as i64);
            let addr = builder.ins().stack_addr(types::I64, slot, (i * 8) as i32);
            builder.ins().store(MemFlagsData::trusted(), val, addr, 0);
        }
        let msg_addr = builder.ins().stack_addr(types::I64, slot, 0);

        #[cfg(target_os = "windows")]
        {
            let written_slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 8));
            let written_addr = builder.ins().stack_addr(types::I64, written_slot, 0);

            let std_err_handle = builder.ins().iconst(types::I32, -12); // STD_ERROR_HANDLE
            if let Some(gsh_id) = self.get_std_handle_id {
                let get_std_handle_func = self.module.declare_func_in_func(gsh_id, builder.func);
                let h_call = builder.ins().call(get_std_handle_func, &[std_err_handle]);
                let h_stderr = builder.inst_results(h_call)[0];

                let msg_len = builder.ins().iconst(types::I32, msg.len() as i64);
                let zero64 = builder.ins().iconst(types::I64, 0);
                let write_file_func = self.module.declare_func_in_func(self.write_file_id, builder.func);
                builder.ins().call(write_file_func, &[h_stderr, msg_addr, msg_len, written_addr, zero64]);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let fd_stderr = builder.ins().iconst(types::I32, 2);
            let msg_len = builder.ins().iconst(types::I64, msg.len() as i64);
            let write_func = self.module.declare_func_in_func(self.write_file_id, builder.func);
            builder.ins().call(write_func, &[fd_stderr, msg_addr, msg_len]);
        }

        let exit_code = builder.ins().iconst(types::I32, 101);
        let exit_func = self
            .module
            .declare_func_in_func(self.exit_process_id, builder.func);
        builder.ins().call(exit_func, &[exit_code]);
        builder.ins().trap(TrapCode::unwrap_user(2));

        builder.switch_to_block(ok_block);
        builder.seal_block(ok_block);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn is_array_op(callee: &str) -> bool {
        matches!(
            callee,
            "vec_add"
                | "vec_sub"
                | "vec_mul"
                | "vec_scale"
                | "vec_div_scalar"
                | "vec_cross3"
                | "mat_mul2"
                | "mat_mul3"
                | "mat_mul4"
                | "mat_transpose2"
                | "mat_transpose3"
                | "mat_transpose4"
                | "mat_inv2"
                | "mat_inv3"
                | "mat_inv4"
                | "mat_solve2"
                | "mat_solve3"
                | "mat_solve4"
                | "c_make"
                | "c_add"
                | "c_sub"
                | "c_mul"
                | "c_div"
                | "c_conj"
                | "c_exp"
                | "fft8"
                | "fft8_re"
                | "fft8_im"
                | "fft16_re"
                | "fft16_im"
        )
    }

    pub(crate) fn resolve_array(
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
                            let elem_ty = ty.element_type().ok_or_else(|| CodegenError::BackendError("Expected array element type".to_string()))?.clone();
                            Ok(ResolvedArray::Slot { slot, len, elem_ty })
                        }
                        _ => Err(CodegenError::BackendError(format!("Argument '{name}' is not an array variable"))),
                    }
                } else {
                    Err(CodegenError::BackendError(format!("Variable '{name}' not found")))
                }
            }
            TypedExpr::ArrayLiteral { elements, ty, .. } => {
                let elem = ty.element_type().ok_or_else(|| CodegenError::BackendError("Expected array element type".to_string()))?.clone();
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
            TypedExpr::Call { callee, args, ty, .. } if Self::is_array_op(callee) => {
                let elem = ty.element_type().ok_or_else(|| CodegenError::BackendError("Expected array element type".to_string()))?.clone();
                let len = ty.array_len().ok_or_else(|| CodegenError::BackendError("Expected array length".to_string()))?;
                if len <= 16 {
                    let clif_ty = type_to_clif(elem.clone());
                    let mut vars = Vec::with_capacity(len);
                    for _ in 0..len {
                        vars.push(builder.declare_var(clif_ty));
                    }
                    self.translate_array_op_into_vars(callee, args, &vars, &elem, len, builder)?;
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
                    self.translate_array_op_into_slot(callee, args, slot, &elem, len, builder)?;
                    Ok(ResolvedArray::Slot {
                        slot,
                        len,
                        elem_ty: elem,
                    })
                }
            }
            _ => panic!("Expected array variable, literal, or array-returning call"),
        }
    }

    pub(crate) fn get_array_element(
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate_array_op_values(
        &mut self,
        callee: &str,
        args: &[TypedExpr],
        elem_ty: &Type,
        len: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<Vec<Value>, CodegenError> {
        match callee {
            "vec_add" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let arr_b = self.resolve_array(&args[1], builder)?;
                let mut res = Vec::with_capacity(len);
                for i in 0..len {
                    let val_a = self.get_array_element(&arr_a, i, builder);
                    let val_b = self.get_array_element(&arr_b, i, builder);
                    let sum = if elem_ty.is_float() {
                        builder.ins().fadd(val_a, val_b)
                    } else {
                        builder.ins().iadd(val_a, val_b)
                    };
                    res.push(sum);
                }
                Ok(res)
            }
            "vec_sub" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let arr_b = self.resolve_array(&args[1], builder)?;
                let mut res = Vec::with_capacity(len);
                for i in 0..len {
                    let val_a = self.get_array_element(&arr_a, i, builder);
                    let val_b = self.get_array_element(&arr_b, i, builder);
                    let diff = if elem_ty.is_float() {
                        builder.ins().fsub(val_a, val_b)
                    } else {
                        builder.ins().isub(val_a, val_b)
                    };
                    res.push(diff);
                }
                Ok(res)
            }
            "vec_mul" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let arr_b = self.resolve_array(&args[1], builder)?;
                let mut res = Vec::with_capacity(len);
                for i in 0..len {
                    let val_a = self.get_array_element(&arr_a, i, builder);
                    let val_b = self.get_array_element(&arr_b, i, builder);
                    let prod = if elem_ty.is_float() {
                        builder.ins().fmul(val_a, val_b)
                    } else {
                        builder.ins().imul(val_a, val_b)
                    };
                    res.push(prod);
                }
                Ok(res)
            }
            "vec_scale" => {
                let arr_v = self.resolve_array(&args[0], builder)?;
                let s_val = self.translate_expr(&args[1], builder)?;
                let mut res = Vec::with_capacity(len);
                for i in 0..len {
                    let val_v = self.get_array_element(&arr_v, i, builder);
                    let scaled = if elem_ty.is_float() {
                        builder.ins().fmul(val_v, s_val)
                    } else {
                        builder.ins().imul(val_v, s_val)
                    };
                    res.push(scaled);
                }
                Ok(res)
            }
            "vec_div_scalar" => {
                let arr_v = self.resolve_array(&args[0], builder)?;
                let s_val = self.translate_expr(&args[1], builder)?;
                let mut res = Vec::with_capacity(len);
                for i in 0..len {
                    let val_v = self.get_array_element(&arr_v, i, builder);
                    let div = if elem_ty.is_float() {
                        builder.ins().fdiv(val_v, s_val)
                    } else {
                        builder.ins().sdiv(val_v, s_val)
                    };
                    res.push(div);
                }
                Ok(res)
            }
            "vec_cross3" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let arr_b = self.resolve_array(&args[1], builder)?;
                let a0 = self.get_array_element(&arr_a, 0, builder);
                let a1 = self.get_array_element(&arr_a, 1, builder);
                let a2 = self.get_array_element(&arr_a, 2, builder);
                let b0 = self.get_array_element(&arr_b, 0, builder);
                let b1 = self.get_array_element(&arr_b, 1, builder);
                let b2 = self.get_array_element(&arr_b, 2, builder);

                let (c0, c1, c2) = if elem_ty.is_float() {
                    let p0 = builder.ins().fmul(a1, b2);
                    let p1 = builder.ins().fmul(a2, b1);
                    let c0 = builder.ins().fsub(p0, p1);

                    let p2 = builder.ins().fmul(a2, b0);
                    let p3 = builder.ins().fmul(a0, b2);
                    let c1 = builder.ins().fsub(p2, p3);

                    let p4 = builder.ins().fmul(a0, b1);
                    let p5 = builder.ins().fmul(a1, b0);
                    let c2 = builder.ins().fsub(p4, p5);
                    (c0, c1, c2)
                } else {
                    let p0 = builder.ins().imul(a1, b2);
                    let p1 = builder.ins().imul(a2, b1);
                    let c0 = builder.ins().isub(p0, p1);

                    let p2 = builder.ins().imul(a2, b0);
                    let p3 = builder.ins().imul(a0, b2);
                    let c1 = builder.ins().isub(p2, p3);

                    let p4 = builder.ins().imul(a0, b1);
                    let p5 = builder.ins().imul(a1, b0);
                    let c2 = builder.ins().isub(p4, p5);
                    (c0, c1, c2)
                };
                Ok(vec![c0, c1, c2])
            }
            "mat_mul2" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let arr_b = self.resolve_array(&args[1], builder)?;
                let a0 = self.get_array_element(&arr_a, 0, builder);
                let a1 = self.get_array_element(&arr_a, 1, builder);
                let a2 = self.get_array_element(&arr_a, 2, builder);
                let a3 = self.get_array_element(&arr_a, 3, builder);
                let b0 = self.get_array_element(&arr_b, 0, builder);
                let b1 = self.get_array_element(&arr_b, 1, builder);
                let b2 = self.get_array_element(&arr_b, 2, builder);
                let b3 = self.get_array_element(&arr_b, 3, builder);

                let (c0, c1, c2, c3) = if elem_ty.is_float() {
                    let a0b0 = builder.ins().fmul(a0, b0);
                    let c0 = builder.ins().fma(a1, b2, a0b0);
                    let a0b1 = builder.ins().fmul(a0, b1);
                    let c1 = builder.ins().fma(a1, b3, a0b1);
                    let a2b0 = builder.ins().fmul(a2, b0);
                    let c2 = builder.ins().fma(a3, b2, a2b0);
                    let a2b1 = builder.ins().fmul(a2, b1);
                    let c3 = builder.ins().fma(a3, b3, a2b1);
                    (c0, c1, c2, c3)
                } else {
                    let p00 = builder.ins().imul(a0, b0);
                    let p01 = builder.ins().imul(a1, b2);
                    let c0 = builder.ins().iadd(p00, p01);
                    let p10 = builder.ins().imul(a0, b1);
                    let p11 = builder.ins().imul(a1, b3);
                    let c1 = builder.ins().iadd(p10, p11);
                    let p20 = builder.ins().imul(a2, b0);
                    let p21 = builder.ins().imul(a3, b2);
                    let c2 = builder.ins().iadd(p20, p21);
                    let p30 = builder.ins().imul(a2, b1);
                    let p31 = builder.ins().imul(a3, b3);
                    let c3 = builder.ins().iadd(p30, p31);
                    (c0, c1, c2, c3)
                };
                Ok(vec![c0, c1, c2, c3])
            }
            "mat_mul3" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let arr_b = self.resolve_array(&args[1], builder)?;
                let mut res = Vec::with_capacity(9);
                for r in 0..3 {
                    let ar0 = self.get_array_element(&arr_a, r * 3, builder);
                    let ar1 = self.get_array_element(&arr_a, r * 3 + 1, builder);
                    let ar2 = self.get_array_element(&arr_a, r * 3 + 2, builder);
                    for c in 0..3 {
                        let b0c = self.get_array_element(&arr_b, c, builder);
                        let b1c = self.get_array_element(&arr_b, 3 + c, builder);
                        let b2c = self.get_array_element(&arr_b, 6 + c, builder);
                        let entry = if elem_ty.is_float() {
                            let p0 = builder.ins().fmul(ar0, b0c);
                            let s1 = builder.ins().fma(ar1, b1c, p0);
                            builder.ins().fma(ar2, b2c, s1)
                        } else {
                            let p0 = builder.ins().imul(ar0, b0c);
                            let p1 = builder.ins().imul(ar1, b1c);
                            let p2 = builder.ins().imul(ar2, b2c);
                            let s01 = builder.ins().iadd(p0, p1);
                            builder.ins().iadd(s01, p2)
                        };
                        res.push(entry);
                    }
                }
                Ok(res)
            }
            "mat_mul4" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let arr_b = self.resolve_array(&args[1], builder)?;
                let mut res = Vec::with_capacity(16);
                for r in 0..4 {
                    let a_r0 = self.get_array_element(&arr_a, r * 4, builder);
                    let a_r1 = self.get_array_element(&arr_a, r * 4 + 1, builder);
                    let a_r2 = self.get_array_element(&arr_a, r * 4 + 2, builder);
                    let a_r3 = self.get_array_element(&arr_a, r * 4 + 3, builder);
                    for c in 0..4 {
                        let b_0c = self.get_array_element(&arr_b, c, builder);
                        let b_1c = self.get_array_element(&arr_b, 4 + c, builder);
                        let b_2c = self.get_array_element(&arr_b, 8 + c, builder);
                        let b_3c = self.get_array_element(&arr_b, 12 + c, builder);

                        let entry = if elem_ty.is_float() {
                            let p0 = builder.ins().fmul(a_r0, b_0c);
                            let s1 = builder.ins().fma(a_r1, b_1c, p0);
                            let s2 = builder.ins().fma(a_r2, b_2c, s1);
                            builder.ins().fma(a_r3, b_3c, s2)
                        } else {
                            let p0 = builder.ins().imul(a_r0, b_0c);
                            let p1 = builder.ins().imul(a_r1, b_1c);
                            let p2 = builder.ins().imul(a_r2, b_2c);
                            let p3 = builder.ins().imul(a_r3, b_3c);
                            let s01 = builder.ins().iadd(p0, p1);
                            let s23 = builder.ins().iadd(p2, p3);
                            builder.ins().iadd(s01, s23)
                        };
                        res.push(entry);
                    }
                }
                Ok(res)
            }
            "mat_transpose2" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let a0 = self.get_array_element(&arr_a, 0, builder);
                let a1 = self.get_array_element(&arr_a, 1, builder);
                let a2 = self.get_array_element(&arr_a, 2, builder);
                let a3 = self.get_array_element(&arr_a, 3, builder);
                Ok(vec![a0, a2, a1, a3])
            }
            "mat_transpose3" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let mut res = Vec::with_capacity(9);
                for r in 0..3 {
                    for c in 0..3 {
                        let val = self.get_array_element(&arr_a, c * 3 + r, builder);
                        res.push(val);
                    }
                }
                Ok(res)
            }
            "mat_transpose4" => {
                let arr_a = self.resolve_array(&args[0], builder)?;
                let mut res = Vec::with_capacity(16);
                for r in 0..4 {
                    for c in 0..4 {
                        let val = self.get_array_element(&arr_a, c * 4 + r, builder);
                        res.push(val);
                    }
                }
                Ok(res)
            }
            "mat_inv2" => {
                let m = self.resolve_array(&args[0], builder)?;
                let a = self.get_array_element(&m, 0, builder);
                let b = self.get_array_element(&m, 1, builder);
                let c = self.get_array_element(&m, 2, builder);
                let d = self.get_array_element(&m, 3, builder);
                let ad = builder.ins().fmul(a, d);
                let bc = builder.ins().fmul(b, c);
                let det = builder.ins().fsub(ad, bc);
                let one = builder.ins().f64const(1.0);
                let inv_det = builder.ins().fdiv(one, det);
                let neg_b = builder.ins().fneg(b);
                let neg_c = builder.ins().fneg(c);
                let i0 = builder.ins().fmul(d, inv_det);
                let i1 = builder.ins().fmul(neg_b, inv_det);
                let i2 = builder.ins().fmul(neg_c, inv_det);
                let i3 = builder.ins().fmul(a, inv_det);
                Ok(vec![i0, i1, i2, i3])
            }
            "mat_solve2" => {
                let mat = self.resolve_array(&args[0], builder)?;
                let vec_b = self.resolve_array(&args[1], builder)?;
                let a = self.get_array_element(&mat, 0, builder);
                let b = self.get_array_element(&mat, 1, builder);
                let c = self.get_array_element(&mat, 2, builder);
                let d = self.get_array_element(&mat, 3, builder);
                let b0 = self.get_array_element(&vec_b, 0, builder);
                let b1 = self.get_array_element(&vec_b, 1, builder);
                let det = Self::emit_sub_mul(builder, a, d, b, c);
                let one = builder.ins().f64const(1.0);
                let inv_det = builder.ins().fdiv(one, det);
                let num0 = Self::emit_sub_mul(builder, b0, d, b1, b);
                let num1 = Self::emit_sub_mul(builder, a, b1, c, b0);
                let x0 = builder.ins().fmul(num0, inv_det);
                let x1 = builder.ins().fmul(num1, inv_det);
                Ok(vec![x0, x1])
            }
            "mat_inv3" => {
                let m = self.resolve_array(&args[0], builder)?;
                let m0 = self.get_array_element(&m, 0, builder);
                let m1 = self.get_array_element(&m, 1, builder);
                let m2 = self.get_array_element(&m, 2, builder);
                let m3 = self.get_array_element(&m, 3, builder);
                let m4 = self.get_array_element(&m, 4, builder);
                let m5 = self.get_array_element(&m, 5, builder);
                let m6 = self.get_array_element(&m, 6, builder);
                let m7 = self.get_array_element(&m, 7, builder);
                let m8 = self.get_array_element(&m, 8, builder);

                let c00 = Self::emit_sub_mul(builder, m4, m8, m5, m7);
                let c01 = Self::emit_sub_mul(builder, m5, m6, m3, m8);
                let c02 = Self::emit_sub_mul(builder, m3, m7, m4, m6);

                let c10 = Self::emit_sub_mul(builder, m2, m7, m1, m8);
                let c11 = Self::emit_sub_mul(builder, m0, m8, m2, m6);
                let c12 = Self::emit_sub_mul(builder, m1, m6, m0, m7);

                let c20 = Self::emit_sub_mul(builder, m1, m5, m2, m4);
                let c21 = Self::emit_sub_mul(builder, m2, m3, m0, m5);
                let c22 = Self::emit_sub_mul(builder, m0, m4, m1, m3);

                let d0 = builder.ins().fmul(m0, c00);
                let s1 = builder.ins().fma(m1, c01, d0);
                let det = builder.ins().fma(m2, c02, s1);
                let one = builder.ins().f64const(1.0);
                let inv_det = builder.ins().fdiv(one, det);

                let i0 = builder.ins().fmul(c00, inv_det);
                let i1 = builder.ins().fmul(c10, inv_det);
                let i2 = builder.ins().fmul(c20, inv_det);
                let i3 = builder.ins().fmul(c01, inv_det);
                let i4 = builder.ins().fmul(c11, inv_det);
                let i5 = builder.ins().fmul(c21, inv_det);
                let i6 = builder.ins().fmul(c02, inv_det);
                let i7 = builder.ins().fmul(c12, inv_det);
                let i8 = builder.ins().fmul(c22, inv_det);
                Ok(vec![i0, i1, i2, i3, i4, i5, i6, i7, i8])
            }
            "mat_solve3" => {
                let mat = self.resolve_array(&args[0], builder)?;
                let vec_b = self.resolve_array(&args[1], builder)?;
                let m0 = self.get_array_element(&mat, 0, builder);
                let m1 = self.get_array_element(&mat, 1, builder);
                let m2 = self.get_array_element(&mat, 2, builder);
                let m3 = self.get_array_element(&mat, 3, builder);
                let m4 = self.get_array_element(&mat, 4, builder);
                let m5 = self.get_array_element(&mat, 5, builder);
                let m6 = self.get_array_element(&mat, 6, builder);
                let m7 = self.get_array_element(&mat, 7, builder);
                let m8 = self.get_array_element(&mat, 8, builder);

                let b0 = self.get_array_element(&vec_b, 0, builder);
                let b1 = self.get_array_element(&vec_b, 1, builder);
                let b2 = self.get_array_element(&vec_b, 2, builder);

                let c00 = Self::emit_sub_mul(builder, m4, m8, m5, m7);
                let c01 = Self::emit_sub_mul(builder, m5, m6, m3, m8);
                let c02 = Self::emit_sub_mul(builder, m3, m7, m4, m6);

                let c10 = Self::emit_sub_mul(builder, m2, m7, m1, m8);
                let c11 = Self::emit_sub_mul(builder, m0, m8, m2, m6);
                let c12 = Self::emit_sub_mul(builder, m1, m6, m0, m7);

                let c20 = Self::emit_sub_mul(builder, m1, m5, m2, m4);
                let c21 = Self::emit_sub_mul(builder, m2, m3, m0, m5);
                let c22 = Self::emit_sub_mul(builder, m0, m4, m1, m3);

                let d0 = builder.ins().fmul(m0, c00);
                let s1 = builder.ins().fma(m1, c01, d0);
                let det = builder.ins().fma(m2, c02, s1);
                let one = builder.ins().f64const(1.0);
                let inv_det = builder.ins().fdiv(one, det);

                let t0 = builder.ins().fmul(c00, b0);
                let s0_1 = builder.ins().fma(c10, b1, t0);
                let num0 = builder.ins().fma(c20, b2, s0_1);

                let t1 = builder.ins().fmul(c01, b0);
                let s1_1 = builder.ins().fma(c11, b1, t1);
                let num1 = builder.ins().fma(c21, b2, s1_1);

                let t2 = builder.ins().fmul(c02, b0);
                let s2_1 = builder.ins().fma(c12, b1, t2);
                let num2 = builder.ins().fma(c22, b2, s2_1);

                let x0 = builder.ins().fmul(num0, inv_det);
                let x1 = builder.ins().fmul(num1, inv_det);
                let x2 = builder.ins().fmul(num2, inv_det);
                Ok(vec![x0, x1, x2])
            }
            "mat_inv4" => {
                let arr = self.resolve_array(&args[0], builder)?;
                let elem_ty = arr.elem_ty();
                let a: Vec<Value> = (0..16).map(|i| self.get_array_element(&arr, i, builder)).collect();

                let c00 = Self::emit_det3_val(builder, &elem_ty, a[5], a[6], a[7], a[9], a[10], a[11], a[13], a[14], a[15]);
                let d01 = Self::emit_det3_val(builder, &elem_ty, a[4], a[6], a[7], a[8], a[10], a[11], a[12], a[14], a[15]);
                let c01 = builder.ins().fneg(d01);
                let c02 = Self::emit_det3_val(builder, &elem_ty, a[4], a[5], a[7], a[8], a[9], a[11], a[12], a[13], a[15]);
                let d03 = Self::emit_det3_val(builder, &elem_ty, a[4], a[5], a[6], a[8], a[9], a[10], a[12], a[13], a[14]);
                let c03 = builder.ins().fneg(d03);

                let d10 = Self::emit_det3_val(builder, &elem_ty, a[1], a[2], a[3], a[9], a[10], a[11], a[13], a[14], a[15]);
                let c10 = builder.ins().fneg(d10);
                let c11 = Self::emit_det3_val(builder, &elem_ty, a[0], a[2], a[3], a[8], a[10], a[11], a[12], a[14], a[15]);
                let d12 = Self::emit_det3_val(builder, &elem_ty, a[0], a[1], a[3], a[8], a[9], a[11], a[12], a[13], a[15]);
                let c12 = builder.ins().fneg(d12);
                let c13 = Self::emit_det3_val(builder, &elem_ty, a[0], a[1], a[2], a[8], a[9], a[10], a[12], a[13], a[14]);

                let c20 = Self::emit_det3_val(builder, &elem_ty, a[1], a[2], a[3], a[5], a[6], a[7], a[13], a[14], a[15]);
                let d21 = Self::emit_det3_val(builder, &elem_ty, a[0], a[2], a[3], a[4], a[6], a[7], a[12], a[14], a[15]);
                let c21 = builder.ins().fneg(d21);
                let c22 = Self::emit_det3_val(builder, &elem_ty, a[0], a[1], a[3], a[4], a[5], a[7], a[12], a[13], a[15]);
                let d23 = Self::emit_det3_val(builder, &elem_ty, a[0], a[1], a[2], a[4], a[5], a[6], a[12], a[13], a[14]);
                let c23 = builder.ins().fneg(d23);

                let d30 = Self::emit_det3_val(builder, &elem_ty, a[1], a[2], a[3], a[5], a[6], a[7], a[9], a[10], a[11]);
                let c30 = builder.ins().fneg(d30);
                let c31 = Self::emit_det3_val(builder, &elem_ty, a[0], a[2], a[3], a[4], a[6], a[7], a[8], a[10], a[11]);
                let d32 = Self::emit_det3_val(builder, &elem_ty, a[0], a[1], a[3], a[4], a[5], a[7], a[8], a[9], a[11]);
                let c32 = builder.ins().fneg(d32);
                let c33 = Self::emit_det3_val(builder, &elem_ty, a[0], a[1], a[2], a[4], a[5], a[6], a[8], a[9], a[10]);

                let d0 = builder.ins().fmul(a[0], c00);
                let d1 = builder.ins().fma(a[1], c01, d0);
                let d2 = builder.ins().fma(a[2], c02, d1);
                let det = builder.ins().fma(a[3], c03, d2);
                let one = builder.ins().f64const(1.0);
                let inv_det = builder.ins().fdiv(one, det);

                let cofactors = [
                    c00, c10, c20, c30,
                    c01, c11, c21, c31,
                    c02, c12, c22, c32,
                    c03, c13, c23, c33,
                ];
                let mut res = Vec::with_capacity(16);
                for c in cofactors {
                    res.push(builder.ins().fmul(c, inv_det));
                }
                Ok(res)
            }
            "mat_solve4" => {
                let mat_inv = self.evaluate_array_op_values("mat_inv4", &args[0..1], elem_ty, 16, builder)?;
                let vec_b = self.resolve_array(&args[1], builder)?;
                let b: Vec<Value> = (0..4).map(|i| self.get_array_element(&vec_b, i, builder)).collect();
                let mut res = Vec::with_capacity(4);
                for r in 0..4 {
                    let p0 = builder.ins().fmul(mat_inv[r * 4], b[0]);
                    let s1 = builder.ins().fma(mat_inv[r * 4 + 1], b[1], p0);
                    let s2 = builder.ins().fma(mat_inv[r * 4 + 2], b[2], s1);
                    let entry = builder.ins().fma(mat_inv[r * 4 + 3], b[3], s2);
                    res.push(entry);
                }
                Ok(res)
            }
            "c_make" => {
                let re = self.translate_expr(&args[0], builder)?;
                let im = self.translate_expr(&args[1], builder)?;
                Ok(vec![re, im])
            }
            "c_add" => {
                let a = self.resolve_array(&args[0], builder)?;
                let b = self.resolve_array(&args[1], builder)?;
                let a0 = self.get_array_element(&a, 0, builder);
                let a1 = self.get_array_element(&a, 1, builder);
                let b0 = self.get_array_element(&b, 0, builder);
                let b1 = self.get_array_element(&b, 1, builder);
                let re = builder.ins().fadd(a0, b0);
                let im = builder.ins().fadd(a1, b1);
                Ok(vec![re, im])
            }
            "c_sub" => {
                let a = self.resolve_array(&args[0], builder)?;
                let b = self.resolve_array(&args[1], builder)?;
                let a0 = self.get_array_element(&a, 0, builder);
                let a1 = self.get_array_element(&a, 1, builder);
                let b0 = self.get_array_element(&b, 0, builder);
                let b1 = self.get_array_element(&b, 1, builder);
                let re = builder.ins().fsub(a0, b0);
                let im = builder.ins().fsub(a1, b1);
                Ok(vec![re, im])
            }
            "c_mul" => {
                let a = self.resolve_array(&args[0], builder)?;
                let b = self.resolve_array(&args[1], builder)?;
                let a_re = self.get_array_element(&a, 0, builder);
                let a_im = self.get_array_element(&a, 1, builder);
                let b_re = self.get_array_element(&b, 0, builder);
                let b_im = self.get_array_element(&b, 1, builder);
                let p0 = builder.ins().fmul(a_re, b_re);
                let p1 = builder.ins().fmul(a_im, b_im);
                let re = builder.ins().fsub(p0, p1);
                let q0 = builder.ins().fmul(a_re, b_im);
                let im = builder.ins().fma(a_im, b_re, q0);
                Ok(vec![re, im])
            }
            "c_div" => {
                let a = self.resolve_array(&args[0], builder)?;
                let b = self.resolve_array(&args[1], builder)?;
                let a_re = self.get_array_element(&a, 0, builder);
                let a_im = self.get_array_element(&a, 1, builder);
                let b_re = self.get_array_element(&b, 0, builder);
                let b_im = self.get_array_element(&b, 1, builder);
                let d0 = builder.ins().fmul(b_re, b_re);
                let denom = builder.ins().fma(b_im, b_im, d0);
                let p0 = builder.ins().fmul(a_re, b_re);
                let num_re = builder.ins().fma(a_im, b_im, p0);
                let q0 = builder.ins().fmul(a_im, b_re);
                let q1 = builder.ins().fmul(a_re, b_im);
                let num_im = builder.ins().fsub(q0, q1);
                let re = builder.ins().fdiv(num_re, denom);
                let im = builder.ins().fdiv(num_im, denom);
                Ok(vec![re, im])
            }
            "c_conj" => {
                let a = self.resolve_array(&args[0], builder)?;
                let re = self.get_array_element(&a, 0, builder);
                let im = self.get_array_element(&a, 1, builder);
                let neg_im = builder.ins().fneg(im);
                Ok(vec![re, neg_im])
            }
            "c_exp" => {
                let a = self.resolve_array(&args[0], builder)?;
                let x = self.get_array_element(&a, 0, builder);
                let y = self.get_array_element(&a, 1, builder);
                let f_exp = self.module.declare_func_in_func(self.exp_id, builder.func);
                let call_exp = builder.ins().call(f_exp, &[x]);
                let r = builder.inst_results(call_exp)[0];
                let f_cos = self.module.declare_func_in_func(self.cos_id, builder.func);
                let call_cos = builder.ins().call(f_cos, &[y]);
                let c = builder.inst_results(call_cos)[0];
                let f_sin = self.module.declare_func_in_func(self.sin_id, builder.func);
                let call_sin = builder.ins().call(f_sin, &[y]);
                let s = builder.inst_results(call_sin)[0];
                let re = builder.ins().fmul(r, c);
                let im = builder.ins().fmul(r, s);
                Ok(vec![re, im])
            }
            "fft8" => {
                let re_arr = self.resolve_array(&args[0], builder)?;
                let im_arr = self.resolve_array(&args[1], builder)?;
                let re_in: Vec<Value> = (0..8).map(|k| self.get_array_element(&re_arr, k, builder)).collect();
                let im_in: Vec<Value> = (0..8).map(|k| self.get_array_element(&im_arr, k, builder)).collect();
                let (r, i) = Self::emit_fft(builder, &re_in, &im_in, 8, false);
                let mut res = r;
                res.extend(i);
                Ok(res)
            }
            "fft8_re" => {
                let re_arr = self.resolve_array(&args[0], builder)?;
                let im_arr = self.resolve_array(&args[1], builder)?;
                let re_in: Vec<Value> = (0..8).map(|k| self.get_array_element(&re_arr, k, builder)).collect();
                let im_in: Vec<Value> = (0..8).map(|k| self.get_array_element(&im_arr, k, builder)).collect();
                let (r, _) = Self::emit_fft(builder, &re_in, &im_in, 8, true);
                Ok(r)
            }
            "fft8_im" => {
                let re_arr = self.resolve_array(&args[0], builder)?;
                let im_arr = self.resolve_array(&args[1], builder)?;
                let re_in: Vec<Value> = (0..8).map(|k| self.get_array_element(&re_arr, k, builder)).collect();
                let im_in: Vec<Value> = (0..8).map(|k| self.get_array_element(&im_arr, k, builder)).collect();
                let (_, i) = Self::emit_fft(builder, &re_in, &im_in, 8, false);
                Ok(i)
            }
            "fft16_re" => {
                let re_arr = self.resolve_array(&args[0], builder)?;
                let im_arr = self.resolve_array(&args[1], builder)?;
                let re_in: Vec<Value> = (0..16).map(|k| self.get_array_element(&re_arr, k, builder)).collect();
                let im_in: Vec<Value> = (0..16).map(|k| self.get_array_element(&im_arr, k, builder)).collect();
                let (r, _) = Self::emit_fft(builder, &re_in, &im_in, 16, true);
                Ok(r)
            }
            "fft16_im" => {
                let re_arr = self.resolve_array(&args[0], builder)?;
                let im_arr = self.resolve_array(&args[1], builder)?;
                let re_in: Vec<Value> = (0..16).map(|k| self.get_array_element(&re_arr, k, builder)).collect();
                let im_in: Vec<Value> = (0..16).map(|k| self.get_array_element(&im_arr, k, builder)).collect();
                let (_, i) = Self::emit_fft(builder, &re_in, &im_in, 16, false);
                Ok(i)
            }
            _ => panic!("Unsupported array op {}", callee),
        }
    }

    pub(crate) fn translate_array_op_into_vars(
        &mut self,
        callee: &str,
        args: &[TypedExpr],
        dst_vars: &[Variable],
        elem_ty: &Type,
        len: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        let vals = self.evaluate_array_op_values(callee, args, elem_ty, len, builder)?;
        for (i, val) in vals.into_iter().enumerate() {
            builder.def_var(dst_vars[i], val);
        }
        Ok(())
    }

    pub(crate) fn copy_array_slots(
        &self,
        src_slot: StackSlot,
        dst_slot: StackSlot,
        total_bytes: usize,
        elem_ty: &Type,
        builder: &mut FunctionBuilder,
    ) {
        let mut offset = 0;
        if total_bytes <= 128 && total_bytes.is_multiple_of(16) {
            let mut chunks = Vec::with_capacity(total_bytes / 16);
            while offset + 16 <= total_bytes {
                let chunk = builder.ins().stack_load(types::I64, types::I8X16, src_slot, offset as i32);
                chunks.push((offset, chunk));
                offset += 16;
            }
            for (off, val) in chunks {
                builder.ins().stack_store(types::I64, val, dst_slot, off as i32);
            }
            return;
        }

        while offset + 16 <= total_bytes {
            let chunk = builder.ins().stack_load(types::I64, types::I8X16, src_slot, offset as i32);
            builder.ins().stack_store(types::I64, chunk, dst_slot, offset as i32);
            offset += 16;
        }

        // 8-byte scalar chunks
        while offset + 8 <= total_bytes {
            let chunk = builder.ins().stack_load(types::I64, types::I64, src_slot, offset as i32);
            builder.ins().stack_store(types::I64, chunk, dst_slot, offset as i32);
            offset += 8;
        }
        // remaining elements
        let clif_ty = type_to_clif(elem_ty.clone());
        let elem_size = elem_ty.size_bytes();
        while offset < total_bytes {
            let chunk = builder.ins().stack_load(types::I64, clif_ty, src_slot, offset as i32);
            builder.ins().stack_store(types::I64, chunk, dst_slot, offset as i32);
            offset += elem_size;
        }
    }

    pub(crate) fn translate_array_op_into_slot(
        &mut self,
        callee: &str,
        args: &[TypedExpr],
        dst_slot: StackSlot,
        elem_ty: &Type,
        len: usize,
        builder: &mut FunctionBuilder,
    ) -> Result<(), CodegenError> {
        if callee == "vec_add" {
            return self.translate_vec_add_into_slot(args, dst_slot, elem_ty, len, builder);
        }
        let elem_size = elem_ty.size_bytes() as i32;
        let vals = self.evaluate_array_op_values(callee, args, elem_ty, len, builder)?;
        for (i, val) in vals.into_iter().enumerate() {
            let offset = (i as i32) * elem_size;
            let addr = builder.ins().stack_addr(types::I64, dst_slot, offset);
            builder.ins().store(MemFlagsData::trusted(), val, addr, 0);
        }
        Ok(())
    }

    pub(crate) fn translate_vec_add_into_slot(
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

        // If both arrays are stack slots, leverage SIMD vector arithmetic instructions
        if let (ResolvedArray::Slot { slot: slot_a, .. }, ResolvedArray::Slot { slot: slot_b, .. }) = (&arr_a, &arr_b) {
            let mut i = 0;
            match elem_ty {
                Type::I64 => {
                    while i + 2 <= len {
                        let offset = (i as i32) * 8;
                        let addr_a = builder.ins().stack_addr(types::I64, *slot_a, offset);
                        let va = builder.ins().load(types::I64X2, MemFlagsData::trusted(), addr_a, 0);
                        let addr_b = builder.ins().stack_addr(types::I64, *slot_b, offset);
                        let vb = builder.ins().load(types::I64X2, MemFlagsData::trusted(), addr_b, 0);
                        let vsum = builder.ins().iadd(va, vb);
                        let addr_d = builder.ins().stack_addr(types::I64, dst_slot, offset);
                        builder.ins().store(MemFlagsData::trusted(), vsum, addr_d, 0);
                        i += 2;
                    }
                }
                Type::F64 => {
                    while i + 2 <= len {
                        let offset = (i as i32) * 8;
                        let addr_a = builder.ins().stack_addr(types::I64, *slot_a, offset);
                        let va = builder.ins().load(types::F64X2, MemFlagsData::trusted(), addr_a, 0);
                        let addr_b = builder.ins().stack_addr(types::I64, *slot_b, offset);
                        let vb = builder.ins().load(types::F64X2, MemFlagsData::trusted(), addr_b, 0);
                        let vsum = builder.ins().fadd(va, vb);
                        let addr_d = builder.ins().stack_addr(types::I64, dst_slot, offset);
                        builder.ins().store(MemFlagsData::trusted(), vsum, addr_d, 0);
                        i += 2;
                    }
                }
                Type::I32 => {
                    while i + 4 <= len {
                        let offset = (i as i32) * 4;
                        let addr_a = builder.ins().stack_addr(types::I64, *slot_a, offset);
                        let va = builder.ins().load(types::I32X4, MemFlagsData::trusted(), addr_a, 0);
                        let addr_b = builder.ins().stack_addr(types::I64, *slot_b, offset);
                        let vb = builder.ins().load(types::I32X4, MemFlagsData::trusted(), addr_b, 0);
                        let vsum = builder.ins().iadd(va, vb);
                        let addr_d = builder.ins().stack_addr(types::I64, dst_slot, offset);
                        builder.ins().store(MemFlagsData::trusted(), vsum, addr_d, 0);
                        i += 4;
                    }
                }
                Type::F32 => {
                    while i + 4 <= len {
                        let offset = (i as i32) * 4;
                        let addr_a = builder.ins().stack_addr(types::I64, *slot_a, offset);
                        let va = builder.ins().load(types::F32X4, MemFlagsData::trusted(), addr_a, 0);
                        let addr_b = builder.ins().stack_addr(types::I64, *slot_b, offset);
                        let vb = builder.ins().load(types::F32X4, MemFlagsData::trusted(), addr_b, 0);
                        let vsum = builder.ins().fadd(va, vb);
                        let addr_d = builder.ins().stack_addr(types::I64, dst_slot, offset);
                        builder.ins().store(MemFlagsData::trusted(), vsum, addr_d, 0);
                        i += 4;
                    }
                }
                _ => {}
            }
            while i < len {
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
                i += 1;
            }
            return Ok(());
        }

        // Fallback when one or both operands are promoted vars
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

    pub(crate) fn emit_dot_product(
        &mut self,
        arr_a: &ResolvedArray,
        arr_b: &ResolvedArray,
        builder: &mut FunctionBuilder,
    ) -> Result<Value, CodegenError> {
        let len_a = arr_a.len();
        let elem_ty = arr_a.elem_ty();
        let clif_ty = type_to_clif(elem_ty.clone());

        if len_a == 4 {
            let v_a0 = self.get_array_element(arr_a, 0, builder);
            let v_b0 = self.get_array_element(arr_b, 0, builder);
            let v_a1 = self.get_array_element(arr_a, 1, builder);
            let v_b1 = self.get_array_element(arr_b, 1, builder);
            let v_a2 = self.get_array_element(arr_a, 2, builder);
            let v_b2 = self.get_array_element(arr_b, 2, builder);
            let v_a3 = self.get_array_element(arr_a, 3, builder);
            let v_b3 = self.get_array_element(arr_b, 3, builder);

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
            let v_a0 = self.get_array_element(arr_a, 0, builder);
            let v_b0 = self.get_array_element(arr_b, 0, builder);
            let v_a1 = self.get_array_element(arr_a, 1, builder);
            let v_b1 = self.get_array_element(arr_b, 1, builder);
            let v_a2 = self.get_array_element(arr_a, 2, builder);
            let v_b2 = self.get_array_element(arr_b, 2, builder);
            let v_a3 = self.get_array_element(arr_a, 3, builder);
            let v_b3 = self.get_array_element(arr_b, 3, builder);
            let v_a4 = self.get_array_element(arr_a, 4, builder);
            let v_b4 = self.get_array_element(arr_b, 4, builder);
            let v_a5 = self.get_array_element(arr_a, 5, builder);
            let v_b5 = self.get_array_element(arr_b, 5, builder);
            let v_a6 = self.get_array_element(arr_a, 6, builder);
            let v_b6 = self.get_array_element(arr_b, 6, builder);
            let v_a7 = self.get_array_element(arr_a, 7, builder);
            let v_b7 = self.get_array_element(arr_b, 7, builder);

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
            let v_a0 = self.get_array_element(arr_a, i, builder);
            let v_b0 = self.get_array_element(arr_b, i, builder);
            let v_a1 = self.get_array_element(arr_a, i + 1, builder);
            let v_b1 = self.get_array_element(arr_b, i + 1, builder);
            let v_a2 = self.get_array_element(arr_a, i + 2, builder);
            let v_b2 = self.get_array_element(arr_b, i + 2, builder);
            let v_a3 = self.get_array_element(arr_a, i + 3, builder);
            let v_b3 = self.get_array_element(arr_b, i + 3, builder);
            let v_a4 = self.get_array_element(arr_a, i + 4, builder);
            let v_b4 = self.get_array_element(arr_b, i + 4, builder);
            let v_a5 = self.get_array_element(arr_a, i + 5, builder);
            let v_b5 = self.get_array_element(arr_b, i + 5, builder);
            let v_a6 = self.get_array_element(arr_a, i + 6, builder);
            let v_b6 = self.get_array_element(arr_b, i + 6, builder);
            let v_a7 = self.get_array_element(arr_a, i + 7, builder);
            let v_b7 = self.get_array_element(arr_b, i + 7, builder);

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
            let v_a0 = self.get_array_element(arr_a, i, builder);
            let v_b0 = self.get_array_element(arr_b, i, builder);
            let v_a1 = self.get_array_element(arr_a, i + 1, builder);
            let v_b1 = self.get_array_element(arr_b, i + 1, builder);
            let v_a2 = self.get_array_element(arr_a, i + 2, builder);
            let v_b2 = self.get_array_element(arr_b, i + 2, builder);
            let v_a3 = self.get_array_element(arr_a, i + 3, builder);
            let v_b3 = self.get_array_element(arr_b, i + 3, builder);

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
            let v_a = self.get_array_element(arr_a, i, builder);
            let v_b = self.get_array_element(arr_b, i, builder);
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
        Ok(total)
    }

    pub(crate) fn collect_expr_reads(expr: &TypedExpr, reads: &mut HashSet<String>) {
        match expr {
            TypedExpr::Ident { name, .. } => {
                reads.insert(name.clone());
            }
            TypedExpr::Unary { expr, .. } => {
                Self::collect_expr_reads(expr, reads);
            }
            TypedExpr::Binary { left, right, .. } => {
                Self::collect_expr_reads(left, reads);
                Self::collect_expr_reads(right, reads);
            }
            TypedExpr::Call { args, .. } => {
                for a in args {
                    Self::collect_expr_reads(a, reads);
                }
            }
            TypedExpr::ArrayLiteral { elements, .. } => {
                for e in elements {
                    Self::collect_expr_reads(e, reads);
                }
            }
            TypedExpr::Index { target, index, .. } => {
                Self::collect_expr_reads(target, reads);
                Self::collect_expr_reads(index, reads);
            }
            TypedExpr::StructLiteral { fields, .. } => {
                for (_, e) in fields {
                    Self::collect_expr_reads(e, reads);
                }
            }
            TypedExpr::FieldAccess { target, .. } => {
                Self::collect_expr_reads(target, reads);
            }
            TypedExpr::Match { scrutinee, arms, .. } => {
                Self::collect_expr_reads(scrutinee, reads);
                for arm in arms {
                    Self::collect_expr_reads(&arm.body, reads);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn collect_stmt_reads(stmt: &TypedStmt, reads: &mut HashSet<String>) {
        match stmt {
            TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } | TypedStmt::Expr(value) => {
                Self::collect_expr_reads(value, reads);
            }
            TypedStmt::IndexAssign { target, index, value, .. } => {
                reads.insert(target.clone());
                Self::collect_expr_reads(index, reads);
                Self::collect_expr_reads(value, reads);
            }
            TypedStmt::FieldAssign { target, value, .. } => {
                reads.insert(target.clone());
                Self::collect_expr_reads(value, reads);
            }
            TypedStmt::Return(Some(expr), _) => {
                Self::collect_expr_reads(expr, reads);
            }
            _ => {}
        }
    }

    pub(crate) fn parse_stride_offset(index: &TypedExpr, loop_var: &str) -> Option<(i64, i64)> {
        match index {
            TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } => {
                let off = get_constant_int(right)?;
                if let TypedExpr::Binary { op: BinaryOp::Mul, left: mul_l, right: mul_r, .. } = &**left {
                    if matches!(&**mul_l, TypedExpr::Ident { name: v, .. } if v == loop_var) {
                        let stride = get_constant_int(mul_r)?;
                        return Some((stride, off));
                    }
                }
                None
            }
            TypedExpr::Binary { op: BinaryOp::Mul, left, right, .. } => {
                if matches!(&**left, TypedExpr::Ident { name: v, .. } if v == loop_var) {
                    let stride = get_constant_int(right)?;
                    return Some((stride, 0));
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn while_covers_array_indices(name: &str, len: usize, condition: &TypedExpr, body: &TypedBlock) -> bool {
        for s in &body.stmts {
            match s {
                TypedStmt::IndexAssign { index, value, .. } => {
                    let mut reads = HashSet::new();
                    Self::collect_expr_reads(index, &mut reads);
                    Self::collect_expr_reads(value, &mut reads);
                    if reads.contains(name) {
                        return false;
                    }
                }
                _ => {
                    let mut reads = HashSet::new();
                    Self::collect_stmt_reads(s, &mut reads);
                    if reads.contains(name) {
                        return false;
                    }
                }
            }
        }
        let (loop_var, bound) = match condition {
            TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                if let (TypedExpr::Ident { name: v, .. }, Some(b)) = (&**left, get_constant_int(right)) {
                    (v.as_str(), b)
                } else {
                    return false;
                }
            }
            _ => return false,
        };
        if bound <= 0 {
            return false;
        }

        let mut has_step = false;
        for s in &body.stmts {
            if let TypedStmt::Assign { name: v, value, .. } = s {
                if v == loop_var {
                    if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = value {
                        if matches!(&**left, TypedExpr::Ident { name: l, .. } if l == loop_var) && get_constant_int(right) == Some(1) {
                            has_step = true;
                        }
                    }
                }
            }
        }
        if !has_step {
            return false;
        }

        let mut offsets = HashSet::new();
        let mut detected_stride = None;

        for s in &body.stmts {
            if let TypedStmt::IndexAssign { target, index, .. } = s {
                if target == name {
                    if let Some((stride, off)) = Self::parse_stride_offset(index, loop_var) {
                        if let Some(s) = detected_stride {
                            if s != stride { return false; }
                        } else {
                            detected_stride = Some(stride);
                        }
                        offsets.insert(off);
                    }
                }
            }
        }

        if let Some(stride) = detected_stride {
            if (bound as usize) * (stride as usize) == len && offsets.len() == (stride as usize) {
                return (0..stride).all(|o| offsets.contains(&o));
            }
        }
        false
    }

    pub(crate) fn is_array_fully_overwritten(name: &str, len: usize, remaining_stmts: &[TypedStmt]) -> bool {
        let mut assigned_indices = HashSet::new();
        for s in remaining_stmts {
            match s {
                TypedStmt::IndexAssign { target, index, value, .. } if target == name => {
                    let mut rhs_reads = HashSet::new();
                    Self::collect_expr_reads(value, &mut rhs_reads);
                    if rhs_reads.contains(name) {
                        return false;
                    }
                    if let Some(idx) = get_constant_int(index) {
                        if idx >= 0 && (idx as usize) < len {
                            assigned_indices.insert(idx as usize);
                            if assigned_indices.len() == len {
                                return true;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                TypedStmt::While { condition, body, .. } => {
                    let mut reads = HashSet::new();
                    Self::collect_stmt_reads(s, &mut reads);
                    if reads.contains(name) {
                        return false;
                    }
                    if Self::while_covers_array_indices(name, len, condition, body) {
                        return true;
                    }
                    return false;
                }
                _ => {
                    let mut reads = HashSet::new();
                    Self::collect_stmt_reads(s, &mut reads);
                    if reads.contains(name) {
                        return false;
                    }
                    if matches!(s, TypedStmt::If { .. } | TypedStmt::Break(_) | TypedStmt::Return(..)) {
                        return false;
                    }
                }
            }
        }
        assigned_indices.len() == len
    }

    #[allow(clippy::needless_range_loop)]
    pub(crate) fn translate_stmt(
        &mut self,
        stmt: &TypedStmt,
        remaining_stmts: &[TypedStmt],
        builder: &mut FunctionBuilder,
    ) -> Result<bool, CodegenError> {
        match stmt {
            TypedStmt::Let {
                name, ty, value, ..
            } => {
                if let Type::Struct(sname) = ty {
                    let layout = self.struct_layouts.get(sname).ok_or_else(|| CodegenError::BackendError(format!("Missing layout for struct {sname}")))?.clone();
                    let slot_data = StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        layout.total_size,
                        layout.align.min(8) as u8,
                    );
                    let slot = builder.create_sized_stack_slot(slot_data);
                    let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);

                    if let TypedExpr::StructLiteral { fields, .. } = value {
                        for (fname, fexpr) in fields {
                            let (foffset, fty) = layout.fields.get(fname).ok_or_else(|| CodegenError::BackendError(format!("Field {fname} not found in struct {sname}")))?.clone();
                            let fval = self.translate_expr(fexpr, builder)?;
                            if let Type::Struct(sub_name) = &fty {
                                let sub_layout = self.struct_layouts.get(sub_name).ok_or_else(|| CodegenError::BackendError(format!("Missing sub-layout for struct {sub_name}")))?;
                                let sub_dst = builder.ins().iadd_imm_s(dst_ptr, foffset as i64);
                                Self::emit_copy_bytes(builder, fval, sub_dst, sub_layout.total_size as usize);
                            } else {
                                builder.ins().store(MemFlagsData::trusted(), fval, dst_ptr, foffset as i32);
                            }
                        }
                    } else if matches!(value, TypedExpr::Literal { .. }) {
                        Self::emit_zero_bytes(builder, dst_ptr, layout.total_size as usize);
                    } else {
                        let src_ptr = self.translate_expr(value, builder)?;
                        Self::emit_copy_bytes(builder, src_ptr, dst_ptr, layout.total_size as usize);
                    }

                    self.variables.insert(
                        name.clone(),
                        Storage::Struct {
                            slot,
                            struct_name: sname.clone(),
                        },
                    );
                    return Ok(false);
                }

                if let Type::Enum(ename) = ty {
                    let layout = self.enum_layouts.get(ename).ok_or_else(|| CodegenError::BackendError(format!("Missing layout for enum {ename}")))?.clone();
                    let slot_data = StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        layout.total_size,
                        layout.align.min(8) as u8,
                    );
                    let slot = builder.create_sized_stack_slot(slot_data);
                    let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);

                    if matches!(value, TypedExpr::Literal { .. }) {
                        Self::emit_zero_bytes(builder, dst_ptr, layout.total_size as usize);
                    } else {
                        let src_ptr = self.translate_expr(value, builder)?;
                        Self::emit_copy_bytes(builder, src_ptr, dst_ptr, layout.total_size as usize);
                    }

                    self.variables.insert(
                        name.clone(),
                        Storage::Enum {
                            slot,
                            enum_name: ename.clone(),
                        },
                    );
                    return Ok(false);
                }

                if let Type::Array(elem, len) = ty {
                    let is_dynamic = self.dynamically_indexed_arrays.contains(name);
                    if *len <= 16 && !is_dynamic && !elem.is_struct() {
                        let clif_ty = type_to_clif((**elem).clone());
                        let vars = match self.variables.get(name) {
                            Some(Storage::PromotedArray { vars: existing_vars, len: existing_len, .. }) if *existing_len == *len => existing_vars.clone(),
                            _ => {
                                let mut vars = Vec::with_capacity(*len);
                                for _ in 0..*len {
                                    vars.push(builder.declare_var(clif_ty));
                                }
                                vars
                            }
                        };

                        let is_dead_init = Self::is_array_fully_overwritten(name, *len, remaining_stmts);
                        if !is_dead_init {
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
                                                    let val = builder.ins().stack_load(types::I64, clif_ty, src_slot, offset);
                                                    builder.def_var(vars[i], val);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                TypedExpr::Call { callee, args, .. } if Self::is_array_op(callee) => {
                                    self.translate_array_op_into_vars(callee, args, &vars, elem, *len, builder)?;
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

                    let elem_size = self.get_type_size(elem) as u32;
                    let total_bytes = (elem_size * (*len as u32)).max(1);
                    let slot = match self.variables.get(name) {
                        Some(Storage::Array { slot: existing_slot, len: existing_len }) if *existing_len == *len => *existing_slot,
                        _ => {
                            let slot_data =
                                StackSlotData::new(StackSlotKind::ExplicitSlot, total_bytes, elem_size.min(8) as u8);
                            builder.create_sized_stack_slot(slot_data)
                        }
                    };

                    let is_dead_init = Self::is_array_fully_overwritten(name, *len, remaining_stmts);
                    if !is_dead_init {
                        match value {
                            TypedExpr::ArrayLiteral { elements, .. } => {
                                for (i, el) in elements.iter().enumerate() {
                                    let el_val = self.translate_expr(el, builder)?;
                                    let offset = (i as i32) * (elem_size as i32);
                                    if el.ty().is_struct() {
                                        let dst_addr = builder.ins().stack_addr(types::I64, slot, offset);
                                        Self::emit_copy_bytes(builder, el_val, dst_addr, elem_size as usize);
                                    } else {
                                        builder.ins().stack_store(types::I64, el_val, slot, offset);
                                    }
                                }
                            }
                            TypedExpr::Ident { name: src_name, .. } => {
                                if let Some(src_storage) = self.variables.get(src_name).cloned() {
                                    match src_storage {
                                        Storage::PromotedArray { vars: src_vars, .. } => {
                                            for i in 0..*len {
                                                let offset = (i as i32) * (elem_size as i32);
                                                let el_val = builder.use_var(src_vars[i]);
                                                builder.ins().stack_store(types::I64, el_val, slot, offset);
                                            }
                                        }
                                        Storage::Array { slot: src_slot, .. } => {
                                            let total_bytes = (*len) * self.get_type_size(elem);
                                            self.copy_array_slots(src_slot, slot, total_bytes, elem, builder);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            TypedExpr::Call { callee, args, .. } if Self::is_array_op(callee) => {
                                self.translate_array_op_into_slot(callee, args, slot, elem, *len, builder)?;
                            }
                            _ => {
                                let clif_ty = type_to_clif((**elem).clone());
                                let zero = if elem.is_float() {
                                    if **elem == Type::F32 { builder.ins().f32const(0.0) } else { builder.ins().f64const(0.0) }
                                } else {
                                    builder.ins().iconst(clif_ty, 0)
                                };
                                for i in 0..*len {
                                    let offset = (i as i32) * (elem_size as i32);
                                    builder.ins().stack_store(types::I64, zero, slot, offset);
                                }
                            }
                        }
                    }

                    self.variables
                        .insert(name.clone(), Storage::Array { slot, len: *len });
                    self.array_load_cache.retain(|(arr, _), (idx_expr, _)| arr != name && !index_reads_var(idx_expr, name));
                    Ok(false)
                } else {
                    let val = self.translate_expr(value, builder)?;
                    let var = match self.variables.get(name) {
                        Some(Storage::Scalar(existing_var)) => *existing_var,
                        _ => {
                            let clif_ty = type_to_clif(ty.clone());
                            let new_var = builder.declare_var(clif_ty);
                            self.variables.insert(name.clone(), Storage::Scalar(new_var));
                            new_var
                        }
                    };
                    builder.def_var(var, val);
                    self.array_load_cache.retain(|(arr, _), (idx_expr, _)| arr != name && !index_reads_var(idx_expr, name));
                    Ok(false)
                }
            }

            TypedStmt::Assign { name, value, .. } => {
                let storage = self
                    .variables
                    .get(name)
                    .cloned()
                    .ok_or_else(|| CodegenError::BackendError("Variable must exist for assignment".to_string()))?;
                match storage {
                    Storage::Struct { slot, struct_name } => {
                        let layout = self.struct_layouts.get(&struct_name).ok_or_else(|| CodegenError::BackendError(format!("Missing layout for struct {struct_name}")))?.clone();
                        let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                        if let TypedExpr::StructLiteral { fields, .. } = value {
                            for (fname, fexpr) in fields {
                                let (foffset, fty) = layout.fields.get(fname).ok_or_else(|| CodegenError::BackendError(format!("Field {fname} not found in struct {struct_name}")))?.clone();
                                let fval = self.translate_expr(fexpr, builder)?;
                                if let Type::Struct(sub_name) = &fty {
                                    let sub_layout = self.struct_layouts.get(sub_name).ok_or_else(|| CodegenError::BackendError(format!("Missing sub-layout for struct {sub_name}")))?;
                                    let sub_dst = builder.ins().iadd_imm_s(dst_ptr, foffset as i64);
                                    Self::emit_copy_bytes(builder, fval, sub_dst, sub_layout.total_size as usize);
                                } else {
                                    builder.ins().store(MemFlagsData::trusted(), fval, dst_ptr, foffset as i32);
                                }
                            }
                        } else if matches!(value, TypedExpr::Literal { .. }) {
                            Self::emit_zero_bytes(builder, dst_ptr, layout.total_size as usize);
                        } else {
                            let src_ptr = self.translate_expr(value, builder)?;
                            Self::emit_copy_bytes(builder, src_ptr, dst_ptr, layout.total_size as usize);
                        }
                    }
                    Storage::Enum { slot, enum_name } => {
                        let layout = self.enum_layouts.get(&enum_name).ok_or_else(|| CodegenError::BackendError(format!("Missing layout for enum {enum_name}")))?.clone();
                        let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                        if matches!(value, TypedExpr::Literal { .. }) {
                            Self::emit_zero_bytes(builder, dst_ptr, layout.total_size as usize);
                        } else {
                            let src_ptr = self.translate_expr(value, builder)?;
                            Self::emit_copy_bytes(builder, src_ptr, dst_ptr, layout.total_size as usize);
                        }
                    }
                    Storage::EnumPtr { var, .. } => {
                        let src_ptr = self.translate_expr(value, builder)?;
                        builder.def_var(var, src_ptr);
                    }
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
                            if Self::is_array_op(callee) {
                                self.translate_array_op_into_vars(callee, args, &vars, &elem_ty, len, builder)?;
                            }
                        }
                    }
                    Storage::Array { slot, len } => {
                        if let TypedExpr::Ident { name: src_name, ty, .. } = value {
                            let elem = ty.element_type().ok_or_else(|| CodegenError::BackendError("Expected array element type".to_string()))?.clone();
                            let elem_size = self.get_type_size(&elem) as i32;
                            if let Some(src_storage) = self.variables.get(src_name).cloned() {
                                match src_storage {
                                    Storage::PromotedArray { vars: src_vars, .. } => {
                                        for i in 0..len {
                                            let offset = (i as i32) * elem_size;
                                            let el_val = builder.use_var(src_vars[i]);
                                            builder.ins().stack_store(types::I64, el_val, slot, offset);
                                        }
                                    }
                                    Storage::Array { slot: src_slot, .. } => {
                                        let total_bytes = len * self.get_type_size(&elem);
                                        self.copy_array_slots(src_slot, slot, total_bytes, &elem, builder);
                                    }
                                    _ => {}
                                }
                            }
                        } else if let TypedExpr::Call { callee, args, ty, .. } = value {
                            if Self::is_array_op(callee) {
                                let elem = ty.element_type().ok_or_else(|| CodegenError::BackendError("Expected array element type".to_string()))?.clone();
                                self.translate_array_op_into_slot(callee, args, slot, &elem, len, builder)?;
                            }
                        }
                    }
                }
                self.array_load_cache.retain(|(arr, _), (idx_expr, _)| arr != name && !index_reads_var(idx_expr, name));
                Ok(false)
            }

            TypedStmt::IndexAssign {
                target,
                index,
                value,
                is_safe,
                ..
            } => {
                self.array_load_cache.retain(|(arr, _), _| arr != target);
                let storage = self
                    .variables
                    .get(target)
                    .cloned()
                    .ok_or_else(|| CodegenError::BackendError("Target must be an array variable".to_string()))?;
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
                        if index.ty().size_bytes() < 8 {
                            idx_val = if index.ty().is_signed() {
                                builder.ins().sextend(types::I64, idx_val)
                            } else {
                                builder.ins().uextend(types::I64, idx_val)
                            };
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
                        let elem_size = self.get_type_size(&elem_ty);
                        let val = self.translate_expr(value, builder)?;

                        if let Some(c) = get_constant_int(index) {
                            if c >= 0 && (c as usize) < len {
                                let offset = (c as i32) * (elem_size as i32);
                                if elem_ty.is_struct() {
                                    let dst_addr = builder.ins().stack_addr(types::I64, slot, offset);
                                    Self::emit_copy_bytes(builder, val, dst_addr, elem_size);
                                } else {
                                    builder.ins().stack_store(types::I64, val, slot, offset);
                                }
                                return Ok(false);
                            }
                        }

                        let mut idx_val = self.translate_expr(index, builder)?;
                        if index.ty().size_bytes() < 8 {
                            idx_val = if index.ty().is_signed() {
                                builder.ins().sextend(types::I64, idx_val)
                            } else {
                                builder.ins().uextend(types::I64, idx_val)
                            };
                        }

                        if !*is_safe {
                            self.emit_bounds_check(idx_val, len, builder);
                        }

                        let offset = match elem_size {
                            8 => builder.ins().ishl_imm_s(idx_val, 3),
                            4 => builder.ins().ishl_imm_s(idx_val, 2),
                            2 => builder.ins().ishl_imm_s(idx_val, 1),
                            1 => idx_val,
                            _ => builder.ins().imul_imm_s(idx_val, elem_size as i64),
                        };
                        let base_addr = builder.ins().stack_addr(types::I64, slot, 0);
                        let elem_addr = builder.ins().iadd(base_addr, offset);

                        if elem_ty.is_struct() {
                            Self::emit_copy_bytes(builder, val, elem_addr, elem_size);
                        } else {
                            builder.ins().store(MemFlagsData::trusted(), val, elem_addr, 0);
                        }
                        Ok(false)
                    }
                    _ => panic!("Target must be an array variable"),
                }
            }

            TypedStmt::FieldAssign { target, field, value, .. } => {
                let storage = self.variables.get(target).cloned().ok_or_else(|| CodegenError::BackendError("FieldAssign target variable must exist".to_string()))?;
                if let Storage::Struct { slot, struct_name } = storage {
                    let layout = self.struct_layouts.get(&struct_name).ok_or_else(|| CodegenError::BackendError(format!("Missing layout for struct {struct_name}")))?.clone();
                    let (foffset, fty) = layout.fields.get(field).ok_or_else(|| CodegenError::BackendError(format!("Field {field} not found in struct {struct_name}")))?.clone();
                    let target_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                    let fval = self.translate_expr(value, builder)?;
                    if let Type::Struct(sub_name) = &fty {
                        let sub_layout = self.struct_layouts.get(sub_name).ok_or_else(|| CodegenError::BackendError(format!("Missing sub-layout for struct {sub_name}")))?;
                        let sub_dst = builder.ins().iadd_imm_s(target_ptr, foffset as i64);
                        Self::emit_copy_bytes(builder, fval, sub_dst, sub_layout.total_size as usize);
                    } else {
                        builder.ins().store(MemFlagsData::trusted(), fval, target_ptr, foffset as i32);
                    }
                }
                Ok(false)
            }

            TypedStmt::Return(opt_expr, ..) => {
                if let Some(expr) = opt_expr {
                    if let Type::Struct(sname) = expr.ty() {
                        let val = self.translate_expr(expr, builder)?;
                        let sret = self.current_sret_ptr.ok_or_else(|| CodegenError::BackendError("sret_ptr must exist when returning struct".to_string()))?;
                        let layout = self.struct_layouts.get(&sname).ok_or_else(|| CodegenError::BackendError(format!("Missing layout for struct {sname}")))?;
                        Self::emit_copy_bytes(builder, val, sret, layout.total_size as usize);
                        builder.ins().return_(&[sret]);
                    } else if let Type::Enum(ename) = expr.ty() {
                        let val = self.translate_expr(expr, builder)?;
                        let sret = self.current_sret_ptr.ok_or_else(|| CodegenError::BackendError("sret_ptr must exist when returning enum".to_string()))?;
                        let layout = self.enum_layouts.get(&ename).ok_or_else(|| CodegenError::BackendError(format!("Missing layout for enum {ename}")))?;
                        Self::emit_copy_bytes(builder, val, sret, layout.total_size as usize);
                        builder.ins().return_(&[sret]);
                    } else {
                        let val = self.translate_expr(expr, builder)?;
                        builder.ins().return_(&[val]);
                    }
                } else {
                    builder.ins().return_(&[]);
                }
                Ok(true)
            }

            TypedStmt::Break(..) => {
                let exit_block = *self
                    .loop_exit_blocks
                    .last()
                    .ok_or_else(|| CodegenError::BackendError("Break outside loop".to_string()))?;
                builder.ins().jump(exit_block, &[]);
                Ok(true)
            }

            TypedStmt::Continue(..) => {
                let cont_block = *self
                    .loop_continue_blocks
                    .last()
                    .ok_or_else(|| CodegenError::BackendError("Continue outside loop".to_string()))?;
                builder.ins().jump(cont_block, &[]);
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
                self.array_load_cache.clear();
                if self.try_emit_branchless_select(
                    condition,
                    then_branch,
                    else_branch.as_ref(),
                    builder,
                )? {
                    return Ok(false);
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
                    let saved_nonneg = self.known_non_negative_vars.clone();
                    let saved_u32 = self.known_u32_vars.clone();
                    if let TypedExpr::Binary { op, left, right, .. } = condition {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_non_negative(right, &self.known_non_negative_vars) {
                                    self.known_non_negative_vars.insert(name.clone());
                                }
                                if is_expr_known_u32(right, &self.known_non_negative_vars, &self.known_u32_vars) {
                                    self.known_u32_vars.insert(name.clone());
                                }
                            }
                        }
                    }
                    let res = self.translate_block(eb, builder)?;
                    self.known_non_negative_vars = saved_nonneg;
                    self.known_u32_vars = saved_u32;
                    res
                } else {
                    false
                };
                if !else_term {
                    builder.ins().jump(merge_block, &[]);
                }

                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);

                if then_term && else_branch.is_none() {
                    if let TypedExpr::Binary { op, left, right, .. } = condition {
                        if *op == BinaryOp::Le || *op == BinaryOp::Lt {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if is_expr_known_non_negative(right, &self.known_non_negative_vars) {
                                    self.known_non_negative_vars.insert(name.clone());
                                }
                                if is_expr_known_u32(right, &self.known_non_negative_vars, &self.known_u32_vars) {
                                    self.known_u32_vars.insert(name.clone());
                                }
                            }
                        }
                    }
                }

                self.array_load_cache.clear();
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
                self.array_load_cache.clear();
                let should_reset = self.should_reset_loop_iteration(body);

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
                        if body.stmts.len() <= 6 && Self::is_simple_induction_body(body, var_name) {
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
                            if should_reset {
                                let loop_reset_func = self.module.declare_func_in_func(self.loop_reset_id, builder.func);
                                builder.ins().call(loop_reset_func, &[]);
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
                            if should_reset {
                                let loop_reset_func = self.module.declare_func_in_func(self.loop_reset_id, builder.func);
                                builder.ins().call(loop_reset_func, &[]);
                            }
                            builder.ins().jump(cleanup_head_block, &[]);
                            builder.seal_block(cleanup_head_block);

                            // Exit block
                            builder.switch_to_block(exit_block);
                            builder.seal_block(exit_block);
                            self.array_load_cache.clear();
                            return Ok(false);
                        }
                    }
                }

                // Rotated while loop: single conditional branch at the bottom of the body
                let body_block = builder.create_block();
                let latch_block = builder.create_block();
                let exit_block = builder.create_block();

                if let TypedExpr::Literal { lit: TypedLiteral::Bool(true), .. } = condition {
                    builder.ins().jump(body_block, &[]);
                } else {
                    let cond_init = self.translate_expr(condition, builder)?;
                    builder
                        .ins()
                        .brif(cond_init, body_block, &[], exit_block, &[]);
                }

                let mut added_nonneg = None;
                if let Some(var) = get_nonneg_var_from_condition(condition, &self.known_non_negative_vars) {
                    if self.known_non_negative_vars.insert(var.clone()) {
                        added_nonneg = Some(var);
                    }
                }

                builder.switch_to_block(body_block);
                self.loop_exit_blocks.push(exit_block);
                self.loop_continue_blocks.push(latch_block);
                let body_term = self.translate_block(body, builder)?;
                self.loop_continue_blocks.pop();
                self.loop_exit_blocks.pop();
                if let Some(ref r_name) = added_nonneg {
                    self.known_non_negative_vars.remove(r_name);
                }
                if !body_term {
                    builder.ins().jump(latch_block, &[]);
                }

                builder.switch_to_block(latch_block);
                if should_reset {
                    let loop_reset_func = self.module.declare_func_in_func(self.loop_reset_id, builder.func);
                    builder.ins().call(loop_reset_func, &[]);
                }
                if let TypedExpr::Literal { lit: TypedLiteral::Bool(true), .. } = condition {
                    builder.ins().jump(body_block, &[]);
                } else {
                    let cond_repeat = self.translate_expr(condition, builder)?;
                    builder
                        .ins()
                        .brif(cond_repeat, body_block, &[], exit_block, &[]);
                }
                builder.seal_block(latch_block);
                builder.seal_block(body_block);

                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);
                self.array_load_cache.clear();
                Ok(false)
            }

            TypedStmt::For {
                var,
                lo,
                hi,
                inclusive,
                body,
                ..
            } => {
                self.array_load_cache.clear();
                let should_reset = self.should_reset_loop_iteration(body);

                let lo_val = self.translate_expr(lo, builder)?;
                let lo_ty = lo.ty();
                let clif_ty = type_to_clif(lo_ty);

                let var_id = match self.variables.get(var) {
                    Some(Storage::Scalar(v)) => *v,
                    _ => {
                        let v = builder.declare_var(clif_ty);
                        self.variables.insert(var.clone(), Storage::Scalar(v));
                        v
                    }
                };
                builder.def_var(var_id, lo_val);

                let hi_val = self.translate_expr(hi, builder)?;
                let cond_cc = if *inclusive {
                    IntCC::SignedLessThanOrEqual
                } else {
                    IntCC::SignedLessThan
                };
                let cond_init = builder.ins().icmp(cond_cc, lo_val, hi_val);

                let body_block = builder.create_block();
                let latch_block = builder.create_block();
                let exit_block = builder.create_block();

                builder.ins().brif(cond_init, body_block, &[], exit_block, &[]);

                builder.switch_to_block(body_block);
                self.loop_exit_blocks.push(exit_block);
                self.loop_continue_blocks.push(latch_block);
                let body_term = self.translate_block(body, builder)?;
                self.loop_continue_blocks.pop();
                self.loop_exit_blocks.pop();
                if !body_term {
                    builder.ins().jump(latch_block, &[]);
                }

                builder.switch_to_block(latch_block);
                if should_reset {
                    let loop_reset_func = self.module.declare_func_in_func(self.loop_reset_id, builder.func);
                    builder.ins().call(loop_reset_func, &[]);
                }
                let cur_val = builder.use_var(var_id);
                let next_val = builder.ins().iadd_imm_s(cur_val, 1);
                builder.def_var(var_id, next_val);

                let hi_val_repeat = self.translate_expr(hi, builder)?;
                let cond_repeat = builder.ins().icmp(cond_cc, next_val, hi_val_repeat);
                builder.ins().brif(cond_repeat, body_block, &[], exit_block, &[]);
                builder.seal_block(latch_block);
                builder.seal_block(body_block);

                builder.switch_to_block(exit_block);
                builder.seal_block(exit_block);
                self.array_load_cache.clear();
                Ok(false)
            }
        }
    }
}


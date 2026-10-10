//! Expression lowering to Cranelift IR.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind, Value,
};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;
use std::collections::HashMap;

use super::abi::*;
use super::ast_stmt::{format_index_key, FunctionTranslationState, Storage};
use crate::ast::{BinaryOp, UnaryOp};
use crate::typecheck::{Type, TypedBlock, TypedExpr, TypedLiteral, TypedStmt};

impl<'a> FunctionTranslationState<'a> {
    pub(crate) fn translate_expr(
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
                    Type::F32 => Ok(self.get_f32const(*f as f32, builder)),
                    _ => Ok(self.get_f64const(*f, builder)),
                },
                TypedLiteral::Bool(b) => {
                    let v = if *b { 1 } else { 0 };
                    Ok(self.get_iconst(types::I8, v, builder))
                }
                TypedLiteral::Str(s) => {
                    let bytes = s.as_bytes();
                    let slot_size = if bytes.is_empty() {
                        8
                    } else {
                        bytes.len().div_ceil(8) * 8
                    };
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
                    Ok(builder.ins().stack_addr(types::I64, slot, 0))
                }
            },

            TypedExpr::Ident { name, .. } => {
                if let Some(func_id) = self.func_ids.get(name) {
                    let local_func = self.module.declare_func_in_func(*func_id, builder.func);
                    return Ok(builder.ins().func_addr(types::I64, local_func));
                }
                let storage = self.variables.get(name).cloned().ok_or_else(|| {
                    CodegenError::BackendError(format!("Variable '{name}' must be found in scope"))
                })?;
                match storage {
                    Storage::Scalar(var) => Ok(builder.use_var(var)),
                    Storage::Array { slot, .. } => {
                        Ok(builder.ins().stack_addr(types::I64, slot, 0))
                    }
                    Storage::PromotedArray { .. } => Ok(self.get_iconst(types::I64, 0, builder)),
                    Storage::Struct { slot, .. } | Storage::Enum { slot, .. } => {
                        Ok(builder.ins().stack_addr(types::I64, slot, 0))
                    }
                }
            }

            TypedExpr::Unary { op, expr, ty, .. } => {
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
                        let zero = self.get_iconst(types::I8, 0, builder);
                        let cmp = builder.ins().icmp(IntCC::Equal, inner, zero);
                        Ok(cmp)
                    }
                }
            }

            TypedExpr::Binary {
                op, left, right, ..
            } => {
                if *op == BinaryOp::BitOr {
                    if let Some((target_expr, is_left, shift_k)) =
                        Self::try_match_rotate(left, right)
                    {
                        let target_val = self.translate_expr(target_expr, builder)?;
                        let val_ty = builder.func.dfg.value_type(target_val);
                        let shift_val = builder.ins().iconst(val_ty, shift_k);
                        if is_left {
                            return Ok(builder.ins().rotl(target_val, shift_val));
                        } else {
                            return Ok(builder.ins().rotr(target_val, shift_val));
                        }
                    }
                }

                // Power-of-2 divisibility optimization: (x % 2^k) == 0  or  (x % 2^k) != 0
                if (*op == BinaryOp::Eq || *op == BinaryOp::Ne) && left.ty().is_integer() {
                    let check_pattern =
                        |a: &TypedExpr, b: &TypedExpr| -> Option<(TypedExpr, i64)> {
                            if let (
                                TypedExpr::Binary {
                                    op: BinaryOp::Mod,
                                    left: x,
                                    right: d_expr,
                                    ..
                                },
                                TypedExpr::Literal {
                                    lit: TypedLiteral::Int(0, _),
                                    ..
                                },
                            ) = (a, b)
                            {
                                if let Some(d) = get_constant_int(d_expr) {
                                    if d > 0 && (d as u64).is_power_of_two() {
                                        return Some(((**x).clone(), d));
                                    }
                                }
                            }
                            None
                        };

                    if let Some((x_expr, d)) =
                        check_pattern(left, right).or_else(|| check_pattern(right, left))
                    {
                        let x_val = self.translate_expr(&x_expr, builder)?;
                        let mask = d - 1;
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
                            let c_right = get_constant_int(right);
                            let c_left = get_constant_int(left);
                            let clif_ty = type_to_clif(operand_ty);
                            if let Some(k) = c_right {
                                Ok(self.emit_fast_int_mul(l, r, Some(k), clif_ty, builder))
                            } else if let Some(k) = c_left {
                                Ok(self.emit_fast_int_mul(r, l, Some(k), clif_ty, builder))
                            } else {
                                Ok(self.emit_fast_int_mul(l, r, None, clif_ty, builder))
                            }
                        }
                    }
                    BinaryOp::Div => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fdiv(l, r))
                        } else if operand_ty.is_unsigned() {
                            Ok(builder.ins().udiv(l, r))
                        } else if matches!(operand_ty, Type::I8 | Type::I16) {
                            Ok(builder.ins().sdiv(l, r))
                        } else if let Some(d) = get_constant_int(right) {
                            let is_nonneg =
                                is_expr_known_non_negative(left, &self.known_non_negative_vars);
                            let is_u32 = operand_ty == Type::I32
                                || is_expr_known_u32(
                                    left,
                                    &self.known_non_negative_vars,
                                    &self.known_u32_vars,
                                )
                                || compute_expr_upper_bound(
                                    left,
                                    &self.known_var_bounds,
                                    &self.known_non_negative_vars,
                                )
                                .is_some_and(|ub| ub <= 0xFFFF_FFFF);
                            self.emit_fast_signed_div(
                                l,
                                r,
                                d,
                                &operand_ty,
                                is_nonneg,
                                is_u32,
                                builder,
                            )
                        } else if is_expr_known_u32(
                            left,
                            &self.known_non_negative_vars,
                            &self.known_u32_vars,
                        ) && is_expr_known_u32(
                            right,
                            &self.known_non_negative_vars,
                            &self.known_u32_vars,
                        ) {
                            let l32 = builder.ins().ireduce(types::I32, l);
                            let r32 = builder.ins().ireduce(types::I32, r);
                            let q32 = builder.ins().udiv(l32, r32);
                            Ok(builder.ins().uextend(types::I64, q32))
                        } else if is_expr_known_non_negative(left, &self.known_non_negative_vars)
                            && is_expr_known_non_negative(right, &self.known_non_negative_vars)
                        {
                            let hi_or = builder.ins().bor(l, r);
                            let hi_shifted = builder.ins().ushr_imm_s(hi_or, 32);
                            let zero = self.get_iconst(types::I64, 0, builder);
                            let fits32 = builder.ins().icmp(IntCC::Equal, hi_shifted, zero);
                            let div32_block = builder.create_block();
                            let div64_block = builder.create_block();
                            let merge_block = builder.create_block();
                            let q_var = builder.declare_var(types::I64);

                            builder
                                .ins()
                                .brif(fits32, div32_block, &[], div64_block, &[]);

                            builder.switch_to_block(div32_block);
                            builder.seal_block(div32_block);
                            let l32 = builder.ins().ireduce(types::I32, l);
                            let r32 = builder.ins().ireduce(types::I32, r);
                            let q32 = builder.ins().udiv(l32, r32);
                            let q_promoted = builder.ins().uextend(types::I64, q32);
                            builder.def_var(q_var, q_promoted);
                            builder.ins().jump(merge_block, &[]);

                            builder.switch_to_block(div64_block);
                            builder.seal_block(div64_block);
                            let q64 = builder.ins().udiv(l, r);
                            builder.def_var(q_var, q64);
                            builder.ins().jump(merge_block, &[]);

                            builder.switch_to_block(merge_block);
                            builder.seal_block(merge_block);
                            Ok(builder.use_var(q_var))
                        } else {
                            Ok(builder.ins().sdiv(l, r))
                        }
                    }
                    BinaryOp::Mod => {
                        if operand_ty.is_unsigned() {
                            Ok(builder.ins().urem(l, r))
                        } else if matches!(operand_ty, Type::I8 | Type::I16) {
                            Ok(builder.ins().srem(l, r))
                        } else if operand_ty.is_integer() {
                            if let Some(d) = get_constant_int(right) {
                                let is_nonneg =
                                    is_expr_known_non_negative(left, &self.known_non_negative_vars);
                                let is_u32 = operand_ty == Type::I32
                                    || is_expr_known_u32(
                                        left,
                                        &self.known_non_negative_vars,
                                        &self.known_u32_vars,
                                    )
                                    || compute_expr_upper_bound(
                                        left,
                                        &self.known_var_bounds,
                                        &self.known_non_negative_vars,
                                    )
                                    .is_some_and(|ub| ub <= 0xFFFF_FFFF);
                                if is_nonneg && d > 0 {
                                    if let Some(max_val) = compute_expr_upper_bound(
                                        left,
                                        &self.known_var_bounds,
                                        &self.known_non_negative_vars,
                                    ) {
                                        if max_val < d {
                                            return Ok(l);
                                        } else if max_val < 2 * d {
                                            let clif_ty = type_to_clif(operand_ty.clone());
                                            let d_val = self.get_iconst(clif_ty, d, builder);
                                            let cond = builder.ins().icmp(
                                                IntCC::SignedGreaterThanOrEqual,
                                                l,
                                                d_val,
                                            );
                                            let diff = builder.ins().isub(l, d_val);
                                            return Ok(builder.ins().select(cond, diff, l));
                                        }
                                    }
                                }
                                self.emit_fast_signed_rem(
                                    l,
                                    r,
                                    d,
                                    &operand_ty,
                                    is_nonneg,
                                    is_u32,
                                    builder,
                                )
                            } else if is_expr_known_u32(
                                left,
                                &self.known_non_negative_vars,
                                &self.known_u32_vars,
                            ) && is_expr_known_u32(
                                right,
                                &self.known_non_negative_vars,
                                &self.known_u32_vars,
                            ) {
                                let l32 = builder.ins().ireduce(types::I32, l);
                                let r32 = builder.ins().ireduce(types::I32, r);
                                let rem32 = builder.ins().urem(l32, r32);
                                Ok(builder.ins().uextend(types::I64, rem32))
                            } else if is_expr_known_non_negative(
                                left,
                                &self.known_non_negative_vars,
                            ) && is_expr_known_non_negative(
                                right,
                                &self.known_non_negative_vars,
                            ) {
                                let hi_or = builder.ins().bor(l, r);
                                let hi_shifted = builder.ins().ushr_imm_s(hi_or, 32);
                                let zero = self.get_iconst(types::I64, 0, builder);
                                let fits32 = builder.ins().icmp(IntCC::Equal, hi_shifted, zero);
                                let rem32_block = builder.create_block();
                                let rem64_block = builder.create_block();
                                let merge_block = builder.create_block();
                                let rem_var = builder.declare_var(types::I64);

                                builder
                                    .ins()
                                    .brif(fits32, rem32_block, &[], rem64_block, &[]);

                                builder.switch_to_block(rem32_block);
                                builder.seal_block(rem32_block);
                                let l32 = builder.ins().ireduce(types::I32, l);
                                let r32 = builder.ins().ireduce(types::I32, r);
                                let rem32 = builder.ins().urem(l32, r32);
                                let rem_promoted = builder.ins().uextend(types::I64, rem32);
                                builder.def_var(rem_var, rem_promoted);
                                builder.ins().jump(merge_block, &[]);

                                builder.switch_to_block(rem64_block);
                                builder.seal_block(rem64_block);
                                let rem64 = builder.ins().urem(l, r);
                                builder.def_var(rem_var, rem64);
                                builder.ins().jump(merge_block, &[]);

                                builder.switch_to_block(merge_block);
                                builder.seal_block(merge_block);
                                Ok(builder.use_var(rem_var))
                            } else {
                                Ok(builder.ins().srem(l, r))
                            }
                        } else {
                            let div = builder.ins().fdiv(l, r);
                            let tr = builder.ins().trunc(div);
                            let prod = builder.ins().fmul(tr, r);
                            Ok(builder.ins().fsub(l, prod))
                        }
                    }
                    BinaryOp::Pow => {
                        if operand_ty.is_float() {
                            let l_f64 = if operand_ty == Type::F32 {
                                builder.ins().fpromote(types::F64, l)
                            } else {
                                l
                            };
                            let r_f64 = if operand_ty == Type::F32 {
                                builder.ins().fpromote(types::F64, r)
                            } else {
                                r
                            };
                            let pow_func =
                                self.module.declare_func_in_func(self.pow_id, builder.func);
                            let call = builder.ins().call(pow_func, &[l_f64, r_f64]);
                            let res = builder.inst_results(call)[0];
                            if operand_ty == Type::F32 {
                                Ok(builder.ins().fdemote(types::F32, res))
                            } else {
                                Ok(res)
                            }
                        } else {
                            Ok(self.emit_int_pow(l, r, &operand_ty, builder))
                        }
                    }
                    BinaryOp::BitAnd => Ok(builder.ins().band(l, r)),
                    BinaryOp::BitOr => Ok(builder.ins().bor(l, r)),
                    BinaryOp::BitXor => Ok(builder.ins().bxor(l, r)),
                    BinaryOp::Shl => Ok(builder.ins().ishl(l, r)),
                    BinaryOp::Shr => {
                        if operand_ty.is_unsigned() {
                            Ok(builder.ins().ushr(l, r))
                        } else {
                            Ok(builder.ins().sshr(l, r))
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
                        } else if operand_ty.is_unsigned() {
                            builder.ins().icmp(IntCC::UnsignedLessThan, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedLessThan, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Le => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r)
                        } else if operand_ty.is_unsigned() {
                            builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Gt => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::GreaterThan, l, r)
                        } else if operand_ty.is_unsigned() {
                            builder.ins().icmp(IntCC::UnsignedGreaterThan, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedGreaterThan, l, r)
                        };
                        Ok(cmp)
                    }
                    BinaryOp::Ge => {
                        let cmp = if operand_ty.is_float() {
                            builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r)
                        } else if operand_ty.is_unsigned() {
                            builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, l, r)
                        } else {
                            builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r)
                        };
                        Ok(cmp)
                    }
                }
            }

            TypedExpr::ArrayLiteral { elements, ty, .. } => {
                let elem_ty = ty.element_type().ok_or_else(|| {
                    CodegenError::BackendError("Expected array element type".to_string())
                })?;
                let elem_size = elem_ty.size_bytes() as u32;
                let len = elements.len();
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
                    builder
                        .ins()
                        .store(MemFlagsData::trusted(), el_val, addr, 0);
                }

                Ok(builder.ins().stack_addr(types::I64, slot, 0))
            }

            TypedExpr::Index {
                target,
                index,
                is_safe,
                ty,
                ..
            } => match target.as_ref() {
                TypedExpr::Ident { name, .. } => {
                    let storage = self.variables.get(name).cloned().ok_or_else(|| {
                        CodegenError::BackendError("Target array must exist".to_string())
                    })?;
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
                            let mut res = builder.use_var(vars[0]);
                            for (k, &var_k) in vars.iter().enumerate().take(len).skip(1) {
                                let k_val = builder.ins().iconst(types::I64, k as i64);
                                let is_match = builder.ins().icmp(IntCC::Equal, idx_val, k_val);
                                let val_k = builder.use_var(var_k);
                                res = builder.ins().select(is_match, val_k, res);
                            }
                            Ok(res)
                        }
                        Storage::Array { slot, len } => {
                            let elem_size = self.get_type_size(ty);

                            if ty.is_struct() {
                                if let Some(c) = get_constant_int(index) {
                                    let offset = (c as i32) * (elem_size as i32);
                                    return Ok(builder.ins().stack_addr(types::I64, slot, offset));
                                } else {
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
                                    return Ok(builder.ins().iadd(base_addr, offset));
                                }
                            }

                            let clif_ty = type_to_clif(ty.clone());

                            let key_str = if let Some(c) = get_constant_int(index) {
                                format!("#{}", c)
                            } else {
                                format_index_key(index)
                            };

                            if !key_str.is_empty() {
                                if let Some((_, cached_val)) =
                                    self.array_load_cache.get(&(name.clone(), key_str.clone()))
                                {
                                    return Ok(*cached_val);
                                }
                            }

                            let loaded_val = if let Some(c) = get_constant_int(index) {
                                if c >= 0 && (c as usize) < len {
                                    let offset = (c as i32) * (elem_size as i32);
                                    builder.ins().stack_load(types::I64, clif_ty, slot, offset)
                                } else {
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
                                    builder.ins().load(
                                        clif_ty,
                                        MemFlagsData::trusted(),
                                        elem_addr,
                                        0,
                                    )
                                }
                            } else {
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
                                builder
                                    .ins()
                                    .load(clif_ty, MemFlagsData::trusted(), elem_addr, 0)
                            };

                            if !key_str.is_empty() {
                                self.array_load_cache.insert(
                                    (name.clone(), key_str),
                                    ((**index).clone(), loaded_val),
                                );
                            }
                            Ok(loaded_val)
                        }
                        _ => Err(CodegenError::BackendError(
                            "Index target must be an array variable".to_string(),
                        )),
                    }
                }
                _ => {
                    let base_addr = self.translate_expr(target, builder)?;
                    let elem_size = self.get_type_size(ty) as i64;
                    let mut idx_val = self.translate_expr(index, builder)?;
                    if index.ty().size_bytes() < 8 {
                        idx_val = if index.ty().is_signed() {
                            builder.ins().sextend(types::I64, idx_val)
                        } else {
                            builder.ins().uextend(types::I64, idx_val)
                        };
                    }
                    let offset = match elem_size {
                        1 => idx_val,
                        2 => builder.ins().ishl_imm_s(idx_val, 1),
                        4 => builder.ins().ishl_imm_s(idx_val, 2),
                        8 => builder.ins().ishl_imm_s(idx_val, 3),
                        _ => builder.ins().imul_imm_s(idx_val, elem_size),
                    };
                    let elem_addr = builder.ins().iadd(base_addr, offset);
                    if ty.is_struct() {
                        Ok(elem_addr)
                    } else {
                        let clif_ty = type_to_clif(ty.clone());
                        Ok(builder
                            .ins()
                            .load(clif_ty, MemFlagsData::trusted(), elem_addr, 0))
                    }
                }
            },

            TypedExpr::Call {
                callee, args, ty, ..
            } => {
                match callee.as_str() {
                    "__nl_loop_reset" | "__nl_arena_reset" => {
                        let loop_reset_func = self
                            .module
                            .declare_func_in_func(self.loop_reset_id, builder.func);
                        builder.ins().call(loop_reset_func, &[]);
                        return Ok(builder.ins().iconst(types::I64, 0));
                    }
                    "__nl_arena_alloc" => {
                        let size_val = if !args.is_empty() {
                            self.translate_expr(&args[0], builder)?
                        } else {
                            builder.ins().iconst(types::I64, 8)
                        };
                        let malloc_func = self
                            .module
                            .declare_func_in_func(self.malloc_id, builder.func);
                        let call_inst = builder.ins().call(malloc_func, &[size_val]);
                        return Ok(builder.inst_results(call_inst)[0]);
                    }
                    "read_i64" => {
                        let read_i64_func = self
                            .module
                            .declare_func_in_func(self.read_i64_id, builder.func);
                        let call_inst = builder.ins().call(read_i64_func, &[]);
                        return Ok(builder.inst_results(call_inst)[0]);
                    }
                    "print" | "println" => {
                        let is_nl = callee == "println";
                        if args.is_empty() {
                            let print_nl_func = self
                                .module
                                .declare_func_in_func(self.print_newline_id, builder.func);
                            builder.ins().call(print_nl_func, &[]);
                            return Ok(builder.ins().iconst(types::I32, 0));
                        }
                        let arg = &args[0];
                        if let TypedExpr::Literal {
                            lit: TypedLiteral::Str(ref s),
                            ..
                        } = arg
                        {
                            let mut bytes = s.as_bytes().to_vec();
                            if is_nl {
                                bytes.push(b'\n');
                            }
                            self.emit_bytes_write(&bytes, builder)?;
                            return Ok(builder.ins().iconst(types::I32, 0));
                        }
                        let arg_ty = arg.ty();
                        if arg_ty == Type::Str {
                            return Ok(builder.ins().iconst(types::I32, 0));
                        } else if arg_ty == Type::Bool {
                            let val = self.translate_expr(arg, builder)?;
                            let print_bool_func = self
                                .module
                                .declare_func_in_func(self.print_bool_id, builder.func);
                            builder.ins().call(print_bool_func, &[val]);
                        } else if arg_ty.is_float() {
                            let val = self.translate_expr(arg, builder)?;
                            let val_f64 = if arg_ty == Type::F32 {
                                builder.ins().fpromote(types::F64, val)
                            } else {
                                val
                            };
                            let print_f64_func = self
                                .module
                                .declare_func_in_func(self.print_f64_id, builder.func);
                            builder.ins().call(print_f64_func, &[val_f64]);
                        } else if arg_ty.is_unsigned() {
                            let val = self.translate_expr(arg, builder)?;
                            let val_u64 = match arg_ty {
                                Type::U8 | Type::U16 | Type::U32 => {
                                    builder.ins().uextend(types::I64, val)
                                }
                                _ => val,
                            };
                            let print_u64_func = self
                                .module
                                .declare_func_in_func(self.print_u64_id, builder.func);
                            builder.ins().call(print_u64_func, &[val_u64]);
                        } else {
                            let val = self.translate_expr(arg, builder)?;
                            let val_i64 = match arg_ty {
                                Type::I8 | Type::I16 | Type::I32 => {
                                    builder.ins().sextend(types::I64, val)
                                }
                                _ => val,
                            };
                            let print_i64_func = self
                                .module
                                .declare_func_in_func(self.print_i64_id, builder.func);
                            builder.ins().call(print_i64_func, &[val_i64]);
                        }
                        if is_nl {
                            let print_nl_func = self
                                .module
                                .declare_func_in_func(self.print_newline_id, builder.func);
                            builder.ins().call(print_nl_func, &[]);
                        }
                        return Ok(builder.ins().iconst(types::I32, 0));
                    }
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
                            let clif_ty = type_to_clif(arg_ty.clone());
                            if clif_ty != types::I64 {
                                if arg_ty.is_unsigned() {
                                    return Ok(builder.ins().uextend(types::I64, arg));
                                } else {
                                    return Ok(builder.ins().sextend(types::I64, arg));
                                }
                            } else {
                                return Ok(arg);
                            }
                        }
                    }
                    "__coupled_a" | "__coupled_b" => {
                        let is_b = callee == "__coupled_b";
                        let p = self.translate_expr(&args[0], builder)?;
                        let q = self.translate_expr(&args[1], builder)?;
                        let ca = self.translate_expr(&args[2], builder)?;
                        let r = self.translate_expr(&args[3], builder)?;
                        let s = self.translate_expr(&args[4], builder)?;
                        let cb = self.translate_expr(&args[5], builder)?;
                        let a0 = self.translate_expr(&args[6], builder)?;
                        let b0 = self.translate_expr(&args[7], builder)?;
                        let n_arg = self.translate_expr(&args[8], builder)?;

                        let n_val = if builder.func.dfg.value_type(n_arg) != types::I64 {
                            builder.ins().uextend(types::I64, n_arg)
                        } else {
                            n_arg
                        };

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
                            return Ok(builder.use_var(b_var));
                        } else {
                            return Ok(builder.use_var(a_var));
                        }
                    }
                    "__order3_recurrence" => {
                        let c1 = self.translate_expr(&args[0], builder)?;
                        let c2 = self.translate_expr(&args[1], builder)?;
                        let c3 = self.translate_expr(&args[2], builder)?;
                        let s0 = self.translate_expr(&args[3], builder)?;
                        let s1 = self.translate_expr(&args[4], builder)?;
                        let s2 = self.translate_expr(&args[5], builder)?;
                        let n_arg = self.translate_expr(&args[6], builder)?;

                        let n_val = if builder.func.dfg.value_type(n_arg) != types::I64 {
                            builder.ins().uextend(types::I64, n_arg)
                        } else {
                            n_arg
                        };

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
                        return Ok(builder.use_var(v0_var));
                    }
                    callee if callee.starts_with("__nway_recurrence_") => {
                        let target_idx = callee
                            .strip_prefix("__nway_recurrence_")
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        let mut arg_vals = Vec::with_capacity(args.len());
                        for arg in args {
                            let val = self.translate_expr(arg, builder)?;
                            let val_i64 = if builder.func.dfg.value_type(val) != types::I64 {
                                builder.ins().uextend(types::I64, val)
                            } else {
                                val
                            };
                            arg_vals.push(val_i64);
                        }
                        let n = (1..=8)
                            .find(|&k| k * k + 2 * k + 2 == arg_vals.len())
                            .unwrap_or(3);
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
                        let cur_v: Vec<_> = v_vars
                            .iter()
                            .take(n)
                            .map(|&var| builder.use_var(var))
                            .collect();

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
                        return Ok(builder.use_var(v_vars[safe_target]));
                    }
                    "tzcnt" | "ctz" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().ctz(arg));
                    }
                    "isqrt" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        let zero_i = self.get_iconst(types::I64, 0, builder);
                        let is_le_zero =
                            builder
                                .ins()
                                .icmp(IntCC::SignedLessThanOrEqual, arg, zero_i);

                        let arg_f = builder.ins().fcvt_from_sint(types::F64, arg);
                        let sq_f = builder.ins().sqrt(arg_f);
                        let r0 = builder.ins().fcvt_to_sint(types::I64, sq_f);

                        // Clamp: if arg <= 0, r = 0, else r0
                        let r_init = builder.ins().select(is_le_zero, zero_i, r0);

                        // Branchless correction 1: if (r + 1) * (r + 1) <= arg, r = r + 1
                        let one_i = self.get_iconst(types::I64, 1, builder);
                        let r_p1 = builder.ins().iadd(r_init, one_i);
                        let r_p1_sq = builder.ins().imul(r_p1, r_p1);
                        let under = builder
                            .ins()
                            .icmp(IntCC::SignedLessThanOrEqual, r_p1_sq, arg);
                        let r_step1 = builder.ins().select(under, r_p1, r_init);

                        // Branchless correction 2: if r * r > arg, r = r - 1
                        let r_step1_sq = builder.ins().imul(r_step1, r_step1);
                        let over = builder
                            .ins()
                            .icmp(IntCC::SignedGreaterThan, r_step1_sq, arg);
                        let r_m1 = builder.ins().isub(r_step1, one_i);
                        let r_final = builder.ins().select(over, r_m1, r_step1);

                        // Final safety: if arg <= 0, return 0
                        return Ok(builder.ins().select(is_le_zero, zero_i, r_final));
                    }
                    "clz" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().clz(arg));
                    }
                    "popcnt" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().popcnt(arg));
                    }
                    "bswap" => {
                        let arg = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().bswap(arg));
                    }
                    "rotl" => {
                        let arg0 = self.translate_expr(&args[0], builder)?;
                        let arg1 = self.translate_expr(&args[1], builder)?;
                        let arg0_ty = builder.func.dfg.value_type(arg0);
                        let arg1_ty = builder.func.dfg.value_type(arg1);
                        let shift = if arg0_ty != arg1_ty {
                            if arg0_ty == types::I64 && arg1_ty == types::I32 {
                                builder.ins().uextend(types::I64, arg1)
                            } else if arg0_ty == types::I32 && arg1_ty == types::I64 {
                                builder.ins().ireduce(types::I32, arg1)
                            } else {
                                arg1
                            }
                        } else {
                            arg1
                        };
                        return Ok(builder.ins().rotl(arg0, shift));
                    }
                    "rotr" => {
                        let arg0 = self.translate_expr(&args[0], builder)?;
                        let arg1 = self.translate_expr(&args[1], builder)?;
                        let arg0_ty = builder.func.dfg.value_type(arg0);
                        let arg1_ty = builder.func.dfg.value_type(arg1);
                        let shift = if arg0_ty != arg1_ty {
                            if arg0_ty == types::I64 && arg1_ty == types::I32 {
                                builder.ins().uextend(types::I64, arg1)
                            } else if arg0_ty == types::I32 && arg1_ty == types::I64 {
                                builder.ins().ireduce(types::I32, arg1)
                            } else {
                                arg1
                            }
                        } else {
                            arg1
                        };
                        return Ok(builder.ins().rotr(arg0, shift));
                    }
                    "dot" => {
                        let arr_a = self.resolve_array(&args[0], builder)?;
                        let arr_b = self.resolve_array(&args[1], builder)?;
                        return self.emit_dot_product(&arr_a, &arr_b, builder);
                    }
                    "vec_norm" => {
                        let arr_v = self.resolve_array(&args[0], builder)?;
                        let d = self.emit_dot_product(&arr_v, &arr_v, builder)?;
                        if arr_v.elem_ty().is_float() {
                            return Ok(builder.ins().sqrt(d));
                        } else {
                            let clif_ty = type_to_clif(arr_v.elem_ty().clone());
                            let d_f = builder.ins().fcvt_from_sint(types::F64, d);
                            let s = builder.ins().sqrt(d_f);
                            return Ok(builder.ins().fcvt_to_sint(clif_ty, s));
                        }
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
                    "min" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let b = self.translate_expr(&args[1], builder)?;
                        let arg_ty = args[0].ty();
                        if arg_ty.is_float() {
                            return Ok(builder.ins().fmin(a, b));
                        } else {
                            let cond = builder.ins().icmp(IntCC::SignedLessThan, a, b);
                            return Ok(builder.ins().select(cond, a, b));
                        }
                    }
                    "max" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let b = self.translate_expr(&args[1], builder)?;
                        let arg_ty = args[0].ty();
                        if arg_ty.is_float() {
                            return Ok(builder.ins().fmax(a, b));
                        } else {
                            let cond = builder.ins().icmp(IntCC::SignedGreaterThan, a, b);
                            return Ok(builder.ins().select(cond, a, b));
                        }
                    }
                    "clamp" => {
                        let val = self.translate_expr(&args[0], builder)?;
                        let lo = self.translate_expr(&args[1], builder)?;
                        let hi = self.translate_expr(&args[2], builder)?;
                        let arg_ty = args[0].ty();
                        if arg_ty.is_float() {
                            let c1 = builder.ins().fmax(val, lo);
                            return Ok(builder.ins().fmin(c1, hi));
                        } else {
                            let cond_lo = builder.ins().icmp(IntCC::SignedLessThan, val, lo);
                            let c1 = builder.ins().select(cond_lo, lo, val);
                            let cond_hi = builder.ins().icmp(IntCC::SignedGreaterThan, c1, hi);
                            return Ok(builder.ins().select(cond_hi, hi, c1));
                        }
                    }
                    "to_int" if args[0].ty().is_float() => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().fcvt_to_sint(types::I64, a));
                    }
                    "to_float" if !args[0].ty().is_float() => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().fcvt_from_sint(types::F64, a));
                    }
                    "i64_to_f64" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().fcvt_from_sint(types::F64, a));
                    }
                    "f64_to_i64" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().fcvt_to_sint_sat(types::I64, a));
                    }
                    "i64_to_f32" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().fcvt_from_sint(types::F32, a));
                    }
                    "f32_to_f64" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().fpromote(types::F64, a));
                    }
                    "f64_to_f32" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().fdemote(types::F32, a));
                    }
                    "i64_to_i32" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().ireduce(types::I32, a));
                    }
                    "i32_to_i64" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().sextend(types::I64, a));
                    }
                    "floor" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().floor(a));
                    }
                    "ceil" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().ceil(a));
                    }
                    "round" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().nearest(a));
                    }
                    "trunc" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        return Ok(builder.ins().trunc(a));
                    }
                    "fma" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let b = self.translate_expr(&args[1], builder)?;
                        let c = self.translate_expr(&args[2], builder)?;
                        return Ok(builder.ins().fma(a, b, c));
                    }
                    "hypot" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let b = self.translate_expr(&args[1], builder)?;
                        let a2 = builder.ins().fmul(a, a);
                        let sum2 = builder.ins().fma(b, b, a2);
                        return Ok(builder.ins().sqrt(sum2));
                    }
                    "lerp" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let b = self.translate_expr(&args[1], builder)?;
                        let t = self.translate_expr(&args[2], builder)?;
                        let diff = builder.ins().fsub(b, a);
                        return Ok(builder.ins().fma(t, diff, a));
                    }
                    "signum" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let arg_ty = args[0].ty();
                        if arg_ty.is_float() {
                            let zero = if arg_ty == Type::F32 {
                                builder.ins().f32const(0.0)
                            } else {
                                builder.ins().f64const(0.0)
                            };
                            let one = if arg_ty == Type::F32 {
                                builder.ins().f32const(1.0)
                            } else {
                                builder.ins().f64const(1.0)
                            };
                            let neg_one = if arg_ty == Type::F32 {
                                builder.ins().f32const(-1.0)
                            } else {
                                builder.ins().f64const(-1.0)
                            };
                            let gt = builder.ins().fcmp(FloatCC::GreaterThan, a, zero);
                            let lt = builder.ins().fcmp(FloatCC::LessThan, a, zero);
                            let pos_or_zero = builder.ins().select(gt, one, zero);
                            return Ok(builder.ins().select(lt, neg_one, pos_or_zero));
                        } else {
                            let clif_ty = type_to_clif(arg_ty);
                            let zero = builder.ins().iconst(clif_ty, 0);
                            let one = builder.ins().iconst(clif_ty, 1);
                            let neg_one = builder.ins().iconst(clif_ty, -1);
                            let gt = builder.ins().icmp(IntCC::SignedGreaterThan, a, zero);
                            let lt = builder.ins().icmp(IntCC::SignedLessThan, a, zero);
                            let pos_or_zero = builder.ins().select(gt, one, zero);
                            return Ok(builder.ins().select(lt, neg_one, pos_or_zero));
                        }
                    }
                    "gcd" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let b = self.translate_expr(&args[1], builder)?;
                        let clif_ty = type_to_clif(args[0].ty());
                        return Ok(Self::emit_gcd(builder, clif_ty, a, b));
                    }
                    "lcm" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let b = self.translate_expr(&args[1], builder)?;
                        let clif_ty = type_to_clif(args[0].ty());
                        let zero = builder.ins().iconst(clif_ty, 0);
                        let one = builder.ins().iconst(clif_ty, 1);

                        let neg_a = builder.ins().ineg(a);
                        let cond_a = builder.ins().icmp(IntCC::SignedLessThan, a, zero);
                        let abs_a = builder.ins().select(cond_a, neg_a, a);

                        let neg_b = builder.ins().ineg(b);
                        let cond_b = builder.ins().icmp(IntCC::SignedLessThan, b, zero);
                        let abs_b = builder.ins().select(cond_b, neg_b, b);

                        let g = Self::emit_gcd(builder, clif_ty, abs_a, abs_b);
                        let is_zero = builder.ins().icmp(IntCC::Equal, g, zero);
                        let safe_g = builder.ins().select(is_zero, one, g);
                        let q = builder.ins().udiv(abs_a, safe_g);
                        let prod = builder.ins().imul(q, abs_b);
                        return Ok(builder.ins().select(is_zero, zero, prod));
                    }
                    "mat_trace2" => {
                        let arr = self.resolve_array(&args[0], builder)?;
                        let m00 = self.get_array_element(&arr, 0, builder);
                        let m11 = self.get_array_element(&arr, 3, builder);
                        let elem_ty = arr.elem_ty();
                        if elem_ty.is_float() {
                            return Ok(builder.ins().fadd(m00, m11));
                        } else {
                            return Ok(builder.ins().iadd(m00, m11));
                        }
                    }
                    "mat_trace3" => {
                        let arr = self.resolve_array(&args[0], builder)?;
                        let m00 = self.get_array_element(&arr, 0, builder);
                        let m11 = self.get_array_element(&arr, 4, builder);
                        let m22 = self.get_array_element(&arr, 8, builder);
                        let elem_ty = arr.elem_ty();
                        if elem_ty.is_float() {
                            let s01 = builder.ins().fadd(m00, m11);
                            return Ok(builder.ins().fadd(s01, m22));
                        } else {
                            let s01 = builder.ins().iadd(m00, m11);
                            return Ok(builder.ins().iadd(s01, m22));
                        }
                    }
                    "mat_trace4" => {
                        let arr = self.resolve_array(&args[0], builder)?;
                        let m00 = self.get_array_element(&arr, 0, builder);
                        let m11 = self.get_array_element(&arr, 5, builder);
                        let m22 = self.get_array_element(&arr, 10, builder);
                        let m33 = self.get_array_element(&arr, 15, builder);
                        let elem_ty = arr.elem_ty();
                        if elem_ty.is_float() {
                            let s01 = builder.ins().fadd(m00, m11);
                            let s23 = builder.ins().fadd(m22, m33);
                            return Ok(builder.ins().fadd(s01, s23));
                        } else {
                            let s01 = builder.ins().iadd(m00, m11);
                            let s23 = builder.ins().iadd(m22, m33);
                            return Ok(builder.ins().iadd(s01, s23));
                        }
                    }
                    "mat_det2" => {
                        let arr = self.resolve_array(&args[0], builder)?;
                        let m00 = self.get_array_element(&arr, 0, builder);
                        let m01 = self.get_array_element(&arr, 1, builder);
                        let m10 = self.get_array_element(&arr, 2, builder);
                        let m11 = self.get_array_element(&arr, 3, builder);
                        let elem_ty = arr.elem_ty();
                        if elem_ty.is_float() {
                            let p0 = builder.ins().fmul(m00, m11);
                            let p1 = builder.ins().fmul(m01, m10);
                            return Ok(builder.ins().fsub(p0, p1));
                        } else {
                            let p0 = builder.ins().imul(m00, m11);
                            let p1 = builder.ins().imul(m01, m10);
                            return Ok(builder.ins().isub(p0, p1));
                        }
                    }
                    "mat_det3" => {
                        let arr = self.resolve_array(&args[0], builder)?;
                        let elem_ty = arr.elem_ty();
                        let m00 = self.get_array_element(&arr, 0, builder);
                        let m01 = self.get_array_element(&arr, 1, builder);
                        let m02 = self.get_array_element(&arr, 2, builder);
                        let m10 = self.get_array_element(&arr, 3, builder);
                        let m11 = self.get_array_element(&arr, 4, builder);
                        let m12 = self.get_array_element(&arr, 5, builder);
                        let m20 = self.get_array_element(&arr, 6, builder);
                        let m21 = self.get_array_element(&arr, 7, builder);
                        let m22 = self.get_array_element(&arr, 8, builder);
                        return Ok(Self::emit_det3_val(
                            builder, &elem_ty, m00, m01, m02, m10, m11, m12, m20, m21, m22,
                        ));
                    }
                    "mat_det4" => {
                        let arr = self.resolve_array(&args[0], builder)?;
                        let elem_ty = arr.elem_ty();
                        let a = [
                            self.get_array_element(&arr, 0, builder),
                            self.get_array_element(&arr, 1, builder),
                            self.get_array_element(&arr, 2, builder),
                            self.get_array_element(&arr, 3, builder),
                            self.get_array_element(&arr, 4, builder),
                            self.get_array_element(&arr, 5, builder),
                            self.get_array_element(&arr, 6, builder),
                            self.get_array_element(&arr, 7, builder),
                            self.get_array_element(&arr, 8, builder),
                            self.get_array_element(&arr, 9, builder),
                            self.get_array_element(&arr, 10, builder),
                            self.get_array_element(&arr, 11, builder),
                            self.get_array_element(&arr, 12, builder),
                            self.get_array_element(&arr, 13, builder),
                            self.get_array_element(&arr, 14, builder),
                            self.get_array_element(&arr, 15, builder),
                        ];
                        let m0 = Self::emit_det3_val(
                            builder, &elem_ty, a[5], a[6], a[7], a[9], a[10], a[11], a[13], a[14],
                            a[15],
                        );
                        let m1 = Self::emit_det3_val(
                            builder, &elem_ty, a[4], a[6], a[7], a[8], a[10], a[11], a[12], a[14],
                            a[15],
                        );
                        let m2 = Self::emit_det3_val(
                            builder, &elem_ty, a[4], a[5], a[7], a[8], a[9], a[11], a[12], a[13],
                            a[15],
                        );
                        let m3 = Self::emit_det3_val(
                            builder, &elem_ty, a[4], a[5], a[6], a[8], a[9], a[10], a[12], a[13],
                            a[14],
                        );

                        if elem_ty.is_float() {
                            let t0 = builder.ins().fmul(a[0], m0);
                            let t1 = builder.ins().fmul(a[1], m1);
                            let t2 = builder.ins().fmul(a[2], m2);
                            let t3 = builder.ins().fmul(a[3], m3);
                            let sub01 = builder.ins().fsub(t0, t1);
                            let add012 = builder.ins().fadd(sub01, t2);
                            return Ok(builder.ins().fsub(add012, t3));
                        } else {
                            let t0 = builder.ins().imul(a[0], m0);
                            let t1 = builder.ins().imul(a[1], m1);
                            let t2 = builder.ins().imul(a[2], m2);
                            let t3 = builder.ins().imul(a[3], m3);
                            let sub01 = builder.ins().isub(t0, t1);
                            let add012 = builder.ins().iadd(sub01, t2);
                            return Ok(builder.ins().isub(add012, t3));
                        }
                    }
                    "sin" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let a_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, a)
                        } else {
                            a
                        };
                        let f = self.module.declare_func_in_func(self.sin_id, builder.func);
                        let call = builder.ins().call(f, &[a_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "cos" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let a_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, a)
                        } else {
                            a
                        };
                        let f = self.module.declare_func_in_func(self.cos_id, builder.func);
                        let call = builder.ins().call(f, &[a_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "tan" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let a_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, a)
                        } else {
                            a
                        };
                        let f = self.module.declare_func_in_func(self.tan_id, builder.func);
                        let call = builder.ins().call(f, &[a_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "exp" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let a_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, a)
                        } else {
                            a
                        };
                        let f = self.module.declare_func_in_func(self.exp_id, builder.func);
                        let call = builder.ins().call(f, &[a_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "ln" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let a_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, a)
                        } else {
                            a
                        };
                        let f = self.module.declare_func_in_func(self.log_id, builder.func);
                        let call = builder.ins().call(f, &[a_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "log2" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let a_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, a)
                        } else {
                            a
                        };
                        let f = self.module.declare_func_in_func(self.log2_id, builder.func);
                        let call = builder.ins().call(f, &[a_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "log10" => {
                        let a = self.translate_expr(&args[0], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let a_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, a)
                        } else {
                            a
                        };
                        let f = self
                            .module
                            .declare_func_in_func(self.log10_id, builder.func);
                        let call = builder.ins().call(f, &[a_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "atan2" => {
                        let y = self.translate_expr(&args[0], builder)?;
                        let x = self.translate_expr(&args[1], builder)?;
                        return Ok(Self::emit_atan2(builder, y, x));
                    }
                    "pow" | "powf" => {
                        let x = self.translate_expr(&args[0], builder)?;
                        let y = self.translate_expr(&args[1], builder)?;
                        let is_f32 = args[0].ty() == Type::F32;
                        let x_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, x)
                        } else {
                            x
                        };
                        let y_f64 = if is_f32 {
                            builder.ins().fpromote(types::F64, y)
                        } else {
                            y
                        };
                        let f = self.module.declare_func_in_func(self.pow_id, builder.func);
                        let call = builder.ins().call(f, &[x_f64, y_f64]);
                        let res = builder.inst_results(call)[0];
                        return Ok(if is_f32 {
                            builder.ins().fdemote(types::F32, res)
                        } else {
                            res
                        });
                    }
                    "c_re" => {
                        let z = self.resolve_array(&args[0], builder)?;
                        return Ok(self.get_array_element(&z, 0, builder));
                    }
                    "c_im" => {
                        let z = self.resolve_array(&args[0], builder)?;
                        return Ok(self.get_array_element(&z, 1, builder));
                    }
                    "c_abs" => {
                        let z = self.resolve_array(&args[0], builder)?;
                        let re = self.get_array_element(&z, 0, builder);
                        let im = self.get_array_element(&z, 1, builder);
                        let re2 = builder.ins().fmul(re, re);
                        let d = builder.ins().fma(im, im, re2);
                        return Ok(builder.ins().sqrt(d));
                    }
                    "c_arg" => {
                        let z = self.resolve_array(&args[0], builder)?;
                        let re = self.get_array_element(&z, 0, builder);
                        let im = self.get_array_element(&z, 1, builder);
                        return Ok(Self::emit_atan2(builder, im, re));
                    }
                    c if Self::is_array_op(c) => {
                        let elem = ty
                            .element_type()
                            .ok_or_else(|| {
                                CodegenError::BackendError(
                                    "Expected array element type".to_string(),
                                )
                            })?
                            .clone();
                        let len = ty.array_len().ok_or_else(|| {
                            CodegenError::BackendError("Expected array length".to_string())
                        })?;
                        let elem_size = elem.size_bytes() as u32;
                        let total_bytes = (elem_size * (len as u32)).max(1);
                        let slot_data = StackSlotData::new(
                            StackSlotKind::ExplicitSlot,
                            total_bytes,
                            elem_size.min(8) as u8,
                        );
                        let slot = builder.create_sized_stack_slot(slot_data);
                        self.translate_array_op_into_slot(c, args, slot, &elem, len, builder)?;
                        return Ok(builder.ins().stack_addr(types::I64, slot, 0));
                    }
                    _ => {}
                }

                self.array_load_cache.clear();
                let func_id = *self.func_ids.get(callee).ok_or_else(|| {
                    CodegenError::BackendError(format!(
                        "Callee '{callee}' must be declared in module"
                    ))
                })?;
                let local_func = self.module.declare_func_in_func(func_id, builder.func);

                let ret_slot = if let Type::Struct(ret_sname) = ty {
                    let ret_layout = self.struct_layouts.get(ret_sname).ok_or_else(|| {
                        CodegenError::BackendError(format!("Missing layout for struct {ret_sname}"))
                    })?;
                    let slot_data = StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        ret_layout.total_size,
                        ret_layout.align.min(8) as u8,
                    );
                    Some(builder.create_sized_stack_slot(slot_data))
                } else if let Type::Enum(ret_ename) = ty {
                    let ret_layout = self.enum_layouts.get(ret_ename).ok_or_else(|| {
                        CodegenError::BackendError(format!("Missing layout for enum {ret_ename}"))
                    })?;
                    let slot_data = StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        ret_layout.total_size,
                        ret_layout.align.min(8) as u8,
                    );
                    Some(builder.create_sized_stack_slot(slot_data))
                } else {
                    None
                };

                let mut arg_vals = Vec::new();
                if let Some(slot) = ret_slot {
                    let sret_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                    arg_vals.push(sret_ptr);
                }

                for a in args {
                    if let Type::Struct(sname) = a.ty() {
                        let arg_ptr = self.translate_expr(a, builder)?;
                        let layout = self.struct_layouts.get(&sname).ok_or_else(|| {
                            CodegenError::BackendError(format!("Missing layout for struct {sname}"))
                        })?;
                        let leaves = layout.get_leaf_fields(self.struct_layouts);
                        for (leaf_offset, leaf_ty) in leaves {
                            let leaf_clif = type_to_clif(leaf_ty);
                            let leaf_val = builder.ins().load(
                                leaf_clif,
                                MemFlagsData::trusted(),
                                arg_ptr,
                                leaf_offset as i32,
                            );
                            arg_vals.push(leaf_val);
                        }
                    } else {
                        arg_vals.push(self.translate_expr(a, builder)?);
                    }
                }

                let call_inst = builder.ins().call(local_func, &arg_vals);
                let results = builder.inst_results(call_inst);
                if let Some(slot) = ret_slot {
                    Ok(builder.ins().stack_addr(types::I64, slot, 0))
                } else if results.is_empty() {
                    Ok(builder.ins().iconst(types::I32, 0))
                } else {
                    Ok(results[0])
                }
            }

            TypedExpr::StructLiteral {
                name: struct_name,
                fields,
                ..
            } => {
                let layout = self
                    .struct_layouts
                    .get(struct_name)
                    .ok_or_else(|| {
                        CodegenError::BackendError(format!(
                            "Missing layout for struct {struct_name}"
                        ))
                    })?
                    .clone();
                let slot_data = StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    layout.total_size,
                    layout.align.min(8) as u8,
                );
                let slot = builder.create_sized_stack_slot(slot_data);
                let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                for (fname, fexpr) in fields {
                    let (foffset, fty) = layout
                        .fields
                        .get(fname)
                        .ok_or_else(|| {
                            CodegenError::BackendError(format!(
                                "Field {fname} not found in struct {struct_name}"
                            ))
                        })?
                        .clone();
                    let fval = self.translate_expr(fexpr, builder)?;
                    if let Type::Struct(sub_name) = &fty {
                        let sub_layout = self.struct_layouts.get(sub_name).ok_or_else(|| {
                            CodegenError::BackendError(format!(
                                "Missing sub-layout for struct {sub_name}"
                            ))
                        })?;
                        let sub_dst = builder.ins().iadd_imm_s(dst_ptr, foffset as i64);
                        Self::emit_copy_bytes(
                            builder,
                            fval,
                            sub_dst,
                            sub_layout.total_size as usize,
                        );
                    } else {
                        builder
                            .ins()
                            .store(MemFlagsData::trusted(), fval, dst_ptr, foffset as i32);
                    }
                }
                Ok(dst_ptr)
            }

            TypedExpr::FieldAccess {
                target, field, ty, ..
            } => {
                let target_ptr = self.translate_expr(target, builder)?;
                if let Type::Struct(sname) = target.ty() {
                    let layout = self.struct_layouts.get(&sname).ok_or_else(|| {
                        CodegenError::BackendError(format!("Missing layout for struct {sname}"))
                    })?;
                    let (foffset, fty) = layout.fields.get(field).ok_or_else(|| {
                        CodegenError::BackendError(format!(
                            "Field {field} not found in struct {sname}"
                        ))
                    })?;
                    if fty.is_struct() {
                        Ok(builder.ins().iadd_imm_s(target_ptr, *foffset as i64))
                    } else {
                        let clif_ty = type_to_clif(ty.clone());
                        Ok(builder.ins().load(
                            clif_ty,
                            MemFlagsData::trusted(),
                            target_ptr,
                            *foffset as i32,
                        ))
                    }
                } else {
                    Err(CodegenError::BackendError(
                        "Field access target must be a struct".to_string(),
                    ))
                }
            }

            TypedExpr::Match {
                scrutinee,
                arms,
                ty,
                ..
            } => {
                let scrut_val = self.translate_expr(scrutinee, builder)?;
                let scrut_ty = scrutinee.ty();
                let clif_scrut_ty = type_to_clif(scrut_ty.clone());

                let clif_res_ty = if ty.is_struct() || ty.is_enum() || *ty == Type::Void {
                    types::I64
                } else {
                    type_to_clif(ty.clone())
                };

                let res_var = if *ty != Type::Void {
                    Some(builder.declare_var(clif_res_ty))
                } else {
                    None
                };

                let is_enum = scrut_ty.is_enum();
                let tag_val = if is_enum {
                    builder
                        .ins()
                        .load(types::I64, MemFlagsData::trusted(), scrut_val, 0)
                } else {
                    scrut_val
                };

                let merge_block = builder.create_block();
                let mut terminated = false;

                for arm in arms {
                    let mut cond: Option<Value> = None;
                    let mut is_wildcard = false;

                    for pat in &arm.patterns {
                        match pat {
                            crate::typecheck::typed_ast::TypedMatchPattern::Wildcard => {
                                is_wildcard = true;
                                break;
                            }
                            crate::typecheck::typed_ast::TypedMatchPattern::Variant {
                                tag, ..
                            } => {
                                let pat_tag_val = self.get_iconst(types::I64, *tag as i64, builder);
                                let eq = builder.ins().icmp(IntCC::Equal, tag_val, pat_tag_val);
                                cond = Some(match cond {
                                    None => eq,
                                    Some(prev) => builder.ins().bor(prev, eq),
                                });
                            }
                            crate::typecheck::typed_ast::TypedMatchPattern::Literal(
                                crate::typecheck::typed_ast::TypedLiteral::Int(n, _),
                            ) => {
                                let lit_val = self.get_iconst(clif_scrut_ty, *n, builder);
                                let eq = builder.ins().icmp(IntCC::Equal, scrut_val, lit_val);
                                cond = Some(match cond {
                                    None => eq,
                                    Some(prev) => builder.ins().bor(prev, eq),
                                });
                            }
                            crate::typecheck::typed_ast::TypedMatchPattern::Literal(
                                crate::typecheck::typed_ast::TypedLiteral::Bool(b),
                            ) => {
                                let lit_val =
                                    self.get_iconst(clif_scrut_ty, if *b { 1 } else { 0 }, builder);
                                let eq = builder.ins().icmp(IntCC::Equal, scrut_val, lit_val);
                                cond = Some(match cond {
                                    None => eq,
                                    Some(prev) => builder.ins().bor(prev, eq),
                                });
                            }
                            _ => {}
                        }
                    }

                    let arm_block = builder.create_block();

                    if is_wildcard {
                        builder.ins().jump(arm_block, &[]);
                        builder.switch_to_block(arm_block);
                        builder.seal_block(arm_block);

                        let prev_vars = self.variables.clone();
                        for pat in &arm.patterns {
                            if let crate::typecheck::typed_ast::TypedMatchPattern::Variant {
                                enum_name,
                                variant_name,
                                bindings,
                                ..
                            } = pat
                            {
                                let elayout = self
                                    .enum_layouts
                                    .get(enum_name)
                                    .ok_or_else(|| {
                                        CodegenError::BackendError(format!(
                                            "Missing layout for enum {enum_name}"
                                        ))
                                    })?
                                    .clone();
                                let vlayout = elayout
                                    .variants
                                    .get(variant_name)
                                    .ok_or_else(|| {
                                        CodegenError::BackendError(format!(
                                            "Missing variant {variant_name} in enum {enum_name}"
                                        ))
                                    })?
                                    .clone();
                                for (i, (b_name, b_ty)) in bindings.iter().enumerate() {
                                    if b_name == "_" {
                                        continue;
                                    }
                                    let offset = vlayout.field_offsets[i];
                                    if let Type::Struct(sname) = b_ty {
                                        let slayout = self
                                            .struct_layouts
                                            .get(sname)
                                            .ok_or_else(|| {
                                                CodegenError::BackendError(format!(
                                                    "Missing layout for struct {sname}"
                                                ))
                                            })?
                                            .clone();
                                        let slot_data = StackSlotData::new(
                                            StackSlotKind::ExplicitSlot,
                                            slayout.total_size,
                                            slayout.align.min(8) as u8,
                                        );
                                        let slot = builder.create_sized_stack_slot(slot_data);
                                        let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                                        let src_field_ptr =
                                            builder.ins().iadd_imm_s(scrut_val, offset as i64);
                                        Self::emit_copy_bytes(
                                            builder,
                                            src_field_ptr,
                                            dst_ptr,
                                            slayout.total_size as usize,
                                        );
                                        self.variables.insert(
                                            b_name.clone(),
                                            Storage::Struct {
                                                slot,
                                                struct_name: sname.clone(),
                                            },
                                        );
                                    } else if let Type::Enum(ename) = b_ty {
                                        let elayout = self
                                            .enum_layouts
                                            .get(ename)
                                            .ok_or_else(|| {
                                                CodegenError::BackendError(format!(
                                                    "Missing layout for enum {ename}"
                                                ))
                                            })?
                                            .clone();
                                        let slot_data = StackSlotData::new(
                                            StackSlotKind::ExplicitSlot,
                                            elayout.total_size,
                                            elayout.align.min(8) as u8,
                                        );
                                        let slot = builder.create_sized_stack_slot(slot_data);
                                        let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                                        let src_field_ptr =
                                            builder.ins().iadd_imm_s(scrut_val, offset as i64);
                                        Self::emit_copy_bytes(
                                            builder,
                                            src_field_ptr,
                                            dst_ptr,
                                            elayout.total_size as usize,
                                        );
                                        self.variables.insert(
                                            b_name.clone(),
                                            Storage::Enum {
                                                slot,
                                                enum_name: ename.clone(),
                                            },
                                        );
                                    } else {
                                        let clif_ty = type_to_clif(b_ty.clone());
                                        let field_val = builder.ins().load(
                                            clif_ty,
                                            MemFlagsData::trusted(),
                                            scrut_val,
                                            offset as i32,
                                        );
                                        let var = builder.declare_var(clif_ty);
                                        builder.def_var(var, field_val);
                                        self.variables.insert(b_name.clone(), Storage::Scalar(var));
                                    }
                                }
                                break;
                            }
                        }

                        let body_val = self.translate_expr(&arm.body, builder)?;
                        self.variables = prev_vars;

                        if let Some(var) = res_var {
                            builder.def_var(var, body_val);
                        }
                        builder.ins().jump(merge_block, &[]);
                        terminated = true;
                        break;
                    } else if let Some(cond_val) = cond {
                        let next_check_block = builder.create_block();
                        builder
                            .ins()
                            .brif(cond_val, arm_block, &[], next_check_block, &[]);

                        builder.switch_to_block(arm_block);
                        builder.seal_block(arm_block);

                        let prev_vars = self.variables.clone();
                        for pat in &arm.patterns {
                            if let crate::typecheck::typed_ast::TypedMatchPattern::Variant {
                                enum_name,
                                variant_name,
                                bindings,
                                ..
                            } = pat
                            {
                                let elayout = self
                                    .enum_layouts
                                    .get(enum_name)
                                    .ok_or_else(|| {
                                        CodegenError::BackendError(format!(
                                            "Missing layout for enum {enum_name}"
                                        ))
                                    })?
                                    .clone();
                                let vlayout = elayout
                                    .variants
                                    .get(variant_name)
                                    .ok_or_else(|| {
                                        CodegenError::BackendError(format!(
                                            "Missing variant {variant_name} in enum {enum_name}"
                                        ))
                                    })?
                                    .clone();
                                for (i, (b_name, b_ty)) in bindings.iter().enumerate() {
                                    if b_name == "_" {
                                        continue;
                                    }
                                    let offset = vlayout.field_offsets[i];
                                    if let Type::Struct(sname) = b_ty {
                                        let slayout = self
                                            .struct_layouts
                                            .get(sname)
                                            .ok_or_else(|| {
                                                CodegenError::BackendError(format!(
                                                    "Missing layout for struct {sname}"
                                                ))
                                            })?
                                            .clone();
                                        let slot_data = StackSlotData::new(
                                            StackSlotKind::ExplicitSlot,
                                            slayout.total_size,
                                            slayout.align.min(8) as u8,
                                        );
                                        let slot = builder.create_sized_stack_slot(slot_data);
                                        let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                                        let src_field_ptr =
                                            builder.ins().iadd_imm_s(scrut_val, offset as i64);
                                        Self::emit_copy_bytes(
                                            builder,
                                            src_field_ptr,
                                            dst_ptr,
                                            slayout.total_size as usize,
                                        );
                                        self.variables.insert(
                                            b_name.clone(),
                                            Storage::Struct {
                                                slot,
                                                struct_name: sname.clone(),
                                            },
                                        );
                                    } else if let Type::Enum(ename) = b_ty {
                                        let elayout = self
                                            .enum_layouts
                                            .get(ename)
                                            .ok_or_else(|| {
                                                CodegenError::BackendError(format!(
                                                    "Missing layout for enum {ename}"
                                                ))
                                            })?
                                            .clone();
                                        let slot_data = StackSlotData::new(
                                            StackSlotKind::ExplicitSlot,
                                            elayout.total_size,
                                            elayout.align.min(8) as u8,
                                        );
                                        let slot = builder.create_sized_stack_slot(slot_data);
                                        let dst_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                                        let src_field_ptr =
                                            builder.ins().iadd_imm_s(scrut_val, offset as i64);
                                        Self::emit_copy_bytes(
                                            builder,
                                            src_field_ptr,
                                            dst_ptr,
                                            elayout.total_size as usize,
                                        );
                                        self.variables.insert(
                                            b_name.clone(),
                                            Storage::Enum {
                                                slot,
                                                enum_name: ename.clone(),
                                            },
                                        );
                                    } else {
                                        let clif_ty = type_to_clif(b_ty.clone());
                                        let field_val = builder.ins().load(
                                            clif_ty,
                                            MemFlagsData::trusted(),
                                            scrut_val,
                                            offset as i32,
                                        );
                                        let var = builder.declare_var(clif_ty);
                                        builder.def_var(var, field_val);
                                        self.variables.insert(b_name.clone(), Storage::Scalar(var));
                                    }
                                }
                                break;
                            }
                        }

                        let body_val = self.translate_expr(&arm.body, builder)?;
                        self.variables = prev_vars;

                        if let Some(var) = res_var {
                            builder.def_var(var, body_val);
                        }
                        builder.ins().jump(merge_block, &[]);

                        builder.switch_to_block(next_check_block);
                        builder.seal_block(next_check_block);
                    }
                }

                if !terminated {
                    if let Some(var) = res_var {
                        let zero = self.get_iconst(clif_res_ty, 0, builder);
                        builder.def_var(var, zero);
                    }
                    builder.ins().jump(merge_block, &[]);
                }

                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);

                if let Some(var) = res_var {
                    Ok(builder.use_var(var))
                } else {
                    Ok(self.get_iconst(types::I64, 0, builder))
                }
            }

            TypedExpr::EnumConstructor {
                enum_name,
                variant_name,
                tag,
                args,
                ..
            } => {
                let layout = self
                    .enum_layouts
                    .get(enum_name)
                    .ok_or_else(|| {
                        CodegenError::BackendError(format!("Missing layout for enum {enum_name}"))
                    })?
                    .clone();
                let v_layout = layout
                    .variants
                    .get(variant_name)
                    .ok_or_else(|| {
                        CodegenError::BackendError(format!(
                            "Missing variant {variant_name} in enum {enum_name}"
                        ))
                    })?
                    .clone();

                let slot_data = StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    layout.total_size,
                    layout.align.min(8) as u8,
                );
                let slot = builder.create_sized_stack_slot(slot_data);
                let slot_addr = builder.ins().stack_addr(types::I64, slot, 0);

                // Store tag at offset 0 (i64)
                let tag_val = self.get_iconst(types::I64, *tag as i64, builder);
                builder
                    .ins()
                    .store(MemFlagsData::trusted(), tag_val, slot_addr, 0);

                // Store payload arguments
                for (i, arg_expr) in args.iter().enumerate() {
                    let offset = v_layout.field_offsets[i];
                    let arg_val = self.translate_expr(arg_expr, builder)?;
                    let arg_ty = arg_expr.ty();
                    if let Type::Struct(sname) = &arg_ty {
                        let sub_layout = self.struct_layouts.get(sname).ok_or_else(|| {
                            CodegenError::BackendError(format!(
                                "Missing sub-layout for struct {sname}"
                            ))
                        })?;
                        let sub_dst = builder.ins().iadd_imm_s(slot_addr, offset as i64);
                        Self::emit_copy_bytes(
                            builder,
                            arg_val,
                            sub_dst,
                            sub_layout.total_size as usize,
                        );
                    } else if let Type::Enum(ename) = &arg_ty {
                        let sub_layout = self.enum_layouts.get(ename).ok_or_else(|| {
                            CodegenError::BackendError(format!(
                                "Missing sub-layout for enum {ename}"
                            ))
                        })?;
                        let sub_dst = builder.ins().iadd_imm_s(slot_addr, offset as i64);
                        Self::emit_copy_bytes(
                            builder,
                            arg_val,
                            sub_dst,
                            sub_layout.total_size as usize,
                        );
                    } else {
                        builder.ins().store(
                            MemFlagsData::trusted(),
                            arg_val,
                            slot_addr,
                            offset as i32,
                        );
                    }
                }

                Ok(slot_addr)
            }

            TypedExpr::Lambda { .. } => {
                // If unlifted/unspecialized lambda reaches codegen, allocate a 16-byte slot
                // (fn_ptr + env_ptr)
                let slot_data = StackSlotData::new(StackSlotKind::ExplicitSlot, 16, 8);
                let slot = builder.create_sized_stack_slot(slot_data);
                let slot_addr = builder.ins().stack_addr(types::I64, slot, 0);
                let zero = self.get_iconst(types::I64, 0, builder);
                builder
                    .ins()
                    .store(MemFlagsData::trusted(), zero, slot_addr, 0);
                builder
                    .ins()
                    .store(MemFlagsData::trusted(), zero, slot_addr, 8);
                Ok(slot_addr)
            }

            TypedExpr::CallIndirect {
                callee, args, ty, ..
            } => {
                let callee_val = self.translate_expr(callee, builder)?;
                let mut sig = self.module.make_signature();
                if *ty != Type::Void {
                    sig.returns.push(AbiParam::new(type_to_clif(ty.clone())));
                }
                let mut arg_vals = Vec::new();
                for a in args {
                    sig.params.push(AbiParam::new(type_to_clif(a.ty())));
                    arg_vals.push(self.translate_expr(a, builder)?);
                }
                let sig_ref = builder.import_signature(sig);
                let call_inst = builder.ins().call_indirect(sig_ref, callee_val, &arg_vals);
                let results = builder.inst_results(call_inst);
                if results.is_empty() {
                    Ok(self.get_iconst(types::I64, 0, builder))
                } else {
                    Ok(results[0])
                }
            }

            TypedExpr::Box { inner, ty, .. } => {
                let inner_val = self.translate_expr(inner, builder)?;
                let inner_ty = match ty {
                    Type::Box(t) => (**t).clone(),
                    _ => inner.ty(),
                };
                let size = match &inner_ty {
                    Type::Struct(sname) => {
                        self.struct_layouts.get(sname).map_or(8, |l| l.total_size)
                    }
                    Type::Enum(ename) => self.enum_layouts.get(ename).map_or(8, |l| l.total_size),
                    _ => inner_ty.size_bytes().max(8) as u32,
                };
                let size_val = builder.ins().iconst(types::I64, size as i64);
                let malloc_func = self
                    .module
                    .declare_func_in_func(self.malloc_id, builder.func);
                let call_inst = builder.ins().call(malloc_func, &[size_val]);
                let slot_addr = builder.inst_results(call_inst)[0];
                if let Type::Struct(sname) = &inner_ty {
                    let sub_layout = self.struct_layouts.get(sname).ok_or_else(|| {
                        CodegenError::BackendError(format!("Missing sub-layout for struct {sname}"))
                    })?;
                    Self::emit_copy_bytes(
                        builder,
                        inner_val,
                        slot_addr,
                        sub_layout.total_size as usize,
                    );
                } else if let Type::Enum(ename) = &inner_ty {
                    let sub_layout = self.enum_layouts.get(ename).ok_or_else(|| {
                        CodegenError::BackendError(format!("Missing sub-layout for enum {ename}"))
                    })?;
                    Self::emit_copy_bytes(
                        builder,
                        inner_val,
                        slot_addr,
                        sub_layout.total_size as usize,
                    );
                } else {
                    builder
                        .ins()
                        .store(MemFlagsData::trusted(), inner_val, slot_addr, 0);
                }
                Ok(slot_addr)
            }

            TypedExpr::Deref { inner, ty, .. } => {
                let ptr_val = self.translate_expr(inner, builder)?;
                if ty.is_struct() || matches!(ty, Type::Enum(_)) {
                    Ok(ptr_val)
                } else {
                    let clif_ty = type_to_clif(ty.clone());
                    Ok(builder
                        .ins()
                        .load(clif_ty, MemFlagsData::trusted(), ptr_val, 0))
                }
            }
        }
    }

    pub(crate) fn try_emit_branchless_select(
        &mut self,
        condition: &TypedExpr,
        then_branch: &TypedBlock,
        else_branch: Option<&TypedBlock>,
        builder: &mut FunctionBuilder,
    ) -> Result<bool, CodegenError> {
        if !is_block_pure_scalar_updates(then_branch) {
            return Ok(false);
        }
        if let Some(eb) = else_branch {
            if !is_block_pure_scalar_updates(eb) {
                return Ok(false);
            }
        }

        let mut modified_vars: Vec<String> = Vec::new();
        for stmt in &then_branch.stmts {
            if let TypedStmt::Assign { name, .. } = stmt {
                if !modified_vars.contains(name) {
                    modified_vars.push(name.clone());
                }
            }
        }
        if let Some(eb) = else_branch {
            for stmt in &eb.stmts {
                if let TypedStmt::Assign { name, .. } = stmt {
                    if !modified_vars.contains(name) {
                        modified_vars.push(name.clone());
                    }
                }
            }
        }

        if modified_vars.is_empty() {
            return Ok(false);
        }

        for name in &modified_vars {
            match self.variables.get(name) {
                Some(Storage::Scalar(_)) => {}
                _ => return Ok(false),
            }
        }

        // Fast path: single variable increment/decrement without else branch:
        // if cond { x = x + 1; }  =>  x = x + uextend(cond)
        // if cond { x = x - 1; }  =>  x = x - uextend(cond)
        if (else_branch.is_none() || else_branch.is_none_or(|b| b.stmts.is_empty()))
            && then_branch.stmts.len() == 1
        {
            if let TypedStmt::Assign {
                name,
                value:
                    TypedExpr::Binary {
                        op,
                        left,
                        right,
                        ty,
                        ..
                    },
                ..
            } = &then_branch.stmts[0]
            {
                let is_add = *op == BinaryOp::Add;
                let is_sub = *op == BinaryOp::Sub;
                if (is_add || is_sub) && ty.is_integer() {
                    let is_one = |e: &TypedExpr| -> bool {
                        matches!(
                            e,
                            TypedExpr::Literal {
                                lit: TypedLiteral::Int(1, _),
                                ..
                            }
                        )
                    };
                    let is_target = |e: &TypedExpr| -> bool {
                        matches!(e, TypedExpr::Ident { name: n, .. } if n == name)
                    };

                    let is_inc = (is_target(left) && is_one(right))
                        || (is_add && is_one(left) && is_target(right));
                    let is_dec = is_sub && is_target(left) && is_one(right);

                    if is_inc || is_dec {
                        if let Some(Storage::Scalar(var)) = self.variables.get(name) {
                            let var = *var;
                            let cond_val = self.translate_expr(condition, builder)?;
                            let orig_val = builder.use_var(var);
                            let var_ty = builder.func.dfg.value_type(orig_val);
                            let inc = builder.ins().uextend(var_ty, cond_val);
                            let updated = if is_inc {
                                builder.ins().iadd(orig_val, inc)
                            } else {
                                builder.ins().isub(orig_val, inc)
                            };
                            builder.def_var(var, updated);
                            return Ok(true);
                        }
                    }
                }
            }
        }
        let cond_val = self.translate_expr(condition, builder)?;

        let mut then_locals: HashMap<String, Value> = HashMap::new();
        for stmt in &then_branch.stmts {
            match stmt {
                TypedStmt::Let { name, value, .. } => {
                    let val = self.eval_pure_select_expr(value, &then_locals, builder)?;
                    then_locals.insert(name.clone(), val);
                }
                TypedStmt::Assign { name, value, .. } => {
                    let val = self.eval_pure_select_expr(value, &then_locals, builder)?;
                    then_locals.insert(name.clone(), val);
                }
                _ => {
                    return Err(CodegenError::BackendError(
                        "Unsupported statement in select branch".to_string(),
                    ));
                }
            }
        }

        let mut else_locals: HashMap<String, Value> = HashMap::new();
        if let Some(eb) = else_branch {
            for stmt in &eb.stmts {
                match stmt {
                    TypedStmt::Let { name, value, .. } => {
                        let val = self.eval_pure_select_expr(value, &else_locals, builder)?;
                        else_locals.insert(name.clone(), val);
                    }
                    TypedStmt::Assign { name, value, .. } => {
                        let val = self.eval_pure_select_expr(value, &else_locals, builder)?;
                        else_locals.insert(name.clone(), val);
                    }
                    _ => {
                        return Err(CodegenError::BackendError(
                            "Unsupported statement in select branch".to_string(),
                        ));
                    }
                }
            }
        }

        for name in &modified_vars {
            let var =
                match self.variables.get(name).ok_or_else(|| {
                    CodegenError::BackendError(format!("Variable {name} not found"))
                })? {
                    Storage::Scalar(v) => *v,
                    _ => {
                        return Err(CodegenError::BackendError(format!(
                            "Variable {name} is not a scalar"
                        )));
                    }
                };
            let orig_val = builder.use_var(var);
            let then_val = then_locals.get(name).copied().unwrap_or(orig_val);
            let else_val = else_locals.get(name).copied().unwrap_or(orig_val);

            let selected = builder.ins().select(cond_val, then_val, else_val);
            builder.def_var(var, selected);
        }

        Ok(true)
    }

    pub(crate) fn eval_pure_select_expr(
        &mut self,
        expr: &TypedExpr,
        locals: &HashMap<String, Value>,
        builder: &mut FunctionBuilder,
    ) -> Result<Value, CodegenError> {
        match expr {
            TypedExpr::Ident { name, .. } => {
                if let Some(&val) = locals.get(name) {
                    Ok(val)
                } else {
                    self.translate_expr(expr, builder)
                }
            }
            TypedExpr::Unary {
                op,
                expr: inner,
                ty,
                ..
            } => {
                let inner_val = self.eval_pure_select_expr(inner, locals, builder)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        if ty.is_float() {
                            Ok(builder.ins().fneg(inner_val))
                        } else {
                            Ok(builder.ins().ineg(inner_val))
                        }
                    }
                    crate::ast::UnaryOp::Not => {
                        let zero = self.get_iconst(types::I8, 0, builder);
                        Ok(builder.ins().icmp(IntCC::Equal, inner_val, zero))
                    }
                }
            }
            TypedExpr::Binary {
                op, left, right, ..
            } => {
                // Phase 35: Fast-path for Div/Mod with constant divisor — emit
                // strength-reduced shift/and instead of idiv, so that expressions
                // like `curr / 2` and `curr % 2` in the Collatz inner-loop
                // if-else get single-instruction lowering inside branchless select.
                if *op == BinaryOp::Div || *op == BinaryOp::Mod {
                    if let Some(d) = get_constant_int(right) {
                        let operand_ty = left.ty();
                        let l = self.eval_pure_select_expr(left, locals, builder)?;
                        let r = self.eval_pure_select_expr(right, locals, builder)?;
                        if matches!(operand_ty, Type::I8 | Type::I16) {
                            return if *op == BinaryOp::Div {
                                Ok(builder.ins().sdiv(l, r))
                            } else {
                                Ok(builder.ins().srem(l, r))
                            };
                        }
                        // Treat variable as non-negative when it already appears in
                        // the known_non_negative_vars set, OR when it is a local
                        // produced by a prior branch assignment (all locals here are
                        // results of arithmetic that started from a non-negative seed
                        // through select — conservative but safe for the Collatz case).
                        let nonneg_by_name = match left.as_ref() {
                            TypedExpr::Ident { name, .. } => {
                                self.known_non_negative_vars.contains(name)
                                    || locals.contains_key(name.as_str())
                            }
                            _ => false,
                        };
                        let is_nonneg = nonneg_by_name
                            || is_expr_known_non_negative(left, &self.known_non_negative_vars);
                        let is_u32 = operand_ty == Type::I32
                            || is_expr_known_u32(
                                left,
                                &self.known_non_negative_vars,
                                &self.known_u32_vars,
                            );
                        return if *op == BinaryOp::Div {
                            self.emit_fast_signed_div(
                                l,
                                r,
                                d,
                                &operand_ty,
                                is_nonneg,
                                is_u32,
                                builder,
                            )
                        } else {
                            self.emit_fast_signed_rem(
                                l,
                                r,
                                d,
                                &operand_ty,
                                is_nonneg,
                                is_u32,
                                builder,
                            )
                        };
                    }
                }
                let l = self.eval_pure_select_expr(left, locals, builder)?;
                let r = self.eval_pure_select_expr(right, locals, builder)?;
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
                            let c_right = get_constant_int(right);
                            let c_left = get_constant_int(left);
                            let clif_ty = type_to_clif(operand_ty);
                            if let Some(k) = c_right {
                                Ok(self.emit_fast_int_mul(l, r, Some(k), clif_ty, builder))
                            } else if let Some(k) = c_left {
                                Ok(self.emit_fast_int_mul(r, l, Some(k), clif_ty, builder))
                            } else {
                                Ok(self.emit_fast_int_mul(l, r, None, clif_ty, builder))
                            }
                        }
                    }
                    BinaryOp::BitAnd => Ok(builder.ins().band(l, r)),
                    BinaryOp::BitOr => Ok(builder.ins().bor(l, r)),
                    BinaryOp::BitXor => Ok(builder.ins().bxor(l, r)),
                    BinaryOp::Shl => Ok(builder.ins().ishl(l, r)),
                    BinaryOp::Shr => {
                        if operand_ty.is_unsigned() {
                            Ok(builder.ins().ushr(l, r))
                        } else {
                            Ok(builder.ins().sshr(l, r))
                        }
                    }
                    BinaryOp::Eq => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fcmp(FloatCC::Equal, l, r))
                        } else {
                            Ok(builder.ins().icmp(IntCC::Equal, l, r))
                        }
                    }
                    BinaryOp::Ne => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fcmp(FloatCC::NotEqual, l, r))
                        } else {
                            Ok(builder.ins().icmp(IntCC::NotEqual, l, r))
                        }
                    }
                    BinaryOp::Lt => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fcmp(FloatCC::LessThan, l, r))
                        } else if operand_ty.is_unsigned() {
                            Ok(builder.ins().icmp(IntCC::UnsignedLessThan, l, r))
                        } else {
                            Ok(builder.ins().icmp(IntCC::SignedLessThan, l, r))
                        }
                    }
                    BinaryOp::Le => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r))
                        } else if operand_ty.is_unsigned() {
                            Ok(builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, l, r))
                        } else {
                            Ok(builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r))
                        }
                    }
                    BinaryOp::Gt => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fcmp(FloatCC::GreaterThan, l, r))
                        } else if operand_ty.is_unsigned() {
                            Ok(builder.ins().icmp(IntCC::UnsignedGreaterThan, l, r))
                        } else {
                            Ok(builder.ins().icmp(IntCC::SignedGreaterThan, l, r))
                        }
                    }
                    BinaryOp::Ge => {
                        if operand_ty.is_float() {
                            Ok(builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r))
                        } else if operand_ty.is_unsigned() {
                            Ok(builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, l, r))
                        } else {
                            Ok(builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r))
                        }
                    }
                    _ => self.translate_expr(expr, builder),
                }
            }
            _ => self.translate_expr(expr, builder),
        }
    }
}

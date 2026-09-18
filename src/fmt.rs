use std::collections::HashMap;

use crate::ast::{
    BinaryOp, Block, Expr, Function, Item, Literal, MatchPattern, Program, Stmt, StructDef,
    UnaryOp,
};
use crate::parser::parse;
use crate::span::Span;
use crate::token::tokenize;
use crate::typecheck::typecheck;
use crate::typecheck::typed_ast::{TypedBlock, TypedProgram, TypedStmt};
use crate::typecheck::types::Type;

fn collect_inferred_types(tp: &TypedProgram, map: &mut HashMap<Span, Type>) {
    for func in &tp.functions {
        collect_inferred_types_block(&func.body, map);
    }
}

fn collect_inferred_types_block(block: &TypedBlock, map: &mut HashMap<Span, Type>) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { ty, span, .. } => {
                map.insert(*span, ty.clone());
            }
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_inferred_types_block(then_branch, map);
                if let Some(eb) = else_branch {
                    collect_inferred_types_block(eb, map);
                }
            }
            TypedStmt::While { body, .. } | TypedStmt::For { body, .. } => {
                collect_inferred_types_block(body, map);
            }
            _ => {}
        }
    }
}

pub fn format_literal(lit: &Literal) -> String {
    match lit {
        Literal::Int(n) => n.to_string(),
        Literal::TypedInt(n, s) => format!("{}{}", n, s),
        Literal::Float(f) => {
            let mut s = f.to_string();
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                s.push_str(".0");
            }
            s
        }
        Literal::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Literal::Str(s) => format!("\"{}\"", s),
    }
}

pub fn format_pattern(pattern: &MatchPattern) -> String {
    match pattern {
        MatchPattern::Literal(lit) => format_literal(lit),
        MatchPattern::Wildcard => "_".to_string(),
    }
}

pub fn format_expr(expr: &Expr) -> String {
    format_expr_at(expr, 1)
}

pub fn format_expr_at(expr: &Expr, depth: usize) -> String {
    match expr {
        Expr::Literal(lit, _) => format_literal(lit),
        Expr::Ident(name, _) => name.clone(),
        Expr::Unary { op, expr, .. } => {
            let op_str = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            format!("{}{}", op_str, format_expr_at(expr, depth))
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            let op_str = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Pow => "**",
                BinaryOp::BitAnd => "&",
                BinaryOp::BitOr => "|",
                BinaryOp::BitXor => "^",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
            };
            format!(
                "{} {} {}",
                format_expr_at(left, depth),
                op_str,
                format_expr_at(right, depth)
            )
        }
        Expr::Call { callee, args, .. } => {
            let args_str = args
                .iter()
                .map(|a| format_expr_at(a, depth))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", callee, args_str)
        }
        Expr::Group(inner, _) => format!("({})", format_expr_at(inner, depth)),
        Expr::ArrayLiteral { elements, .. } => {
            let elems_str = elements
                .iter()
                .map(|e| format_expr_at(e, depth))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", elems_str)
        }
        Expr::Index { target, index, .. } => {
            format!(
                "{}[{}]",
                format_expr_at(target, depth),
                format_expr_at(index, depth)
            )
        }
        Expr::FieldAccess { target, field, .. } => {
            format!("{}.{}", format_expr_at(target, depth), field)
        }
        Expr::StructLiteral { name, fields, .. } => {
            if fields.is_empty() {
                format!("{} {{}}", name)
            } else {
                let f_str = fields
                    .iter()
                    .map(|(f, v)| format!("{}: {}", f, format_expr_at(v, depth)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} {{ {} }}", name, f_str)
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            let mut s = format!("match {} {{\n", format_expr_at(scrutinee, depth));
            let arm_indent = " ".repeat((depth + 1) * 4);
            for arm in arms {
                let pats = arm
                    .patterns
                    .iter()
                    .map(format_pattern)
                    .collect::<Vec<_>>()
                    .join(" | ");
                s.push_str(&format!(
                    "{arm_indent}{pats} => {},\n",
                    format_expr_at(&arm.body, depth + 1)
                ));
            }
            let close_indent = " ".repeat(depth * 4);
            s.push_str(&format!("{close_indent}}}"));
            s
        }
    }
}

fn format_block(
    block: &Block,
    depth: usize,
    inferred_types: &HashMap<Span, Type>,
    out: &mut String,
) {
    for stmt in &block.stmts {
        format_stmt(stmt, depth, inferred_types, out);
    }
}

fn format_stmt(
    stmt: &Stmt,
    depth: usize,
    inferred_types: &HashMap<Span, Type>,
    out: &mut String,
) {
    let indent = " ".repeat(depth * 4);
    match stmt {
        Stmt::Let {
            name,
            is_mutable,
            ty,
            value,
            span,
        } => {
            let mut_str = if *is_mutable { "mut " } else { "" };
            let ty_str = if let Some(ref t) = ty {
                format!(": {}", t)
            } else if let Some(inferred) = inferred_types.get(span) {
                format!(": {}", inferred)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{indent}let {mut_str}{name}{ty_str} = {};\n",
                format_expr_at(value, depth)
            ));
        }
        Stmt::Assign { name, value, .. } => {
            out.push_str(&format!(
                "{indent}{name} = {};\n",
                format_expr_at(value, depth)
            ));
        }
        Stmt::IndexAssign {
            target,
            index,
            value,
            ..
        } => {
            out.push_str(&format!(
                "{indent}{target}[{}] = {};\n",
                format_expr_at(index, depth),
                format_expr_at(value, depth)
            ));
        }
        Stmt::FieldAssign {
            target,
            field,
            value,
            ..
        } => {
            out.push_str(&format!(
                "{indent}{target}.{field} = {};\n",
                format_expr_at(value, depth)
            ));
        }
        Stmt::Return(opt_expr, ..) => {
            if let Some(ref e) = opt_expr {
                out.push_str(&format!("{indent}return {};\n", format_expr_at(e, depth)));
            } else {
                out.push_str(&format!("{indent}return;\n"));
            }
        }
        Stmt::Break(_) => {
            out.push_str(&format!("{indent}break;\n"));
        }
        Stmt::Continue(_) => {
            out.push_str(&format!("{indent}continue;\n"));
        }
        Stmt::For {
            var,
            lo,
            hi,
            inclusive,
            body,
            ..
        } => {
            let dotdot = if *inclusive { "..=" } else { ".." };
            out.push_str(&format!(
                "{indent}for {var} in {}{dotdot}{} {{\n",
                format_expr_at(lo, depth),
                format_expr_at(hi, depth)
            ));
            format_block(body, depth + 1, inferred_types, out);
            out.push_str(&format!("{indent}}}\n"));
        }
        Stmt::While {
            condition, body, ..
        } => {
            out.push_str(&format!(
                "{indent}while {} {{\n",
                format_expr_at(condition, depth)
            ));
            format_block(body, depth + 1, inferred_types, out);
            out.push_str(&format!("{indent}}}\n"));
        }
        Stmt::Loop { body, .. } => {
            out.push_str(&format!("{indent}loop {{\n"));
            format_block(body, depth + 1, inferred_types, out);
            out.push_str(&format!("{indent}}}\n"));
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            out.push_str(&format!("{indent}if {} {{\n", format_expr_at(condition, depth)));
            format_block(then_branch, depth + 1, inferred_types, out);

            let mut current_else = else_branch.as_ref();
            while let Some(eb) = current_else {
                if eb.stmts.len() == 1 {
                    if let Stmt::If {
                        condition: ref c,
                        then_branch: ref tb,
                        else_branch: ref next_eb,
                        ..
                    } = eb.stmts[0]
                    {
                        out.push_str(&format!("{indent}}} else if {} {{\n", format_expr_at(c, depth)));
                        format_block(tb, depth + 1, inferred_types, out);
                        current_else = next_eb.as_ref();
                        continue;
                    }
                }
                out.push_str(&format!("{indent}}} else {{\n"));
                format_block(eb, depth + 1, inferred_types, out);
                break;
            }
            out.push_str(&format!("{indent}}}\n"));
        }
        Stmt::Expr(expr) => {
            out.push_str(&format!("{indent}{};\n", format_expr_at(expr, depth)));
        }
    }
}

fn format_struct(s: &StructDef, out: &mut String) {
    if let Some(ref doc) = s.doc_comment {
        for line in doc.lines() {
            if line.is_empty() {
                out.push_str("///\n");
            } else {
                out.push_str(&format!("/// {}\n", line));
            }
        }
    }
    if s.fields.is_empty() {
        out.push_str(&format!("struct {} {{}}\n", s.name));
    } else {
        out.push_str(&format!("struct {} {{\n", s.name));
        for (f_name, f_ty) in &s.fields {
            out.push_str(&format!("    {}: {},\n", f_name, f_ty));
        }
        out.push_str("}\n");
    }
}

fn format_function(
    f: &Function,
    inferred_types: &HashMap<Span, Type>,
    out: &mut String,
) {
    if let Some(ref doc) = f.doc_comment {
        for line in doc.lines() {
            if line.is_empty() {
                out.push_str("///\n");
            } else {
                out.push_str(&format!("/// {}\n", line));
            }
        }
    }
    let params_str = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, p.ty))
        .collect::<Vec<_>>()
        .join(", ");
    let ret_str = if let Some(ref r) = f.return_ty {
        format!(" -> {}", r)
    } else {
        String::new()
    };
    out.push_str(&format!("fn {}({}){} {{\n", f.name, params_str, ret_str));
    format_block(&f.body, 1, inferred_types, out);
    out.push_str("}\n");
}

pub fn format_program(program: &Program, inferred_types: &HashMap<Span, Type>) -> String {
    let mut out = String::new();

    if !program.items.is_empty() {
        for (i, item) in program.items.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match item {
                Item::Function(f) => format_function(f, inferred_types, &mut out),
                Item::Struct(s) => format_struct(s, &mut out),
            }
        }
    } else {
        let mut first = true;
        for s in &program.structs {
            if !first {
                out.push('\n');
            }
            first = false;
            format_struct(s, &mut out);
        }
        for f in &program.functions {
            if !first {
                out.push('\n');
            }
            first = false;
            format_function(f, inferred_types, &mut out);
        }
    }

    out
}

pub fn format_source(source: &str) -> Result<String, String> {
    let tokens = tokenize(source).map_err(|e| format!("Lex error: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("Parse error: {:?}", e))?;

    let mut inferred_types = HashMap::new();
    if let Ok(typed_program) = typecheck(&program) {
        collect_inferred_types(&typed_program, &mut inferred_types);
    }

    Ok(format_program(&program, &inferred_types))
}

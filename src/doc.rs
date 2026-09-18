use crate::ast::Program;

pub fn generate_doc(program: &Program) -> String {
    let mut out = String::new();
    out.push_str("# API Reference\n\n");

    if !program.structs.is_empty() {
        out.push_str("## Structs\n\n");
        for s in &program.structs {
            out.push_str(&format!("### `struct {}`\n\n", s.name));
            if let Some(ref doc) = s.doc_comment {
                out.push_str(doc);
                out.push_str("\n\n");
            } else {
                out.push_str("_No description provided._\n\n");
            }

            if !s.fields.is_empty() {
                out.push_str("| Field | Type |\n");
                out.push_str("| --- | --- |\n");
                for (f_name, f_ty) in &s.fields {
                    out.push_str(&format!("| `{}` | `{}` |\n", f_name, f_ty));
                }
                out.push('\n');
            }
        }
    }

    if !program.functions.is_empty() {
        out.push_str("## Functions\n\n");
        for f in &program.functions {
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
            out.push_str(&format!("### `fn {}({}){}`\n\n", f.name, params_str, ret_str));
            if let Some(ref doc) = f.doc_comment {
                out.push_str(doc);
                out.push_str("\n\n");
            } else {
                out.push_str("_No description provided._\n\n");
            }
        }
    }

    out
}

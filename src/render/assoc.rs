//! Shared rendering of associated items (methods, associated constants, and
//! associated types) used by both the type and trait renderers.

use std::path::Path;

use rustdoc_types::{Item, ItemEnum};

use super::{Ctx, doc_text, signature};

/// Render one associated item as a `heading`-level subsection: a signature code
/// block followed by its docs. `pub_kw` controls whether methods are prefixed
/// with `pub` (true for inherent impls, false for trait items).
pub fn render(ctx: &Ctx, out: &mut String, file: &Path, raw: &Item, heading: &str, pub_kw: bool) {
    let Some(name) = &raw.name else { return };
    let Some(code) = signature(name, raw, pub_kw) else {
        return;
    };

    out.push_str(&format!("{heading} `{name}`\n\n"));
    out.push_str(&format!("```rust\n{code}\n```\n\n"));

    if let Some(docs) = &raw.docs
        && !docs.is_empty()
    {
        let level = heading.chars().take_while(|&c| c == '#').count();
        out.push_str(&doc_text::render_docs(docs, level));
        out.push_str("\n\n");
    }
    let defs = ctx.intra_doc_definitions(file, raw);
    if !defs.is_empty() {
        out.push_str(&defs);
        out.push('\n');
    }
}

fn signature(name: &str, raw: &Item, pub_kw: bool) -> Option<String> {
    match &raw.inner {
        ItemEnum::Function(func) => {
            let prefix = if pub_kw { "pub " } else { "" };
            Some(format!(
                "{prefix}{}",
                signature::function_signature(name, func)
            ))
        }
        ItemEnum::AssocConst { type_, value } => {
            let mut s = format!("const {name}: {}", signature::type_str(type_));
            if let Some(v) = value {
                s.push_str(&format!(" = {v}"));
            }
            Some(s)
        }
        ItemEnum::AssocType { bounds, type_, .. } => {
            let mut s = format!("type {name}");
            if !bounds.is_empty() {
                s.push_str(&format!(": {}", signature::bounds_str(bounds)));
            }
            if let Some(default) = type_ {
                s.push_str(&format!(" = {}", signature::type_str(default)));
            }
            Some(s)
        }
        _ => None,
    }
}

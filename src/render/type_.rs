//! Render a type's markdown file: its definition, inherent impls (with methods),
//! and the list of traits it implements (auto traits and blanket impls omitted).

use std::path::Path;

use rustdoc_types::{Id, Impl, Item, ItemEnum, Struct, StructKind, Type, VariantKind};

use crate::model::DocItem;

use super::{Ctx, assoc, doc_text, signature};

pub fn render(ctx: &Ctx, item: &DocItem) -> String {
    let file = item.file.as_deref().unwrap_or(Path::new(""));
    let Some(raw) = ctx.raw(item.id) else {
        return String::new();
    };

    let (keyword, definition, impls) = match &raw.inner {
        ItemEnum::Struct(s) => ("Struct", struct_def(ctx, &item.name, s), s.impls.clone()),
        ItemEnum::Enum(e) => ("Enum", enum_def(ctx, &item.name, e), e.impls.clone()),
        ItemEnum::Union(u) => (
            "Union",
            union_def(ctx, &item.name, &u.generics, &u.fields, u.has_stripped_fields),
            u.impls.clone(),
        ),
        _ => return String::new(),
    };

    let mut out = String::new();
    let path = item.canonical.0.join("::");
    out.push_str(&format!("# {keyword} `{path}`\n\n"));

    if let Some(src) = ctx.source_ref(raw) {
        out.push_str(&format!("{src}\n\n"));
    }
    if let Some(docs) = &raw.docs {
        if !docs.is_empty() {
            out.push_str(&doc_text::render_docs(docs, 1));
            out.push_str("\n\n");
        }
    }
    let defs = ctx.intra_doc_definitions(file, raw);
    if !defs.is_empty() {
        out.push_str(&defs);
        out.push('\n');
    }

    out.push_str("## Definition\n\n");
    out.push_str(&format!("```rust\n{definition}\n```\n\n"));

    render_impls(ctx, &mut out, file, &impls);

    out.trim_end().to_string() + "\n"
}

// --- Definitions ----------------------------------------------------------

fn struct_def(ctx: &Ctx, name: &str, s: &Struct) -> String {
    let generics = signature::generics_decl(&s.generics);
    let where_clause = signature::where_clause(&s.generics);
    match &s.kind {
        StructKind::Unit => format!("pub struct {name}{generics}{where_clause};"),
        StructKind::Tuple(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|f| match f {
                    Some(id) => field_type(ctx, *id)
                        .map(|t| format!("pub {t}"))
                        .unwrap_or_else(|| "/* private */".to_string()),
                    None => "/* private */".to_string(),
                })
                .collect();
            format!("pub struct {name}{generics}({}){where_clause};", parts.join(", "))
        }
        StructKind::Plain {
            fields,
            has_stripped_fields,
        } => {
            let body = named_fields(ctx, fields, *has_stripped_fields, true);
            format!("pub struct {name}{generics}{where_clause} {{\n{body}}}")
        }
    }
}

fn union_def(
    ctx: &Ctx,
    name: &str,
    generics: &rustdoc_types::Generics,
    fields: &[Id],
    has_stripped_fields: bool,
) -> String {
    let generics_str = signature::generics_decl(generics);
    let where_clause = signature::where_clause(generics);
    let body = named_fields(ctx, fields, has_stripped_fields, true);
    format!("pub union {name}{generics_str}{where_clause} {{\n{body}}}")
}

fn enum_def(ctx: &Ctx, name: &str, e: &rustdoc_types::Enum) -> String {
    let generics = signature::generics_decl(&e.generics);
    let where_clause = signature::where_clause(&e.generics);
    let mut body = String::new();
    for &vid in &e.variants {
        let Some(item) = ctx.raw(vid) else { continue };
        let ItemEnum::Variant(variant) = &item.inner else {
            continue;
        };
        let Some(vname) = &item.name else { continue };
        push_doc_comment(&mut body, item.docs.as_deref(), "    ");
        let shape = match &variant.kind {
            VariantKind::Plain => String::new(),
            VariantKind::Tuple(fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|f| {
                        f.and_then(|id| field_type(ctx, id))
                            .unwrap_or_else(|| "/* private */".to_string())
                    })
                    .collect();
                format!("({})", parts.join(", "))
            }
            VariantKind::Struct {
                fields,
                has_stripped_fields,
            } => {
                // Enum-variant fields take no `pub` keyword.
                let inner = named_fields(ctx, fields, *has_stripped_fields, false);
                format!(" {{\n{}    }}", indent_block(&inner))
            }
        };
        let disc = variant
            .discriminant
            .as_ref()
            .map(|d| format!(" = {}", d.expr))
            .unwrap_or_default();
        body.push_str(&format!("    {vname}{shape}{disc},\n"));
    }
    if e.has_stripped_variants {
        body.push_str("    // some variants omitted\n");
    }
    format!("pub enum {name}{generics}{where_clause} {{\n{body}}}")
}

/// Render named fields as `[pub ]name: Type,` lines with their doc comments.
/// Struct and union fields take `pub`; enum-variant fields do not.
fn named_fields(ctx: &Ctx, fields: &[Id], has_stripped_fields: bool, pub_prefix: bool) -> String {
    let prefix = if pub_prefix { "pub " } else { "" };
    let mut body = String::new();
    for &fid in fields {
        let Some(item) = ctx.raw(fid) else { continue };
        let ItemEnum::StructField(ty) = &item.inner else {
            continue;
        };
        let Some(fname) = &item.name else { continue };
        push_doc_comment(&mut body, item.docs.as_deref(), "    ");
        body.push_str(&format!("    {prefix}{fname}: {},\n", signature::type_str(ty)));
    }
    if has_stripped_fields {
        body.push_str("    // some fields omitted\n");
    }
    body
}

fn field_type(ctx: &Ctx, id: Id) -> Option<String> {
    match &ctx.raw(id)?.inner {
        ItemEnum::StructField(ty) => Some(signature::type_str(ty)),
        _ => None,
    }
}

fn push_doc_comment(out: &mut String, docs: Option<&str>, indent: &str) {
    let Some(docs) = docs else { return };
    for line in docs.lines() {
        if line.is_empty() {
            out.push_str(&format!("{indent}///\n"));
        } else {
            out.push_str(&format!("{indent}/// {line}\n"));
        }
    }
}

fn indent_block(block: &str) -> String {
    block
        .lines()
        .map(|l| format!("    {l}\n"))
        .collect::<String>()
}

// --- Impls ----------------------------------------------------------------

fn render_impls(ctx: &Ctx, out: &mut String, file: &Path, impl_ids: &[Id]) {
    let mut inherent: Vec<&Impl> = Vec::new();
    let mut trait_impls: Vec<(String, Option<String>)> = Vec::new();

    for &id in impl_ids {
        let Some(Item {
            inner: ItemEnum::Impl(im),
            ..
        }) = ctx.raw(id)
        else {
            continue;
        };
        // Per GOALS: omit auto traits and blanket impls.
        if im.is_synthetic || im.blanket_impl.is_some() {
            continue;
        }
        match &im.trait_ {
            None => inherent.push(im),
            Some(trait_path) => {
                let name = signature::type_str(&Type::ResolvedPath(trait_path.clone()));
                let link = ctx.link(file, trait_path.id);
                trait_impls.push((name, link));
            }
        }
    }

    render_inherent(ctx, out, file, &inherent);

    if !trait_impls.is_empty() {
        trait_impls.sort();
        trait_impls.dedup();
        out.push_str("## Trait Implementations\n\n");
        for (name, link) in trait_impls {
            match link {
                Some(href) => out.push_str(&format!("- [`{name}`]({href})\n")),
                None => out.push_str(&format!("- `{name}`\n")),
            }
        }
        out.push('\n');
    }
}

fn render_inherent(ctx: &Ctx, out: &mut String, file: &Path, blocks: &[&Impl]) {
    let has_items = blocks.iter().any(|im| !im.items.is_empty());
    if !has_items {
        return;
    }
    out.push_str("## Implementations\n\n");
    for im in blocks {
        if im.items.is_empty() {
            continue;
        }
        let header = format!(
            "impl{} {}{}",
            signature::generics_decl(&im.generics),
            signature::type_str(&im.for_),
            signature::where_clause(&im.generics),
        );
        out.push_str(&format!("### `{header}`\n\n"));

        let mut items: Vec<&Item> = im.items.iter().filter_map(|&id| ctx.raw(id)).collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        for item in items {
            assoc::render(ctx, out, file, item, "####", true);
        }
    }
}

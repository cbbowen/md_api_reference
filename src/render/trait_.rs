//! Render a trait's markdown file: its declaration (supertraits, generics), its
//! associated items, and the list of in-crate implementors.

use std::path::Path;

use rustdoc_types::{Item, ItemEnum, Type};

use crate::model::DocItem;

use super::{Ctx, assoc, doc_text, signature};

pub fn render(ctx: &Ctx, item: &DocItem) -> String {
    let file = item.file.as_deref().unwrap_or(Path::new(""));
    let Some(raw) = ctx.raw(item.id) else {
        return String::new();
    };
    let ItemEnum::Trait(tr) = &raw.inner else {
        return String::new();
    };

    let mut out = String::new();
    let path = item.canonical.0.join("::");
    out.push_str(&format!("# Trait `{path}`\n\n"));

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

    // Declaration line.
    out.push_str("## Declaration\n\n");
    out.push_str(&format!("```rust\n{}\n```\n\n", declaration(&item.name, tr)));

    // Associated items, grouped: types, then constants, then methods.
    let mut assoc_types = Vec::new();
    let mut assoc_consts = Vec::new();
    let mut methods = Vec::new();
    for &id in &tr.items {
        let Some(member) = ctx.raw(id) else { continue };
        match &member.inner {
            ItemEnum::AssocType { .. } => assoc_types.push(member),
            ItemEnum::AssocConst { .. } => assoc_consts.push(member),
            ItemEnum::Function(_) => methods.push(member),
            _ => {}
        }
    }
    sort_by_name(&mut assoc_types);
    sort_by_name(&mut assoc_consts);
    sort_by_name(&mut methods);

    assoc_section(ctx, &mut out, file, "Associated Types", &assoc_types);
    assoc_section(ctx, &mut out, file, "Associated Constants", &assoc_consts);
    assoc_section(ctx, &mut out, file, "Required and Provided Methods", &methods);

    implementors(ctx, &mut out, file, raw);

    out.trim_end().to_string() + "\n"
}

/// The `pub trait Name<..>: Supertraits where ..` header line.
fn declaration(name: &str, tr: &rustdoc_types::Trait) -> String {
    let mut s = String::from("pub ");
    if tr.is_unsafe {
        s.push_str("unsafe ");
    }
    s.push_str(&format!("trait {name}{}", signature::generics_decl(&tr.generics)));
    if !tr.bounds.is_empty() {
        s.push_str(&format!(": {}", signature::bounds_str(&tr.bounds)));
    }
    s.push_str(&signature::where_clause(&tr.generics));
    s
}

fn assoc_section(ctx: &Ctx, out: &mut String, file: &Path, title: &str, items: &[&Item]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("## {title}\n\n"));
    for item in items {
        // Trait items have no `pub` keyword.
        assoc::render(ctx, out, file, item, "###", false);
    }
}

/// List the in-crate types that implement this trait (those that are documented).
fn implementors(ctx: &Ctx, out: &mut String, file: &Path, trait_raw: &Item) {
    // rustdoc records a trait's implementations on the `Trait.implementations`
    // list; resolve each impl's `for_` type to a documented item.
    let ItemEnum::Trait(tr) = &trait_raw.inner else {
        return;
    };

    let mut entries: Vec<(String, Option<String>)> = Vec::new();
    for &impl_id in &tr.implementations {
        let Some(Item {
            inner: ItemEnum::Impl(im),
            ..
        }) = ctx.raw(impl_id)
        else {
            continue;
        };
        if im.is_synthetic || im.blanket_impl.is_some() {
            continue;
        }
        let name = signature::type_str(&im.for_);
        let link = type_link(ctx, file, &im.for_);
        entries.push((name, link));
    }

    if entries.is_empty() {
        return;
    }
    entries.sort();
    entries.dedup();
    out.push_str("## Implementors\n\n");
    for (name, link) in entries {
        match link {
            Some(href) => out.push_str(&format!("- [`{name}`]({href})\n")),
            None => out.push_str(&format!("- `{name}`\n")),
        }
    }
    out.push('\n');
}

/// A link to the documented item underlying a type, if any.
fn type_link(ctx: &Ctx, file: &Path, ty: &Type) -> Option<String> {
    match ty {
        Type::ResolvedPath(path) => ctx.link(file, path.id),
        _ => None,
    }
}

fn sort_by_name(items: &mut [&Item]) {
    items.sort_by(|a, b| a.name.cmp(&b.name));
}

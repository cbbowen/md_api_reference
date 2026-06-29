//! Render a module's markdown file: its docs, then its children grouped by kind
//! (submodules, types, traits, functions, constants, type aliases, macros) with
//! inline items rendered in place and file-owning items linked.

use std::path::Path;

use rustdoc_types::{Item, ItemEnum};

use crate::model::DocItem;

use super::{Ctx, doc_text, signature};

pub fn render(ctx: &Ctx, module: &DocItem) -> String {
    let file = module.file.as_deref().unwrap_or(Path::new(""));
    let raw = ctx.raw(module.id);

    let mut out = String::new();

    // Title.
    let path = module.canonical.0.join("::");
    if module.canonical.0.len() == 1 {
        out.push_str(&format!("# Crate `{path}`\n\n"));
    } else {
        out.push_str(&format!("# Module `{path}`\n\n"));
    }

    // Source reference and docs.
    if let Some(raw) = raw {
        if let Some(src) = ctx.source_ref(raw) {
            out.push_str(&format!("{src}\n\n"));
        }
        push_docs(ctx, &mut out, file, raw);
    }

    // Gather children placed canonically in this module.
    let self_path = &module.canonical.0;
    let mut submodules = Vec::new();
    let mut types = Vec::new();
    let mut traits = Vec::new();
    let mut functions = Vec::new();
    let mut constants = Vec::new();
    let mut type_aliases = Vec::new();
    let mut macros = Vec::new();

    for child in ctx.model.items() {
        if child.id == module.id || child.canonical.module() != self_path.as_slice() {
            continue;
        }
        match child.raw_inner(ctx) {
            Some(ItemEnum::Module(_)) => submodules.push(child),
            Some(ItemEnum::Struct(_) | ItemEnum::Enum(_) | ItemEnum::Union(_)) => types.push(child),
            Some(ItemEnum::Trait(_) | ItemEnum::TraitAlias(_)) => traits.push(child),
            Some(ItemEnum::Function(_)) => functions.push(child),
            Some(ItemEnum::Constant { .. } | ItemEnum::Static(_)) => constants.push(child),
            Some(ItemEnum::TypeAlias(_)) => type_aliases.push(child),
            Some(ItemEnum::Macro(_) | ItemEnum::ProcMacro(_)) => macros.push(child),
            _ => {}
        }
    }

    sort_by_name(&mut submodules);
    sort_by_name(&mut types);
    sort_by_name(&mut traits);
    sort_by_name(&mut functions);
    sort_by_name(&mut constants);
    sort_by_name(&mut type_aliases);
    sort_by_name(&mut macros);

    link_section(ctx, &mut out, file, "Modules", &submodules);
    link_section(ctx, &mut out, file, "Types", &types);
    link_section(ctx, &mut out, file, "Traits", &traits);
    inline_section(ctx, &mut out, file, "Functions", &functions);
    inline_section(ctx, &mut out, file, "Constants", &constants);
    inline_section(ctx, &mut out, file, "Type Aliases", &type_aliases);
    inline_section(ctx, &mut out, file, "Macros", &macros);

    reexport_section(ctx, &mut out, file, self_path);

    out.trim_end().to_string() + "\n"
}

/// A bulleted list linking to file-owning children, each with a short description.
fn link_section(ctx: &Ctx, out: &mut String, file: &Path, title: &str, items: &[&DocItem]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("## {title}\n\n"));
    for item in items {
        let link = ctx
            .link(file, item.id)
            .unwrap_or_else(|| item.name.clone());
        let desc = item
            .short_desc(ctx)
            .map(|d| format!(" — {d}"))
            .unwrap_or_default();
        out.push_str(&format!("- [`{}`]({link}){desc}\n", item.name));
    }
    out.push('\n');
}

/// A section that renders inline items (functions, constants, …) in place.
fn inline_section(ctx: &Ctx, out: &mut String, file: &Path, title: &str, items: &[&DocItem]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("## {title}\n\n"));
    for item in items {
        let Some(raw) = ctx.raw(item.id) else { continue };
        out.push_str(&format!("### `{}`\n\n", item.name));
        if let Some(code) = inline_signature(&item.name, raw) {
            out.push_str(&format!("```rust\n{code}\n```\n\n"));
        }
        push_docs(ctx, out, file, raw);
    }
}

/// The code-block signature for an inline item.
fn inline_signature(name: &str, raw: &Item) -> Option<String> {
    match &raw.inner {
        ItemEnum::Function(func) => Some(format!("pub {}", signature::function_signature(name, func))),
        ItemEnum::Constant { type_, .. } => {
            Some(format!("pub const {name}: {}", signature::type_str(type_)))
        }
        ItemEnum::Static(s) => {
            let m = if s.is_mutable { "mut " } else { "" };
            Some(format!("pub static {m}{name}: {}", signature::type_str(&s.type_)))
        }
        ItemEnum::TypeAlias(alias) => Some(format!(
            "pub type {name}{} = {};",
            signature::generics_decl(&alias.generics),
            signature::type_str(&alias.type_),
        )),
        ItemEnum::Macro(def) => Some(def.clone()),
        ItemEnum::ProcMacro(_) => None,
        _ => None,
    }
}

/// List items reexported into this module whose canonical home is elsewhere.
fn reexport_section(ctx: &Ctx, out: &mut String, file: &Path, self_path: &[String]) {
    let mut stubs: Vec<(String, String)> = Vec::new();
    for item in ctx.model.items() {
        for alt in &item.alternates {
            if alt.module() == self_path {
                if let Some(link) = ctx.link(file, item.id) {
                    stubs.push((alt.name().to_string(), link));
                }
            }
        }
    }
    if stubs.is_empty() {
        return;
    }
    stubs.sort();
    stubs.dedup();
    out.push_str("## Reexports\n\n");
    for (name, link) in stubs {
        out.push_str(&format!("- [`{name}`]({link})\n"));
    }
    out.push('\n');
}

fn push_docs(ctx: &Ctx, out: &mut String, file: &Path, raw: &Item) {
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
}

fn sort_by_name(items: &mut [&DocItem]) {
    items.sort_by(|a, b| a.name.cmp(&b.name));
}

impl DocItem {
    fn raw_inner<'a>(&self, ctx: &'a Ctx) -> Option<&'a ItemEnum> {
        ctx.raw(self.id).map(|i| &i.inner)
    }

    /// First non-empty line of the item's docs, for use as a one-line summary.
    fn short_desc(&self, ctx: &Ctx) -> Option<String> {
        let docs = ctx.raw(self.id)?.docs.as_ref()?;
        docs.lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(str::to_string)
    }
}

//! Rendering: turn a [`DocModel`] into a set of markdown files.
//!
//! The driver walks the model's documented items and dispatches each file-owning
//! item to the module / type / trait renderer. Shared concerns live in
//! submodules: [`links`] (cross-references and source spans), [`doc_text`]
//! (doc-comment transforms), and [`signature`] (Rust signature formatting).

mod assoc;
mod doc_text;
mod links;
mod module;
mod signature;
mod trait_;
mod type_;

use std::collections::HashMap;
use std::path::PathBuf;

use rustdoc_types::{Id, Item};

use crate::model::{DocItem, DocKind, DocModel};

/// One generated markdown file: a path relative to the output root and its contents.
pub struct RenderedFile {
    pub path: PathBuf,
    pub contents: String,
}

/// Render every documented file in the model.
pub fn render(model: &DocModel) -> Vec<RenderedFile> {
    let ctx = Ctx::new(model);
    let mut files = Vec::new();

    for item in model.items() {
        let Some(path) = item.file.clone() else {
            continue; // inline items are rendered inside their module's file
        };
        let contents = match item.kind {
            DocKind::Module => module::render(&ctx, item),
            DocKind::Type => type_::render(&ctx, item),
            DocKind::Trait => trait_::render(&ctx, item),
            DocKind::Inline | DocKind::Other => continue,
        };
        files.push(RenderedFile { path, contents });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

/// Shared rendering context: the model plus lookups used for cross-referencing.
pub(crate) struct Ctx<'a> {
    pub model: &'a DocModel,
    /// Canonical module path → that module's output file, for resolving links to
    /// inline items (which live in their module's file).
    module_files: HashMap<Vec<String>, PathBuf>,
}

impl<'a> Ctx<'a> {
    fn new(model: &'a DocModel) -> Ctx<'a> {
        let module_files = model
            .items()
            .filter(|i| i.kind == DocKind::Module)
            .filter_map(|i| Some((i.canonical.0.clone(), i.file.clone()?)))
            .collect();
        Ctx {
            model,
            module_files,
        }
    }

    /// The placed item for `id`, if it is documented.
    pub fn doc(&self, id: Id) -> Option<&DocItem> {
        self.model.items.get(&id)
    }

    /// The raw rustdoc item for `id`.
    pub fn raw(&self, id: Id) -> Option<&Item> {
        self.model.krate.index.get(&id)
    }
}

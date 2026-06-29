//! The documentation model: the publicly accessible items of a crate, each
//! assigned a canonical output location plus any alternate (stub) paths.
//!
//! This is built from a [`rustdoc_types::Crate`] by [`DocModel::build`] and is
//! the structure the renderer consumes; rendering never touches raw rustdoc
//! JSON directly.

mod paths;
mod reachability;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rustdoc_types::{Crate, Id, ItemEnum};

/// A fully-qualified public path from the crate root, including the item's own
/// public name as the final segment (e.g. `["example", "top", "Bar"]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemPath(pub Vec<String>);

impl ItemPath {
    /// The item's public name (the last segment).
    pub fn name(&self) -> &str {
        self.0.last().map(String::as_str).unwrap_or_default()
    }

    /// The module path containing the item (everything but the last segment).
    pub fn module(&self) -> &[String] {
        let len = self.0.len();
        &self.0[..len.saturating_sub(1)]
    }

    /// Ordering key for canonical selection: shortest path first, then
    /// lexicographic, so the choice is deterministic.
    fn order_key(&self) -> (usize, &[String]) {
        (self.0.len(), self.0.as_slice())
    }
}

/// How an item is represented in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    /// A `mod` — rendered as a directory with a `mod.md` (or `lib.md` at the root).
    Module,
    /// A `struct`, `enum`, or `union` — its own file.
    Type,
    /// A `trait` (or trait alias) — its own file.
    Trait,
    /// A free function, constant, static, type alias, or macro — rendered inline
    /// in its parent module's file.
    Inline,
    /// Anything else that may be reached but is not separately documented.
    Other,
}

impl DocKind {
    /// Classify a rustdoc item by its inner kind.
    pub fn of(inner: &ItemEnum) -> DocKind {
        match inner {
            ItemEnum::Module(_) => DocKind::Module,
            ItemEnum::Struct(_) | ItemEnum::Enum(_) | ItemEnum::Union(_) => DocKind::Type,
            ItemEnum::Trait(_) | ItemEnum::TraitAlias(_) => DocKind::Trait,
            ItemEnum::Function(_)
            | ItemEnum::Constant { .. }
            | ItemEnum::Static(_)
            | ItemEnum::TypeAlias(_)
            | ItemEnum::Macro(_)
            | ItemEnum::ProcMacro(_) => DocKind::Inline,
            _ => DocKind::Other,
        }
    }

    /// Whether items of this kind are rendered to their own file.
    pub fn has_own_file(self) -> bool {
        matches!(self, DocKind::Module | DocKind::Type | DocKind::Trait)
    }
}

/// A single publicly accessible item with its resolved placement.
//
// `id` and `name` are part of the public placement record and are consumed by
// the renderer (Phase 3); allow them to be unread until then.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DocItem {
    pub id: Id,
    /// Public name (the canonical path's final segment; may be a reexport rename).
    pub name: String,
    pub kind: DocKind,
    /// The chosen canonical path.
    pub canonical: ItemPath,
    /// Other public paths to this item; each becomes a stub entry.
    pub alternates: Vec<ItemPath>,
    /// Relative output file for items with their own file; `None` for inline items.
    pub file: Option<PathBuf>,
}

/// The complete model for one crate.
pub struct DocModel {
    pub krate: Crate,
    /// The crate root id (consumed by the renderer in Phase 3).
    #[allow(dead_code)]
    pub root: Id,
    /// Documented items keyed by id, in a deterministic order.
    pub items: BTreeMap<Id, DocItem>,
}

impl DocModel {
    /// Build the model from a parsed crate: run reachability analysis, choose
    /// canonical placements, and assign output file paths.
    pub fn build(krate: Crate) -> DocModel {
        let discoveries = reachability::discover(&krate);
        let items = paths::assemble(&krate, discoveries);
        let root = krate.root;
        DocModel { krate, root, items }
    }

    /// Documented items in deterministic order.
    pub fn items(&self) -> impl Iterator<Item = &DocItem> {
        self.items.values()
    }
}

/// Constructors for building synthetic crates in unit tests.
#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::HashMap;

    use rustdoc_types::{
        Crate, FORMAT_VERSION, Generics, Id, Item, ItemEnum, Module, Struct, StructKind, Target,
        Use, Visibility,
    };

    fn item(id: u32, name: Option<&str>, visibility: Visibility, inner: ItemEnum) -> Item {
        Item {
            id: Id(id),
            crate_id: 0,
            name: name.map(str::to_string),
            span: None,
            visibility,
            docs: None,
            links: HashMap::new(),
            attrs: Vec::new(),
            deprecation: None,
            stability: None,
            const_stability: None,
            inner,
        }
    }

    pub fn krate_with(root: u32, items: Vec<Item>) -> Crate {
        let index = items.into_iter().map(|i| (i.id, i)).collect();
        Crate {
            root: Id(root),
            crate_version: None,
            includes_private: true,
            index,
            paths: HashMap::new(),
            external_crates: HashMap::new(),
            target: Target {
                triple: String::new(),
                target_features: Vec::new(),
            },
            format_version: FORMAT_VERSION,
        }
    }

    pub fn module(id: u32, name: &str, children: &[u32]) -> Item {
        module_vis(id, name, children, Visibility::Public)
    }

    pub fn module_vis(id: u32, name: &str, children: &[u32], visibility: Visibility) -> Item {
        let inner = ItemEnum::Module(Module {
            is_crate: false,
            items: children.iter().map(|&c| Id(c)).collect(),
            is_stripped: false,
        });
        item(id, Some(name), visibility, inner)
    }

    pub fn unit_struct(id: u32, name: &str) -> Item {
        let inner = ItemEnum::Struct(Struct {
            kind: StructKind::Unit,
            generics: Generics {
                params: Vec::new(),
                where_predicates: Vec::new(),
            },
            impls: Vec::new(),
        });
        item(id, Some(name), Visibility::Public, inner)
    }

    pub fn macro_item(id: u32, name: &str) -> Item {
        item(
            id,
            Some(name),
            Visibility::Public,
            ItemEnum::Macro(format!("macro_rules! {name} {{}}")),
        )
    }

    /// A `pub use`. The wrapping item is unnamed (as rustdoc emits); the public
    /// name and glob-ness live on the [`Use`].
    pub fn reexport(id: u32, name: &str, target: Option<u32>, is_glob: bool) -> Item {
        let inner = ItemEnum::Use(Use {
            source: name.to_string(),
            name: name.to_string(),
            id: target.map(Id),
            is_glob,
        });
        item(id, None, Visibility::Public, inner)
    }
}

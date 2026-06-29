//! Resolve cross-crate reexports.
//!
//! When a `--crate` reexports an item from a `--reexport-crate`, rustdoc records
//! the `pub use` target id only in the primary crate's `paths` (as an external
//! item), not in its `index` — the definition lives in the dependency's own
//! JSON. We bridge this by *inlining*: copying the referenced item's subgraph
//! out of the dependency crate into the primary crate's `index` under fresh ids,
//! and repointing the `use` at the copy. Downstream reachability and rendering
//! then treat it like any other item, so files are produced exactly for what the
//! primary crate reexports.

use std::collections::HashMap;

use rustdoc_types::{Crate, Id, Item, ItemEnum, StructKind, VariantKind};

use crate::model::ReexportOrigin;

/// The crate name of a parsed crate (its root module's name).
pub fn crate_name(krate: &Crate) -> Option<String> {
    krate.index.get(&krate.root)?.name.clone()
}

/// Inline every cross-crate reexport in `primary` that targets one of the named
/// `references` crates. Returns the origin (dependency-side path) of every
/// inlined item that is itself addressable, so the renderer can annotate it.
pub fn inline_reexports(
    primary: &mut Crate,
    references: &HashMap<String, Crate>,
) -> HashMap<Id, ReexportOrigin> {
    let mut origins = HashMap::new();
    if references.is_empty() {
        return origins;
    }

    // path → id lookups for each reference crate, so an external path from the
    // primary crate can be resolved to a definition.
    let path_maps: HashMap<&str, HashMap<Vec<String>, Id>> = references
        .iter()
        .map(|(name, krate)| (name.as_str(), reverse_paths(krate)))
        .collect();

    let targets = collect_cross_crate_uses(primary, references);
    let mut next_id = next_free_id(primary);

    for target in targets {
        let Some(src) = references.get(&target.crate_name) else {
            continue;
        };
        let Some(src_id) = path_maps
            .get(target.crate_name.as_str())
            .and_then(|m| m.get(&target.path))
            .copied()
        else {
            continue;
        };

        let mut memo = HashMap::new();
        if let Some(new_id) = copy_item(primary, src, src_id, &mut memo, &mut next_id, &mut origins)
            && let Some(Item {
                inner: ItemEnum::Use(use_),
                ..
            }) = primary.index.get_mut(&target.use_item)
            {
                use_.id = Some(new_id);
            }
    }

    origins
}

/// A `use` in the primary crate whose target lives in a reference crate.
struct CrossCrateUse {
    use_item: Id,
    crate_name: String,
    path: Vec<String>,
}

fn collect_cross_crate_uses(
    primary: &Crate,
    references: &HashMap<String, Crate>,
) -> Vec<CrossCrateUse> {
    let mut out = Vec::new();
    for (id, item) in &primary.index {
        let ItemEnum::Use(use_) = &item.inner else {
            continue;
        };
        let Some(target_id) = use_.id else { continue };
        if primary.index.contains_key(&target_id) {
            continue; // a local reexport; handled by ordinary reachability
        }
        let Some(summary) = primary.paths.get(&target_id) else {
            continue;
        };
        let Some(ext) = primary.external_crates.get(&summary.crate_id) else {
            continue;
        };
        if references.contains_key(&ext.name) {
            out.push(CrossCrateUse {
                use_item: *id,
                crate_name: ext.name.clone(),
                path: summary.path.clone(),
            });
        }
    }
    // Deterministic processing order.
    out.sort_by_key(|a| a.use_item.0);
    out
}

fn reverse_paths(krate: &Crate) -> HashMap<Vec<String>, Id> {
    let mut map = HashMap::new();
    for (id, summary) in &krate.paths {
        map.entry(summary.path.clone()).or_insert(*id);
    }
    map
}

fn next_free_id(krate: &Crate) -> u32 {
    let max_index = krate.index.keys().map(|id| id.0).max().unwrap_or(0);
    let max_paths = krate.paths.keys().map(|id| id.0).max().unwrap_or(0);
    max_index.max(max_paths) + 1
}

/// Recursively copy `src_id`'s subgraph from `src` into `target`, returning the
/// new id. Containment references (module items, fields, variants, impls, …) are
/// followed and remapped; type/path references are left as-is. Items that are
/// addressable in `src` (have a `paths` entry) get an origin recorded.
fn copy_item(
    target: &mut Crate,
    src: &Crate,
    src_id: Id,
    memo: &mut HashMap<Id, Id>,
    next_id: &mut u32,
    origins: &mut HashMap<Id, ReexportOrigin>,
) -> Option<Id> {
    if let Some(&existing) = memo.get(&src_id) {
        return Some(existing);
    }
    let item = src.index.get(&src_id)?.clone();

    let new_id = Id(*next_id);
    *next_id += 1;
    // Insert into the memo before recursing so cycles terminate.
    memo.insert(src_id, new_id);

    // Record the dependency-side path for addressable items, so the renderer can
    // show "Reexported from `dep::…`".
    if let Some(summary) = src.paths.get(&src_id) {
        origins.insert(
            new_id,
            ReexportOrigin {
                path: summary.path.clone(),
            },
        );
    }

    let inner = remap_inner(item.inner.clone(), target, src, memo, next_id, origins);
    let new_item = Item {
        id: new_id,
        inner,
        ..item
    };
    target.index.insert(new_id, new_item);
    Some(new_id)
}

fn remap_inner(
    inner: ItemEnum,
    target: &mut Crate,
    src: &Crate,
    memo: &mut HashMap<Id, Id>,
    next_id: &mut u32,
    origins: &mut HashMap<Id, ReexportOrigin>,
) -> ItemEnum {
    let copy = |ids: &[Id],
                target: &mut Crate,
                memo: &mut HashMap<Id, Id>,
                next: &mut u32,
                origins: &mut HashMap<Id, ReexportOrigin>| {
        ids.iter()
            .filter_map(|&id| copy_item(target, src, id, memo, next, origins))
            .collect::<Vec<_>>()
    };

    match inner {
        ItemEnum::Module(mut m) => {
            m.items = copy(&m.items, target, memo, next_id, origins);
            ItemEnum::Module(m)
        }
        ItemEnum::Struct(mut s) => {
            s.kind = remap_struct_kind(s.kind, target, src, memo, next_id, origins);
            s.impls = copy(&s.impls, target, memo, next_id, origins);
            ItemEnum::Struct(s)
        }
        ItemEnum::Enum(mut e) => {
            e.variants = copy(&e.variants, target, memo, next_id, origins);
            e.impls = copy(&e.impls, target, memo, next_id, origins);
            ItemEnum::Enum(e)
        }
        ItemEnum::Union(mut u) => {
            u.fields = copy(&u.fields, target, memo, next_id, origins);
            u.impls = copy(&u.impls, target, memo, next_id, origins);
            ItemEnum::Union(u)
        }
        ItemEnum::Variant(mut v) => {
            v.kind = remap_variant_kind(v.kind, target, src, memo, next_id, origins);
            ItemEnum::Variant(v)
        }
        ItemEnum::Trait(mut t) => {
            t.items = copy(&t.items, target, memo, next_id, origins);
            t.implementations = copy(&t.implementations, target, memo, next_id, origins);
            ItemEnum::Trait(t)
        }
        ItemEnum::Impl(mut im) => {
            im.items = copy(&im.items, target, memo, next_id, origins);
            ItemEnum::Impl(im)
        }
        other => other,
    }
}

fn remap_struct_kind(
    kind: StructKind,
    target: &mut Crate,
    src: &Crate,
    memo: &mut HashMap<Id, Id>,
    next_id: &mut u32,
    origins: &mut HashMap<Id, ReexportOrigin>,
) -> StructKind {
    match kind {
        StructKind::Unit => StructKind::Unit,
        StructKind::Tuple(fields) => StructKind::Tuple(
            fields
                .into_iter()
                .map(|f| f.and_then(|id| copy_item(target, src, id, memo, next_id, origins)))
                .collect(),
        ),
        StructKind::Plain {
            fields,
            has_stripped_fields,
        } => StructKind::Plain {
            fields: fields
                .iter()
                .filter_map(|&id| copy_item(target, src, id, memo, next_id, origins))
                .collect(),
            has_stripped_fields,
        },
    }
}

fn remap_variant_kind(
    kind: VariantKind,
    target: &mut Crate,
    src: &Crate,
    memo: &mut HashMap<Id, Id>,
    next_id: &mut u32,
    origins: &mut HashMap<Id, ReexportOrigin>,
) -> VariantKind {
    match kind {
        VariantKind::Plain => VariantKind::Plain,
        VariantKind::Tuple(fields) => VariantKind::Tuple(
            fields
                .into_iter()
                .map(|f| f.and_then(|id| copy_item(target, src, id, memo, next_id, origins)))
                .collect(),
        ),
        VariantKind::Struct {
            fields,
            has_stripped_fields,
        } => VariantKind::Struct {
            fields: fields
                .iter()
                .filter_map(|&id| copy_item(target, src, id, memo, next_id, origins))
                .collect(),
            has_stripped_fields,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::test_support::*;
    use rustdoc_types::{ExternalCrate, ItemKind, ItemSummary};

    /// Build a dependency crate `dep` containing `pub struct Widget;` reachable
    /// at path `["dep", "Widget"]`.
    fn dep_crate() -> Crate {
        let mut dep = krate_with(
            100,
            vec![module(100, "dep", &[105]), unit_struct(105, "Widget")],
        );
        dep.paths.insert(
            Id(105),
            ItemSummary {
                crate_id: 0,
                path: vec!["dep".into(), "Widget".into()],
                kind: ItemKind::Struct,
            },
        );
        dep
    }

    /// Build a facade crate with `pub use dep::Widget;` whose target (id 43) is
    /// external — present only in `paths`/`external_crates`.
    fn facade_crate() -> Crate {
        let mut facade = krate_with(
            0,
            vec![
                module(0, "facade", &[1]),
                reexport(1, "Widget", Some(43), false),
            ],
        );
        facade.paths.insert(
            Id(43),
            ItemSummary {
                crate_id: 14,
                path: vec!["dep".into(), "Widget".into()],
                kind: ItemKind::Struct,
            },
        );
        facade.external_crates.insert(
            14,
            ExternalCrate {
                name: "dep".into(),
                html_root_url: None,
                path: std::path::PathBuf::new(),
            },
        );
        facade
    }

    #[test]
    fn records_origin_for_inlined_item() {
        let mut facade = facade_crate();
        let refs = HashMap::from([("dep".to_string(), dep_crate())]);

        let origins = inline_reexports(&mut facade, &refs);

        // The inlined Widget should carry its dependency-side path.
        let note = origins
            .values()
            .find(|o| o.path == vec!["dep".to_string(), "Widget".to_string()]);
        assert!(note.is_some(), "expected an origin for dep::Widget");
        assert_eq!(note.unwrap().display(), "dep::Widget");
    }

    #[test]
    fn inlines_cross_crate_reexport() {
        let mut facade = facade_crate();
        let refs = HashMap::from([("dep".to_string(), dep_crate())]);

        inline_reexports(&mut facade, &refs);

        // The use now points at an id that is present in the facade index.
        let ItemEnum::Use(use_) = &facade.index[&Id(1)].inner else {
            panic!("expected use");
        };
        let new_id = use_.id.expect("use should resolve");
        assert!(facade.index.contains_key(&new_id), "inlined item missing");
        let copied = &facade.index[&new_id];
        assert_eq!(copied.name.as_deref(), Some("Widget"));
        assert!(matches!(copied.inner, ItemEnum::Struct(_)));
    }

    #[test]
    fn inlined_reexport_is_documented_by_model() {
        let mut facade = facade_crate();
        let refs = HashMap::from([("dep".to_string(), dep_crate())]);
        inline_reexports(&mut facade, &refs);

        let model = crate::model::DocModel::build(facade);
        let widget = model
            .items()
            .find(|i| i.name == "Widget")
            .expect("Widget should be documented");
        assert_eq!(widget.canonical.0.join("::"), "facade::Widget");
    }

    #[test]
    fn leaves_unreferenced_dep_items_undocumented() {
        // dep also has `Unexported`, never used by facade.
        let mut dep = dep_crate();
        let extra = unit_struct(106, "Unexported");
        dep.index.insert(Id(106), extra);

        let mut facade = facade_crate();
        let refs = HashMap::from([("dep".to_string(), dep)]);
        inline_reexports(&mut facade, &refs);

        let model = crate::model::DocModel::build(facade);
        assert!(model.items().all(|i| i.name != "Unexported"));
    }

    #[test]
    fn no_references_is_a_noop() {
        let mut facade = facade_crate();
        let before = facade.index.len();
        inline_reexports(&mut facade, &HashMap::new());
        assert_eq!(facade.index.len(), before);
    }
}

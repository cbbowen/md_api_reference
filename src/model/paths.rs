//! Turn discovered paths into placed [`DocItem`]s: choose each item's canonical
//! path, assign output files, and disambiguate the rare case-insensitive file
//! collision.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use rustdoc_types::{Crate, Id};

use super::{DocItem, DocKind, ItemPath};

/// Assemble placed items from the raw discoveries.
pub fn assemble(krate: &Crate, discoveries: BTreeMap<Id, Vec<ItemPath>>) -> BTreeMap<Id, DocItem> {
    // First pass: pick canonical/alternate paths and kinds.
    let mut items: BTreeMap<Id, DocItem> = BTreeMap::new();
    for (id, mut paths) in discoveries {
        // Shortest path wins, then lexicographic — deterministic.
        paths.sort_by(|a, b| a.order_key().cmp(&b.order_key()));
        let canonical = paths.remove(0);
        let kind = krate
            .index
            .get(&id)
            .map(|item| DocKind::of(&item.inner))
            .unwrap_or(DocKind::Other);
        let name = canonical.name().to_string();
        items.insert(
            id,
            DocItem {
                id,
                name,
                kind,
                canonical,
                alternates: paths,
                file: None,
            },
        );
    }

    assign_files(&mut items);
    items
}

/// Assign a relative output file to every item with its own file, numbering any
/// case-insensitive collisions (`Foo-2.md`).
fn assign_files(items: &mut BTreeMap<Id, DocItem>) {
    // Track lower-cased file paths already used so collisions can be detected
    // regardless of filesystem case sensitivity. Iterating the BTreeMap keeps
    // assignment deterministic (ordered by id).
    let mut used: HashMap<String, u32> = HashMap::new();

    for item in items.values_mut() {
        if !item.kind.has_own_file() {
            continue;
        }
        let base = base_file_path(item.kind, &item.canonical);
        item.file = Some(deduplicate(base, &mut used));
    }
}

/// The natural output path for an item before collision handling.
fn base_file_path(kind: DocKind, path: &ItemPath) -> PathBuf {
    match kind {
        DocKind::Module => {
            // The crate root (a single-segment path) is `lib.md`; every other
            // module is a `mod.md` inside its own directory.
            let mut buf = PathBuf::from_iter(&path.0);
            if path.0.len() == 1 {
                buf.set_file_name(format!("{}/lib.md", path.0[0]));
            } else {
                buf.push("mod.md");
            }
            buf
        }
        // Types and traits live as `<Name>.md` inside their module's directory.
        _ => {
            let mut buf = PathBuf::from_iter(path.module());
            buf.push(format!("{}.md", path.name()));
            buf
        }
    }
}

/// Append `-N` before the `.md` extension if the (case-folded) path is taken.
fn deduplicate(path: PathBuf, used: &mut HashMap<String, u32>) -> PathBuf {
    let key = path.to_string_lossy().to_lowercase();
    let count = used.entry(key).or_insert(0);
    *count += 1;
    if *count == 1 {
        return path;
    }

    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let parent = path.parent().map(PathBuf::from).unwrap_or_default();
    let numbered = parent.join(format!("{stem}-{}.md", *count));
    // Record the numbered name too, in case it collides with something later.
    used.entry(numbered.to_string_lossy().to_lowercase())
        .or_insert(1);
    numbered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::test_support::*;

    fn build(items: Vec<rustdoc_types::Item>) -> BTreeMap<Id, DocItem> {
        let krate = krate_with(0, items);
        let discoveries = crate::model::reachability::discover(&krate);
        assemble(&krate, discoveries)
    }

    fn file_of(items: &BTreeMap<Id, DocItem>, id: u32) -> String {
        items[&Id(id)]
            .file
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    }

    #[test]
    fn root_module_is_lib_md() {
        let items = build(vec![module(0, "example", &[])]);
        assert_eq!(file_of(&items, 0), "example/lib.md");
    }

    #[test]
    fn nested_module_is_mod_md_in_dir() {
        let items = build(vec![module(0, "example", &[58]), module(58, "top", &[])]);
        assert_eq!(file_of(&items, 58), "example/top/mod.md");
    }

    #[test]
    fn type_is_named_file_in_module_dir() {
        let items = build(vec![
            module(0, "example", &[58]),
            module(58, "top", &[43]),
            unit_struct(43, "Bar"),
        ]);
        assert_eq!(file_of(&items, 43), "example/top/Bar.md");
    }

    #[test]
    fn inline_items_get_no_file() {
        let items = build(vec![module(0, "example", &[5]), macro_item(5, "make")]);
        assert!(items[&Id(5)].file.is_none());
        assert_eq!(items[&Id(5)].kind, DocKind::Inline);
    }

    #[test]
    fn canonical_is_shortest_path() {
        // Foo defined deep, reexported at the root: canonical is the short one.
        let items = build(vec![
            module(0, "example", &[58, 105]),
            module(58, "deep", &[90]),
            unit_struct(90, "Foo"),
            reexport(105, "Foo", Some(90), false),
        ]);
        let foo = &items[&Id(90)];
        assert_eq!(foo.canonical.0.join("::"), "example::Foo");
        assert_eq!(foo.alternates.len(), 1);
        assert_eq!(foo.alternates[0].0.join("::"), "example::deep::Foo");
        assert_eq!(file_of(&items, 90), "example/Foo.md");
    }

    #[test]
    fn case_insensitive_collision_is_numbered() {
        // A module `config` and a struct `Config` in the same parent would map to
        // `example/config/...` and `example/Config.md` — distinct. But two types
        // differing only in case collide on case-insensitive filesystems.
        let items = build(vec![
            module(0, "example", &[10, 11]),
            unit_struct(10, "Foo"),
            unit_struct(11, "foo"),
        ]);
        let f10 = file_of(&items, 10);
        let f11 = file_of(&items, 11);
        assert_ne!(f10.to_lowercase(), f11.to_lowercase());
        // One keeps the plain name, the other is numbered.
        assert!(f11.ends_with("-2.md"), "got {f11}");
    }
}

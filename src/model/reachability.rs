//! Graph reachability: discover every public path to every publicly accessible
//! item, starting from the crate root and following public modules and `pub use`
//! reexports.
//!
//! Visibility matters because rustdoc JSON is generated with
//! `--document-private-items` (so reexports out of private modules can be
//! resolved): the index contains private items that are *not* part of the public
//! API. We therefore descend only public modules and reach private-module items
//! solely through reexports. `#[doc(hidden)]` items are already absent from the
//! JSON, so they need no special handling here.

use std::collections::{BTreeMap, HashSet};

use rustdoc_types::{Crate, Id, Item, ItemEnum, Use, Visibility};

use super::ItemPath;

/// Discover all public paths for every reachable item, grouped by id.
pub fn discover(krate: &Crate) -> BTreeMap<Id, Vec<ItemPath>> {
    let mut walker = Walker {
        krate,
        discoveries: BTreeMap::new(),
        ancestors: HashSet::new(),
        expanded: HashSet::new(),
    };

    let root_name = krate
        .index
        .get(&krate.root)
        .and_then(|item| item.name.clone())
        .unwrap_or_else(|| "crate".to_string());

    walker.visit_module(krate.root, vec![root_name]);
    walker.discoveries
}

struct Walker<'a> {
    krate: &'a Crate,
    discoveries: BTreeMap<Id, Vec<ItemPath>>,
    /// Module ids on the current DFS stack, to break reexport cycles.
    ancestors: HashSet<Id>,
    /// (module id, path) pairs already expanded, to avoid redundant work on
    /// diamond-shaped reexports.
    expanded: HashSet<(Id, Vec<String>)>,
}

impl Walker<'_> {
    /// Record `id` as reachable at `path` (the full path including its own name).
    fn record(&mut self, id: Id, path: Vec<String>) {
        let paths = self.discoveries.entry(id).or_default();
        let candidate = ItemPath(path);
        if !paths.contains(&candidate) {
            paths.push(candidate);
        }
    }

    /// Visit a module reached as `path` (its own name is the last segment):
    /// record it, then walk its contents.
    fn visit_module(&mut self, id: Id, path: Vec<String>) {
        self.record(id, path.clone());
        self.visit_contents(id, path);
    }

    /// Walk the public children of module `id`, placing each under `base` (which
    /// for a normal module is its own path, and for a glob import is the
    /// importing module's path).
    fn visit_contents(&mut self, id: Id, base: Vec<String>) {
        if self.ancestors.contains(&id) {
            return; // cycle
        }
        if !self.expanded.insert((id, base.clone())) {
            return; // already expanded at this exact path
        }
        self.ancestors.insert(id);

        if let Some(Item {
            inner: ItemEnum::Module(module),
            ..
        }) = self.krate.index.get(&id)
        {
            for &child_id in &module.items {
                if let Some(child) = self.krate.index.get(&child_id) {
                    self.visit_child(child, &base);
                }
            }
        }

        self.ancestors.remove(&id);
    }

    /// Process one child item of a module being walked.
    fn visit_child(&mut self, child: &Item, base: &[String]) {
        match &child.inner {
            // Reexports are followed regardless of where they point; the import
            // itself must be public.
            ItemEnum::Use(use_) if is_public(&child.visibility) => {
                self.follow_use(use_, base);
            }
            // Public modules are descended.
            ItemEnum::Module(_) if is_public(&child.visibility) => {
                if let Some(name) = &child.name {
                    let path = extend(base, name);
                    self.visit_module(child.id, path);
                }
            }
            // Other public items are leaves placed at this location.
            _ if is_public(&child.visibility) => {
                if let Some(name) = &child.name {
                    self.record(child.id, extend(base, name));
                }
            }
            // Non-public items are only in the JSON because of
            // `--document-private-items`; skip unless reached via a reexport.
            _ => {}
        }
    }

    /// Follow a `pub use`, bringing the target into scope under `base`.
    fn follow_use(&mut self, use_: &Use, base: &[String]) {
        let Some(target_id) = use_.id else {
            return; // reexport of a primitive; nothing to document
        };
        let Some(target) = self.krate.index.get(&target_id) else {
            return; // target lives in another crate; handled in a later phase
        };

        if use_.is_glob {
            // `use path::*` — the target is a module whose public contents are
            // pulled directly into `base` (no new path segment).
            self.visit_contents(target_id, base.to_vec());
            return;
        }

        let path = extend(base, &use_.name);
        match &target.inner {
            ItemEnum::Module(_) => self.visit_module(target_id, path),
            _ => self.record(target_id, path),
        }
    }
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public)
}

fn extend(base: &[String], segment: &str) -> Vec<String> {
    let mut path = base.to_vec();
    path.push(segment.to_string());
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::test_support::*;

    /// Collect canonical-ish: the set of full path strings discovered for a name.
    fn paths_for(
        discoveries: &BTreeMap<Id, Vec<ItemPath>>,
        krate: &Crate,
        name: &str,
    ) -> Vec<String> {
        discoveries
            .iter()
            .filter(|(id, _)| {
                krate
                    .index
                    .get(id)
                    .and_then(|i| i.name.as_deref())
                    .map(|n| {
                        // For reexports the discovered name may differ; match on
                        // the path's last segment too.
                        n == name
                    })
                    .unwrap_or(false)
            })
            .flat_map(|(_, paths)| paths.iter().map(|p| p.0.join("::")))
            .collect()
    }

    #[test]
    fn descends_public_modules_and_skips_private() {
        // crate root contains: pub struct Foo; pub mod top { pub struct Bar; }
        // mod private { pub struct Hidden; }  (private, not reexported)
        let krate = krate_with(
            0,
            vec![
                module(0, "example", &[90, 58, 89]),
                unit_struct(90, "Foo"),
                module(58, "top", &[43]),
                unit_struct(43, "Bar"),
                module_vis(89, "private", &[74], Visibility::Crate),
                unit_struct(74, "Hidden"),
            ],
        );

        let d = discover(&krate);
        assert_eq!(paths_for(&d, &krate, "Foo"), vec!["example::Foo"]);
        assert_eq!(paths_for(&d, &krate, "Bar"), vec!["example::top::Bar"]);
        // Hidden is inside a private module and never reexported.
        assert!(paths_for(&d, &krate, "Hidden").is_empty());
    }

    #[test]
    fn follows_reexport_from_private_module() {
        // mod private { pub struct Reexported; }  pub use private::Reexported;
        let krate = krate_with(
            0,
            vec![
                module(0, "example", &[89, 105]),
                module_vis(89, "private", &[59], Visibility::Crate),
                unit_struct(59, "Reexported"),
                reexport(105, "Reexported", Some(59), false),
            ],
        );

        let d = discover(&krate);
        assert_eq!(
            paths_for(&d, &krate, "Reexported"),
            vec!["example::Reexported"]
        );
    }

    #[test]
    fn renamed_reexport_uses_new_name() {
        let krate = krate_with(
            0,
            vec![
                module(0, "example", &[89, 105]),
                module_vis(89, "private", &[59], Visibility::Crate),
                unit_struct(59, "Original"),
                reexport(105, "Renamed", Some(59), false),
            ],
        );

        let d = discover(&krate);
        // Discovered path uses the reexport name.
        let paths: Vec<String> = d[&Id(59)].iter().map(|p| p.0.join("::")).collect();
        assert_eq!(paths, vec!["example::Renamed"]);
    }

    #[test]
    fn records_multiple_paths_for_reexported_item() {
        // pub struct Foo at root, also pub use Foo as Bar; → two paths.
        let krate = krate_with(
            0,
            vec![
                module(0, "example", &[90, 105]),
                unit_struct(90, "Foo"),
                reexport(105, "Bar", Some(90), false),
            ],
        );

        let d = discover(&krate);
        let mut paths: Vec<String> = d[&Id(90)].iter().map(|p| p.0.join("::")).collect();
        paths.sort();
        assert_eq!(paths, vec!["example::Bar", "example::Foo"]);
    }

    #[test]
    fn glob_reexport_pulls_contents_without_extra_segment() {
        // mod private { pub struct A; pub struct B; }  pub use private::*;
        let krate = krate_with(
            0,
            vec![
                module(0, "example", &[89, 105]),
                module_vis(89, "private", &[10, 11], Visibility::Crate),
                unit_struct(10, "A"),
                unit_struct(11, "B"),
                reexport(105, "*", Some(89), true),
            ],
        );

        let d = discover(&krate);
        assert_eq!(d[&Id(10)][0].0.join("::"), "example::A");
        assert_eq!(d[&Id(11)][0].0.join("::"), "example::B");
    }

    #[test]
    fn breaks_reexport_cycles() {
        // Two modules that glob-reexport each other; must terminate.
        let krate = krate_with(
            0,
            vec![
                module(0, "example", &[1, 2]),
                module(1, "a", &[20]),
                module(2, "b", &[21]),
                reexport(20, "*", Some(2), true), // a: use b::*
                reexport(21, "*", Some(1), true), // b: use a::*
            ],
        );

        // Should not loop forever.
        let d = discover(&krate);
        assert!(d.contains_key(&Id(1)));
        assert!(d.contains_key(&Id(2)));
    }
}

//! Cross-reference resolution: relative links between output files, link
//! targets for ids, intra-doc link reference definitions, and source spans.

use std::path::{Path, PathBuf};

use rustdoc_types::{Id, Item};

use crate::model::DocKind;

use super::Ctx;

impl Ctx<'_> {
    /// The output file that documents `id` (its own file, or for an inline item
    /// the file of its containing module). `None` if `id` is not documented.
    pub fn target_file(&self, id: Id) -> Option<PathBuf> {
        let item = self.doc(id)?;
        if let Some(file) = &item.file {
            return Some(file.clone());
        }
        // Inline item: link to its module's file.
        self.module_files.get(item.canonical.module()).cloned()
    }

    /// A markdown link target (relative path, plus an anchor for inline items)
    /// from `from` to the documented item `id`, if documented.
    pub fn link(&self, from: &Path, id: Id) -> Option<String> {
        let target = self.target_file(id)?;
        let mut href = relative_link(from, &target);
        if let Some(item) = self.doc(id)
            && item.file.is_none()
        {
            // Inline items render under a heading slugged from their name.
            href.push('#');
            href.push_str(&heading_slug(&item.name));
        }
        Some(href)
    }

    /// Reference-style link definitions resolving an item's intra-doc links to
    /// the items being documented. Appending these lets shortcut references like
    /// `` [`Foo`] `` in the doc text resolve without rewriting the body. Links to
    /// undocumented items (e.g. `std`) are skipped and render as plain text.
    pub fn intra_doc_definitions(&self, from: &Path, item: &Item) -> String {
        let mut defs: Vec<(String, String)> = Vec::new();
        for (text, id) in &item.links {
            if let Some(href) = self.link(from, *id) {
                defs.push((text.clone(), href));
            }
        }
        // Deterministic order.
        defs.sort();
        defs.dedup();

        let mut out = String::new();
        for (text, href) in defs {
            out.push_str(&format!("[{text}]: {href}\n"));
        }
        out
    }

    /// A `_Defined at_` source reference for an item, rendered as plain inline
    /// code (no hyperlink). Modules using the `mod name;` form omit the line
    /// number; everything else includes it.
    pub fn source_ref(&self, item: &Item) -> Option<String> {
        let span = item.span.as_ref()?;
        let file = span.filename.to_string_lossy().replace('\\', "/");

        let is_module = matches!(self.classify(item), DocKind::Module);
        let omit_line = is_module && module_in_own_file(&file, item.name.as_deref());

        let location = if omit_line {
            format!("`{file}`")
        } else {
            format!("`{file}:{}`", span.begin.0)
        };
        Some(format!("_Defined at_ {location}"))
    }

    fn classify(&self, item: &Item) -> DocKind {
        DocKind::of(&item.inner)
    }

    /// A `_Reexported from_` annotation for items inlined from an external
    /// dependency, or `None` for native items (including those reexported from a
    /// private module of this crate).
    pub fn reexport_note(&self, id: Id) -> Option<String> {
        let origin = self.model.origins.get(&id)?;
        Some(format!("_Reexported from_ `{}`.", origin.display()))
    }
}

/// Compute a relative markdown link from file `from` to file `to`, both given as
/// paths relative to the output root.
pub fn relative_link(from: &Path, to: &Path) -> String {
    let from_dirs: Vec<_> = from
        .parent()
        .map(|p| p.components().collect())
        .unwrap_or_default();

    let to_dirs: Vec<_> = to
        .parent()
        .map(|p| p.components().collect())
        .unwrap_or_default();
    let to_name = to.file_name().map(|n| n.to_string_lossy().into_owned());

    // Length of the shared directory prefix.
    let common = from_dirs
        .iter()
        .zip(&to_dirs)
        .take_while(|(a, b)| a == b)
        .count();

    let ups = from_dirs.len() - common;
    let mut segments: Vec<String> = vec!["..".to_string(); ups];
    for comp in &to_dirs[common..] {
        segments.push(comp.as_os_str().to_string_lossy().into_owned());
    }
    if let Some(name) = to_name {
        segments.push(name);
    }

    if segments.is_empty() {
        // Linking a file to itself.
        to_name_only(to)
    } else {
        segments.join("/")
    }
}

fn to_name_only(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Slug used for a markdown heading anchor (GitHub-style: lowercase, spaces to
/// dashes, drop characters other than alphanumerics, `-`, `_`).
pub fn heading_slug(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// Heuristic: does this module live in its own file (`mod name;`, i.e. `name.rs`
/// or `name/mod.rs`) rather than being declared inline (`mod name { ... }`)?
fn module_in_own_file(file: &str, name: Option<&str>) -> bool {
    let Some(name) = name else { return false };
    let stem = Path::new(file).file_stem().map(|s| s.to_string_lossy());
    match stem.as_deref() {
        Some("mod") => {
            // `name/mod.rs`
            Path::new(file)
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n == name)
                .unwrap_or(false)
        }
        Some(s) => s == name,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn sibling_link() {
        assert_eq!(
            relative_link(&p("example/top/mod.md"), &p("example/top/Bar.md")),
            "Bar.md"
        );
    }

    #[test]
    fn link_down_into_subdir() {
        assert_eq!(
            relative_link(&p("example/lib.md"), &p("example/top/mod.md")),
            "top/mod.md"
        );
    }

    #[test]
    fn link_up_and_over() {
        assert_eq!(
            relative_link(&p("example/top/inner/Baz.md"), &p("example/Foo.md")),
            "../../Foo.md"
        );
    }

    #[test]
    fn slug_basics() {
        assert_eq!(heading_slug("make_thing"), "make_thing");
        assert_eq!(heading_slug("My Heading!"), "my-heading");
    }

    #[test]
    fn module_file_heuristic() {
        assert!(module_in_own_file("src/foo.rs", Some("foo")));
        assert!(module_in_own_file("src/foo/mod.rs", Some("foo")));
        assert!(!module_in_own_file("src/lib.rs", Some("foo")));
    }
}

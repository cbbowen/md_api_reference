//! End-to-end golden test.
//!
//! Parses a committed rustdoc JSON fixture (so the test is hermetic and does not
//! need a nightly toolchain), renders it, and compares every file against the
//! committed golden output under `tests/golden/`.
//!
//! To regenerate the golden files after an intentional change, run:
//!
//! ```text
//! BLESS=1 cargo test --test golden
//! ```
//!
//! If the *fixture crate* itself changes, first regenerate `example.json`:
//!
//! ```text
//! cd tests/fixtures/example
//! cargo +nightly rustdoc --lib -- --document-private-items \
//!     -Z unstable-options --output-format json
//! # then sanitize external_crates[].path to "" and copy to ../example.json
//! ```

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rustdoc_public_md::{generate, parse::parse_crate, reexport};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn golden_root() -> PathBuf {
    manifest_dir().join("tests/golden")
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = manifest_dir().join("tests/fixtures").join(name);
    fs::read(&path).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

/// Render every fixture case into one `relative path → contents` map. Each case
/// produces files under its own crate-name directory, so they never collide.
fn render_all() -> BTreeMap<PathBuf, String> {
    let mut files = BTreeMap::new();
    insert(&mut files, render_single());
    insert(&mut files, render_facade());
    files
}

fn insert(into: &mut BTreeMap<PathBuf, String>, files: Vec<rustdoc_public_md::render::RenderedFile>) {
    for f in files {
        into.insert(f.path, normalize(&f.contents));
    }
}

/// The single-crate `example` fixture.
fn render_single() -> Vec<rustdoc_public_md::render::RenderedFile> {
    let krate = parse_crate(&fixture_bytes("example.json"), "example.json").expect("parse example");
    generate(krate)
}

/// The cross-crate case: `--crate facade --reexport-crate dep`. Mirrors the
/// pipeline's inlining of reexported items before rendering.
fn render_facade() -> Vec<rustdoc_public_md::render::RenderedFile> {
    let mut facade = parse_crate(&fixture_bytes("facade.json"), "facade.json").expect("parse facade");
    let dep = parse_crate(&fixture_bytes("dep.json"), "dep.json").expect("parse dep");

    let name = reexport::crate_name(&dep).expect("dep crate name");
    let mut references = HashMap::new();
    references.insert(name, dep);

    reexport::inline_reexports(&mut facade, &references);
    generate(facade)
}

/// Normalize line endings so the comparison is stable across platforms / git
/// autocrlf settings.
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn golden_matches() {
    let rendered = render_all();

    if std::env::var_os("BLESS").is_some() {
        bless(&rendered);
        return;
    }

    let mut problems = Vec::new();

    // Every rendered file must match its golden counterpart.
    for (rel, contents) in &rendered {
        let golden_path = golden_root().join(rel);
        match fs::read_to_string(&golden_path) {
            Ok(expected) => {
                let expected = normalize(&expected);
                if &expected != contents {
                    problems.push(format!(
                        "mismatch in {}:\n{}",
                        rel.display(),
                        first_difference(&expected, contents)
                    ));
                }
            }
            Err(_) => problems.push(format!(
                "missing golden file {} (run `BLESS=1 cargo test --test golden`)",
                rel.display()
            )),
        }
    }

    // No stale golden files that the renderer no longer produces.
    for golden in walk(&golden_root()) {
        let rel = golden.strip_prefix(golden_root()).unwrap().to_path_buf();
        if !rendered.contains_key(&rel) {
            problems.push(format!("stale golden file with no rendered counterpart: {}", rel.display()));
        }
    }

    assert!(
        problems.is_empty(),
        "golden comparison failed ({} problem(s)):\n\n{}",
        problems.len(),
        problems.join("\n\n")
    );
}

fn bless(rendered: &BTreeMap<PathBuf, String>) {
    let root = golden_root();
    if root.exists() {
        fs::remove_dir_all(&root).expect("clear golden dir");
    }
    for (rel, contents) in rendered {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create golden dir");
        }
        fs::write(&path, contents).expect("write golden file");
    }
    eprintln!("blessed {} golden files under {}", rendered.len(), root.display());
}

/// A human-readable description of the first line that differs.
fn first_difference(expected: &str, actual: &str) -> String {
    for (i, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("  line {}:\n  - expected: {e:?}\n  - actual:   {a:?}", i + 1);
        }
    }
    let (el, al) = (expected.lines().count(), actual.lines().count());
    if el != al {
        format!("  differing line count: expected {el}, actual {al}")
    } else {
        "  (files differ in trailing whitespace)".to_string()
    }
}

/// Recursively collect files under `dir` (empty if it does not exist).
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

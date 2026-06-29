//! `rustdoc_public_md` — generate structured markdown documentation for the
//! public API of Rust crates. See `GOALS.md` for the specification.
//!
//! The binary wires these modules into a pipeline; they are exposed as a library
//! so integration tests can drive the model → render stages directly.

pub mod cli;
pub mod model;
pub mod output;
pub mod parse;
pub mod reexport;
pub mod render;
pub mod source;

use std::collections::HashMap;

use anyhow::{Context, Result};
use rustdoc_types::{Crate, Id};

use crate::cli::{Cli, CrateSpec};
use crate::model::{DocModel, ReexportOrigin};
use crate::render::RenderedFile;

/// Render the markdown files for one parsed crate. This is the pure
/// model → render core, with no I/O, used by both the binary and golden tests.
pub fn generate(krate: Crate) -> Vec<RenderedFile> {
    generate_with_origins(krate, HashMap::new())
}

/// Like [`generate`], but with reexport origins (from [`reexport::inline_reexports`])
/// so items inlined from an external dependency are annotated with their source.
pub fn generate_with_origins(
    krate: Crate,
    origins: HashMap<Id, ReexportOrigin>,
) -> Vec<RenderedFile> {
    let mut model = DocModel::build(krate);
    model.origins = origins;
    render::render(&model)
}

/// Run the full documentation pipeline: acquire and parse each crate, inline
/// cross-crate reexports, render, and write to `--out`.
pub fn run(cli: Cli) -> Result<()> {
    // Load reference crates once; their reexported items get inlined into each
    // primary crate so cross-crate `pub use`s are documented.
    let references = load_references(&cli)?;

    // Render every crate first; each contributes its own top-level directory
    // under `--out`. Writing happens once at the end so the empty-directory
    // check sees the original state, not files written for an earlier crate.
    let mut all_files = Vec::new();
    for spec in &cli.crates {
        let mut krate = load_crate(spec, &cli)?;
        let origins = reexport::inline_reexports(&mut krate, &references);
        let files = generate_with_origins(krate, origins);
        println!("{spec:?}: {} files", files.len());
        all_files.extend(files);
    }

    output::write_all(&cli.out, &all_files)
        .with_context(|| format!("writing documentation to {}", cli.out.display()))?;

    println!("Wrote {} files to {}", all_files.len(), cli.out.display());
    Ok(())
}

/// Acquire and parse the rustdoc JSON for a single crate spec.
fn load_crate(spec: &CrateSpec, cli: &Cli) -> Result<Crate> {
    let raw = source::acquire(spec, cli)
        .with_context(|| format!("acquiring rustdoc JSON for {spec:?}"))?;
    parse::parse_crate(&raw.bytes, &raw.origin)
}

/// Load every `--reexport-crate`, keyed by crate name for cross-crate resolution.
fn load_references(cli: &Cli) -> Result<HashMap<String, Crate>> {
    let mut references = HashMap::new();
    for spec in &cli.reexport_crates {
        let krate =
            load_crate(spec, cli).with_context(|| format!("loading reexport crate {spec:?}"))?;
        let name = reexport::crate_name(&krate)
            .with_context(|| format!("reexport crate {spec:?} has no root name"))?;
        references.insert(name, krate);
    }
    Ok(references)
}

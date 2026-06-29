//! `rustdoc_public_md` — generate structured markdown documentation for the
//! public API of Rust crates. See `GOALS.md` for the specification.

mod cli;
mod model;
mod parse;
mod render;
mod source;

use anyhow::{Context, Result};
use clap::Parser;
use rustdoc_types::Crate;

use crate::cli::{Cli, CrateSpec};
use crate::model::DocModel;

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

/// Run the documentation pipeline. Through Phase 1 this acquires and parses the
/// rustdoc JSON for every crate and reports a summary; later phases add the
/// model → render → write stages.
fn run(cli: Cli) -> Result<()> {
    for spec in &cli.crates {
        let krate = load_crate(spec, &cli)?;
        let model = DocModel::build(krate);
        let files = render::render(&model);
        report(spec, &model, &files);
    }

    Ok(())
}

/// Acquire and parse the rustdoc JSON for a single crate spec.
fn load_crate(spec: &CrateSpec, cli: &Cli) -> Result<Crate> {
    let raw = source::acquire(spec, cli)
        .with_context(|| format!("acquiring rustdoc JSON for {spec:?}"))?;
    parse::parse_crate(&raw.bytes, &raw.origin)
}

fn report(spec: &CrateSpec, model: &DocModel, files: &[render::RenderedFile]) {
    println!(
        "{spec:?}: {} documented items, {} files rendered ({} raw index entries)",
        model.items.len(),
        files.len(),
        model.krate.index.len(),
    );
    // Temporary Phase 3 preview: dump each rendered file. Phase 4 writes to disk.
    for file in files {
        let path = file.path.to_string_lossy().replace('\\', "/");
        println!("\n===== {path} =====");
        println!("{}", file.contents);
    }
}

//! `rustdoc_public_md` — generate structured markdown documentation for the
//! public API of Rust crates. See `GOALS.md` for the specification.

mod cli;
mod model;
mod output;
mod parse;
mod render;
mod source;

use anyhow::{Context, Result};
use clap::Parser;
use rustdoc_types::Crate;

use crate::cli::{Cli, CrateSpec};
use crate::model::DocModel;

/// Run the documentation pipeline end to end.

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    // Render every crate first; each contributes its own top-level directory
    // under `--out`. Writing happens once at the end so the empty-directory
    // check sees the original state, not files written for an earlier crate.
    let mut all_files = Vec::new();
    for spec in &cli.crates {
        let krate = load_crate(spec, &cli)?;
        let model = DocModel::build(krate);
        let files = render::render(&model);
        println!(
            "{spec:?}: {} documented items, {} files",
            model.items.len(),
            files.len(),
        );
        all_files.extend(files);
    }

    output::write_all(&cli.out, &all_files)
        .with_context(|| format!("writing documentation to {}", cli.out.display()))?;

    println!(
        "Wrote {} files to {}",
        all_files.len(),
        cli.out.display()
    );
    Ok(())
}

/// Acquire and parse the rustdoc JSON for a single crate spec.
fn load_crate(spec: &CrateSpec, cli: &Cli) -> Result<Crate> {
    let raw = source::acquire(spec, cli)
        .with_context(|| format!("acquiring rustdoc JSON for {spec:?}"))?;
    parse::parse_crate(&raw.bytes, &raw.origin)
}

//! `rustdoc_public_md` — generate structured markdown documentation for the
//! public API of Rust crates. See `GOALS.md` for the specification.

mod cli;
mod parse;
mod source;

use anyhow::{Context, Result};
use clap::Parser;
use rustdoc_types::Crate;

use crate::cli::{Cli, CrateSpec};

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
        report(spec, &krate);
    }

    for spec in &cli.reexport_crates {
        let krate = load_crate(spec, &cli)?;
        report(spec, &krate);
    }

    Ok(())
}

/// Acquire and parse the rustdoc JSON for a single crate spec.
fn load_crate(spec: &CrateSpec, cli: &Cli) -> Result<Crate> {
    let raw = source::acquire(spec, cli)
        .with_context(|| format!("acquiring rustdoc JSON for {spec:?}"))?;
    parse::parse_crate(&raw.bytes, &raw.origin)
}

fn report(spec: &CrateSpec, krate: &Crate) {
    let root_name = krate
        .index
        .get(&krate.root)
        .and_then(|item| item.name.as_deref())
        .unwrap_or("<unknown>");
    println!(
        "{spec:?}: root `{root_name}`, {} items in index, format_version {}",
        krate.index.len(),
        krate.format_version,
    );
}

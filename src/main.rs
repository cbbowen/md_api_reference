//! `rustdoc_public_md` — generate structured markdown documentation for the
//! public API of Rust crates. See `GOALS.md` for the specification.

mod cli;
mod model;
mod parse;
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
        report(spec, &model);
    }

    Ok(())
}

/// Acquire and parse the rustdoc JSON for a single crate spec.
fn load_crate(spec: &CrateSpec, cli: &Cli) -> Result<Crate> {
    let raw = source::acquire(spec, cli)
        .with_context(|| format!("acquiring rustdoc JSON for {spec:?}"))?;
    parse::parse_crate(&raw.bytes, &raw.origin)
}

fn report(spec: &CrateSpec, model: &DocModel) {
    println!(
        "{spec:?}: {} documented items ({} in raw index)",
        model.items.len(),
        model.krate.index.len(),
    );
    for item in model.items() {
        let canonical = item.canonical.0.join("::");
        let file = item
            .file
            .as_ref()
            .map(|f| f.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| "(inline)".to_string());
        let alts = if item.alternates.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = item
                .alternates
                .iter()
                .map(|p| p.0.join("::"))
                .collect();
            format!("  [also: {}]", list.join(", "))
        };
        println!("  {canonical}  →  {file}{alts}");
    }
}

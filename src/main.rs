//! Thin binary entry point; the pipeline lives in the library crate.

use anyhow::Result;
use clap::Parser;

use rustdoc_public_md::cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    rustdoc_public_md::run(cli)
}

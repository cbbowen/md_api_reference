//! Thin binary entry point; the pipeline lives in the library crate.

use anyhow::Result;
use clap::Parser;

use md_api_reference::cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    md_api_reference::run(cli)
}

//! Deserialize rustdoc JSON into [`rustdoc_types::Crate`], with an up-front
//! `format_version` compatibility check that produces a clear error.

use anyhow::{Context, Result};
use rustdoc_types::{Crate, FORMAT_VERSION};
use serde::Deserialize;

/// Minimal view used to read `format_version` before attempting a full parse,
/// so a format mismatch reports a helpful message instead of an opaque serde
/// error about an unexpected shape.
#[derive(Deserialize)]
struct VersionProbe {
    format_version: u32,
}

/// Parse rustdoc JSON `bytes`, checking the format version first.
///
/// `origin` describes where the bytes came from (a URL or file path) and is
/// woven into error messages.
pub fn parse_crate(bytes: &[u8], origin: &str) -> Result<Crate> {
    let probe: VersionProbe = serde_json::from_slice(bytes)
        .with_context(|| format!("reading rustdoc JSON from {origin}"))?;

    if probe.format_version != FORMAT_VERSION {
        println!(
            "WARNING: rustdoc JSON from {origin} has format_version {found}, but this build \
             supports format_version {expected}.\n\
             The JSON was produced by a different rustdoc/toolchain. Regenerate it with a \
             matching nightly toolchain, or rebuild this tool against a `rustdoc-types` \
             version that targets format_version {found}.",
            found = probe.format_version,
            expected = FORMAT_VERSION,
        );
    }

    serde_json::from_slice(bytes).with_context(|| format!("parsing rustdoc JSON from {origin}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_origin_on_invalid_json() {
        let err = parse_crate(b"not json", "somewhere").unwrap_err();
        assert!(err.to_string().contains("somewhere"));
    }
}

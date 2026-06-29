//! Acquire rustdoc JSON for a crate, either by downloading it from docs.rs or
//! generating it locally with the `rustdoc-json` crate.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::cli::{Cli, CrateSpec, SourceMode};

/// User-Agent sent to docs.rs. docs.rs rejects requests without one.
const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/)"
);

/// Upper bound on the compressed download size, raised well above ureq's 10 MB
/// default since large crates produce large JSON. Decompression is streamed.
const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;

/// Toolchain used for local JSON generation. rustdoc JSON is nightly-only.
const LOCAL_TOOLCHAIN: &str = "nightly";

/// Decompressed rustdoc JSON together with a human-readable origin for errors.
pub struct RawJson {
    pub bytes: Vec<u8>,
    pub origin: String,
}

/// Obtain rustdoc JSON for `spec`, honoring the chosen [`SourceMode`].
pub fn acquire(spec: &CrateSpec, cli: &Cli) -> Result<RawJson> {
    match effective_source(spec, cli.source_mode()) {
        EffectiveSource::DocsRs => acquire_from_docs_rs(spec, cli),
        EffectiveSource::Local => acquire_locally(spec, cli),
    }
}

/// Which concrete source to use after resolving auto-detection.
enum EffectiveSource {
    DocsRs,
    Local,
}

fn effective_source(spec: &CrateSpec, mode: SourceMode) -> EffectiveSource {
    match mode {
        SourceMode::DocsRs => EffectiveSource::DocsRs,
        SourceMode::Local => EffectiveSource::Local,
        SourceMode::Auto => match spec {
            CrateSpec::Named { .. } => EffectiveSource::DocsRs,
            CrateSpec::Path(_) => EffectiveSource::Local,
        },
    }
}

fn acquire_from_docs_rs(spec: &CrateSpec, cli: &Cli) -> Result<RawJson> {
    let (name, version) = match spec {
        CrateSpec::Named { name, version } => (name.as_str(), version.as_deref()),
        CrateSpec::Path(path) => bail!(
            "cannot download `{}` from docs.rs: it is a local path. \
             Drop `--from-docs-rs` or pass a crate name instead.",
            path.display(),
        ),
    };
    let version = version.unwrap_or("latest");
    let target_path = if cli.target.is_empty() {
        "".to_owned()
    } else {
        format!("/{}", cli.target)
    };
    let url = format!("https://docs.rs/crate/{name}/{version}{target_path}/json",);

    let bytes = download_and_decompress(&url)
        .with_context(|| format!("downloading rustdoc JSON for `{name}` from {url}"))?;

    Ok(RawJson { bytes, origin: url })
}

/// GET `url` and stream-decompress the zstd body docs.rs returns.
fn download_and_decompress(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("HTTP request to {url} failed"))?;

    let reader = response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .reader();

    let mut decoder = ruzstd::decoding::StreamingDecoder::new(reader)
        .context("initializing zstd decoder (unexpected response encoding?)")?;

    let mut bytes = Vec::new();
    decoder
        .read_to_end(&mut bytes)
        .context("decompressing zstd response body")?;

    Ok(bytes)
}

fn acquire_locally(spec: &CrateSpec, cli: &Cli) -> Result<RawJson> {
    let mut builder = rustdoc_json::Builder::default()
        .toolchain(LOCAL_TOOLCHAIN)
        // Private items must be visible so we can follow reexports out of
        // private modules during reachability analysis.
        .document_private_items(true)
        .all_features(true);

    let origin: String;

    match spec {
        CrateSpec::Path(path) => {
            let manifest = manifest_path_for(path);
            origin = format!("local crate at {}", manifest.display());
            builder = builder.manifest_path(&manifest);
        }
        CrateSpec::Named { name, .. } => {
            let manifest = cli.manifest_path.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "`--local` with crate name `{name}` requires `--manifest-path` \
                     pointing at the workspace that contains it"
                )
            })?;
            origin = format!("local package `{name}` via {}", manifest.display());
            builder = builder.manifest_path(&manifest).package(name);
        }
    }

    let json_path = builder
        .build()
        .with_context(|| format!("generating rustdoc JSON for {origin}"))?;

    let bytes = std::fs::read(&json_path)
        .with_context(|| format!("reading generated JSON at {}", json_path.display()))?;

    Ok(RawJson { bytes, origin })
}

/// Resolve a `--crate` path to a `Cargo.toml`: accept either a manifest file
/// directly or a directory containing one.
fn manifest_path_for(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("Cargo.toml")
    } else {
        path.to_path_buf()
    }
}

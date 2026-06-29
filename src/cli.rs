//! Command-line interface definition and crate-spec parsing.

use std::path::{Path, PathBuf};

use clap::Parser;

/// Default target triple used for docs.rs JSON downloads.
pub const DEFAULT_TARGET: &str = "x86_64-unknown-linux-gnu";

/// Generate structured markdown documentation for the public API of Rust crates.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Crate to document: a crate name, `name@version`, or a path to a local
    /// crate. Repeat to document multiple crates together.
    #[arg(
        long = "crate",
        value_name = "SPEC",
        required = true,
        value_parser = parse_crate_spec,
    )]
    pub crates: Vec<CrateSpec>,

    /// Additional crate whose items are documented only where publicly
    /// reexported from a `--crate`. Same SPEC forms as `--crate`.
    #[arg(
        long = "reexport-crate",
        value_name = "SPEC",
        value_parser = parse_crate_spec,
    )]
    pub reexport_crates: Vec<CrateSpec>,

    /// Output directory. The tool errors if it already exists and is non-empty.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,

    /// Force downloading rustdoc JSON from docs.rs for every crate.
    #[arg(long, conflicts_with = "local")]
    pub from_docs_rs: bool,

    /// Force generating rustdoc JSON locally for every crate.
    #[arg(long)]
    pub local: bool,

    /// Manifest path used when generating JSON locally.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Target triple for docs.rs downloads.
    #[arg(long, value_name = "TRIPLE", default_value = DEFAULT_TARGET)]
    pub target: String,
}

impl Cli {
    /// How the JSON source for each crate should be chosen.
    pub fn source_mode(&self) -> SourceMode {
        match (self.from_docs_rs, self.local) {
            (true, _) => SourceMode::DocsRs,
            (_, true) => SourceMode::Local,
            _ => SourceMode::Auto,
        }
    }
}

/// How rustdoc JSON is obtained for a crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    /// Auto-detect from each [`CrateSpec`] (named ⇒ docs.rs, path ⇒ local).
    Auto,
    /// Always download from docs.rs.
    DocsRs,
    /// Always generate locally.
    Local,
}

/// A crate to document, as named on the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrateSpec {
    /// A registry crate, optionally pinned to a version.
    Named {
        name: String,
        version: Option<String>,
    },
    /// A path to a local crate (directory or `Cargo.toml`).
    Path(PathBuf),
}

/// Parse a `--crate` / `--reexport-crate` value into a [`CrateSpec`].
///
/// Detection: anything path-like (contains a separator, starts with `.`/`~`,
/// ends with `.toml`, or already exists on disk) is a [`CrateSpec::Path`];
/// otherwise it is a named crate, with an optional `@version` suffix.
fn parse_crate_spec(s: &str) -> Result<CrateSpec, String> {
    if s.is_empty() {
        return Err("crate spec must not be empty".to_string());
    }

    if looks_like_path(s) {
        return Ok(CrateSpec::Path(PathBuf::from(s)));
    }

    match s.split_once('@') {
        Some((name, version)) => {
            validate_crate_name(name)?;
            if version.is_empty() {
                return Err(format!("crate spec `{s}` has an empty version"));
            }
            Ok(CrateSpec::Named {
                name: name.to_string(),
                version: Some(version.to_string()),
            })
        }
        None => {
            validate_crate_name(s)?;
            Ok(CrateSpec::Named {
                name: s.to_string(),
                version: None,
            })
        }
    }
}

/// Whether `s` should be treated as a filesystem path rather than a crate name.
fn looks_like_path(s: &str) -> bool {
    s.contains('/')
        || s.contains('\\')
        || s.starts_with('.')
        || s.starts_with('~')
        || s.ends_with(".toml")
        || Path::new(s).exists()
}

/// Validate that `name` is a plausible crate name (alphanumerics, `-`, `_`).
fn validate_crate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("crate name must not be empty".to_string());
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
    {
        return Err(format!(
            "invalid crate name `{name}`: unexpected character `{bad}` \
             (use a path like `./{name}` to document a local crate)"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_bare_name() {
        assert_eq!(
            parse_crate_spec("serde").unwrap(),
            CrateSpec::Named {
                name: "serde".to_string(),
                version: None,
            },
        );
    }

    #[test]
    fn parses_name_with_version() {
        assert_eq!(
            parse_crate_spec("serde@1.0.200").unwrap(),
            CrateSpec::Named {
                name: "serde".to_string(),
                version: Some("1.0.200".to_string()),
            },
        );
    }

    #[test]
    fn version_dots_do_not_trigger_path_detection() {
        // `1.0.200` contains dots, but the `@` form keeps it a named crate.
        assert!(matches!(
            parse_crate_spec("serde@1.0.200").unwrap(),
            CrateSpec::Named { .. }
        ));
    }

    #[test]
    fn detects_paths() {
        for s in ["./mycrate", "../mycrate", "a/b", "crate/Cargo.toml"] {
            assert!(
                matches!(parse_crate_spec(s).unwrap(), CrateSpec::Path(_)),
                "expected `{s}` to parse as a path",
            );
        }
    }

    #[test]
    fn windows_separator_is_a_path() {
        assert!(matches!(
            parse_crate_spec(r"C:\crates\mine").unwrap(),
            CrateSpec::Path(_)
        ));
    }

    #[test]
    fn rejects_empty_version() {
        assert!(parse_crate_spec("serde@").is_err());
    }

    #[test]
    fn rejects_bogus_name() {
        assert!(parse_crate_spec("not a crate!").is_err());
    }

    #[test]
    fn source_mode_reflects_flags() {
        let base = ["prog", "--crate", "serde", "--out", "docs"];
        let auto = Cli::parse_from(base);
        assert_eq!(auto.source_mode(), SourceMode::Auto);

        let docs_rs = Cli::parse_from([
            "prog",
            "--crate",
            "serde",
            "--out",
            "docs",
            "--from-docs-rs",
        ]);
        assert_eq!(docs_rs.source_mode(), SourceMode::DocsRs);

        let local = Cli::parse_from(["prog", "--crate", "serde", "--out", "docs", "--local"]);
        assert_eq!(local.source_mode(), SourceMode::Local);
    }

    #[test]
    fn from_docs_rs_and_local_conflict() {
        let res = Cli::try_parse_from([
            "prog",
            "--crate",
            "serde",
            "--out",
            "docs",
            "--from-docs-rs",
            "--local",
        ]);
        assert!(res.is_err());
    }
}

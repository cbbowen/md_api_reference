//! Write rendered markdown files to the output directory.
//!
//! The output directory is created if missing. If it already exists and is
//! non-empty, writing is refused so an accidental run cannot clobber unrelated
//! files. Per-file collision numbering already happened during model
//! construction, so paths here are unique.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::render::RenderedFile;

/// Write all `files` (paths relative to `out`) beneath `out`.
pub fn write_all(out: &Path, files: &[RenderedFile]) -> Result<()> {
    prepare_out_dir(out)?;

    for file in files {
        let full = out.join(&file.path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        fs::write(&full, &file.contents).with_context(|| format!("writing {}", full.display()))?;
    }

    Ok(())
}

/// Ensure `out` exists and is safe to write into: create it if absent, error if
/// it exists and already contains entries.
fn prepare_out_dir(out: &Path) -> Result<()> {
    match fs::read_dir(out) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                bail!(
                    "output directory {} already exists and is not empty; \
                     refusing to overwrite. Choose an empty or new --out directory.",
                    out.display(),
                );
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(out)
            .with_context(|| format!("creating output directory {}", out.display())),
        Err(err) => {
            Err(err).with_context(|| format!("inspecting output directory {}", out.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rendered(path: &str, contents: &str) -> RenderedFile {
        RenderedFile {
            path: PathBuf::from(path),
            contents: contents.to_string(),
        }
    }

    /// A unique temp directory under the system temp dir.
    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rdpm-{tag}-{nanos}"))
    }

    #[test]
    fn writes_nested_files_creating_dirs() {
        let dir = temp_dir("write");
        let files = vec![
            rendered("example/lib.md", "# Crate"),
            rendered("example/top/mod.md", "# Module"),
            rendered("example/top/Bar.md", "# Bar"),
        ];

        write_all(&dir, &files).unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("example/lib.md")).unwrap(),
            "# Crate"
        );
        assert_eq!(
            fs::read_to_string(dir.join("example/top/Bar.md")).unwrap(),
            "# Bar"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn creates_missing_output_dir() {
        let dir = temp_dir("missing");
        assert!(!dir.exists());
        write_all(&dir, &[rendered("a.md", "x")]).unwrap();
        assert!(dir.join("a.md").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refuses_non_empty_dir() {
        let dir = temp_dir("nonempty");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("preexisting.txt"), "keep me").unwrap();

        let err = write_all(&dir, &[rendered("a.md", "x")]).unwrap_err();
        assert!(err.to_string().contains("not empty"), "got: {err}");
        // The pre-existing file is untouched.
        assert_eq!(
            fs::read_to_string(dir.join("preexisting.txt")).unwrap(),
            "keep me"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_existing_dir_is_allowed() {
        let dir = temp_dir("empty");
        fs::create_dir_all(&dir).unwrap();
        write_all(&dir, &[rendered("a.md", "x")]).unwrap();
        assert!(dir.join("a.md").exists());
        fs::remove_dir_all(&dir).ok();
    }
}

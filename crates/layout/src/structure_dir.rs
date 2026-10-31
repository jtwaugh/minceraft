//! Lists the template files in a directory for `build.rs`, which includes
//! this file by path, so keep it free of dependencies.

use std::path::{Path, PathBuf};

/// The `*.toml` files in `dir` as `(name, path)`, `name` being the file
/// stem, sorted by file name. A missing directory has none.
pub fn template_files(dir: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for e in entries {
        let path = e?.path();
        if path.extension().is_some_and(|x| x == "toml") {
            let stem = path.file_stem().and_then(|s| s.to_str());
            let name = stem.ok_or_else(|| std::io::Error::other("non-UTF-8 file name"))?;
            out.push((name.to_string(), path));
        }
    }
    out.sort();
    Ok(out)
}

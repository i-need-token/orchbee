//! Where a Windows fixture that runs real Sessions makes its root: the temporary directory in its
//! long form. `TEMP` may name that directory in its 8.3 form, as a CI runner's does, while a
//! terminal's shell reports the long form. A Workspace keeps the Directory it was given and compares
//! what its first terminal reports by spelling, so in a short-form root every Workspace would
//! record its own Directory as a move, a Workbench change its test never made. That short-form case
//! stays uncovered here until directories compare by identity.

use std::path::PathBuf;

/// The temporary directory in its long form, without the `\\?\` prefix that canonicalizing adds.
pub fn long_temporary_directory() -> PathBuf {
    let canonical = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let path = canonical.to_str().unwrap();
    match path.strip_prefix(r"\\?\UNC\") {
        Some(share) => PathBuf::from(format!(r"\\{share}")),
        None => PathBuf::from(path.strip_prefix(r"\\?\").unwrap_or(path)),
    }
}

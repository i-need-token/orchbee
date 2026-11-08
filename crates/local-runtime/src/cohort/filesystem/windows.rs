//! Windows cohort admission pins private ancestors and artifact handles without delete sharing.
use crate::{cohort::CohortArtifact, environment::environment_stem};
use std::{fs::File, io, path::Path};
/// Environments ship distinct cohort names (ORC-382); the environment owns the compiled stem.
pub(crate) fn has_release_names() -> bool {
    crate::environment::is_release()
}
pub(crate) fn open_directory(path: &Path) -> io::Result<Directory> {
    Ok(Directory {
        _handles: crate::private_filesystem::open_tree(path)?,
    })
}
pub(crate) fn artifact_file_name(artifact: CohortArtifact) -> String {
    match artifact {
        CohortArtifact::Client => format!("{}.exe", environment_stem()),
        CohortArtifact::Server => format!("{}-server.exe", environment_stem()),
        CohortArtifact::Worker => format!("{}-worker.exe", environment_stem()),
        CohortArtifact::TerminalHost => format!("{}-terminal-host.exe", environment_stem()),
    }
}
pub(crate) struct Directory {
    _handles: Vec<File>,
}
pub(crate) fn open_file(
    _directory: &Directory,
    _name: &str,
    _executable: bool,
    path: &Path,
) -> io::Result<File> {
    crate::private_filesystem::open_file(path, false, false)
}

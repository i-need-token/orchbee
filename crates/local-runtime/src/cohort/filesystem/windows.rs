//! Windows cohort admission pins private ancestors and artifact handles without delete sharing.
use crate::cohort::CohortArtifact;
use std::{fs::File, io, path::Path};
pub(crate) fn open_directory(path: &Path) -> io::Result<Directory> {
    Ok(Directory {
        _handles: crate::private_filesystem::open_tree(path)?,
    })
}
pub(crate) const fn artifact_file_name(artifact: CohortArtifact) -> &'static str {
    match artifact {
        CohortArtifact::Client => "orchbee.exe",
        CohortArtifact::Server => "orchbee-server.exe",
        CohortArtifact::Worker => "orchbee-worker.exe",
        CohortArtifact::TerminalHost => "orchbee-terminal-host.exe",
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

//! Windows cohort admission pins private ancestors and artifact handles without delete sharing.
use crate::cohort::CohortArtifact;
use std::{fs::File, io, path::Path};
/// The stem of this build's cohort file names. Packaging sets `ORCHBEE_ARTIFACT_STEM` while it
/// compiles the cohort, so each environment ships its own names (ORC-382). A build without it keeps
/// the shipped `orchbee` names.
const ARTIFACT_STEM: &str = match option_env!("ORCHBEE_ARTIFACT_STEM") {
    Some(stem) => stem,
    None => "orchbee",
};
pub(crate) fn open_directory(path: &Path) -> io::Result<Directory> {
    Ok(Directory {
        _handles: crate::private_filesystem::open_tree(path)?,
    })
}
pub(crate) fn artifact_file_name(artifact: CohortArtifact) -> String {
    match artifact {
        CohortArtifact::Client => format!("{ARTIFACT_STEM}.exe"),
        CohortArtifact::Server => format!("{ARTIFACT_STEM}-server.exe"),
        CohortArtifact::Worker => format!("{ARTIFACT_STEM}-worker.exe"),
        CohortArtifact::TerminalHost => format!("{ARTIFACT_STEM}-terminal-host.exe"),
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

//! Windows cohort admission pins private ancestors and artifact handles without delete sharing.
use crate::cohort::CohortArtifact;
use std::{fs::File, io, path::Path};
const RELEASE_ARTIFACT_STEM: &str = "orchbee";
/// The stem of this build's cohort file names. Staging sets `ORCHBEE_ARTIFACT_STEM` while it
/// compiles the cohort, so each environment ships its own names (ORC-382). A build without it keeps
/// the shipped `orchbee` names.
const ARTIFACT_STEM: &str = match option_env!("ORCHBEE_ARTIFACT_STEM") {
    Some(stem) => stem,
    None => RELEASE_ARTIFACT_STEM,
};
// A malformed stem would name files no staging produces, so it fails the build instead.
const _: () = assert!(
    valid_artifact_stem(ARTIFACT_STEM.as_bytes()),
    "ORCHBEE_ARTIFACT_STEM must be orchbee, ob-dev-<16 lowercase hex>, or ob-test-<16 lowercase hex>"
);
/// Mirrors `ARTIFACT_STEM_PATTERN` in `scripts/stage-native-windows.mjs`.
const fn valid_artifact_stem(stem: &[u8]) -> bool {
    let digest = match stem {
        b"orchbee" => return true,
        [b'o', b'b', b'-', b'd', b'e', b'v', b'-', digest @ ..]
        | [b'o', b'b', b'-', b't', b'e', b's', b't', b'-', digest @ ..] => digest,
        _ => return false,
    };
    if digest.len() != 16 {
        return false;
    }
    let mut index = 0;
    while index < digest.len() {
        if !matches!(digest[index], b'0'..=b'9' | b'a'..=b'f') {
            return false;
        }
        index += 1;
    }
    true
}
pub(crate) fn has_release_names() -> bool {
    ARTIFACT_STEM == RELEASE_ARTIFACT_STEM
}
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

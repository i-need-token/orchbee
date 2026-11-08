//! Windows scrollback documents opened in an editor (ORC-397). The Terminal Host writes each
//! capture it hands an editor to the data root's `internal/tmp/` as `scrollback-<time>.txt`, the
//! UTC time spelled `YYYY-MM-DDTHH-MM-SSZ` and followed by `-2`, `-3` and so on when that name is
//! taken, and removes the document when its editor closes. A document outlives its editor only when
//! the Host could not remove it, so this owner removes documents last written more than a day ago.
//! It never creates the folder, follows a link out of it, or touches a name outside that scheme.
//!
//! Cross-runtime contract: search for `editor-document-name` before changing the name; the Terminal
//! Host writes it.
use crate::{private_filesystem as permissions, time_name};
use std::{
    fs, io,
    path::Path,
    time::{Duration, SystemTime},
};

#[cfg(test)]
mod tests;

/// How long a document stays after it was last written. The Host writes a document immediately
/// before it starts the editor that opens it, so a document an editor opened within the last day
/// is never removed.
const RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
/// The entries one pass examines, so a pass stays short however crowded the folder is.
const MAXIMUM_ENTRIES: usize = 1024;
const PREFIX: &str = "scrollback-";
const SUFFIX: &str = ".txt";

/// What one removal pass did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EditorDocumentRemoval {
    /// Documents removed.
    pub removed: u64,
    /// Expired documents that stay because removing them failed, for example while another
    /// process holds one open without delete sharing; a later pass tries again.
    pub failed: u64,
}

/// Removes the documents in `folder` last written more than one day before `now`. A missing folder
/// holds nothing and stays missing. Only regular files whose names follow the document scheme are
/// removed; a link or directory with such a name stays, and so does a document last written after
/// `now`. A pass examines at most 1,024 entries.
///
/// # Errors
///
/// The folder, or a folder above it, is a link, or the folder is not private; nothing is removed.
pub fn remove_stale_editor_documents(
    folder: &Path,
    now: SystemTime,
) -> io::Result<EditorDocumentRemoval> {
    // The pins keep every folder on the path from being renamed or replaced during the pass.
    let _pins = match permissions::open_tree(folder) {
        Ok(pins) => pins,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(EditorDocumentRemoval::default());
        }
        Err(error) => return Err(error),
    };
    let mut removal = EditorDocumentRemoval::default();
    for entry in fs::read_dir(folder)?.take(MAXIMUM_ENTRIES).flatten() {
        if !entry.file_name().to_str().is_some_and(document_name) {
            continue;
        }
        let path = entry.path();
        // Not following a link, a link with a document's name is never a regular file here, and
        // removing a name removes only that name.
        let expired = fs::symlink_metadata(&path).is_ok_and(|metadata| {
            metadata.is_file()
                && metadata
                    .modified()
                    .is_ok_and(|modified| expired(modified, now))
        });
        if !expired {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => removal.removed += 1,
            // The Host removed it after its editor closed.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => removal.failed += 1,
        }
    }
    Ok(removal)
}

/// Whether `name` is exactly a document name the Host writes.
fn document_name(name: &str) -> bool {
    name.strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .and_then(time_name::parse)
        .is_some()
}

/// A document last written more than a day before `now`; a time after `now` never expires.
fn expired(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age > RETENTION)
}

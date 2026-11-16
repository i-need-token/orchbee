//! The restore after a fallback for the checkpoint slots and kept terminal output. The Server that
//! holds the store is their only writer, so it puts back what a held version migrated, as the data
//! root layout's restore after a fallback states for these records.
use super::{SessionStore, publish_private};
use crate::{
    DataRoot, LogLevel, LogValue,
    checkpoint_rotation::{Candidate, HEADER_BYTES, SLOTS},
    private_filesystem as permissions, terminal_output as output,
};
use std::{
    io::{self, Read, Write},
    num::NonZeroU64,
    path::Path,
};

impl SessionStore {
    /// While the store's counting move carries `restore` for the version this process runs from,
    /// puts back each checkpoint slot and kept output file this build cannot use from the
    /// unnumbered copy that an upgrade folder of a held version keeps at the same path, when this
    /// build can use that copy. `checkpoint` and `output` are the Server's payload checks.
    ///
    /// A copy exists only for a file a held version migrated, and it is the file as the version
    /// before that one left it, so the Session opens as it was before the move. A file the held
    /// version created has no copy and stays as it is, and so does every file when anything fails:
    /// the restore is best effort, and loading then treats what is left as it always does. A slot
    /// it replaces moves aside as a corrupt original first, as publication keeps one.
    pub fn restore_after_fallback(
        &self,
        root: &DataRoot,
        checkpoint: impl FnMut(&[u8]) -> bool,
        output: impl FnMut(&[u8]) -> bool,
    ) {
        let held = crate::release_store::restore_sources();
        if !held.is_empty() {
            self.restore_from(root, &held, checkpoint, output);
        }
    }

    /// [`Self::restore_after_fallback`] from the upgrade folders of `held`, the version held last
    /// first.
    pub(crate) fn restore_from(
        &self,
        root: &DataRoot,
        held: &[String],
        mut checkpoint: impl FnMut(&[u8]) -> bool,
        mut output: impl FnMut(&[u8]) -> bool,
    ) {
        let maximum = self.maximum_bytes;
        let mut usable_slot = |bytes: &[u8]| {
            matches!(
                Candidate::decode(bytes, maximum),
                Candidate::Valid { payload, .. } if checkpoint(&payload)
            )
        };
        for name in SLOTS {
            let path = self.directory.join(name);
            let copy = replacement(
                root,
                held,
                &path,
                maximum + HEADER_BYTES + 1,
                &mut usable_slot,
            );
            if copy.is_some_and(|copy| self.restore_slot(&path, &copy).is_ok()) {
                logged("checkpoint", name);
            }
        }
        let Ok(tokens) = self.terminal_output_tokens() else {
            return;
        };
        let directory = self.directory.join(output::DIRECTORY);
        for token in tokens.into_iter().filter_map(NonZeroU64::new) {
            let path = directory.join(output::entry_name(token));
            let copy = replacement(root, held, &path, output::FRAME_LIMIT, |bytes| {
                output::unframe(token, bytes).is_some_and(|payload| output(&payload))
            });
            // Kept output this build cannot read is no recovery data, as reading it as absent shows.
            let pending = directory.join(output::PENDING);
            if copy.is_some_and(|copy| publish_private(&pending, &path, &copy, "output").is_ok()) {
                logged("terminal-output", "entry");
            }
        }
    }

    /// Replaces a slot as publication replaces an invalid one: the copy is flushed to the staging
    /// file, the unusable original moves aside as a corrupt original, and the copy takes its name.
    fn restore_slot(&self, target: &Path, copy: &[u8]) -> io::Result<()> {
        let pending = self.directory.join("checkpoint.pending");
        self.cleanup_pending()?;
        let mut file = permissions::open_file(&pending, true, true)?;
        let written = file.write_all(copy).and_then(|()| file.sync_all());
        drop(file);
        let restored = written.and_then(|()| {
            self.preserve_invalid(target)?;
            permissions::replace(&pending, target)
        });
        if restored.is_err() {
            // The next store open discards the staging file if this removal fails too.
            let _ = self.cleanup_pending();
        }
        restored
    }
}

/// The held copy to put back at `path`: `None` unless the file exists, `usable` refuses it, and a
/// held copy `usable` accepts exists.
fn replacement(
    root: &DataRoot,
    held: &[String],
    path: &Path,
    maximum: usize,
    mut usable: impl FnMut(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    let current = read(path, maximum)?;
    if usable(&current) {
        return None;
    }
    crate::record_migration::held_copy(root, path, held, maximum, usable)
}

/// The file's first `maximum` bytes, or `None` when it is absent or cannot be read.
fn read(path: &Path, maximum: usize) -> Option<Vec<u8>> {
    let file = permissions::open_snapshot_file(path).ok()?;
    let mut bytes = Vec::new();
    file.take(maximum as u64).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn logged(record: &'static str, file: &'static str) {
    crate::log_event(
        LogLevel::Warning,
        "record-restored",
        &[
            ("record", LogValue::Token(record)),
            ("file", LogValue::Token(file)),
        ],
    );
}

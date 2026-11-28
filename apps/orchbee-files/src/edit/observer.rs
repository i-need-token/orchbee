//! What a successful edit tells whoever records Local History: each save the Client asked to
//! record, and each item a rename or move took to another path. The caller of
//! [`Service::new`](super::Service::new) injects the [`Observer`]; [`Unobserved`] hears nothing.
//!
//! An observer hears an edit on the edit's own thread once its reply went out, and a request
//! repeated under the same identity is heard only when it ran. An observer returns nothing, since
//! nothing it does fails the edit. It may block briefly, as a lock shared with other Sessions does,
//! but never holds back another edit of the same path.

use orchbee_files_protocol::{FsWriteResult, WorkspaceId, WriteHistory};

use super::write::Write;
use crate::{fs::Encoder, scope::path::NativePath};

/// Hears the edits that succeeded.
pub(crate) trait Observer: Send + Sync {
    /// An `fs.write` whose request named a `history` reason wrote its file.
    fn saved(&self, saved: &Saved<'_>);

    /// An `fs.rename` or `fs.move` took an item from one path to another: a file, a link, which
    /// moves itself, or a folder with everything in it, which a merge moved into a folder already
    /// there.
    fn moved(&self, moved: &Moved<'_>);
}

/// One save to record.
#[expect(
    dead_code,
    reason = "Local History, not yet served, reads what a save reports"
)]
pub(crate) struct Saved<'a> {
    pub(crate) workspace: WorkspaceId,
    /// The file as the request named it; a link there still names what it named.
    pub(crate) path: &'a NativePath,
    /// Why the Client wrote it.
    pub(crate) history: WriteHistory,
    /// The file after the write: its time, size and the SHA-256 digest of its content.
    pub(crate) written: &'a FsWriteResult,
    pub(super) write: &'a Write,
}

impl Saved<'_> {
    /// The bytes the write put in the file, encoded again from the text it was given, so a change
    /// another writer made since is not taken for the save.
    #[expect(
        dead_code,
        reason = "Local History, not yet served, keeps a copy of each saved file"
    )]
    pub(crate) fn content(&self) -> Vec<u8> {
        let mut encoder = Encoder::new(self.write.target, &self.write.text);
        let (mut content, mut piece) = (Vec::new(), Vec::new());
        while encoder.next(&mut piece) {
            content.extend_from_slice(&piece);
        }
        content
    }
}

/// How an item came to its new path, which Local History labels as VS Code does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Relocation {
    /// `fs.rename`.
    Rename,
    /// `fs.move`.
    Move,
}

/// One item taken to another path.
#[expect(
    dead_code,
    reason = "Local History, not yet served, moves the entries of what moved"
)]
pub(crate) struct Moved<'a> {
    pub(crate) workspace: WorkspaceId,
    pub(crate) from: &'a NativePath,
    /// Its final path: the name a conflict rule chose, or the folder it merged into.
    pub(crate) to: &'a NativePath,
    pub(crate) relocation: Relocation,
}

/// The observer while nothing records Local History.
pub(crate) struct Unobserved;

impl Observer for Unobserved {
    fn saved(&self, _: &Saved<'_>) {}

    fn moved(&self, _: &Moved<'_>) {}
}

//! What a successful edit tells whoever records Local History, which batch ④ serves: each save the
//! Client asked to record. The caller of [`Service::new`](super::Service::new) injects the
//! [`Observer`]; [`Unobserved`] hears nothing.
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
}

/// One save to record.
#[expect(
    dead_code,
    reason = "Local History, batch ④, reads what a save reports"
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
        reason = "Local History, batch ④, keeps a copy of each saved file"
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

/// The observer while nothing records Local History.
pub(crate) struct Unobserved;

impl Observer for Unobserved {
    fn saved(&self, _: &Saved<'_>) {}
}

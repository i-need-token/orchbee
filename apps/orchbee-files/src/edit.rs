//! File edits, the operations `files-edit` adds. Each mutation runs under its request identity
//! ([`outcomes`]), on a thread where waiting is allowed. Each path it changes passes the scope's
//! one mutation check, [`View::mutable`], under the view its call started with, when the change
//! starts. A mutation never logs a path, a name or file content, and tells the [`Observer`] its
//! caller injected what it changed.
//!
//! - `fs.write` ([`write`]) takes the content its Puts staged and rewrites the file in place.

mod observer;
mod outcomes;
mod write;

use std::sync::Arc;

use orchbee_files_protocol::{
    ErrorCode, ErrorReply, FilePath, FilesError, FilesReply, FsWrite, MutationReply, Operation,
    RequestOutcome,
};

pub(crate) use self::observer::{Observer, Saved, Unobserved};
use self::{
    outcomes::{Outcomes, finish},
    write::{System, Write, Writing},
};
use crate::{
    blocking::Helpers,
    calls::Call,
    fs::Target,
    scope::{Change, View, path::NativePath},
};

/// The file edits of every connection.
pub(crate) struct Service {
    helpers: Helpers,
    outcomes: Outcomes,
    writing: Writing,
    observer: Arc<dyn Observer>,
}

impl Service {
    /// Edits whose filesystem calls run on `helpers` and which `observer` hears of.
    pub(crate) fn new(helpers: Helpers, observer: Arc<dyn Observer>) -> Self {
        Self {
            helpers,
            outcomes: Outcomes::new(),
            writing: Writing::default(),
            observer,
        }
    }

    /// `fs.write` of `content`, the Puts its call staged, under `view`. It waits on the
    /// filesystem, so it runs on a thread of its own.
    pub(crate) fn write(&self, call: Call, view: View, request: &FsWrite, content: Vec<u8>) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        let view = Arc::new(view);
        let mut saved = None;
        self.outcomes.run(workspace, identity, call, |call| {
            let path = &request.path;
            let written = self
                .admit(Operation::FsWrite, &view, path, Change::Content)
                .and_then(|path| {
                    let text = String::from_utf8(content)
                        .map_err(|_| FilesError::new(ErrorCode::InvalidRequest))?;
                    let write = Write {
                        path: path.as_path().to_path_buf(),
                        text,
                        target: Target::new(request.encoding.as_str(), request.bom),
                        precondition: request.precondition,
                        unlock: request.unlock,
                    };
                    let written = self.writing.exclusive(path.key(), || {
                        write::run(&self.helpers, &System, &write, &|| call.cancelled())
                    })?;
                    Ok((path, write, written))
                });
            match written {
                Ok((path, write, written)) => {
                    let reply = MutationReply::FsWrite(written.clone());
                    saved = Some((path, write, written));
                    reply
                }
                Err(error) => failed(error),
            }
        });
        if let (Some(history), Some((path, write, written))) = (request.history, saved) {
            self.observer.saved(&Saved {
                workspace,
                path: &path,
                history,
                written: &written,
                write: &write,
            });
        }
    }

    /// Answers `request.outcome` on `call` when a file mutation ran under its identity, and hands
    /// `call` back otherwise, for the Git mutations to answer.
    pub(crate) fn outcome(&self, call: Call, request: &RequestOutcome) -> Option<Call> {
        match self
            .outcomes
            .outcome(request.workspace_id, request.request_id)
        {
            Some(outcome) => {
                finish(call, &FilesReply::RequestOutcome(outcome));
                None
            }
            None => Some(call),
        }
    }

    /// Admits `path` for a mutation that changes it as `change` says, by the scope's check, on a
    /// helper under its deadline, since the check reads the filesystem.
    fn admit(
        &self,
        operation: Operation,
        view: &Arc<View>,
        path: &FilePath,
        change: Change,
    ) -> Result<NativePath, FilesError> {
        let (view, path) = (Arc::clone(view), path.clone());
        self.helpers
            .run(operation, move || view.mutable(&path, change))?
    }
}

/// The recorded reply of a mutation that failed.
fn failed(error: FilesError) -> MutationReply {
    MutationReply::Error(ErrorReply { error })
}

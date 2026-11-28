//! Local History: a copy of each file the File Bar saves or restores, which follows the file when
//! the File Bar moves or renames it, kept on the machine holding the file ([`store`]). Recording
//! never fails the save or the move that caused it, and nothing it records is logged.
//!
//! The service serves the `history.*` operations and the content `fs.read` reads of an entry, and
//! it is the edits' [`Observer`]: it records each save whose request names a `history` reason, and
//! takes the history of what a rename or a move took elsewhere along, recording each moved file's
//! content at its new path as VS Code does. A path a request names passes the scope's read check
//! under the view its call started with, and `history.restore` its mutation check for the file's
//! content, so once a Workspace closed only the files a connection opened stay reachable. Mutations
//! run under their request identities ([`Outcomes`]). Every store call runs on a helper under its
//! deadline, since the data root is a filesystem too. Without a data root nothing is recorded: a
//! file has no entries and an entry is `not-found`.
//!
//! `history.restore` ([`restore`]) writes an entry's content over its file, then records what it
//! wrote as a restore, as VS Code does. It waits for an edit writing the same path, so the two
//! never interleave.

mod restore;
mod store;

use std::{
    fs::{self, File},
    io::Read as _,
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};

use orchbee_files_protocol::{
    Empty, ErrorCode, ErrorReply, FilesError, FilesReply, HistoryDelete, HistoryEntry,
    HistoryEntryId, HistoryListResult, HistoryRename, HistoryRestore, HistorySource, HistoryTarget,
    List, MutationReply, Operation, PathRequest, RequestOutcome, SafeInteger, Signed, Text,
    WriteHistory,
};
use orchbee_local_runtime::{LogLevel, LogValue, log_event};

use self::store::{Entry, Hold, Limits, Source, Store, StoreError};
use crate::{
    blocking::Helpers,
    calls::Call,
    edit::{self, Moved, Observer, Relocation, Saved},
    entry, error,
    outcomes::{Outcomes, finish},
    scope::{Change, View, path::NativePath},
};

/// The most entries `history.list` answers, as many as a file keeps.
const LISTED: usize = 50;

/// Local History of one data root.
pub(crate) struct Service {
    /// `None` without a data root.
    store: Option<Arc<Store>>,
    helpers: Helpers,
    outcomes: Outcomes,
}

impl Service {
    /// The history kept in `root`, the data root's `internal/local-history/`, whose store calls
    /// run on `helpers`; `None` keeps nothing.
    pub(crate) fn new(helpers: Helpers, root: Option<PathBuf>) -> Self {
        Self {
            store: root.map(|root| Arc::new(Store::new(root, Limits::DEFAULT))),
            helpers,
            outcomes: Outcomes::new(),
        }
    }

    /// `history.list` under `view`. It waits on the store, so it runs on a thread of its own.
    pub(crate) fn list(&self, call: Call, view: &View, request: &PathRequest) {
        let listed = view
            .read(&request.path)
            .and_then(|path| self.entries(&path))
            .and_then(|entries| {
                let skipped = entries.len().saturating_sub(LISTED);
                let entries = entries.into_iter().skip(skipped).filter_map(listed);
                List::new(entries.collect()).map_err(|_| FilesError::new(ErrorCode::Failed))
            });
        let reply = listed.map_or_else(FilesReply::Error, |entries| {
            FilesReply::HistoryList(HistoryListResult { entries })
        });
        finish(call, &reply);
    }

    /// The content entry `entry` of the file at `path` keeps, which `fs.read` reads; scope
    /// admitted `path`.
    ///
    /// # Errors
    ///
    /// `not-found` for an entry the file does not have, and the store's failure.
    pub(crate) fn content(
        &self,
        path: &NativePath,
        entry: HistoryEntryId,
    ) -> Result<Vec<u8>, FilesError> {
        let Some(store) = &self.store else {
            return Err(FilesError::new(ErrorCode::NotFound));
        };
        let (store, path, id) = (Arc::clone(store), path.clone(), entry.to_string());
        self.helpers
            .run(Operation::FsRead, move || {
                store.read(|hold| hold.content(&path, &id))
            })?
            .map_err(failure)?
            .ok_or_else(|| FilesError::new(ErrorCode::NotFound))
    }

    /// `history.restore` under `view`, while no edit writes the same path, as `edit` keeps them.
    /// It waits on the filesystem, so it runs on a thread of its own.
    pub(crate) fn restore(
        &self,
        call: Call,
        view: View,
        request: &HistoryRestore,
        edit: &edit::Service,
    ) {
        let view = Arc::new(view);
        let mut restored = None;
        self.outcomes
            .run(request.workspace_id, request.request_id, call, |call| {
                let written = self.admit(&view, request).and_then(|path| {
                    let content = self.content(&path, request.entry)?;
                    if call.cancelled() {
                        return Err(FilesError::new(ErrorCode::Cancelled));
                    }
                    let timestamp = edit.exclusive(&path, || {
                        restore::write(&self.helpers, &path, content.clone())
                    })?;
                    Ok((path, content, timestamp))
                });
                match written {
                    Ok(written) => {
                        restored = Some(written);
                        MutationReply::Empty(Empty {})
                    }
                    Err(error) => failed(error),
                }
            });
        if let Some((path, content, timestamp)) = restored {
            self.record(
                Operation::HistoryRestore,
                path,
                Source::Restore,
                content,
                timestamp,
            );
        }
    }

    /// `history.rename` under `view`. It waits on the store, so it runs on a thread of its own.
    pub(crate) fn rename(&self, call: Call, view: &View, request: &HistoryRename) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        self.outcomes.run(workspace, identity, call, |_| {
            let renamed = view.read(&request.path).and_then(|path| {
                let (id, label) = (request.entry.to_string(), request.label.as_str().to_owned());
                self.change(Operation::HistoryRename, move |hold| {
                    hold.rename(&path, &id, label)
                })?
                .ok_or_else(|| FilesError::new(ErrorCode::NotFound))
            });
            done(renamed)
        });
    }

    /// `history.delete` under `view`. It waits on the store, so it runs on a thread of its own.
    pub(crate) fn delete(&self, call: Call, view: &View, request: &HistoryDelete) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        self.outcomes.run(workspace, identity, call, |_| {
            let deleted = match &request.target {
                HistoryTarget::Entries { path, entries } => view.read(path).and_then(|path| {
                    let ids: Vec<String> =
                        entries.as_slice().iter().map(ToString::to_string).collect();
                    self.change(Operation::HistoryDelete, move |hold| {
                        hold.delete(&path, &ids)
                    })
                }),
                HistoryTarget::All => {
                    self.change(Operation::HistoryDelete, |hold| hold.delete_all())
                }
            };
            // Without a store there is nothing to delete.
            done(deleted.map(drop))
        });
    }

    /// Answers `request.outcome` on `call` when a Local History mutation ran under its identity,
    /// and hands `call` back otherwise, for the Git mutations to answer.
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

    /// The entries of the file at `path`, oldest first.
    fn entries(&self, path: &NativePath) -> Result<Vec<Entry>, FilesError> {
        let Some(store) = &self.store else {
            return Ok(Vec::new());
        };
        let (store, path) = (Arc::clone(store), path.clone());
        self.helpers
            .run(Operation::HistoryList, move || {
                store.read(|hold| hold.entries(&path))
            })?
            .map(Option::unwrap_or_default)
            .map_err(failure)
    }

    /// Runs `change` on the store as it is, on a helper; `None` without a store.
    fn change(
        &self,
        operation: Operation,
        change: impl FnOnce(&mut Hold<'_>) -> Result<(), StoreError> + Send + 'static,
    ) -> Result<Option<()>, FilesError> {
        let Some(store) = &self.store else {
            return Ok(None);
        };
        let store = Arc::clone(store);
        self.helpers
            .run(operation, move || store.change(change))?
            .map_err(failure)
    }

    /// Admits the file a restore writes by the scope's check for its content, on a helper, since
    /// the check reads the filesystem.
    fn admit(&self, view: &Arc<View>, request: &HistoryRestore) -> Result<NativePath, FilesError> {
        let (view, path) = (Arc::clone(view), request.path.clone());
        self.helpers.run(Operation::HistoryRestore, move || {
            view.mutable(&path, Change::Content)
        })?
    }

    /// Records `content`, the file at `path` once `source` wrote it, at `timestamp`; a failure
    /// is logged without the path.
    fn record(
        &self,
        operation: Operation,
        path: NativePath,
        source: Source,
        content: Vec<u8>,
        timestamp: i64,
    ) {
        let Some(store) = &self.store else {
            return;
        };
        let store = Arc::clone(store);
        let recorded = self.helpers.run(operation, move || {
            store.write(|hold| hold.record(&path, source, &content, timestamp))
        });
        report(operation, recorded);
    }
}

impl Observer for Service {
    fn saved(&self, saved: &Saved<'_>) {
        let Some(store) = &self.store else {
            return;
        };
        // A larger file records nothing, so its content is never encoded again for the record.
        if saved.written.size.get() > store.content_limit() {
            return;
        }
        let source = match saved.history {
            WriteHistory::Save => Source::Save,
            WriteHistory::Restore => Source::Restore,
        };
        self.record(
            Operation::FsWrite,
            saved.path.clone(),
            source,
            saved.content(),
            saved.written.mtime_ms.get(),
        );
    }

    fn moved(&self, moved: &Moved<'_>) {
        let Some(store) = &self.store else {
            return;
        };
        let operation = match moved.relocation {
            Relocation::Rename => Operation::FsRename,
            Relocation::Move => Operation::FsMove,
        };
        let (store, from, to) = (Arc::clone(store), moved.from.clone(), moved.to.clone());
        let now = entry::milliseconds(Ok(SystemTime::now())).get();
        let relocated = self.helpers.run(operation, move || {
            let limit = store.content_limit();
            store.write(|hold| hold.relocate(&from, &to, now, &mut |path| current(path, limit)))
        });
        report(operation, relocated);
    }
}

/// The content of the file at `path` when it is a file of at most `limit` bytes.
fn current(path: &NativePath, limit: u64) -> Option<Vec<u8>> {
    // Anything but a file, such as a pipe, could block a read.
    if !fs::metadata(path.as_path()).ok()?.is_file() {
        return None;
    }
    let mut content = Vec::new();
    File::open(path.as_path())
        .ok()?
        .take(limit.saturating_add(1))
        .read_to_end(&mut content)
        .ok()?;
    (u64::try_from(content.len()).ok()? <= limit).then_some(content)
}

/// An entry as `history.list` answers it; the store admits only values the protocol carries.
fn listed(entry: Entry) -> Option<HistoryEntry> {
    Some(HistoryEntry {
        id: HistoryEntryId::parse(&entry.id).ok()?,
        source: match entry.source {
            Source::Save => HistorySource::Save,
            Source::Restore => HistorySource::Restore,
            Source::Move => HistorySource::Move,
            Source::Rename => HistorySource::Rename,
        },
        label: entry.label.map(Text::new).transpose().ok()?,
        timestamp: Signed::new(entry.timestamp).ok()?,
        size: SafeInteger::new(entry.size).ok()?,
    })
}

/// The File Bar error of a store's failure. A history a newer Orchbee wrote is one this build
/// leaves alone, which fails the request.
fn failure(error: StoreError) -> FilesError {
    match error {
        StoreError::Busy => FilesError::new(ErrorCode::Busy),
        StoreError::Newer => FilesError::new(ErrorCode::Failed),
        StoreError::NotFound => FilesError::new(ErrorCode::NotFound),
        StoreError::Io(error) => error::from_io(&error),
    }
}

/// The recorded reply of a mutation that answers only its success.
fn done(result: Result<(), FilesError>) -> MutationReply {
    result.map_or_else(failed, |()| MutationReply::Empty(Empty {}))
}

/// The recorded reply of a mutation that failed.
fn failed(error: FilesError) -> MutationReply {
    MutationReply::Error(ErrorReply { error })
}

/// Logs a record or a move of history that failed, by the operation that caused it and the
/// failure's kind; a helper that passed its deadline logged its own record.
fn report(operation: Operation, outcome: Result<Result<(), StoreError>, FilesError>) {
    let kind = match outcome {
        Ok(Ok(())) | Err(_) => return,
        Ok(Err(StoreError::Busy)) => LogValue::Token("busy"),
        Ok(Err(StoreError::Newer)) => LogValue::Token("newer"),
        Ok(Err(StoreError::NotFound)) => LogValue::Token("not-found"),
        Ok(Err(StoreError::Io(error))) => LogValue::from(error.kind()),
    };
    log_event(
        LogLevel::Warning,
        "history-not-recorded",
        &[
            ("operation", LogValue::Token(operation.name())),
            ("error", kind),
        ],
    );
}

#[cfg(test)]
mod tests;

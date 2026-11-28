//! File edits, the operations `files-edit` adds. Each mutation runs under its request identity
//! ([`Outcomes`]), on a thread where waiting is allowed. Each path it changes passes the scope's
//! one mutation check, [`View::mutable`], under the view its call started with, when the change
//! starts. A mutation never logs a path, a name or file content, and tells the [`Observer`] its
//! caller injected what it changed.
//!
//! - `fs.write` ([`write`]) takes the content its Puts staged and rewrites the file in place.
//! - `fs.create`, `fs.rename` and `fs.exists` ([`entries`]) handle one item, under the names this
//!   machine holds ([`names`]).
//! - `fs.move` and `fs.copy` ([`transfer`]) take each source into a folder.
//! - `fs.delete` and `fs.restore` ([`delete`]) remove items, to the trash or permanently, and
//!   recreate what a delete kept.
//! - `picker.*` ([`pickers`]) browse the whole machine for a Save As or clone dialog, and
//!   `fs.clipboard` ([`clipboard`]) keeps what a paste may take from beyond the root.
//! - `fs.export-begin` and `fs.export-entries` ([`exports`]) number what a download takes, whose
//!   files `fs.read` then reads.
//! - `fs.import-begin`, `fs.import-items`, `fs.import-file` and `fs.import-end` ([`import`])
//!   bring the Client's own files in, each staged beside its target and published whole.

mod clipboard;
mod delete;
mod entries;
mod exports;
mod import;
mod names;
mod observer;
mod pickers;
mod transfer;
mod write;

use std::sync::Arc;

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, ErrorReply, FilePath, FilesError, FilesReply, FsClipboard, FsCreate,
    FsDelete, FsDeleteResult, FsEntryResult, FsExists, FsExistsResult, FsExportBegin,
    FsExportEntries, FsImportBegin, FsImportEnd, FsImportFile, FsImportFileResult, FsImportItems,
    FsImportResult, FsRename, FsRestore, FsTransfer, FsTransferResult, FsWrite, List,
    MutationReply, Operation, PathItemsResult, PickerList, PickerRequest, RequestOutcome,
    SessionId, WorkspaceId,
};

use self::{
    clipboard::Clipboards,
    delete::{Deleting, Stashes},
    exports::Exports,
    import::{Bringing, Declaration, Imports},
    pickers::Pickers,
    write::{System, Write, Writing},
};
pub(crate) use self::{
    observer::{Moved, Observer, Relocation, Saved},
    transfer::Transfer,
};
use crate::{
    blocking::Helpers,
    calls::Call,
    fs::{Streaming, Target, Unopened},
    git::ports::Trash,
    outcomes::{Outcomes, finish},
    scope::{Change, View, path::NativePath},
};

/// The file edits of every connection.
pub(crate) struct Service {
    helpers: Helpers,
    outcomes: Outcomes,
    writing: Writing,
    stashes: Stashes,
    trash: Arc<dyn Trash>,
    pickers: Pickers,
    clipboard: Clipboards,
    exports: Exports,
    imports: Imports,
    observer: Arc<dyn Observer>,
}

impl Service {
    /// Edits whose filesystem calls run on `helpers`, which delete to `trash`, and which
    /// `observer` hears of.
    pub(crate) fn new(
        helpers: Helpers,
        trash: Arc<dyn Trash>,
        observer: Arc<dyn Observer>,
    ) -> Self {
        Self {
            helpers,
            outcomes: Outcomes::new(),
            writing: Writing::default(),
            stashes: Stashes::new(),
            trash,
            pickers: Pickers::new(),
            clipboard: Clipboards::new(),
            exports: Exports::new(),
            imports: Imports::new(),
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
                path: &path,
                history,
                written: &written,
                write: &write,
            });
        }
    }

    /// `fs.create` under `view`.
    pub(crate) fn create(&self, call: Call, view: View, request: &FsCreate) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        let view = Arc::new(view);
        self.outcomes.run(workspace, identity, call, |call| {
            let path = &request.path;
            let created = names::created(path)
                .and_then(|_| self.admit(Operation::FsCreate, &view, path, Change::Entry))
                .and_then(|path| {
                    unless_cancelled(call)?;
                    entries::create(&self.helpers, &path, request.kind)
                });
            created.map_or_else(failed, |entry| {
                MutationReply::FsEntry(FsEntryResult { entry })
            })
        });
    }

    /// `fs.rename` under `view`.
    pub(crate) fn rename(&self, call: Call, view: View, request: &FsRename) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        let view = Arc::new(view);
        let mut moved = None;
        self.outcomes.run(workspace, identity, call, |call| {
            let operation = Operation::FsRename;
            let renamed = self
                .admit(operation, &view, &request.from, Change::Entry)
                .and_then(|from| {
                    names::created(&request.to)?;
                    let to = self.admit(operation, &view, &request.to, Change::Entry)?;
                    unless_cancelled(call)?;
                    let entry = entries::rename(&self.helpers, &from, &to)?;
                    Ok((from, to, entry))
                });
            match renamed {
                Ok((from, to, entry)) => {
                    moved = Some((from, to));
                    MutationReply::FsEntry(FsEntryResult { entry })
                }
                Err(error) => failed(error),
            }
        });
        if let Some((from, to)) = moved {
            self.moved(&from, &to, Relocation::Rename);
        }
    }

    /// `fs.move` or `fs.copy`, as `transfer` says, under `view`. A move changes each source's
    /// entry; a copy reads its sources as any read does.
    pub(crate) fn transfer(
        &self,
        call: Call,
        view: View,
        request: &FsTransfer,
        transfer: Transfer,
    ) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        let (view, operation) = (Arc::new(view), transfer.operation());
        let mut done = Vec::new();
        self.outcomes.run(workspace, identity, call, |call| {
            let directory = NativePath::parse(request.target_directory.as_str());
            let admit = |requested: &FilePath| {
                let directory = directory.clone()?;
                // A source on the connection's clipboard needs no place in the current boundary.
                let connection = call.connection();
                let source = match transfer {
                    Transfer::Move => self
                        .admit(operation, &view, requested, Change::Entry)
                        .or_else(|refused| {
                            self.clipboard.cut(connection, requested).ok_or(refused)
                        }),
                    Transfer::Copy => view.read(requested).or_else(|refused| {
                        self.clipboard.copied(connection, requested).ok_or(refused)
                    }),
                }?;
                let target = names::child(&directory, source.name())?;
                let named = FilePath::new(target.as_str())
                    .map_err(|_| FilesError::new(ErrorCode::InvalidName))?;
                self.admit(operation, &view, &named, Change::Entry)?;
                Ok((source, target))
            };
            let sources = request.sources.as_slice();
            let cancelled = || call.cancelled();
            let transferred = transfer::run(
                &self.helpers,
                transfer,
                sources,
                &admit,
                request.conflict,
                &cancelled,
            );
            let items = transferred
                .into_iter()
                .map(|transferred| {
                    done.extend(transferred.done);
                    transferred.item
                })
                .collect();
            List::new(items).map_or_else(
                |_| failed(FilesError::new(ErrorCode::Failed)),
                |items| MutationReply::FsTransfer(FsTransferResult { items }),
            )
        });
        if transfer == Transfer::Move {
            for (from, to) in &done {
                self.moved(from, to, Relocation::Move);
            }
        }
    }

    /// `fs.delete` under `view`.
    pub(crate) fn delete(&self, call: Call, view: View, request: &FsDelete) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        let view = Arc::new(view);
        self.outcomes.run(workspace, identity, call, |call| {
            let admit =
                |path: &FilePath| self.admit(Operation::FsDelete, &view, path, Change::Entry);
            let cancelled = || call.cancelled();
            let deleting = Deleting {
                helpers: &self.helpers,
                trash: self.trash.as_ref(),
                mode: request.mode,
                capture: request.capture,
                cancelled: &cancelled,
            };
            let (items, kept) = deleting.run(request.paths.as_slice(), &admit);
            let stash = self.stashes.keep(workspace, kept);
            List::new(items).map_or_else(
                |_| failed(FilesError::new(ErrorCode::Failed)),
                |items| match stash {
                    Some(stash) => MutationReply::FsDelete(FsDeleteResult {
                        items,
                        stash: Some(stash),
                    }),
                    None => MutationReply::PathItems(PathItemsResult { items }),
                },
            )
        });
    }

    /// `fs.restore` under `view`: takes its stash and recreates what the delete kept.
    pub(crate) fn restore(&self, call: Call, view: View, request: &FsRestore) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        let view = Arc::new(view);
        self.outcomes.run(workspace, identity, call, |_| {
            let Some(kept) = self.stashes.take(workspace, request.stash) else {
                return failed(FilesError::new(ErrorCode::UndoExpired));
            };
            let admit =
                |path: &FilePath| self.admit(Operation::FsRestore, &view, path, Change::Entry);
            let items = delete::restore(&self.helpers, kept, &admit);
            List::new(items).map_or_else(
                |_| failed(FilesError::new(ErrorCode::Failed)),
                |items| MutationReply::PathItems(PathItemsResult { items }),
            )
        });
    }

    /// `fs.clipboard` under `view`: each path must lie in its boundary as a copy reads it, or for
    /// cut items as a move changes it.
    pub(crate) fn clipboard(&self, call: Call, view: View, request: &FsClipboard) {
        let view = Arc::new(view);
        let admitted: Result<Vec<_>, _> = request
            .paths
            .as_slice()
            .iter()
            .map(|path| {
                if request.cut {
                    self.admit(Operation::FsClipboard, &view, path, Change::Entry)
                } else {
                    view.read(path)
                }
            })
            .collect();
        let reply = admitted.map_or_else(FilesReply::Error, |paths| {
            self.clipboard
                .replace(call.connection(), paths, request.cut);
            FilesReply::Empty
        });
        finish(call, &reply);
    }

    /// `path` for a read when it is one of the call's connection's clipboard sources.
    pub(crate) fn clipboard_source(&self, call: &Call, path: &FilePath) -> Option<NativePath> {
        self.clipboard.copied(call.connection(), path)
    }

    /// `picker.open`: opens the picker at once, so its next requests may follow before it
    /// answers.
    ///
    /// # Errors
    ///
    /// As the pickers refuse a name.
    pub(crate) fn open_picker(
        &self,
        call: &Call,
        request: &PickerRequest,
    ) -> Result<(), FilesError> {
        let (connection, workspace) = (call.connection(), request.workspace_id);
        self.pickers.open(connection, workspace, request.picker)
    }

    /// Answers `picker.open` with where the picker starts, which it reads, so it runs on the
    /// call's own thread; a picker with nowhere to start closes.
    pub(crate) fn picker_places(&self, call: Call, request: &PickerRequest) {
        let places = pickers::places(&self.helpers);
        if places.is_err() {
            self.pickers.close(call.connection(), request.picker);
        }
        finish(
            call,
            &places.map_or_else(FilesReply::Error, FilesReply::PickerOpen),
        );
    }

    /// The directory `picker.list` pages, which the open picker admits wherever it is.
    ///
    /// # Errors
    ///
    /// As [`Self::picked`].
    pub(crate) fn picker_folder(
        &self,
        call: &Call,
        request: &PickerList,
    ) -> Result<NativePath, FilesError> {
        let (workspace, picker) = (request.workspace_id, request.picker);
        self.picked(call.connection(), workspace, picker, &request.path)
    }

    /// `picker.close`.
    pub(crate) fn close_picker(&self, call: Call, request: &PickerRequest) {
        self.pickers.close(call.connection(), request.picker);
        call.reply(&FilesReply::Empty);
    }

    /// The picker grant: `path`, anywhere on the machine, for what the person chose through
    /// `picker`, which `connection` must hold open in `workspace`. Each use keeps the picker open
    /// for ten more minutes. `picker.list` and any target chosen in a picker pass it.
    ///
    /// # Errors
    ///
    /// `invalid-request` when the picker is not open there, and the error of a form this machine
    /// does not admit.
    pub(crate) fn picked(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        picker: SessionId,
        path: &FilePath,
    ) -> Result<NativePath, FilesError> {
        if self.pickers.holds(connection, workspace, picker) {
            NativePath::parse(path.as_str())
        } else {
            Err(FilesError::new(ErrorCode::InvalidRequest))
        }
    }

    /// `fs.export-begin` under `view`, which captures the selection at once, so the export's
    /// pages may be asked for before it answers. Each path must lie in the boundary as a read
    /// admits it.
    pub(crate) fn export_begin(&self, call: Call, view: View, request: &FsExportBegin) {
        let paths = request.paths.as_slice().iter();
        let admitted: Result<Vec<_>, _> = paths.map(|path| view.read(path)).collect();
        let begun = admitted.and_then(|paths| {
            let (connection, workspace) = (call.connection(), request.workspace_id);
            self.exports
                .begin(connection, workspace, view, request.export, paths)
        });
        finish(
            call,
            &begun.map_or_else(FilesReply::Error, |()| FilesReply::Empty),
        );
    }

    /// `fs.export-entries`, which reads the filesystem, so it runs on the call's own thread.
    pub(crate) fn export_entries(&self, call: Call, request: &FsExportEntries) {
        let (connection, workspace) = (call.connection(), request.workspace_id);
        let page = self.exports.page(
            &self.helpers,
            connection,
            workspace,
            request.export,
            request.cursor,
        );
        finish(
            call,
            &page.map_or_else(FilesReply::Error, FilesReply::FsExportEntries),
        );
    }

    /// The file entry `index` of `export`, which `fs.read` of that entry reads.
    ///
    /// # Errors
    ///
    /// `invalid-request` for an export the call's connection does not hold open in `workspace`,
    /// or a number that names none of its files.
    pub(crate) fn exported(
        &self,
        call: &Call,
        workspace: WorkspaceId,
        export: SessionId,
        index: u64,
    ) -> Result<NativePath, FilesError> {
        self.exports
            .file(call.connection(), workspace, export, index)
    }

    /// Opens `fs.import-begin`'s import under `view` and declares its first items at once, so the
    /// import's later requests may follow before it answers. The target folder must lie in the
    /// boundary as a read admits it; each item passes the mutation check as it is created.
    ///
    /// # Errors
    ///
    /// The target's refusal, and as the import's opening.
    pub(crate) fn open_import(
        &self,
        call: &Call,
        view: &View,
        request: &FsImportBegin,
    ) -> Result<Declaration, FilesError> {
        let target = view.read(&request.target_directory)?;
        let (connection, workspace) = (call.connection(), request.workspace_id);
        let items = request.items.as_slice();
        self.imports
            .open(connection, request.import, workspace, target, items)
    }

    /// `fs.import-begin` under `view`, once [`Self::open_import`] opened it: under its request
    /// identity, it checks that the target is a folder and creates the declared directory items.
    /// An identity that already ran answers its outcome and closes the import it opened.
    pub(crate) fn import_begin(
        &self,
        call: Call,
        view: View,
        request: &FsImportBegin,
        declared: Declaration,
    ) {
        let (workspace, identity) = (request.workspace_id, request.request_id);
        let view = Arc::new(view);
        self.outcomes.run(workspace, identity, call, move |_| {
            if let Err(error) = declared.target_folder(&self.helpers) {
                return failed(error);
            }
            let operation = Operation::FsImportBegin;
            let admit = |target: &NativePath| self.admit_entry(operation, &view, target);
            let items = declared.settle(&self.helpers, operation, &admit);
            List::new(items).map_or_else(
                |_| failed(FilesError::new(ErrorCode::Failed)),
                |items| MutationReply::FsImport(FsImportResult { items }),
            )
        });
    }

    /// Declares `fs.import-items`' items at once, so the import's later requests may follow
    /// before it answers.
    ///
    /// # Errors
    ///
    /// `invalid-request` for an import the call's connection does not hold open in the
    /// Workspace, and as the import's declaration.
    pub(crate) fn declare_import(
        &self,
        call: &Call,
        request: &FsImportItems,
    ) -> Result<Declaration, FilesError> {
        let (connection, workspace) = (call.connection(), request.workspace_id);
        let import = self.imports.find(connection, workspace, request.import);
        let import = import.ok_or_else(|| FilesError::new(ErrorCode::InvalidRequest))?;
        import.declare(request.items.as_slice())
    }

    /// `fs.import-items` under `view`, once [`Self::declare_import`] declared them: creates the
    /// declared directory items.
    pub(crate) fn import_items(&self, call: Call, view: View, declared: Declaration) {
        let (view, operation) = (Arc::new(view), Operation::FsImportItems);
        let admit = |target: &NativePath| self.admit_entry(operation, &view, target);
        let items = declared.settle(&self.helpers, operation, &admit);
        let reply = List::new(items).map_or_else(
            |_| FilesReply::Error(FilesError::new(ErrorCode::Failed)),
            |items| FilesReply::FsImport(FsImportResult { items }),
        );
        finish(call, &reply);
    }

    /// Opens `fs.import-file`'s file item at once, with the stream `stream` opens for its size, so
    /// its Puts may follow before it answers.
    ///
    /// # Errors
    ///
    /// `invalid-request` for an import the call's connection does not hold open in the Workspace,
    /// and as the import's file opening.
    pub(crate) fn open_import_file(
        &self,
        call: &Call,
        request: &FsImportFile,
        stream: impl FnOnce(u64) -> Result<Streaming, Unopened>,
    ) -> Result<Bringing, Unopened> {
        let (connection, workspace) = (call.connection(), request.workspace_id);
        let import = self.imports.find(connection, workspace, request.import);
        let refused = || Unopened::Failed(FilesError::new(ErrorCode::InvalidRequest));
        import
            .ok_or_else(refused)?
            .open_file(request.index.get(), stream)
    }

    /// `fs.import-file` under `view`, once [`Self::open_import_file`] opened it: brings the file's
    /// bytes in. A call its Client cancelled closes as cancelled; what was published stays.
    pub(crate) fn import_file(&self, call: Call, view: View, bringing: Bringing) {
        let (view, operation) = (Arc::new(view), Operation::FsImportFile);
        let admit = |target: &NativePath| self.admit_entry(operation, &view, target);
        let brought = bringing.bring(&self.helpers, &admit);
        if call.cancelled() {
            // Dropping a cancelled call closes it as cancelled.
            return drop(call);
        }
        let reply = brought.map_or_else(FilesReply::Error, |path| {
            FilesReply::FsImportFile(FsImportFileResult { path })
        });
        finish(call, &reply);
    }

    /// `fs.import-end`, which closes the import; the files it is bringing go on.
    pub(crate) fn end_import(&self, call: Call, request: &FsImportEnd) {
        let (connection, workspace) = (call.connection(), request.workspace_id);
        self.imports.close(connection, workspace, request.import);
        finish(call, &FilesReply::Empty);
    }

    /// Releases what `connection` held: its pickers, clipboard sources, exports and imports.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.pickers.connection_lost(connection);
        self.clipboard.connection_lost(connection);
        self.exports.connection_lost(connection);
        self.imports.connection_lost(connection);
    }

    /// `fs.exists` under `view`, whose names are read as any read is.
    pub(crate) fn exists(&self, call: Call, view: &View, request: &FsExists) {
        let existing = view.read(&request.directory).and_then(|directory| {
            let names = request.names.as_slice().to_vec();
            entries::exists(&self.helpers, &directory, names)
        });
        let reply = existing
            .and_then(|existing| {
                List::new(existing).map_err(|_| FilesError::new(ErrorCode::Failed))
            })
            .map_or_else(FilesReply::Error, |existing| {
                FilesReply::FsExists(FsExistsResult { existing })
            });
        finish(call, &reply);
    }

    /// Runs `work` while no edit writes the file at `path`, as Local History's restore writes it.
    pub(crate) fn exclusive<T>(&self, path: &NativePath, work: impl FnOnce() -> T) -> T {
        self.writing.exclusive(path.key(), work)
    }

    /// Answers `request.outcome` on `call` when a file mutation ran under its identity, and hands
    /// `call` back otherwise, for another service to answer.
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

    /// Tells the observer that an item went from `from` to `to`, unless it stayed where it was.
    fn moved(&self, from: &NativePath, to: &NativePath, how: Relocation) {
        if from.as_str() != to.as_str() {
            self.observer.moved(&Moved {
                from,
                to,
                relocation: how,
            });
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

    /// Admits `target`, where a new entry goes, by the scope's check for [`Change::Entry`].
    fn admit_entry(
        &self,
        operation: Operation,
        view: &Arc<View>,
        target: &NativePath,
    ) -> Result<(), FilesError> {
        let named =
            FilePath::new(target.as_str()).map_err(|_| FilesError::new(ErrorCode::InvalidName))?;
        self.admit(operation, view, &named, Change::Entry).map(drop)
    }
}

/// The recorded reply of a mutation that failed.
fn failed(error: FilesError) -> MutationReply {
    MutationReply::Error(ErrorReply { error })
}

/// `cancelled` when `call` was cancelled before its mutation changed anything.
fn unless_cancelled(call: &Call) -> Result<(), FilesError> {
    if call.cancelled() {
        Err(FilesError::new(ErrorCode::Cancelled))
    } else {
        Ok(())
    }
}

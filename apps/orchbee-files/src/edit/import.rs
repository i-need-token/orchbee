//! Imports, which bring the Client's own files in: Web upload, Desktop drag-in from the operating
//! system and Desktop paste of its files, so no path outside the boundary is ever granted.
//! `fs.import-begin` opens an import into a folder under the session the Client chose and declares
//! its first items, `fs.import-items` declares more, `fs.import-file` brings one file item's bytes
//! as Puts, and `fs.import-end` or the connection's end closes the import, while the files it is
//! bringing go on. Items are numbered from 0 across the import in the order they arrive; an item
//! of several names lies in a directory item declared earlier, and so in that directory's final
//! path.
//!
//! - Declarations take effect in the order they arrive, and a file waits for those that arrived
//!   before it. A directory item is created when its declaration takes effect, once
//!   `View::mutable` admits it with `Change::Entry`, and its outcome answers the request that
//!   declared it; an item in a directory item that failed fails as it did.
//! - A file is staged under a [staging name](crate::staging) beside its target, once
//!   `View::mutable` admits the target with `Change::Entry`, as its Puts arrive, and published
//!   without replacement once complete: hard-linked to its final name, then the staging name
//!   removed; where the hard link fails it claims the name with an empty file and renames the
//!   staging file onto the claim. A failure, `transfer.close`, the end of the file's lane or
//!   connection, or 60 s without a Put removes the staging file; published items stay.
//! - Each item's conflict rule decides at an existing item: `fail` answers `exists`, `replace`
//!   removes it permanently first, `increment` takes VS Code's next free copy name, and `merge`
//!   keeps an existing directory for a directory item and otherwise replaces.
//! - An import holds at most 100,000 items, and a connection at most 16 open imports. A file's
//!   writes run on its own thread rather than one helper call per Put, so a volume that stops
//!   answering holds that file until its Client gives up on it.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs::{self, File},
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
};

use orchbee_files_protocol::{
    Conflict, ConnectionId, ErrorCode, FilesError, FilesReply, ImportItem, ImportKind,
    ImportOutcome, Operation, RelativePath, SafeInteger, SessionId, Token, WorkspaceId,
};

use super::{entries, names};
use crate::{
    blocking::Helpers,
    calls::Call,
    error,
    fs::{Delivery, Streaming, Unopened},
    scope::path::NativePath,
    staging,
};

/// Items one import declares.
const MAXIMUM_ITEMS: usize = 100_000;
/// Imports one connection holds open.
const MAXIMUM_IMPORTS: usize = 16;
/// Names `increment` tries before it answers `exists`.
const MAXIMUM_ATTEMPTS: usize = 10_000;

/// Admits an item's path for its change, or refuses it.
pub(super) type Admit<'a> = dyn Fn(&NativePath) -> Result<(), FilesError> + 'a;

/// The open imports of every connection.
pub(super) struct Imports {
    open: Mutex<HashMap<(ConnectionId, SessionId), Arc<Import>>>,
}

/// One import. Its lock is taken after the open imports' lock, never before it.
pub(super) struct Import {
    workspace: WorkspaceId,
    /// The folder the items go into.
    target: NativePath,
    state: Mutex<State>,
    /// Signals each declaration that took effect.
    turn: Condvar,
}

struct State {
    items: Vec<Item>,
    /// Every declared path.
    paths: HashSet<String>,
    /// The directory items by declared path.
    directories: HashMap<String, usize>,
    /// Declarations that arrived; each one's turn is the count before it.
    issued: u64,
    /// The turns taken in order.
    served: u64,
    /// Turns taken out of order, which `served` passes once it reaches them.
    taken: BTreeSet<u64>,
    closed: bool,
}

enum Item {
    Directory {
        declared: RelativePath,
        conflict: Conflict,
        /// Its final path below the target folder, or why it failed; `None` until its
        /// declaration takes effect.
        outcome: Option<Result<String, FilesError>>,
    },
    File {
        declared: RelativePath,
        size: u64,
        conflict: Conflict,
        opened: bool,
    },
}

/// The items one request declared, with its turn. Settling it takes the turn; dropping it unsettled
/// takes the turn with its directory items failed, and closes the import it would have opened.
pub(crate) struct Declaration {
    import: Arc<Import>,
    first: usize,
    count: usize,
    /// `None` once settled.
    ticket: Option<u64>,
    opens: bool,
}

/// A file item an `fs.import-file` opened, with the stream its Puts bring.
pub(crate) struct Bringing {
    import: Arc<Import>,
    index: usize,
    /// The declarations that arrived before it.
    after: u64,
    streaming: Streaming,
}

impl Imports {
    pub(super) fn new() -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
        }
    }

    /// Opens `import` for `connection` in `workspace` into `target`, which the boundary admitted,
    /// and declares `items`.
    ///
    /// # Errors
    ///
    /// `invalid-request` when the connection holds an open import under that name, `unavailable`
    /// with reason `too-many-imports` when it holds as many as it may, and as [`Import::declare`];
    /// the import stays unopened then.
    pub(super) fn open(
        &self,
        connection: ConnectionId,
        import: SessionId,
        workspace: WorkspaceId,
        target: NativePath,
        items: &[ImportItem],
    ) -> Result<Declaration, FilesError> {
        let mut open = self.lock();
        open.retain(|_, held| !held.lock().closed);
        if open.contains_key(&(connection, import)) {
            return Err(FilesError::new(ErrorCode::InvalidRequest));
        }
        if open.keys().filter(|key| key.0 == connection).count() >= MAXIMUM_IMPORTS {
            let reason = Token::new("too-many-imports").expect("the reason is a token");
            return Err(FilesError::unavailable(reason));
        }
        let opened = Arc::new(Import {
            workspace,
            target,
            state: Mutex::new(State {
                items: Vec::new(),
                paths: HashSet::new(),
                directories: HashMap::new(),
                issued: 0,
                served: 0,
                taken: BTreeSet::new(),
                closed: false,
            }),
            turn: Condvar::new(),
        });
        let mut declaration = opened.declare(items)?;
        declaration.opens = true;
        open.insert((connection, import), opened);
        Ok(declaration)
    }

    /// `connection`'s open `import` in `workspace`.
    pub(super) fn find(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        import: SessionId,
    ) -> Option<Arc<Import>> {
        self.lock()
            .get(&(connection, import))
            .filter(|open| open.workspace == workspace && !open.lock().closed)
            .cloned()
    }

    /// Closes `connection`'s `import` in `workspace`; one that is not open there is closed
    /// already.
    pub(super) fn close(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        import: SessionId,
    ) {
        let key = (connection, import);
        let mut open = self.lock();
        let held = open
            .get(&key)
            .is_some_and(|open| open.workspace == workspace);
        let closed = if held { open.remove(&key) } else { None };
        drop(open);
        if let Some(closed) = closed {
            closed.lock().closed = true;
        }
    }

    /// Closes every import of `connection`.
    pub(super) fn connection_lost(&self, connection: ConnectionId) {
        let mut open = self.lock();
        let ended: Vec<_> = open.extract_if(|key, _| key.0 == connection).collect();
        drop(open);
        for (_, import) in ended {
            import.lock().closed = true;
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<(ConnectionId, SessionId), Arc<Import>>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Import {
    /// Numbers `items` after the import's earlier items and takes the next turn.
    ///
    /// # Errors
    ///
    /// `invalid-request` for an import that closed, a path declared twice, or an item of several
    /// names outside a directory item declared earlier; `too-large` past the items an import
    /// holds. Nothing is declared then.
    pub(super) fn declare(
        self: &Arc<Self>,
        items: &[ImportItem],
    ) -> Result<Declaration, FilesError> {
        let invalid = || FilesError::new(ErrorCode::InvalidRequest);
        let mut state = self.lock();
        let first = state.items.len();
        if state.closed {
            return Err(invalid());
        }
        if first + items.len() > MAXIMUM_ITEMS {
            return Err(FilesError::new(ErrorCode::TooLarge));
        }
        let (mut fresh, mut folders) = (HashSet::new(), HashSet::new());
        for item in items {
            let declared = item.path.as_str();
            let placed = parent(declared).is_none_or(|folder| {
                state.directories.contains_key(folder) || folders.contains(folder)
            });
            if state.paths.contains(declared) || !fresh.insert(declared) || !placed {
                return Err(invalid());
            }
            if item.kind == ImportKind::Directory {
                folders.insert(declared);
            }
        }
        for item in items {
            let (declared, conflict) = (item.path.clone(), item.conflict);
            let index = state.items.len();
            state.paths.insert(declared.as_str().to_owned());
            let item = match item.kind {
                ImportKind::Directory => {
                    state
                        .directories
                        .insert(declared.as_str().to_owned(), index);
                    Item::Directory {
                        declared,
                        conflict,
                        outcome: None,
                    }
                }
                ImportKind::File(size) => Item::File {
                    declared,
                    size: size.get(),
                    conflict,
                    opened: false,
                },
            };
            state.items.push(item);
        }
        let ticket = state.issued;
        state.issued += 1;
        Ok(Declaration {
            import: Arc::clone(self),
            first,
            count: items.len(),
            ticket: Some(ticket),
            opens: false,
        })
    }

    /// Opens file item `index` for its one `fs.import-file`; `stream` opens the stream its Puts
    /// bring for the item's size.
    ///
    /// # Errors
    ///
    /// `invalid-request` for an import that closed, or a number that names no file item or one
    /// already opened; the failure of `stream`, which leaves the item unopened.
    pub(super) fn open_file(
        self: &Arc<Self>,
        index: u64,
        stream: impl FnOnce(u64) -> Result<Streaming, Unopened>,
    ) -> Result<Bringing, Unopened> {
        let (index, size, after) = {
            let mut state = self.lock();
            let (after, closed) = (state.issued, state.closed);
            let index = usize::try_from(index).ok();
            match index.and_then(|index| Some((index, state.items.get_mut(index)?))) {
                Some((index, Item::File { size, opened, .. })) if !*opened && !closed => {
                    *opened = true;
                    (index, *size, after)
                }
                _ => {
                    let invalid = FilesError::new(ErrorCode::InvalidRequest);
                    return Err(Unopened::Failed(invalid));
                }
            }
        };
        let streaming = stream(size).inspect_err(|_| {
            if let Some(Item::File { opened, .. }) = self.lock().items.get_mut(index) {
                *opened = false;
            }
        })?;
        Ok(Bringing {
            import: Arc::clone(self),
            index,
            after,
            streaming,
        })
    }

    /// Waits until the declarations before turn `ticket` took effect.
    fn wait(&self, ticket: u64) {
        let state = self.lock();
        let _turn = self
            .turn
            .wait_while(state, |state| state.served < ticket)
            .unwrap_or_else(PoisonError::into_inner);
    }

    /// Records that turn `ticket` was taken.
    fn served(&self, ticket: u64) {
        let mut state = self.lock();
        state.taken.insert(ticket);
        loop {
            let next = state.served;
            if !state.taken.remove(&next) {
                break;
            }
            state.served += 1;
        }
        drop(state);
        self.turn.notify_all();
    }

    /// The folder an item declared at `declared` goes into, with that folder's final path below
    /// the target folder; `None` for the target folder itself.
    ///
    /// # Errors
    ///
    /// The failure of the directory item it lies in.
    fn folder_of(&self, declared: &str) -> Result<(NativePath, Option<String>), FilesError> {
        let Some(folder) = parent(declared) else {
            return Ok((self.target.clone(), None));
        };
        let state = self.lock();
        let outcome = state
            .directories
            .get(folder)
            .and_then(|index| match &state.items[*index] {
                Item::Directory { outcome, .. } => outcome.clone(),
                Item::File { .. } => None,
            });
        drop(state);
        let below = outcome.unwrap_or_else(|| Err(FilesError::new(ErrorCode::Failed)))?;
        let mut path = self.target.clone();
        for name in below.split('/') {
            path = names::child(&path, name)?;
        }
        Ok((path, Some(below)))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Declaration {
    /// Checks, on a helper, that the import's target is a folder.
    ///
    /// # Errors
    ///
    /// `not-directory` when it is not, or the failure to read it.
    pub(super) fn target_folder(&self, helpers: &Helpers) -> Result<(), FilesError> {
        let target = self.import.target.clone();
        let folder = entries::run(helpers, Operation::FsImportBegin, move || {
            fs::metadata(target.as_path()).map(|own| own.is_dir())
        })?;
        if folder {
            Ok(())
        } else {
            Err(FilesError::new(ErrorCode::NotDirectory))
        }
    }

    /// Creates the declared directory items once the declarations before them took effect, each
    /// where `admit` admits it, and answers each one's outcome in order; `operation` is the
    /// request that declared them.
    pub(super) fn settle(
        mut self,
        helpers: &Helpers,
        operation: Operation,
        admit: &Admit<'_>,
    ) -> Vec<ImportOutcome> {
        let Some(ticket) = self.ticket.take() else {
            return Vec::new();
        };
        let import = &self.import;
        import.wait(ticket);
        let mut outcomes = Vec::new();
        for index in self.first..self.first + self.count {
            let (declared, conflict) = match &import.lock().items[index] {
                Item::Directory {
                    declared, conflict, ..
                } => (declared.clone(), *conflict),
                Item::File { .. } => continue,
            };
            let made = import
                .folder_of(declared.as_str())
                .and_then(|(folder, below)| {
                    let target = names::child(&folder, last(declared.as_str()))?;
                    admit(&target)?;
                    let made = create_folder(helpers, operation, &target, conflict)?;
                    Ok(join(below.as_deref(), &made))
                });
            if let Item::Directory { outcome, .. } = &mut import.lock().items[index] {
                *outcome = Some(made.clone());
            }
            let number = u64::try_from(index)
                .ok()
                .and_then(|index| SafeInteger::new(index).ok());
            if let Some(index) = number {
                let outcome = made.and_then(|path| {
                    RelativePath::new(path).map_err(|_| FilesError::new(ErrorCode::Failed))
                });
                outcomes.push(ImportOutcome { index, outcome });
            }
        }
        import.served(ticket);
        outcomes
    }
}

impl Drop for Declaration {
    fn drop(&mut self) {
        let Some(ticket) = self.ticket.take() else {
            return;
        };
        let mut state = self.import.lock();
        for index in self.first..self.first + self.count {
            if let Some(Item::Directory { outcome, .. }) = state.items.get_mut(index) {
                *outcome = Some(Err(FilesError::new(ErrorCode::Failed)));
            }
        }
        state.closed |= self.opens;
        drop(state);
        self.import.served(ticket);
    }
}

impl Bringing {
    /// Brings the file's data once the declarations before it took effect: stages it beside its
    /// target where `admit` admits the target, answering each Put as its bytes are written, and
    /// publishes it by its conflict rule. Answers its final path below the target folder.
    ///
    /// # Errors
    ///
    /// The failure of the item's folder or of its staging, writing or publishing; `cancelled`,
    /// `timeout` or `invalid-request` as the stream ended, and `cancelled` when its lane or
    /// connection ended.
    pub(super) fn bring(
        self,
        helpers: &Helpers,
        admit: &Admit<'_>,
    ) -> Result<RelativePath, FilesError> {
        let Self {
            import,
            index,
            after,
            streaming,
        } = self;
        import.wait(after);
        let (declared, conflict) = match &import.lock().items[index] {
            Item::File {
                declared, conflict, ..
            } => (declared.clone(), *conflict),
            Item::Directory { .. } => return Err(FilesError::new(ErrorCode::Failed)),
        };
        let name = last(declared.as_str());
        let staged = import
            .folder_of(declared.as_str())
            .and_then(|(folder, below)| {
                admit(&names::child(&folder, name)?)?;
                let (staging, file) = stage(helpers, &folder, name)?;
                Ok((folder, below, staging, file))
            });
        let (folder, below, staging, file) = match staged {
            Ok(staged) => staged,
            Err(failure) => {
                refuse_all(&streaming, &failure);
                return Err(failure);
            }
        };
        let published =
            receive(&streaming, file).and_then(|()| publish(&staging, &folder, name, conflict));
        if published.is_err() {
            let _ = entries::run(helpers, Operation::FsImportFile, move || {
                remove_staging(&staging)
            });
        }
        RelativePath::new(join(below.as_deref(), &published?))
            .map_err(|_| FilesError::new(ErrorCode::Failed))
    }
}

/// The folder part of a declared path; `None` for a single name.
fn parent(declared: &str) -> Option<&str> {
    declared.rsplit_once('/').map(|(folder, _)| folder)
}

/// The last name of a declared path.
fn last(declared: &str) -> &str {
    declared.rsplit_once('/').map_or(declared, |(_, name)| name)
}

/// `name` below the final path `below`, or alone in the target folder.
fn join(below: Option<&str>, name: &str) -> String {
    below.map_or_else(|| name.to_owned(), |below| format!("{below}/{name}"))
}

/// Creates the folder at `target` by `conflict`, and answers the name it took.
fn create_folder(
    helpers: &Helpers,
    operation: Operation,
    target: &NativePath,
    conflict: Conflict,
) -> Result<String, FilesError> {
    let create = |path: &NativePath| {
        let path = path.clone();
        entries::run(helpers, operation, move || fs::create_dir(path.as_path()))
    };
    let failure = match create(target) {
        Ok(()) => return Ok(target.name().to_owned()),
        Err(failure) if failure.code == ErrorCode::Exists => failure,
        Err(failure) => return Err(failure),
    };
    match conflict {
        Conflict::Fail => Err(failure),
        Conflict::Merge | Conflict::Replace => {
            let existing = target.clone();
            let folder = entries::run(helpers, operation, move || {
                fs::symlink_metadata(existing.as_path()).map(|own| own.is_dir())
            })?;
            if conflict == Conflict::Merge && folder {
                return Ok(target.name().to_owned());
            }
            entries::long(|| entries::remove(target.as_path()))?;
            create(target).map(|()| target.name().to_owned())
        }
        Conflict::Increment => {
            let folder = target
                .parent()
                .ok_or_else(|| FilesError::new(ErrorCode::InvalidName))?;
            let mut candidate = target.name().to_owned();
            for _ in 0..MAXIMUM_ATTEMPTS {
                candidate = names::increment(&candidate, true);
                match create(&names::child(&folder, &candidate)?) {
                    Err(failure) if failure.code == ErrorCode::Exists => {}
                    created => return created.map(|()| candidate),
                }
            }
            Err(failure)
        }
    }
}

/// Creates a staging file for `name` in `folder`, exclusively, on a helper.
fn stage(
    helpers: &Helpers,
    folder: &NativePath,
    name: &str,
) -> Result<(PathBuf, File), FilesError> {
    let staged = staging::name(name).map_err(|failure| error::from_io(&failure))?;
    let path = names::child(folder, &staged)?.as_path().to_path_buf();
    let created = path.clone();
    let file = entries::run(helpers, Operation::FsImportFile, move || {
        File::create_new(&created)
    })?;
    Ok((path, file))
}

/// Writes the stream's deliveries to `file`, answering each Put once its bytes are written, until
/// the Put flagged `commit` completes them.
///
/// # Errors
///
/// The failure to write, and the code the stream ended with.
fn receive(streaming: &Streaming, mut file: File) -> Result<(), FilesError> {
    loop {
        let (bytes, put, last) = match streaming.deliveries.recv() {
            Ok(Delivery::Data(bytes, put)) => (bytes, put, false),
            Ok(Delivery::Commit(bytes, put)) => (bytes, put, true),
            Ok(Delivery::Ended(code)) => return Err(FilesError::new(code)),
            // The stream's lane or connection ended, or the process drains.
            Err(_) => return Err(FilesError::new(ErrorCode::Cancelled)),
        };
        if let Err(failure) = file.write_all(&bytes) {
            let failure = error::from_io(&failure);
            answer(put, Err(failure.clone()));
            refuse_all(streaming, &failure);
            return Err(failure);
        }
        streaming.written(bytes.len());
        answer(put, Ok(()));
        if last {
            return Ok(());
        }
    }
}

/// Ends the stream from the consumer's side, refusing the Puts that wait in it.
fn refuse_all(streaming: &Streaming, failure: &FilesError) {
    streaming.end();
    for waiting in streaming.deliveries.try_iter() {
        if let Delivery::Data(_, put) | Delivery::Commit(_, put) = waiting {
            answer(put, Err(failure.clone()));
        }
    }
}

/// Answers a Put's own call: empty once its bytes are written.
fn answer(put: Call, written: Result<(), FilesError>) {
    if put.cancelled() {
        // Dropping a cancelled call closes it as cancelled.
        return drop(put);
    }
    put.reply(&written.map_or_else(FilesReply::Error, |()| FilesReply::Empty));
}

/// Publishes the complete staging file at `staging` as `name` in `folder` by `conflict`, never
/// over an item that appears meanwhile, and answers the name it took.
fn publish(
    staging: &Path,
    folder: &NativePath,
    name: &str,
    conflict: Conflict,
) -> Result<String, FilesError> {
    let place = |name: &str| {
        let target = names::child(folder, name)?;
        entries::long(|| publish_at(staging, target.as_path()))
    };
    let failure = match place(name) {
        Ok(()) => return Ok(name.to_owned()),
        Err(failure) if failure.code == ErrorCode::Exists => failure,
        Err(failure) => return Err(failure),
    };
    match conflict {
        Conflict::Fail => Err(failure),
        Conflict::Replace | Conflict::Merge => {
            let existing = names::child(folder, name)?;
            entries::long(|| entries::remove(existing.as_path()))?;
            place(name).map(|()| name.to_owned())
        }
        Conflict::Increment => {
            let mut candidate = name.to_owned();
            for _ in 0..MAXIMUM_ATTEMPTS {
                candidate = names::increment(&candidate, false);
                match place(&candidate) {
                    Err(failure) if failure.code == ErrorCode::Exists => {}
                    placed => return placed.map(|()| candidate),
                }
            }
            Err(failure)
        }
    }
}

/// Gives the staging file at `staging` the name `target`, where nothing may be.
fn publish_at(staging: &Path, target: &Path) -> io::Result<()> {
    match fs::hard_link(staging, target) {
        // A staging name that stays is hidden and harmless.
        Ok(()) => remove_staging(staging).or(Ok(())),
        Err(failure) if failure.kind() == io::ErrorKind::AlreadyExists => Err(failure),
        // A filesystem without hard links.
        Err(_) => entries::relocate(staging, target, &fs::symlink_metadata(staging)?.file_type()),
    }
}

/// Removes a staging file that was not published.
fn remove_staging(staging: &Path) -> io::Result<()> {
    match fs::remove_file(staging) {
        Err(failure) if failure.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}

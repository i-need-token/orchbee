//! The native watcher through `notify`: ReadDirectoryChangesW on Windows, FSEvents on macOS and
//! inotify on Linux, where [`linux`] walks each recursive watch's folders itself. The adapter
//! thread owns the one watcher, created on first use, and applies the service's orders; a path an
//! order both starts and stops stops first. A watch that cannot start is refused to the service as
//! missing, past a limit or broken.
//!
//! Events arrive on the watcher's own thread, which translates them into changes: a creation is
//! added, a removal deleted, a rename's old path deleted and its new path added, any other change
//! changed, and access ignored. A rename whose side the platform does not tell is added when its
//! path exists and deleted otherwise. FSEvents reports a path's flags without their order, so on
//! macOS a creation of a path that no longer exists is dropped and a removal of one that exists
//! again is changed. A changed path the platform does not type is checked for being a directory.
//! A rescan notice, and a path this machine cannot name, is lost changes.

#[cfg(target_os = "linux")]
mod linux;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use notify::{
    Config, ErrorKind, Event, EventKind, EventKindMask, PathOp, RecommendedWatcher, Watcher,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};
use orchbee_files_protocol::ChangeKind;

use super::{
    Input,
    service::{Native, Need, Order, Why},
    stream::Change,
};
use crate::scope::path::{self, NativePath};

/// Whether the platform reports a path's flags without their order.
const UNORDERED_FLAGS: bool = cfg!(target_os = "macos");

/// The adapter thread, which stops when this is dropped.
pub(super) struct Adapter {
    commands: Sender<Command>,
}

enum Command {
    Apply(Order),
    #[cfg(target_os = "linux")]
    Hint(Hint),
    /// The watcher failed as a whole; the next order creates a new one.
    Reset,
    Stop,
}

/// What an event tells the walker of recursive watches.
#[cfg(target_os = "linux")]
enum Hint {
    /// A folder appeared at the path.
    Appeared(NativePath),
    /// Whatever was at the path is gone.
    Vanished(NativePath),
}

impl Adapter {
    /// Starts the adapter thread, which reports to `service`; `None` when no thread can start.
    pub(super) fn start(service: Sender<Input>) -> Option<Self> {
        let (commands, received) = mpsc::channel();
        let own = commands.clone();
        thread::Builder::new()
            .name("orchbee-files-watcher".into())
            .spawn(move || run(&received, own, service))
            .ok()?;
        Some(Self { commands })
    }

    pub(super) fn apply(&self, order: Order) {
        let _ = self.commands.send(Command::Apply(order));
    }
}

impl Drop for Adapter {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
    }
}

fn run(received: &Receiver<Command>, commands: Sender<Command>, service: Sender<Input>) {
    let mut watches = Watches {
        watcher: None,
        commands,
        service,
        #[cfg(target_os = "linux")]
        trees: linux::Trees::default(),
    };
    while let Ok(command) = received.recv() {
        match command {
            Command::Apply(order) => watches.apply(order),
            #[cfg(target_os = "linux")]
            Command::Hint(hint) => watches.hint(hint),
            Command::Reset => watches.reset(),
            Command::Stop => return,
        }
    }
}

struct Watches {
    watcher: Option<RecommendedWatcher>,
    /// The adapter's own commands, which the watcher's thread sends hints and resets to.
    commands: Sender<Command>,
    service: Sender<Input>,
    /// Linux's recursive watches, which other platforms keep themselves.
    #[cfg(target_os = "linux")]
    trees: linux::Trees,
}

/// What an operation's failure means for its watch.
fn why(kind: &ErrorKind) -> Why {
    match kind {
        ErrorKind::PathNotFound => Why::Missing,
        ErrorKind::MaxFilesWatch => Why::Limit,
        ErrorKind::Io(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Why::Missing
        }
        _ => Why::Broken,
    }
}

impl Watches {
    fn apply(&mut self, order: Order) {
        let restarting: HashSet<String> = order.watch.iter().map(|(path, _)| path.key()).collect();
        let (first, later): (Vec<_>, Vec<_>) = order
            .unwatch
            .into_iter()
            .partition(|(path, _)| restarting.contains(&path.key()));
        self.stop(first);
        self.start(order.watch);
        self.stop(later);
    }

    fn refuse(&self, path: NativePath, need: Need, why: Why) {
        let _ = self
            .service
            .send(Input::Native(Native::Refused { path, need, why }));
    }

    /// The watcher, created on first use; `None` after reporting that it cannot be created.
    fn watcher(&mut self) -> Option<&mut RecommendedWatcher> {
        if self.watcher.is_none() {
            let service = self.service.clone();
            let commands = self.commands.clone();
            let handler = move |event| handle(event, &service, &commands);
            let config = Config::default().with_event_kinds(EventKindMask::CORE);
            match RecommendedWatcher::new(handler, config) {
                Ok(watcher) => self.watcher = Some(watcher),
                Err(_) => {
                    let _ = self.service.send(Input::Native(Native::Failed(None)));
                    return None;
                }
            }
        }
        self.watcher.as_mut()
    }

    /// Applies `operations` in order and returns each that failed with what its failure means;
    /// `None` when the watcher failed as a whole, which is reported and ends it, so the next order
    /// creates a new one.
    fn update(&mut self, mut operations: Vec<PathOp>) -> Option<Vec<(PathBuf, Why)>> {
        let mut failed = Vec::new();
        let mut whole = false;
        let watcher = self.watcher()?;
        while !operations.is_empty() {
            match watcher.update_paths(operations) {
                Ok(()) => break,
                Err(error) => {
                    let Some(origin) = error.origin else {
                        whole = true;
                        break;
                    };
                    failed.push((origin.into_path(), why(&error.source.kind)));
                    operations = error.remaining;
                }
            }
        }
        if whole {
            self.reset();
            let _ = self.service.send(Input::Native(Native::Failed(None)));
            return None;
        }
        Some(failed)
    }

    /// Stops watching `paths`, when a watcher exists; a watch that already ended is no failure.
    fn unwatch(&mut self, paths: impl Iterator<Item = PathBuf>) {
        let operations: Vec<PathOp> = paths.map(PathOp::unwatch).collect();
        if self.watcher.is_some() && !operations.is_empty() {
            let _ = self.update(operations);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn start(&mut self, requests: Vec<(NativePath, Need)>) {
        if requests.is_empty() {
            return;
        }
        let operations = requests
            .iter()
            .map(|(path, need)| match need {
                Need::Tree => PathOp::watch_recursive(path.as_path()),
                Need::Folder => PathOp::watch_non_recursive(path.as_path()),
            })
            .collect();
        let Some(failed) = self.update(operations) else {
            return;
        };
        for (failed, why) in failed {
            if let Some((path, need)) = requests.iter().find(|(path, _)| path.as_path() == failed) {
                self.refuse(path.clone(), *need, why);
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn stop(&mut self, requests: Vec<(NativePath, Need)>) {
        self.unwatch(
            requests
                .into_iter()
                .map(|(path, _)| path.as_path().to_path_buf()),
        );
    }

    #[cfg(not(target_os = "linux"))]
    fn reset(&mut self) {
        self.watcher = None;
    }

    /// Starts each request: a recursive one watches each folder the walk finds, within the
    /// budget; a folder below its root that fails is left unwatched.
    #[cfg(target_os = "linux")]
    fn start(&mut self, requests: Vec<(NativePath, Need)>) {
        for (path, need) in requests {
            let folders = match need {
                Need::Tree => match linux::directories(&path, self.trees.budget()) {
                    Some(folders) => folders,
                    None => {
                        self.refuse(path, need, Why::Limit);
                        continue;
                    }
                },
                Need::Folder => vec![path.clone()],
            };
            match self.watch_folders(&folders) {
                Ok(watched) => self.trees.hold(&path, need, watched),
                Err(why) => self.refuse(path, need, why),
            }
        }
    }

    /// Watches the folders of `folders` not yet watched, the first being the request's own path.
    /// Returns the folders now held, or why the request is refused.
    #[cfg(target_os = "linux")]
    fn watch_folders(&mut self, folders: &[NativePath]) -> Result<Vec<NativePath>, Why> {
        let fresh: Vec<&NativePath> = self.trees.fresh(folders);
        if !self.trees.fits(fresh.len()) {
            return Err(Why::Limit);
        }
        let operations = fresh
            .iter()
            .map(|folder| PathOp::watch_non_recursive(folder.as_path()))
            .collect();
        let failed = self.update(operations).ok_or(Why::Broken)?;
        let own = folders
            .first()
            .and_then(|own| failed.iter().find(|(failed, _)| failed == own.as_path()));
        let refused = own.map(|(_, why)| *why).or_else(|| {
            failed
                .iter()
                .any(|(_, why)| *why == Why::Limit)
                .then_some(Why::Limit)
        });
        if let Some(why) = refused {
            let failed: Vec<&PathBuf> = failed.iter().map(|(path, _)| path).collect();
            let started: Vec<PathBuf> = fresh
                .iter()
                .map(|folder| folder.as_path().to_path_buf())
                .filter(|folder| !failed.contains(&folder))
                .collect();
            self.unwatch(started.into_iter());
            return Err(why);
        }
        Ok(folders
            .iter()
            .filter(|folder| !failed.iter().any(|(path, _)| path == folder.as_path()))
            .cloned()
            .collect())
    }

    #[cfg(target_os = "linux")]
    fn stop(&mut self, requests: Vec<(NativePath, Need)>) {
        let mut ended = Vec::new();
        for (path, need) in requests {
            ended.extend(self.trees.release(&path, need));
        }
        self.unwatch(ended.iter().map(|folder| folder.as_path().to_path_buf()));
    }

    #[cfg(target_os = "linux")]
    fn hint(&mut self, hint: Hint) {
        match hint {
            Hint::Appeared(folder) => {
                let Some(tree) = self.trees.tree_of(&folder) else {
                    return;
                };
                if super::excluded(&tree, &folder) {
                    return;
                }
                let added = linux::directories(&folder, self.trees.budget())
                    .ok_or(Why::Limit)
                    .and_then(|folders| self.watch_folders(&folders));
                match added {
                    Ok(added) => self.trees.extend(&tree, added),
                    Err(Why::Limit) => self.refuse(tree, Need::Tree, Why::Limit),
                    // The folder went away again or cannot be watched; the tree keeps the rest.
                    Err(_) => {}
                }
            }
            Hint::Vanished(path) => {
                let ended = self.trees.vanished(&path);
                self.unwatch(ended.iter().map(|folder| folder.as_path().to_path_buf()));
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn reset(&mut self) {
        self.watcher = None;
        self.trees = linux::Trees::default();
    }
}

/// Handles one event or failure on the watcher's thread.
fn handle(event: notify::Result<Event>, service: &Sender<Input>, commands: &Sender<Command>) {
    let native = match event {
        Ok(event) if event.need_rescan() => Native::Lost(event.paths.first().and_then(nameable)),
        Ok(event) => {
            let (changes, lost) = translate(&event);
            hint(&changes, commands);
            if let Some(lost) = lost {
                let _ = service.send(Input::Native(Native::Lost(Some(lost))));
            }
            if changes.is_empty() {
                return;
            }
            Native::Changes(changes)
        }
        Err(error) => {
            let path = error.paths.first().and_then(nameable);
            if path.is_none() {
                let _ = commands.send(Command::Reset);
            }
            Native::Failed(path)
        }
    };
    let _ = service.send(Input::Native(native));
}

/// Tells the walker of recursive watches about the folders that appeared and the paths that
/// vanished.
#[cfg(target_os = "linux")]
fn hint(changes: &[Change], commands: &Sender<Command>) {
    for change in changes {
        let hint = match change.kind {
            ChangeKind::Added if change.directory => Hint::Appeared(change.path.clone()),
            ChangeKind::Deleted => Hint::Vanished(change.path.clone()),
            ChangeKind::Added | ChangeKind::Changed => continue,
        };
        let _ = commands.send(Command::Hint(hint));
    }
}

/// Other platforms walk nothing.
#[cfg(not(target_os = "linux"))]
const fn hint(_: &[Change], _: &Sender<Command>) {}

/// What an event claims happened to one path.
#[derive(Clone, Copy)]
enum Claim {
    /// Added, and whether as a directory when the platform tells.
    Added(Option<bool>),
    Changed,
    Deleted(Option<bool>),
    /// Renamed, to or from the path.
    Renamed,
}

/// The changes `event` reports, and the nearest folder this machine can name above a path it
/// cannot.
fn translate(event: &Event) -> (Vec<Change>, Option<NativePath>) {
    let claim = |index: usize| match event.kind {
        EventKind::Create(kind) => Some(Claim::Added(match kind {
            CreateKind::File => Some(false),
            CreateKind::Folder => Some(true),
            CreateKind::Any | CreateKind::Other => None,
        })),
        EventKind::Remove(kind) => Some(Claim::Deleted(match kind {
            RemoveKind::File => Some(false),
            RemoveKind::Folder => Some(true),
            RemoveKind::Any | RemoveKind::Other => None,
        })),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => Some(Claim::Deleted(None)),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => Some(Claim::Added(None)),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => Some(if index == 0 {
            Claim::Deleted(None)
        } else {
            Claim::Added(None)
        }),
        EventKind::Modify(ModifyKind::Name(RenameMode::Any | RenameMode::Other)) => {
            Some(Claim::Renamed)
        }
        EventKind::Modify(_) | EventKind::Any => Some(Claim::Changed),
        EventKind::Access(_) | EventKind::Other => None,
    };
    let mut changes = Vec::new();
    let mut lost = None;
    for (index, path) in event.paths.iter().enumerate() {
        let Some(claim) = claim(index) else {
            continue;
        };
        match nameable(path) {
            Some(path) => changes.extend(settle(path, claim)),
            None => {
                lost = lost.or_else(|| path.ancestors().skip(1).find_map(nameable));
            }
        }
    }
    (changes, lost)
}

/// The change `claim` makes of `path`, checked against the path where the platform leaves its
/// order or type open.
fn settle(path: NativePath, claim: Claim) -> Option<Change> {
    let (kind, directory) = match claim {
        Claim::Added(directory) if UNORDERED_FLAGS => {
            let found = kind_of(&path)?;
            (ChangeKind::Added, directory.unwrap_or(found))
        }
        Claim::Added(directory) => (
            ChangeKind::Added,
            directory.unwrap_or_else(|| kind_of(&path).unwrap_or(false)),
        ),
        Claim::Changed if UNORDERED_FLAGS => (ChangeKind::Changed, kind_of(&path)?),
        Claim::Changed => (ChangeKind::Changed, kind_of(&path).unwrap_or(false)),
        Claim::Deleted(directory) if UNORDERED_FLAGS => match present(&path) {
            Some(directory) => (ChangeKind::Changed, directory),
            None => (ChangeKind::Deleted, directory.unwrap_or(false)),
        },
        Claim::Deleted(directory) => (ChangeKind::Deleted, directory.unwrap_or(false)),
        Claim::Renamed => match present(&path) {
            Some(directory) => (ChangeKind::Added, directory),
            None => (ChangeKind::Deleted, false),
        },
    };
    Some(Change {
        kind,
        path,
        directory,
    })
}

/// Whether something exists at `path`, as a directory or not.
fn kind_of(path: &NativePath) -> Option<bool> {
    std::fs::symlink_metadata(path.as_path())
        .ok()
        .map(|metadata| metadata.is_dir())
}

/// Whether an entry with exactly `path`'s name exists, as a directory or not. Where names ignore
/// case, a name differing only in case, such as a case-only rename's old name, does not count.
fn present(path: &NativePath) -> Option<bool> {
    let directory = kind_of(path)?;
    if path::CASE_SENSITIVE {
        return Some(directory);
    }
    let parent = path.parent()?;
    let named = std::fs::read_dir(parent.as_path())
        .ok()?
        .flatten()
        .any(|entry| entry.file_name().to_str() == Some(path.name()));
    named.then_some(directory)
}

fn nameable(path: impl AsRef<Path>) -> Option<NativePath> {
    NativePath::parse(path.as_ref().to_str()?).ok()
}

//! Follows changes with the platform's own watcher through `notify`. macOS and Windows follow a
//! whole tree with one watch. Elsewhere every directory needs a watch of its own, so the lane walks
//! the tree itself and leaves out what Git ignores, such as dependencies and build output, unless
//! the right bar shows it; it watches directories that appear and forgets those that go.
use super::repository::Repository;
use super::{Backend, Notice, WatchRoot};
use notify::event::{ModifyKind, RenameMode};
use notify::{Config, Event, EventKind, EventKindMask, RecommendedWatcher, RecursiveMode, Watcher};
use orchbee_workbench_protocol::{WorkspaceFilesWatchState, is_workspace_relative_path};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SendError, Sender};
use std::sync::{Arc, Mutex, Weak};

/// Whether one watch follows a whole tree; elsewhere every directory is watched on its own.
pub(super) const WATCHES_TREES: bool = cfg!(any(target_os = "macos", windows));
/// Directories watched one by one across every root; a root whose tree would pass it is not
/// followed, and Clients refresh it only when asked.
const MAXIMUM_WATCHED_DIRECTORIES: usize = 65_536;

/// The platform's watcher behind the watch lane.
pub(crate) struct Platform {
    inner: Arc<Mutex<Inner>>,
}

impl Platform {
    /// Starts the platform's watcher and the thread that turns its events into `notices`. Where
    /// the platform has no watcher, every root is unavailable.
    pub(crate) fn start(notices: Sender<Notice>) -> Self {
        let (events, received) = mpsc::channel();
        let watcher = RecommendedWatcher::new(
            move |event| {
                // The receiving thread ends only with the watcher, so a failed send is shutdown.
                let _sent = events.send(event);
            },
            Config::default()
                .with_event_kinds(EventKindMask::CORE)
                .with_follow_symlinks(false),
        )
        .ok();
        let inner = Arc::new(Mutex::new(Inner {
            watcher,
            notices,
            roots: BTreeMap::new(),
            repositories: BTreeMap::new(),
            registered: BTreeMap::new(),
        }));
        let weak = Arc::downgrade(&inner);
        let keeper = std::thread::Builder::new()
            .name("orchbee-workspace-files-watch-events".into())
            .spawn(move || keep(&weak, &received));
        if keeper.is_err()
            && let Ok(mut inner) = inner.lock()
        {
            inner.watcher = None;
        }
        Self { inner }
    }
}

#[cfg(test)]
impl Platform {
    /// Every path the platform watches now.
    pub(super) fn watched(&self) -> Vec<PathBuf> {
        self.inner
            .lock()
            .map(|inner| inner.registered.keys().cloned().collect())
            .unwrap_or_default()
    }
}

impl Backend for Platform {
    fn watch(&mut self, roots: &[WatchRoot]) -> Vec<(String, WorkspaceFilesWatchState)> {
        match self.inner.lock() {
            Ok(mut inner) => inner.replace(roots),
            Err(_) => unavailable(roots),
        }
    }
}

fn unavailable(roots: &[WatchRoot]) -> Vec<(String, WorkspaceFilesWatchState)> {
    roots
        .iter()
        .map(|root| (root.root.clone(), WorkspaceFilesWatchState::Unavailable))
        .collect()
}

/// Turns the watcher's events into notices until the watcher or the lane is gone.
fn keep(inner: &Weak<Mutex<Inner>>, events: &Receiver<notify::Result<Event>>) {
    for event in events {
        let Some(inner) = inner.upgrade() else {
            return;
        };
        let Ok(mut inner) = inner.lock() else {
            return;
        };
        if inner.handle(event).is_err() {
            return;
        }
    }
}

/// A root's tree passed [`MAXIMUM_WATCHED_DIRECTORIES`], or the platform's own watch limit.
struct Full;

/// How many holds of roots keep a watched path.
struct Registration {
    holders: usize,
}

/// One followed root.
struct Followed {
    canonical: PathBuf,
    /// The working tree's top level, a key of [`Inner::repositories`].
    repository: Option<PathBuf>,
    /// Paths this root's tree and its repository's directories need.
    holds: BTreeSet<PathBuf>,
    /// Directories watched because the right bar shows them, or an open file is in them, where
    /// the walk may leave them out.
    shown: BTreeSet<PathBuf>,
}

struct Inner {
    watcher: Option<RecommendedWatcher>,
    notices: Sender<Notice>,
    /// Followed roots by the name the Server gave.
    roots: BTreeMap<String, Followed>,
    repositories: BTreeMap<PathBuf, Repository>,
    registered: BTreeMap<PathBuf, Registration>,
}

/// What an event says happened at one of its paths.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Role {
    Arrived,
    Gone,
    Changed,
}

impl Inner {
    fn replace(&mut self, roots: &[WatchRoot]) -> Vec<(String, WorkspaceFilesWatchState)> {
        if self.watcher.is_none() {
            return unavailable(roots);
        }
        let wanted: BTreeSet<&str> = roots.iter().map(|root| root.root.as_str()).collect();
        let gone: Vec<String> = self
            .roots
            .keys()
            .filter(|name| !wanted.contains(name.as_str()))
            .cloned()
            .collect();
        for name in gone {
            self.drop_root(&name);
        }
        let mut refused = Vec::new();
        for root in roots {
            if !self.roots.contains_key(&root.root) {
                match self.follow(&root.root) {
                    Some(followed) => {
                        self.roots.insert(root.root.clone(), followed);
                    }
                    None => {
                        refused.push((root.root.clone(), WorkspaceFilesWatchState::Unavailable));
                        continue;
                    }
                }
            }
            if !WATCHES_TREES {
                self.show(root);
            }
        }
        self.forget_repositories();
        refused
    }

    /// Starts following one root, or none when it is not a directory, the platform refuses it, or
    /// its tree is too large.
    fn follow(&mut self, name: &str) -> Option<Followed> {
        let canonical = std::fs::canonicalize(name)
            .ok()
            .filter(|path| path.is_dir())?;
        let repository = Repository::discover(&canonical).map(|repository| {
            let top = repository.top().to_path_buf();
            self.repositories.entry(top.clone()).or_insert(repository);
            top
        });
        let mut followed = Followed {
            canonical,
            repository,
            holds: BTreeSet::new(),
            shown: BTreeSet::new(),
        };
        let Self {
            watcher: Some(watcher),
            registered,
            repositories,
            ..
        } = self
        else {
            return None;
        };
        let repository = followed
            .repository
            .as_ref()
            .and_then(|top| repositories.get_mut(top));
        match attach(watcher, registered, &mut followed, repository) {
            Ok(true) => Some(followed),
            Ok(false) | Err(Full) => {
                release_all(watcher, registered, &followed);
                None
            }
        }
    }

    /// Keeps the directories `root` shows, and those holding its open files, watched.
    fn show(&mut self, root: &WatchRoot) {
        let Self {
            watcher: Some(watcher),
            registered,
            roots,
            ..
        } = self
        else {
            return;
        };
        let Some(followed) = roots.get_mut(&root.root) else {
            return;
        };
        let directories = root
            .directories
            .iter()
            .filter(|path| path.is_empty() || is_workspace_relative_path(path))
            .map(|path| beneath(&followed.canonical, path));
        let parents = root
            .files
            .iter()
            .filter(|path| is_workspace_relative_path(path))
            .filter_map(|path| {
                beneath(&followed.canonical, path)
                    .parent()
                    .map(Path::to_path_buf)
            });
        let wanted: BTreeSet<PathBuf> = directories.chain(parents).collect();
        let stale: Vec<PathBuf> = followed.shown.difference(&wanted).cloned().collect();
        for path in stale {
            followed.shown.remove(&path);
            release(watcher, registered, &path);
        }
        for path in wanted {
            // A shown directory past the limit is left to refresh; the root stays followed.
            let _held = hold(
                watcher,
                registered,
                &mut followed.shown,
                &path,
                RecursiveMode::NonRecursive,
            );
        }
    }

    fn drop_root(&mut self, name: &str) {
        let Some(followed) = self.roots.remove(name) else {
            return;
        };
        if let Some(watcher) = &mut self.watcher {
            release_all(watcher, &mut self.registered, &followed);
        }
    }

    fn forget_repositories(&mut self) {
        let used: BTreeSet<&PathBuf> = self
            .roots
            .values()
            .filter_map(|followed| followed.repository.as_ref())
            .collect();
        self.repositories.retain(|top, _| used.contains(top));
    }

    fn handle(&mut self, event: notify::Result<Event>) -> Result<(), SendError<Notice>> {
        let event = match event {
            Ok(event) => event,
            // Events may have been lost wherever the watcher failed.
            Err(error) => return self.overflow(&error.paths),
        };
        if event.need_rescan() {
            return self.overflow(&event.paths);
        }
        let roles: Vec<(PathBuf, Role)> = match event.kind {
            EventKind::Access(_) => return Ok(()),
            EventKind::Create(_) => with_role(event.paths, Role::Arrived),
            EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                with_role(event.paths, Role::Gone)
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                with_role(event.paths, Role::Arrived)
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                let mut paths = event.paths.into_iter();
                paths
                    .next()
                    .map(|from| (from, Role::Gone))
                    .into_iter()
                    .chain(paths.map(|to| (to, Role::Arrived)))
                    .collect()
            }
            EventKind::Modify(ModifyKind::Name(_)) => event
                .paths
                .into_iter()
                .map(|path| {
                    let role = if path.symlink_metadata().is_ok() {
                        Role::Arrived
                    } else {
                        Role::Gone
                    };
                    (path, role)
                })
                .collect(),
            EventKind::Modify(_) | EventKind::Any | EventKind::Other => {
                with_role(event.paths, Role::Changed)
            }
        };
        for (path, role) in roles {
            self.invalidate(&path);
            if role == Role::Gone && self.lose_root(&path)? {
                continue;
            }
            if !WATCHES_TREES {
                match role {
                    Role::Gone => self.forget_beneath(&path),
                    Role::Arrived => self.extend(&path)?,
                    Role::Changed => {}
                }
                self.follow_rules(&path)?;
            }
            self.note(&path)?;
        }
        Ok(())
    }

    /// A root that is itself removed or moved is no longer followed; true when `path` was one.
    fn lose_root(&mut self, path: &Path) -> Result<bool, SendError<Notice>> {
        let lost: Vec<String> = self
            .roots
            .iter()
            .filter(|(_, followed)| followed.canonical == path)
            .map(|(name, _)| name.clone())
            .collect();
        if lost.is_empty() {
            return Ok(false);
        }
        for name in &lost {
            self.drop_root(name);
        }
        self.notices.send(Notice::Lost(path.to_path_buf()))?;
        Ok(true)
    }

    /// Forgets the watches at and beneath a directory that went away; a moved directory keeps a
    /// watch that would report its old paths.
    fn forget_beneath(&mut self, path: &Path) {
        let gone: Vec<PathBuf> = self
            .registered
            .range(path.to_path_buf()..)
            .map(|(watched, _)| watched)
            .take_while(|watched| watched.starts_with(path))
            .cloned()
            .collect();
        for watched in gone {
            self.registered.remove(&watched);
            if let Some(watcher) = &mut self.watcher {
                // The platform has usually dropped it already.
                let _unwatched = watcher.unwatch(&watched);
            }
            for followed in self.roots.values_mut() {
                followed.holds.remove(&watched);
                followed.shown.remove(&watched);
            }
        }
    }

    /// Watches a directory that appeared, with what lies beneath it, for each root that follows
    /// it.
    fn extend(&mut self, path: &Path) -> Result<(), SendError<Notice>> {
        if !path
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.is_dir())
        {
            return Ok(());
        }
        let mut walks = Vec::new();
        for (name, followed) in &self.roots {
            let repository = followed
                .repository
                .as_ref()
                .and_then(|top| self.repositories.get(top));
            if let Some(repository) = repository
                && repository.git_path(path).is_some()
            {
                // Inside Git's directories only references are followed, every one of them.
                if path.starts_with(repository.common_dir().join("refs")) {
                    walks.push((name.clone(), path.to_path_buf(), false));
                }
                continue;
            }
            if path.starts_with(&followed.canonical)
                && path.file_name().is_none_or(|name| name != ".git")
            {
                walks.push((name.clone(), path.to_path_buf(), true));
            }
        }
        self.walk_again(walks)
    }

    /// Walks again beneath a changed ignore rules file, so a directory the rules no longer ignore
    /// is watched; one they now ignore stays watched until it goes.
    fn follow_rules(&mut self, path: &Path) -> Result<(), SendError<Notice>> {
        let mut walks = Vec::new();
        for (name, followed) in &self.roots {
            let rules = followed
                .repository
                .as_ref()
                .and_then(|top| self.repositories.get(top))
                .is_some_and(|repository| repository.is_rule_file(path));
            if rules {
                let start = path
                    .parent()
                    .filter(|parent| parent.starts_with(&followed.canonical))
                    .unwrap_or(&followed.canonical);
                walks.push((name.clone(), start.to_path_buf(), true));
            }
        }
        self.walk_again(walks)
    }

    /// Walks each root's `start`, by its repository's ignore rules where `ignoring`; a root whose
    /// tree grows past the limit is lost.
    fn walk_again(&mut self, walks: Vec<(String, PathBuf, bool)>) -> Result<(), SendError<Notice>> {
        for (name, start, ignoring) in walks {
            let Self {
                watcher: Some(watcher),
                registered,
                roots,
                repositories,
                ..
            } = self
            else {
                return Ok(());
            };
            let Some(followed) = roots.get_mut(&name) else {
                continue;
            };
            let mut repository = followed
                .repository
                .as_ref()
                .filter(|_| ignoring)
                .and_then(|top| repositories.get_mut(top));
            let skipped = start != followed.canonical
                && !followed.shown.contains(&start)
                && repository
                    .as_deref_mut()
                    .is_some_and(|repository| repository.ignored(&start, true));
            if skipped {
                continue;
            }
            if walk(watcher, registered, &mut followed.holds, repository, &start).is_err() {
                let canonical = followed.canonical.clone();
                self.lose_root(&canonical)?;
            }
        }
        Ok(())
    }

    /// Forgets what a changed ignore rules file or index made stale.
    fn invalidate(&mut self, path: &Path) {
        for repository in self.repositories.values_mut() {
            if repository.is_rule_file(path) {
                repository.rules_changed();
            }
            if repository.is_index(path) {
                repository.index_changed();
            }
        }
    }

    /// Reports one changed path: the repository's own state, Git at work, or a working-tree path
    /// with whether Git follows it.
    fn note(&mut self, path: &Path) -> Result<(), SendError<Notice>> {
        let mut inside_git = false;
        let mut states: Vec<PathBuf> = Vec::new();
        for (top, repository) in &self.repositories {
            let Some(state) = repository.git_path(path) else {
                continue;
            };
            inside_git = true;
            if state {
                states.extend(
                    self.roots
                        .values()
                        .filter(|followed| followed.repository.as_ref() == Some(top))
                        .map(|followed| followed.canonical.clone()),
                );
            }
        }
        if inside_git {
            for root in states {
                self.notices.send(Notice::Repository(root))?;
            }
            return Ok(());
        }
        let is_dir = path
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.is_dir());
        let repository = self
            .repositories
            .values_mut()
            .filter(|repository| path.starts_with(repository.top()))
            .max_by_key(|repository| repository.top().as_os_str().len())
            .is_none_or(|repository| !repository.ignored(path, is_dir));
        self.notices.send(Notice::Path {
            path: path.to_path_buf(),
            repository,
        })
    }

    /// Reports that events may have been lost at `paths`, or anywhere when the platform did not
    /// say where.
    fn overflow(&self, paths: &[PathBuf]) -> Result<(), SendError<Notice>> {
        for followed in self.roots.values() {
            let repository = followed
                .repository
                .as_ref()
                .and_then(|top| self.repositories.get(top));
            let affected = paths.is_empty()
                || paths.iter().any(|path| {
                    path.starts_with(&followed.canonical)
                        || followed.canonical.starts_with(path)
                        || repository.is_some_and(|repository| repository.git_path(path).is_some())
                });
            if affected {
                self.notices
                    .send(Notice::Overflow(followed.canonical.clone()))?;
            }
        }
        Ok(())
    }
}

fn with_role(paths: Vec<PathBuf>, role: Role) -> Vec<(PathBuf, Role)> {
    paths.into_iter().map(|path| (path, role)).collect()
}

/// `relative`, `/`-separated, beneath `root`, pushed name by name: a verbatim Windows path reads
/// no `/` as a separator.
fn beneath(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for name in relative.split('/').filter(|name| !name.is_empty()) {
        path.push(name);
    }
    path
}

/// Watches one root's tree and its repository's directories; false when the root itself cannot
/// be watched.
fn attach(
    watcher: &mut RecommendedWatcher,
    registered: &mut BTreeMap<PathBuf, Registration>,
    followed: &mut Followed,
    mut repository: Option<&mut Repository>,
) -> Result<bool, Full> {
    let root = followed.canonical.clone();
    let mode = if WATCHES_TREES {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    if !hold(watcher, registered, &mut followed.holds, &root, mode)? {
        return Ok(false);
    }
    if !WATCHES_TREES {
        walk(
            watcher,
            registered,
            &mut followed.holds,
            repository.as_deref_mut(),
            &root,
        )?;
    }
    let Some(repository) = repository else {
        return Ok(true);
    };
    // The index and `HEAD`, then `packed-refs` and the references, wherever the tree's watch
    // does not already reach them.
    let references = repository.common_dir().join("refs");
    for directory in [repository.git_dir(), repository.common_dir()] {
        if !(WATCHES_TREES && directory.starts_with(&root)) {
            hold(
                watcher,
                registered,
                &mut followed.holds,
                directory,
                RecursiveMode::NonRecursive,
            )?;
        }
    }
    if !WATCHES_TREES {
        walk(watcher, registered, &mut followed.holds, None, &references)?;
    } else if !references.starts_with(&root) {
        hold(
            watcher,
            registered,
            &mut followed.holds,
            &references,
            RecursiveMode::Recursive,
        )?;
    }
    Ok(true)
}

/// Watches `start` and every directory beneath it one by one, leaving out `.git` and, given the
/// repository, every directory Git ignores. A directory that cannot be read is left out, as Git
/// leaves it out.
fn walk(
    watcher: &mut RecommendedWatcher,
    registered: &mut BTreeMap<PathBuf, Registration>,
    holds: &mut BTreeSet<PathBuf>,
    mut repository: Option<&mut Repository>,
    start: &Path,
) -> Result<(), Full> {
    let mut pending = vec![start.to_path_buf()];
    while let Some(directory) = pending.pop() {
        if !hold(
            watcher,
            registered,
            holds,
            &directory,
            RecursiveMode::NonRecursive,
        )? {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            // A symbolic link is not followed, as Git does not follow one.
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) || entry.file_name() == ".git" {
                continue;
            }
            let path = entry.path();
            if let Some(repository) = repository.as_deref_mut()
                && repository.ignored(&path, true)
            {
                continue;
            }
            pending.push(path);
        }
    }
    Ok(())
}

/// Adds one hold on `path` to `holds`, watching it if no root held it; true when it is watched.
/// A path that vanished or cannot be read is not watched; a limit stops the root.
fn hold(
    watcher: &mut RecommendedWatcher,
    registered: &mut BTreeMap<PathBuf, Registration>,
    holds: &mut BTreeSet<PathBuf>,
    path: &Path,
    mode: RecursiveMode,
) -> Result<bool, Full> {
    if holds.contains(path) {
        return Ok(true);
    }
    if let Some(registration) = registered.get_mut(path) {
        registration.holders += 1;
        holds.insert(path.to_path_buf());
        return Ok(true);
    }
    if !WATCHES_TREES && registered.len() >= MAXIMUM_WATCHED_DIRECTORIES {
        return Err(Full);
    }
    match watcher.watch(path, mode) {
        Ok(()) => {
            registered.insert(path.to_path_buf(), Registration { holders: 1 });
            holds.insert(path.to_path_buf());
            Ok(true)
        }
        Err(error) if matches!(error.kind, notify::ErrorKind::MaxFilesWatch) => Err(Full),
        Err(_) => Ok(false),
    }
}

/// Drops one hold on `path`, unwatching it when no root holds it any more.
fn release(
    watcher: &mut RecommendedWatcher,
    registered: &mut BTreeMap<PathBuf, Registration>,
    path: &Path,
) {
    let Some(registration) = registered.get_mut(path) else {
        return;
    };
    registration.holders = registration.holders.saturating_sub(1);
    if registration.holders == 0 {
        registered.remove(path);
        let _unwatched = watcher.unwatch(path);
    }
}

fn release_all(
    watcher: &mut RecommendedWatcher,
    registered: &mut BTreeMap<PathBuf, Registration>,
    followed: &Followed,
) {
    for path in followed.holds.iter().chain(&followed.shown) {
        release(watcher, registered, path);
    }
}

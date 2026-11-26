//! The watch lane: a separate service process that follows changes beneath the Workspaces the
//! Server names and reports, per root, whether what the right bar shows or the change list may
//! differ. It reports no paths, so a burst of changes costs one line per root per interval.
mod platform;
mod repository;
#[cfg(test)]
mod tests;

use orchbee_workbench_protocol::{WorkspaceFilesWatchState, is_workspace_relative_path};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// How long changes collect before the lane reports them, so a burst is one report.
pub const COALESCE: Duration = Duration::from_millis(100);
/// The lane exits after this long with nothing to watch.
const IDLE_EXIT: Duration = super::IDLE_EXIT;
/// Roots one watch set names: one per Workspace a window shows.
pub const MAXIMUM_WATCHED_ROOTS: usize = 64;
/// One watch-set line's bytes: every root's directories and files.
const MAXIMUM_WATCH_SET_LINE_BYTES: u64 = 4 * 1024 * 1024;

/// What to watch beneath one Workspace directory, as the Server's Clients name it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchRoot {
    /// The Workspace directory, absolute.
    pub root: String,
    /// Workspace-relative directories the right bar shows; the empty path is the root itself.
    pub directories: Vec<String>,
    /// Workspace-relative files open from the right bar.
    pub files: Vec<String>,
}

/// One request line: every root to watch, replacing the set before it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchSet {
    pub roots: Vec<WatchRoot>,
}

/// One report line.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WatchEvent {
    /// Whether the lane follows `root`; it reports this for each root it is asked to watch, and
    /// again whenever it changes.
    State {
        root: String,
        state: WorkspaceFilesWatchState,
    },
    /// Something beneath `root` changed since the last report: `files` when a directory it names
    /// may list differently or a file it names changed, `repository` when the change list may
    /// differ.
    Changed {
        root: String,
        files: bool,
        repository: bool,
    },
}

/// A change a platform watcher reports. Paths are canonical, as the platform reports them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Notice {
    /// Something changed at `path` in a working tree; `repository` is false when the change cannot
    /// change the change list, at a path Git ignores and does not track.
    Path { path: PathBuf, repository: bool },
    /// The index, `HEAD` or a reference changed in the repository of the root at this path.
    Repository(PathBuf),
    /// The platform dropped events for the root at this path; everything there may have changed.
    Overflow(PathBuf),
    /// The platform stopped following the root at this path.
    Lost(PathBuf),
}

/// A platform's way of following changes. `watch` replaces what it follows; each root it cannot
/// follow comes back with the state it reports.
pub trait Backend {
    fn watch(&mut self, roots: &[WatchRoot]) -> Vec<(String, WorkspaceFilesWatchState)>;
}

/// What a change means for one root.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Effect {
    files: bool,
    repository: bool,
}

impl Effect {
    const EVERYTHING: Self = Self {
        files: true,
        repository: true,
    };
}

/// One watched root with the paths it names, kept relative and `/`-separated.
#[derive(Debug)]
struct Watched {
    /// The root as the Server names it, which its reports carry.
    root: String,
    /// The root as the platform reports paths beneath it.
    canonical: PathBuf,
    directories: BTreeSet<String>,
    files: BTreeSet<String>,
}

impl Watched {
    fn new(root: &WatchRoot) -> Self {
        Self {
            root: root.root.clone(),
            canonical: std::fs::canonicalize(&root.root)
                .unwrap_or_else(|_| PathBuf::from(&root.root)),
            directories: root
                .directories
                .iter()
                .filter(|path| path.is_empty() || is_workspace_relative_path(path))
                .cloned()
                .collect(),
            files: root
                .files
                .iter()
                .filter(|path| is_workspace_relative_path(path))
                .cloned()
                .collect(),
        }
    }

    /// Whether a change at `relative` changes something shown: an entry of a shown directory,
    /// the shown directory itself, or an open file.
    fn shows(&self, relative: &str) -> bool {
        let parent = relative.rsplit_once('/').map_or("", |(parent, _)| parent);
        self.directories.contains(parent)
            || self.directories.contains(relative)
            || self.files.contains(relative)
    }

    /// `path` relative to the root with `/` separators, or none when it is not beneath it.
    fn relative(&self, path: &Path) -> Option<String> {
        let rest = path.strip_prefix(&self.canonical).ok()?;
        let mut names = Vec::new();
        for component in rest.components() {
            match component {
                Component::Normal(name) => names.push(name.to_str()?),
                _ => return None,
            }
        }
        Some(names.join("/"))
    }
}

/// Collects what changed beneath each root until the next report.
#[derive(Debug, Default)]
pub struct Coalescer {
    watched: Vec<Watched>,
    pending: BTreeMap<String, Effect>,
}

impl Coalescer {
    /// Follows `roots` from now on; changes pending for a root no longer named are dropped.
    pub fn replace(&mut self, roots: &[WatchRoot]) {
        self.watched = roots.iter().map(Watched::new).collect();
        let names: BTreeSet<&str> = roots.iter().map(|root| root.root.as_str()).collect();
        self.pending.retain(|root, _| names.contains(root.as_str()));
    }

    /// Records one notice against every root it is beneath; a nested Workspace sees a change in
    /// itself and in the Workspace around it.
    pub fn record(&mut self, notice: &Notice) {
        for watched in &self.watched {
            let effect = match notice {
                Notice::Path { path, repository } => match watched.relative(path) {
                    Some(relative) => Effect {
                        files: watched.shows(&relative),
                        repository: *repository,
                    },
                    None => continue,
                },
                Notice::Repository(root) if watched.canonical == *root => Effect {
                    files: false,
                    repository: true,
                },
                Notice::Overflow(path) | Notice::Lost(path) if watched.relative(path).is_some() => {
                    Effect::EVERYTHING
                }
                Notice::Repository(_) | Notice::Overflow(_) | Notice::Lost(_) => continue,
            };
            if effect == Effect::default() {
                continue;
            }
            let pending = self.pending.entry(watched.root.clone()).or_default();
            pending.files |= effect.files;
            pending.repository |= effect.repository;
        }
    }

    /// The roots, as the Server names them, followed at the canonical `path`.
    fn named(&self, path: &Path) -> impl Iterator<Item = &str> {
        self.watched
            .iter()
            .filter(move |watched| watched.canonical == path)
            .map(|watched| watched.root.as_str())
    }

    /// The reports collected since the last drain, one per root that changed.
    pub fn drain(&mut self) -> Vec<WatchEvent> {
        std::mem::take(&mut self.pending)
            .into_iter()
            .map(|(root, effect)| WatchEvent::Changed {
                root,
                files: effect.files,
                repository: effect.repository,
            })
            .collect()
    }
}

/// What reaches the lane's loop: a new watch set, a platform notice, or the end of input.
enum Input {
    Set(WatchSet),
    Notice(Notice),
    Closed,
}

/// Serves watch sets from `input` and writes reports to `output` until input closes, output
/// fails, or nothing has been watched for the idle time. `start` makes the backend, handing it
/// the sender its notices go to.
pub fn serve_watch<B: Backend>(
    input: impl BufRead + Send + 'static,
    mut output: impl Write,
    start: impl FnOnce(Sender<Notice>) -> B,
) {
    let (sender, inputs) = mpsc::channel();
    let (notices, notified) = mpsc::channel();
    let mut backend = start(notices);
    let forward = sender.clone();
    let reader = std::thread::Builder::new()
        .name("orchbee-workspace-files-watch-input".into())
        .spawn(move || read_sets(input, &sender));
    let relay = std::thread::Builder::new()
        .name("orchbee-workspace-files-watch-notices".into())
        .spawn(move || {
            for notice in notified {
                if forward.send(Input::Notice(notice)).is_err() {
                    return;
                }
            }
        });
    if reader.is_err() || relay.is_err() {
        return;
    }
    run(&inputs, &mut output, &mut backend);
}

/// Reads watch-set lines until input ends; a line that is not a watch set is skipped.
fn read_sets(mut input: impl BufRead, sender: &Sender<Input>) {
    loop {
        let mut line = Vec::new();
        match (&mut input)
            .take(MAXIMUM_WATCH_SET_LINE_BYTES)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => {
                let _closed = sender.send(Input::Closed);
                return;
            }
            Ok(_) => {}
        }
        if line.last() != Some(&b'\n') {
            let mut rest = Vec::new();
            if input
                .read_until(b'\n', &mut rest)
                .map_or(true, |read| read == 0)
            {
                let _closed = sender.send(Input::Closed);
                return;
            }
            continue;
        }
        if let Ok(mut set) = serde_json::from_slice::<WatchSet>(&line) {
            set.roots.truncate(MAXIMUM_WATCHED_ROOTS);
            if sender.send(Input::Set(set)).is_err() {
                return;
            }
        }
    }
}

fn run(inputs: &Receiver<Input>, output: &mut impl Write, backend: &mut impl Backend) {
    let mut coalescer = Coalescer::default();
    let mut states: BTreeMap<String, WorkspaceFilesWatchState> = BTreeMap::new();
    let mut flush_at: Option<Instant> = None;
    let mut idle_since = Some(Instant::now());
    loop {
        let wait = match (flush_at, idle_since) {
            (Some(at), _) => at.saturating_duration_since(Instant::now()),
            (None, Some(since)) => IDLE_EXIT.saturating_sub(since.elapsed()),
            (None, None) => Duration::from_secs(60),
        };
        let mut reports = Vec::new();
        match inputs.recv_timeout(wait) {
            Ok(Input::Set(set)) => {
                coalescer.replace(&set.roots);
                idle_since = set.roots.is_empty().then(Instant::now);
                let mut next = BTreeMap::new();
                for root in &set.roots {
                    next.insert(root.root.clone(), WorkspaceFilesWatchState::Watching);
                }
                for (root, state) in backend.watch(&set.roots) {
                    next.insert(root, state);
                }
                for (root, state) in &next {
                    if states.get(root) != Some(state) {
                        reports.push(WatchEvent::State {
                            root: root.clone(),
                            state: *state,
                        });
                    }
                }
                states = next;
            }
            Ok(Input::Notice(notice)) => {
                if let Notice::Lost(path) = &notice {
                    for root in coalescer.named(path) {
                        if let Some(state) = states.get_mut(root)
                            && *state != WorkspaceFilesWatchState::Unavailable
                        {
                            *state = WorkspaceFilesWatchState::Unavailable;
                            reports.push(WatchEvent::State {
                                root: root.to_owned(),
                                state: *state,
                            });
                        }
                    }
                }
                coalescer.record(&notice);
                flush_at.get_or_insert_with(|| Instant::now() + COALESCE);
            }
            Ok(Input::Closed) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                if flush_at.is_some_and(|at| at <= Instant::now()) {
                    flush_at = None;
                    reports.extend(coalescer.drain());
                } else if idle_since.is_some_and(|since| since.elapsed() >= IDLE_EXIT) {
                    return;
                }
            }
        }
        for report in reports {
            let Ok(mut line) = serde_json::to_vec(&report) else {
                continue;
            };
            line.push(b'\n');
            if output
                .write_all(&line)
                .and_then(|()| output.flush())
                .is_err()
            {
                return;
            }
        }
    }
}

/// The watch lane process's work: serves `input` until it closes, following changes with the
/// platform's own watcher.
pub fn serve_platform_watch(input: impl BufRead + Send + 'static, output: impl Write) {
    serve_watch(input, output, platform::Platform::start);
}

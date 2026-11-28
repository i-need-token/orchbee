//! One Workspace's repositories, as a pure state owner: callers pass the time and each event and
//! carry out the returned effects. It owns the display set (at most 256 repositories, root first),
//! the warm pool (the 3 repositories most recently left behind, which keep their last state and
//! refresh once when they return), each repository's refresh schedule, its latest status and the
//! summary every subscriber receives. The summary lists the display set: a repository is warm once
//! a status of it was read, and stale while a newer one is read or after Agent completion found it
//! in the warm pool.
//!
//! Discovery follows a root change after 500 ms. A repository above the root, or holding a file
//! opened outside it, is displayed only with consent and is otherwise a candidate; one the user
//! closed is neither until consented again. Watches and automatic refreshes run only while a Client
//! subscribes, and a new display set's watches start before the old set's stop.

use super::{
    discovery::{Discovered, Found},
    errors::{GitError, GitErrorCode},
    json::escaped_len,
    native_paths,
    porcelain::Entry,
    ports::Change,
    refresh::{self, Refresh},
    status::{Counts, Head, Snapshot, Upstream},
};
use orchbee_files_protocol::{
    BusyOperation, ConnectionId, ErrorCode, RepositoryKind, RepositoryOperation,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

const DISPLAY_LIMIT: usize = 256;
const WARM_LIMIT: usize = 3;
const DISCOVERY_DEBOUNCE: Duration = Duration::from_millis(500);
/// A status page holds at most this many entries...
const PAGE_ENTRIES: usize = 2000;
/// ...and about this many bytes once encoded, inside a 128 KiB result.
const PAGE_BYTES: usize = 120 * 1024;
/// What an entry's JSON takes beside its paths: its letters, flag, field names and separator...
const ENTRY_OVERHEAD: usize = 46;
/// ...and what the source of a rename or copy adds beside its path.
const ORIGINAL_OVERHEAD: usize = 14;

/// What the caller carries out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Effect {
    /// Discover the repositories of `root`; the result returns through [`Workspace::discovered`].
    Discover {
        generation: u64,
        root: String,
    },
    /// Run status; its end returns through [`Workspace::status_ended`].
    Status {
        repository: String,
        generation: u64,
    },
    CancelStatus {
        repository: String,
        generation: u64,
    },
    /// Watch the repository's working tree and the first level of its Git directory.
    Watch {
        repository: String,
    },
    Unwatch {
        repository: String,
    },
    /// The summary changed: send it to every subscriber.
    Publish,
}

/// One repository the Workspace displays or keeps warm.
#[derive(Debug)]
pub(super) struct Repository {
    pub(super) found: Found,
    pub(super) kind: RepositoryKind,
    refresh: Refresh,
    pub(super) snapshot: Option<Arc<Snapshot>>,
    /// Counts completed statuses; 0 until the first.
    pub(super) revision: u64,
    /// Writes queued or running.
    writes: usize,
    /// The write that runs.
    busy: Option<BusyOperation>,
    /// In the warm pool rather than the display set.
    warm: bool,
    /// Agent completion found it in the warm pool; its next status clears the mark.
    stale: bool,
}

impl Repository {
    fn new(found: Found, kind: RepositoryKind) -> Self {
        Self {
            found,
            kind,
            refresh: Refresh::new(),
            snapshot: None,
            revision: 0,
            writes: 0,
            busy: None,
            warm: false,
            stale: false,
        }
    }
}

/// The SCM summary of one Workspace.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Summary {
    pub(super) repositories: Vec<RepositorySummary>,
    pub(super) candidates: Vec<String>,
    pub(super) unsafe_paths: Vec<String>,
    pub(super) repositories_truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RepositorySummary {
    pub(super) repository: String,
    pub(super) kind: RepositoryKind,
    pub(super) head: Head,
    pub(super) upstream: Option<Upstream>,
    pub(super) operation: RepositoryOperation,
    pub(super) counts: Counts,
    pub(super) truncated: bool,
    pub(super) revision: u64,
    pub(super) busy: Option<BusyOperation>,
    pub(super) warm: bool,
    pub(super) stale: bool,
}

/// One status entry as a page carries it: absolute native paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PageEntry {
    pub(super) x: char,
    pub(super) y: char,
    pub(super) path: String,
    pub(super) original: Option<String>,
    pub(super) submodule: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StatusPage {
    pub(super) entries: Vec<PageEntry>,
    pub(super) cursor: Option<u64>,
}

#[derive(Debug, Default)]
struct Discovery {
    due: Option<Instant>,
    running: Option<u64>,
    generation: u64,
    /// The root changed while a discovery ran: discover again when it ends.
    again: bool,
}

#[derive(Debug, Default)]
pub(super) struct Workspace {
    root: Option<String>,
    discovery: Discovery,
    last: Discovered,
    /// Repositories of files opened outside the root.
    files: Vec<Found>,
    consents: HashSet<String>,
    closed: HashSet<String>,
    repositories: Vec<Repository>,
    /// Displayed repositories in display order.
    displayed: Vec<String>,
    /// Warm repositories, least recently left first.
    warm: VecDeque<String>,
    candidates: Vec<String>,
    truncated: bool,
    /// Subscriptions and the connection each belongs to.
    subscribers: HashMap<u64, ConnectionId>,
    /// Connections whose window has focus on this Workspace.
    focused: HashSet<ConnectionId>,
}

impl Workspace {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Forgets everything the Workspace showed and every subscription, as its closing does,
    /// keeping only which Clients have focus for when it opens again.
    pub(super) fn reset(&mut self) {
        let focused = std::mem::take(&mut self.focused);
        *self = Self::new();
        self.focused = focused;
    }

    /// The root changed; `None` for a Workspace without one. Discovery follows after 500 ms.
    pub(super) fn set_root(&mut self, now: Instant, root: Option<String>) -> Vec<Effect> {
        if self.root == root {
            return Vec::new();
        }
        self.root = root;
        if self.root.is_some() {
            self.rediscover(now);
            return Vec::new();
        }
        self.discovery.due = None;
        self.last = Discovered::default();
        self.recompute(now)
    }

    /// Discovers again after the debounce, as after `git init` or a repository vanishing.
    pub(super) fn rediscover(&mut self, now: Instant) {
        if self.discovery.running.is_some() {
            self.discovery.again = true;
        } else {
            self.discovery.due = Some(now + DISCOVERY_DEBOUNCE);
        }
    }

    pub(super) fn discovered(
        &mut self,
        now: Instant,
        generation: u64,
        discovered: Discovered,
    ) -> Vec<Effect> {
        if self.discovery.running != Some(generation) {
            return Vec::new();
        }
        self.discovery.running = None;
        if std::mem::take(&mut self.discovery.again) {
            self.discovery.due = Some(now);
        }
        self.last = discovered;
        let mut effects = self.recompute(now);
        effects.extend(self.wake(now));
        effects
    }

    /// The repository that holds a file opened outside the root, found by probing its folder.
    pub(super) fn file_opened(&mut self, now: Instant, found: Found) -> Vec<Effect> {
        let inside_root = self
            .root
            .as_deref()
            .is_some_and(|root| native_paths::relative(root, &found.toplevel).is_some());
        if inside_root
            || self
                .files
                .iter()
                .any(|file| native_paths::same(&file.toplevel, &found.toplevel))
        {
            return Vec::new();
        }
        self.files.push(found);
        self.recompute(now)
    }

    /// The persisted consents changed, here or in another process.
    pub(super) fn set_consents(&mut self, now: Instant, consents: HashSet<String>) -> Vec<Effect> {
        if self.consents == consents {
            return Vec::new();
        }
        self.consents = consents;
        self.recompute(now)
    }

    /// `repository.consent`: displays a candidate, or reopens a closed repository.
    pub(super) fn consent(&mut self, now: Instant, repository: &str) -> Vec<Effect> {
        self.closed
            .retain(|closed| !native_paths::same(closed, repository));
        self.consents.insert(repository.to_owned());
        self.recompute(now)
    }

    /// `repository.close`: neither displayed nor a candidate until consented again.
    pub(super) fn close(&mut self, now: Instant, repository: &str) -> Vec<Effect> {
        self.consents
            .retain(|consented| !native_paths::same(consented, repository));
        self.closed.insert(repository.to_owned());
        self.recompute(now)
    }

    pub(super) fn subscribe(
        &mut self,
        now: Instant,
        subscription: u64,
        connection: ConnectionId,
    ) -> Vec<Effect> {
        let was = self.subscribed();
        self.subscribers.insert(subscription, connection);
        self.subscription_changed(now, was)
    }

    pub(super) fn unsubscribe(&mut self, now: Instant, subscription: u64) -> Vec<Effect> {
        let was = self.subscribed();
        self.subscribers.remove(&subscription);
        self.subscription_changed(now, was)
    }

    /// A Client connection ended with everything it held.
    pub(super) fn connection_lost(
        &mut self,
        now: Instant,
        connection: ConnectionId,
    ) -> Vec<Effect> {
        let was = self.subscribed();
        self.subscribers.retain(|_, holder| *holder != connection);
        self.focused.remove(&connection);
        let mut effects = self.subscription_changed(now, was);
        effects.extend(self.focus_changed(now));
        effects
    }

    /// `client.focus` from one connection.
    pub(super) fn focus(
        &mut self,
        now: Instant,
        connection: ConnectionId,
        focused: bool,
    ) -> Vec<Effect> {
        if focused {
            self.focused.insert(connection);
        } else {
            self.focused.remove(&connection);
        }
        self.focus_changed(now)
    }

    /// A watched file changed in `repository`.
    pub(super) fn changed(
        &mut self,
        now: Instant,
        repository: &str,
        change: Change,
    ) -> Vec<Effect> {
        let Some(index) = self.displayed_index(repository) else {
            return Vec::new();
        };
        let effects = self.repositories[index].refresh.changed(now, change);
        self.translate(index, effects)
    }

    /// An explicit refresh of one displayed repository, or of all of them.
    pub(super) fn refresh(&mut self, now: Instant, repository: Option<&str>) -> Vec<Effect> {
        let mut effects = Vec::new();
        for index in self.displayed_indices() {
            if repository.is_none_or(|path| {
                native_paths::same(path, &self.repositories[index].found.toplevel)
            }) {
                let started = self.repositories[index].refresh.refresh(now);
                effects.extend(self.translate(index, started));
            }
        }
        effects
    }

    /// An Agent in the Workspace settled: every displayed repository refreshes at once, and every
    /// warm one is marked stale.
    pub(super) fn agent_settled(&mut self, now: Instant) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut marked = false;
        for repository in &mut self.repositories {
            if repository.warm && !repository.stale {
                repository.stale = true;
                marked = true;
            }
        }
        if marked {
            effects.push(Effect::Publish);
        }
        if self.subscribed() {
            effects.extend(self.refresh(now, None));
        }
        effects
    }

    /// A write was queued for `repository`; false when the Workspace has no such repository.
    pub(super) fn write_queued(&mut self, repository: &str) -> bool {
        let Some(index) = self.index(repository) else {
            return false;
        };
        let entry = &mut self.repositories[index];
        entry.writes += 1;
        entry.refresh.write_started();
        true
    }

    /// The write queued first for `repository` started running.
    pub(super) fn write_running(&mut self, repository: &str, busy: Option<BusyOperation>) {
        if let Some(index) = self.index(repository) {
            self.repositories[index].busy = busy;
        }
    }

    /// A write queued for `repository` ended, whether it ran or not.
    pub(super) fn write_ended(&mut self, now: Instant, repository: &str) -> Vec<Effect> {
        let Some(index) = self.index(repository) else {
            return Vec::new();
        };
        let entry = &mut self.repositories[index];
        entry.writes = entry.writes.saturating_sub(1);
        entry.busy = None;
        let effects = entry.refresh.write_ended(now);
        let mut effects = self.translate(index, effects);
        effects.push(Effect::Publish);
        effects
    }

    pub(super) fn status_ended(
        &mut self,
        now: Instant,
        repository: &str,
        generation: u64,
        result: Result<Snapshot, GitError>,
    ) -> Vec<Effect> {
        let Some(index) = self.index(repository) else {
            return Vec::new();
        };
        if self.repositories[index].refresh.running() != Some(generation) {
            return Vec::new();
        }
        let truncated = result.as_ref().ok().map(|snapshot| snapshot.truncated);
        let started = self.repositories[index]
            .refresh
            .status_ended(now, generation, truncated);
        let mut effects = self.translate(index, started);
        match result {
            Ok(snapshot) => {
                let entry = &mut self.repositories[index];
                entry.snapshot = Some(Arc::new(snapshot));
                entry.revision += 1;
                entry.stale = false;
                effects.push(Effect::Publish);
            }
            Err(error)
                if error.git_error_code == Some(GitErrorCode::NotAGitRepository)
                    || error.code == ErrorCode::UnsafeRepository =>
            {
                self.rediscover(now);
            }
            Err(_) => {}
        }
        effects
    }

    /// The timers due at `now` fired.
    pub(super) fn wake(&mut self, now: Instant) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.discovery.running.is_none()
            && self.discovery.due.is_some_and(|due| due <= now)
            && let Some(root) = self.root.clone()
        {
            self.discovery.due = None;
            self.discovery.generation += 1;
            self.discovery.running = Some(self.discovery.generation);
            effects.push(Effect::Discover {
                generation: self.discovery.generation,
                root,
            });
        }
        for index in 0..self.repositories.len() {
            let woken = self.repositories[index].refresh.wake(now);
            effects.extend(self.translate(index, woken));
        }
        effects
    }

    pub(super) fn next_wake(&self) -> Option<Instant> {
        let discovery = self
            .discovery
            .due
            .filter(|_| self.discovery.running.is_none() && self.root.is_some());
        self.repositories
            .iter()
            .filter_map(|repository| repository.refresh.next_wake())
            .chain(discovery)
            .min()
    }

    /// Whether some Client subscribes to the summary.
    pub(super) fn subscribed(&self) -> bool {
        !self.subscribers.is_empty()
    }

    /// A displayed or warm repository.
    pub(super) fn repository(&self, path: &str) -> Option<&Repository> {
        self.index(path).map(|index| &self.repositories[index])
    }

    /// Whether `path` is a displayed repository.
    pub(super) fn displays(&self, path: &str) -> bool {
        self.displayed_index(path).is_some()
    }

    /// The toplevels of the displayed repositories a consent admitted, whose working trees are in
    /// scope while they stay displayed.
    pub(super) fn consented(&self) -> Vec<String> {
        self.repositories
            .iter()
            .filter(|repository| {
                !repository.warm
                    && matches!(
                        repository.kind,
                        RepositoryKind::Parent | RepositoryKind::File
                    )
            })
            .map(|repository| repository.found.toplevel.clone())
            .collect()
    }

    /// Whether `path` is a candidate the user may consent to, or a closed repository to reopen.
    pub(super) fn consentable(&self, path: &str) -> bool {
        self.candidates
            .iter()
            .chain(self.closed.iter())
            .any(|candidate| native_paths::same(candidate, path))
    }

    /// Whether `path` is listed as unsafe.
    pub(super) fn lists_unsafe(&self, path: &str) -> bool {
        self.last
            .unsafe_paths
            .iter()
            .any(|listed| native_paths::same(listed, path))
    }

    pub(super) fn root(&self) -> Option<&str> {
        self.root.as_deref()
    }

    pub(super) fn summary(&self) -> Summary {
        let summarize = |repository: &Repository| {
            let snapshot = repository.snapshot.as_deref();
            RepositorySummary {
                repository: repository.found.toplevel.clone(),
                kind: repository.kind,
                head: snapshot
                    .map(|snapshot| snapshot.head.clone())
                    .unwrap_or_default(),
                upstream: snapshot.and_then(|snapshot| snapshot.upstream.clone()),
                operation: snapshot
                    .map_or(RepositoryOperation::None, |snapshot| snapshot.operation),
                counts: snapshot.map(|snapshot| snapshot.counts).unwrap_or_default(),
                truncated: snapshot.is_some_and(|snapshot| snapshot.truncated),
                revision: repository.revision,
                busy: repository.busy,
                warm: snapshot.is_some(),
                stale: repository.stale || repository.refresh.running().is_some(),
            }
        };
        let repositories = self
            .displayed
            .iter()
            .filter_map(|path| self.repository(path))
            .map(summarize)
            .collect();
        Summary {
            repositories,
            candidates: self.candidates.clone(),
            unsafe_paths: self.last.unsafe_paths.clone(),
            repositories_truncated: self.truncated,
        }
    }

    /// The page of `repository`'s status `revision` that starts at entry `cursor`.
    pub(super) fn page(
        &self,
        repository: &str,
        revision: u64,
        cursor: u64,
    ) -> Result<StatusPage, GitError> {
        let entry = self
            .repository(repository)
            .ok_or(GitError::new(ErrorCode::NotFound))?;
        let Some(snapshot) = entry
            .snapshot
            .as_deref()
            .filter(|_| entry.revision == revision)
        else {
            return Err(GitError::new(ErrorCode::StaleRevision));
        };
        let start = usize::try_from(cursor).unwrap_or(usize::MAX);
        let mut entries = Vec::new();
        let mut bytes = 0;
        let mut next = start;
        for porcelain in snapshot.entries.iter().skip(start) {
            let entry = page_entry(&entry.found.toplevel, porcelain);
            let size = ENTRY_OVERHEAD
                + escaped_len(&entry.path)
                + entry
                    .original
                    .as_deref()
                    .map_or(0, |original| ORIGINAL_OVERHEAD + escaped_len(original));
            if entries.len() == PAGE_ENTRIES || (!entries.is_empty() && bytes + size > PAGE_BYTES) {
                break;
            }
            bytes += size;
            entries.push(entry);
            next += 1;
        }
        let cursor = (next < snapshot.entries.len()).then_some(next as u64);
        Ok(StatusPage { entries, cursor })
    }

    fn subscription_changed(&mut self, now: Instant, was: bool) -> Vec<Effect> {
        let subscribed = self.subscribed();
        if subscribed == was {
            return Vec::new();
        }
        let mut effects = Vec::new();
        for index in self.displayed_indices() {
            let path = self.repositories[index].found.toplevel.clone();
            effects.push(if subscribed {
                Effect::Watch { repository: path }
            } else {
                Effect::Unwatch { repository: path }
            });
            let mut changed = self.repositories[index]
                .refresh
                .set_subscribed(now, subscribed);
            if subscribed {
                changed.extend(self.repositories[index].refresh.refresh(now));
            }
            effects.extend(self.translate(index, changed));
        }
        effects.extend(self.focus_changed(now));
        effects
    }

    fn focus_changed(&mut self, now: Instant) -> Vec<Effect> {
        let focused = self
            .subscribers
            .values()
            .any(|connection| self.focused.contains(connection));
        let mut effects = Vec::new();
        for index in self.displayed_indices() {
            let changed = self.repositories[index].refresh.set_focused(now, focused);
            effects.extend(self.translate(index, changed));
        }
        effects
    }

    /// Rebuilds the display set and the candidates from the last discovery, the repositories of
    /// opened files, the consents and the closed repositories.
    fn recompute(&mut self, now: Instant) -> Vec<Effect> {
        let closed = |path: &str| {
            self.closed
                .iter()
                .any(|closed| native_paths::same(closed, path))
        };
        let consented = |path: &str| {
            self.consents
                .iter()
                .any(|consent| native_paths::same(consent, path))
        };
        let mut next: Vec<(RepositoryKind, Found)> = Vec::new();
        let mut candidates = Vec::new();
        let local = self
            .last
            .root
            .iter()
            .map(|found| (RepositoryKind::Root, found))
            .chain(
                self.last
                    .nested
                    .iter()
                    .map(|found| (RepositoryKind::Nested, found)),
            );
        let outside = self
            .last
            .parent
            .iter()
            .map(|found| (RepositoryKind::Parent, found))
            .chain(self.files.iter().map(|found| (RepositoryKind::File, found)));
        for (kind, found) in local {
            if !closed(&found.toplevel) {
                next.push((kind, found.clone()));
            }
        }
        for (kind, found) in outside {
            let listed = next
                .iter()
                .any(|(_, shown)| native_paths::same(&shown.toplevel, &found.toplevel))
                || candidates
                    .iter()
                    .any(|candidate: &String| native_paths::same(candidate, &found.toplevel));
            if listed || closed(&found.toplevel) {
                continue;
            }
            if consented(&found.toplevel) {
                next.push((kind, found.clone()));
            } else {
                candidates.push(found.toplevel.clone());
            }
        }
        self.truncated = next.len() > DISPLAY_LIMIT;
        next.truncate(DISPLAY_LIMIT);
        self.candidates = candidates;

        let subscribed = self.subscribed();
        let mut watches = Vec::new();
        let mut unwatches = Vec::new();
        let mut started = Vec::new();
        let mut displayed = Vec::with_capacity(next.len());
        for (kind, found) in next {
            let path = found.toplevel.clone();
            displayed.push(path.clone());
            let index = if let Some(index) = self.index(&path) {
                let entry = &mut self.repositories[index];
                entry.kind = kind;
                entry.found = found;
                if !entry.warm {
                    continue;
                }
                entry.warm = false;
                self.warm.retain(|warm| !native_paths::same(warm, &path));
                index
            } else {
                self.repositories.push(Repository::new(found, kind));
                self.repositories.len() - 1
            };
            if subscribed {
                watches.push(Effect::Watch { repository: path });
                let entry = &mut self.repositories[index];
                let mut changed = entry.refresh.set_subscribed(now, true);
                changed.extend(entry.refresh.refresh(now));
                started.push((index, changed));
            }
        }
        for path in std::mem::replace(&mut self.displayed, displayed) {
            if self
                .displayed
                .iter()
                .any(|shown| native_paths::same(shown, &path))
            {
                continue;
            }
            let Some(index) = self.index(&path) else {
                continue;
            };
            let entry = &mut self.repositories[index];
            entry.warm = true;
            entry.refresh.set_subscribed(now, false);
            if subscribed {
                unwatches.push(Effect::Unwatch {
                    repository: path.clone(),
                });
            }
            self.warm.push_back(path);
        }
        let mut effects = watches;
        for (index, changed) in started {
            effects.extend(self.translate(index, changed));
        }
        effects.extend(unwatches);
        effects.extend(self.evict());
        effects.extend(self.focus_changed(now));
        effects.push(Effect::Publish);
        effects
    }

    /// Drops the least recently left warm repositories past the pool's size; one with a write
    /// still queued stays until the write ends.
    fn evict(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut kept = VecDeque::new();
        let mut excess = self.warm.len().saturating_sub(WARM_LIMIT);
        while let Some(path) = self.warm.pop_front() {
            let busy = self.repository(&path).is_some_and(|entry| entry.writes > 0);
            if excess == 0 || busy {
                kept.push_back(path);
                continue;
            }
            excess -= 1;
            if let Some(index) = self.index(&path) {
                let entry = self.repositories.remove(index);
                if let Some(generation) = entry.refresh.running() {
                    effects.push(Effect::CancelStatus {
                        repository: path,
                        generation,
                    });
                }
            }
        }
        self.warm = kept;
        effects
    }

    fn translate(&self, index: usize, effects: Vec<refresh::Effect>) -> Vec<Effect> {
        let repository = &self.repositories[index].found.toplevel;
        effects
            .into_iter()
            .map(|effect| match effect {
                refresh::Effect::Start(generation) => Effect::Status {
                    repository: repository.clone(),
                    generation,
                },
                refresh::Effect::Cancel(generation) => Effect::CancelStatus {
                    repository: repository.clone(),
                    generation,
                },
            })
            .collect()
    }

    fn index(&self, path: &str) -> Option<usize> {
        self.repositories
            .iter()
            .position(|repository| native_paths::same(&repository.found.toplevel, path))
    }

    fn displayed_index(&self, path: &str) -> Option<usize> {
        self.index(path)
            .filter(|index| !self.repositories[*index].warm)
    }

    fn displayed_indices(&self) -> Vec<usize> {
        (0..self.repositories.len())
            .filter(|index| !self.repositories[*index].warm)
            .collect()
    }
}

/// A porcelain entry with absolute native paths. A name that is not UTF-8 reads with replacement
/// characters, as VS Code shows it.
fn page_entry(toplevel: &str, entry: &Entry) -> PageEntry {
    let text = |bytes: &[u8]| native_paths::join(toplevel, &String::from_utf8_lossy(bytes));
    PageEntry {
        x: char::from(entry.x),
        y: char::from(entry.y),
        path: text(&entry.path),
        original: entry.original.as_deref().map(text),
        submodule: entry.submodule,
    }
}

#[cfg(test)]
mod tests;

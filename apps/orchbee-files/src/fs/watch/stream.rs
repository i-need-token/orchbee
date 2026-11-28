//! One Workspace's delivery of watched changes to the Clients subscribed to it. A Client receives
//! the changes of entries in its loaded folders and of its opened files, as the frontend's model
//! backend delivers them; a Client that loaded more folders than are tracked receives every change
//! inside the root. A deletion of an opened file, or of a folder holding one, holds the file's
//! deletion for 100 ms, absorbing its other changes, and then reports it by whether the file
//! exists again. Each Client's changes gather for 75 ms from the first, coalesce as VS Code
//! coalesces them, and pass through VS Code's throttle, whose overflow becomes a rescan of the
//! loaded folders whose changes it dropped. Changes the native watcher lost, and those that went
//! unreported before a watch covered their folder, become a rescan of the loaded folders they were
//! lost in. Updates hold at most 500 items and fit the Notice record.
//! Time is the caller's, so every rule holds under any clock.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::{Duration, Instant},
};

use super::{
    coalesce::{self, Paths},
    throttle::Throttle,
};
use crate::scope::path::{self, NativePath};
use orchbee_files_protocol::{
    Change as WireChange, ChangeKind, ConnectionId, FilePath, FilesNotice, FsUpdate, List,
    MAXIMUM_REPLY_BYTES, Separator, encode_notice,
};

/// How long a Client's changes gather before they coalesce.
const WINDOW: Duration = Duration::from_millis(75);
/// How long a deletion of an opened file waits before its file is checked.
const HOLD: Duration = Duration::from_millis(100);
/// Items in one Update.
const MAXIMUM_UPDATE_ITEMS: usize = 500;

/// One change of one item, as the native watcher reports it and as Clients receive it.
pub(super) type Change = coalesce::Event<NativePath>;

/// What each subscribed Client of the Workspace loaded and opened.
#[derive(Default)]
pub(super) struct Interest {
    /// The Workspace's root while it is present.
    pub(super) root: Option<NativePath>,
    pub(super) holders: HashMap<ConnectionId, Holder>,
    /// Every holder's opened files, by [`NativePath::key`].
    pub(super) opened: HashMap<String, NativePath>,
}

/// What one Client loaded and opened in the Workspace.
#[derive(Default)]
pub(super) struct Holder {
    /// Its loaded folders, by [`NativePath::key`].
    pub(super) loaded: HashMap<String, NativePath>,
    /// It loaded more folders than are tracked, so every change inside the root reaches it.
    pub(super) everything: bool,
    /// Its opened files, by [`NativePath::key`].
    pub(super) opened: HashSet<String>,
}

/// The changes on their way to the Workspace's Clients.
#[derive(Default)]
pub(super) struct Stream {
    holds: HashMap<String, Hold>,
    windows: HashMap<ConnectionId, Window>,
}

struct Hold {
    path: NativePath,
    due: Instant,
    /// The caller is checking whether the file exists.
    checking: bool,
}

struct Window {
    gathered: Vec<Change>,
    /// Folders to list again, by [`NativePath::key`].
    rescan: BTreeMap<String, NativePath>,
    /// When the gathered changes coalesce; absent while none gather.
    flush: Option<Instant>,
    throttle: Throttle<Change>,
}

/// What one Client receives at once.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Delivery {
    pub(super) connection: ConnectionId,
    pub(super) changes: Vec<Change>,
    pub(super) rescan: Vec<NativePath>,
}

/// The stream's work that came due.
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Due {
    /// Held files whose existence the caller checks, answering with [`Stream::resolved`].
    pub(super) checks: Vec<NativePath>,
    pub(super) deliveries: Vec<Delivery>,
}

impl Stream {
    /// Takes one change the native watcher reported at `now`.
    pub(super) fn observe(&mut self, change: Change, interest: &Interest, now: Instant) {
        let key = change.path.key();
        if self.holds.contains_key(&key) {
            return;
        }
        if change.kind == ChangeKind::Deleted {
            let below: Vec<NativePath> = interest
                .opened
                .iter()
                .filter(|(opened, file)| {
                    **opened != key
                        && file.is_within(&change.path)
                        && !self.holds.contains_key(*opened)
                })
                .map(|(_, file)| file.clone())
                .collect();
            for file in below {
                self.hold(file, now);
            }
            if interest.opened.contains_key(&key) {
                self.hold(change.path, now);
                return;
            }
        }
        self.route(change, interest, now);
    }

    /// Answers a check that [`Self::tick`] asked for: the held file exists, as a directory or not,
    /// or it does not.
    pub(super) fn resolved(
        &mut self,
        file: &NativePath,
        exists: Option<bool>,
        interest: &Interest,
        now: Instant,
    ) {
        let Some(hold) = self.holds.remove(&file.key()) else {
            return;
        };
        let (kind, directory) = match exists {
            Some(directory) => (ChangeKind::Changed, directory),
            None => (ChangeKind::Deleted, false),
        };
        let change = Change {
            kind,
            path: hold.path,
            directory,
        };
        self.route(change, interest, now);
    }

    /// Asks each Client to list again its loaded folders at or below `under`, or all of them,
    /// whose changes the native watcher lost.
    pub(super) fn lost(&mut self, under: Option<&NativePath>, interest: &Interest, now: Instant) {
        self.rescan(interest, now, |folder| {
            under.is_none_or(|under| folder.is_within(under))
        });
    }

    /// Asks each Client to list again its loaded folders that `stale` picks.
    pub(super) fn rescan(
        &mut self,
        interest: &Interest,
        now: Instant,
        stale: impl Fn(&NativePath) -> bool,
    ) {
        for (connection, holder) in &interest.holders {
            let mut folders = holder
                .loaded
                .values()
                .filter(|folder| stale(folder))
                .peekable();
            if folders.peek().is_none() {
                continue;
            }
            let window = self.windows.entry(*connection).or_insert_with(Window::new);
            for folder in folders {
                window
                    .rescan
                    .entry(folder.key())
                    .or_insert_with(|| folder.clone());
            }
            window.flush.get_or_insert(now + WINDOW);
        }
    }

    /// Forgets the changes on their way to Clients no longer subscribed.
    pub(super) fn retain(&mut self, interest: &Interest) {
        self.windows
            .retain(|connection, _| interest.holders.contains_key(connection));
        if interest.holders.is_empty() {
            self.holds.clear();
        }
    }

    /// When the next work comes due.
    pub(super) fn due(&self) -> Option<Instant> {
        let holds = self
            .holds
            .values()
            .filter(|hold| !hold.checking)
            .map(|hold| hold.due);
        let windows = self
            .windows
            .values()
            .flat_map(|window| [window.flush, window.throttle.due()].into_iter().flatten());
        holds.chain(windows).min()
    }

    /// The work due at `now`.
    pub(super) fn tick(&mut self, now: Instant, interest: &Interest) -> Due {
        let mut due = Due::default();
        for hold in self.holds.values_mut() {
            if !hold.checking && hold.due <= now {
                hold.checking = true;
                due.checks.push(hold.path.clone());
            }
        }
        for (connection, window) in &mut self.windows {
            let mut changes = Vec::new();
            let mut rescan = Vec::new();
            if window.flush.is_some_and(|flush| flush <= now) {
                window.flush = None;
                let gathered = std::mem::take(&mut window.gathered);
                if !gathered.is_empty() {
                    let coalesced = coalesce::coalesce(gathered, MACHINE);
                    match window.throttle.offer(coalesced, now) {
                        Ok(chunk) => changes = chunk,
                        Err(refused) => {
                            let holder = interest.holders.get(connection);
                            for folder in dropped_folders(&refused, holder) {
                                window.rescan.entry(folder.key()).or_insert(folder);
                            }
                        }
                    }
                }
                rescan = std::mem::take(&mut window.rescan).into_values().collect();
            }
            if window.throttle.due().is_some_and(|resume| resume <= now) {
                changes.extend(window.throttle.resume(now));
            }
            if !changes.is_empty() || !rescan.is_empty() {
                due.deliveries.push(Delivery {
                    connection: *connection,
                    changes,
                    rescan,
                });
            }
        }
        self.windows
            .retain(|_, window| window.flush.is_some() || window.throttle.due().is_some());
        due
    }

    fn hold(&mut self, file: NativePath, now: Instant) {
        self.holds.entry(file.key()).or_insert(Hold {
            path: file,
            due: now + HOLD,
            checking: false,
        });
    }

    /// Gathers `change` for each Client it reaches.
    fn route(&mut self, change: Change, interest: &Interest, now: Instant) {
        let key = change.path.key();
        let parent = change.path.parent().map(|parent| parent.key());
        let inside = interest
            .root
            .as_ref()
            .is_some_and(|root| change.path.is_within(root));
        for (connection, holder) in &interest.holders {
            let reaches = holder.opened.contains(&key)
                || parent
                    .as_ref()
                    .is_some_and(|parent| holder.loaded.contains_key(parent))
                || (holder.everything && inside);
            if reaches {
                let window = self.windows.entry(*connection).or_insert_with(Window::new);
                window.gathered.push(change.clone());
                window.flush.get_or_insert(now + WINDOW);
            }
        }
    }
}

impl Window {
    const fn new() -> Self {
        Self {
            gathered: Vec::new(),
            rescan: BTreeMap::new(),
            flush: None,
            throttle: Throttle::new(),
        }
    }
}

/// How this machine's paths compare, as coalescing compares them.
const MACHINE: Paths = Paths {
    case_sensitive: path::CASE_SENSITIVE,
    separator: match path::SEPARATOR {
        Separator::Backslash => '\\',
        Separator::Slash => '/',
    },
};

/// The folders whose changes the throttle refused that the Client lists: each refused change's
/// loaded folder, once.
fn dropped_folders(refused: &[Change], holder: Option<&Holder>) -> Vec<NativePath> {
    let mut seen = HashSet::new();
    refused
        .iter()
        .filter_map(|change| change.path.parent())
        .filter(|folder| {
            holder.is_some_and(|holder| {
                holder.everything || holder.loaded.contains_key(&folder.key())
            })
        })
        .filter(|folder| seen.insert(folder.key()))
        .collect()
}

/// The Updates that carry `changes` and then `rescan`, each within the Notice record's bound. A
/// path over 4,096 bytes, which no Client can name, is left out.
pub(super) fn updates(changes: Vec<Change>, rescan: Vec<NativePath>) -> Vec<FilesNotice> {
    let changes = changes.into_iter().filter_map(|change| {
        let change = WireChange {
            kind: change.kind,
            path: FilePath::new(change.path.as_str()).ok()?,
            directory: change.directory,
        };
        let size = serde_json::to_vec(&change).map_or(MAXIMUM_REPLY_BYTES, |json| json.len());
        Some((change, size))
    });
    let rescan = rescan.iter().filter_map(|folder| {
        let path = FilePath::new(folder.as_str()).ok()?;
        let size = serde_json::to_vec(&path).map_or(MAXIMUM_REPLY_BYTES, |json| json.len());
        Some((path, size))
    });
    let mut notices = batches(changes, |changes| FsUpdate::Changes { changes });
    notices.extend(batches(rescan, |paths| FsUpdate::Rescan { paths }));
    notices
}

/// `items`, each with the length of its JSON, in Updates made by `update`, each of at most 500
/// items within the record's bound.
fn batches<T: Clone>(
    items: impl Iterator<Item = (T, usize)>,
    update: impl Fn(List<T, 1, 500>) -> FsUpdate,
) -> Vec<FilesNotice> {
    let mut notices = Vec::new();
    let mut batch: Vec<T> = Vec::new();
    // The Update's bytes around its items, measured once with the first item.
    let mut envelope = None;
    let mut bytes = 0;
    for (item, json) in items {
        // Its JSON and the comma before the next.
        let size = json + 1;
        let envelope = *envelope.get_or_insert_with(|| {
            List::new(vec![item.clone()]).map_or(0, |one| {
                (encoded_length(&update(one)) + 1).saturating_sub(size)
            })
        });
        if batch.len() == MAXIMUM_UPDATE_ITEMS
            || (!batch.is_empty() && envelope + bytes + size > MAXIMUM_REPLY_BYTES)
        {
            notices.extend(seal(std::mem::take(&mut batch), &update));
            bytes = 0;
        }
        batch.push(item);
        bytes += size;
    }
    notices.extend(seal(batch, &update));
    notices
}

fn encoded_length(update: &FsUpdate) -> usize {
    encode_notice(&FilesNotice::FsUpdate(update.clone())).len()
}

fn seal<T>(batch: Vec<T>, update: &impl Fn(List<T, 1, 500>) -> FsUpdate) -> Option<FilesNotice> {
    List::new(batch)
        .ok()
        .map(|items| FilesNotice::FsUpdate(update(items)))
}

#[cfg(test)]
mod tests;

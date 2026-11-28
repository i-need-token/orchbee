//! The Quick Open file lists this process holds: one per `cache` a connection names in a
//! Workspace, made by the cache's first query and serving every later one, as VS Code's search
//! process keeps a cache per `cacheKey`. A list goes on `search.close-cache`, when its Workspace's
//! scope changes, when its connection ends, after 10 minutes without a query, and when room is
//! needed: a Workspace keeps at most 2 lists, and the lists made total at most 256 MiB, the least
//! recently used going first. A query that names its cache with other folders, globs or flags
//! replaces the list. A list is made on a thread of its own, so a query that stops waiting leaves
//! it to the next; queries waiting for a list that goes answer `cancelled`, and a list that fails
//! is forgotten, so the next query lists again, as VS Code forgets a failed walk.

use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{CacheId, ConnectionId, ErrorCode, FilesError, WorkspaceId};

use super::{file_list::FileList, listing, rg::Launch, slots::Slots};
use crate::timer::Timer;

/// How long a list lives without a query.
const IDLE: Duration = Duration::from_secs(10 * 60);
/// The most lists one Workspace keeps: the one serving queries and one being made to replace it.
const MAXIMUM_PER_WORKSPACE: usize = 2;
/// The most the lists made take together.
const MAXIMUM_BYTES: usize = 256 * 1024 * 1024;
/// The longest a waiting query sleeps before it looks at its cancellation again.
const WAIT_SLICE: Duration = Duration::from_millis(10);

/// The list a connection names in a Workspace.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct Key {
    pub(super) connection: ConnectionId,
    pub(super) workspace: WorkspaceId,
    pub(super) cache: CacheId,
}

pub(super) struct Lists {
    shared: Arc<Shared>,
    launch: Arc<dyn Launch>,
    slots: Arc<Slots>,
}

/// The held lists and the timer that expires them. The lock is never held while a list is made
/// or scored; a panic ends the process, so a poisoned lock is never observed.
struct Shared {
    state: Mutex<State>,
    timer: Timer,
}

#[derive(Default)]
struct State {
    lists: HashMap<Key, Entry>,
    /// Counts uses, so the least recently used list is the one with the smallest count.
    clock: u64,
}

struct Entry {
    request: listing::Request,
    making: Arc<Making>,
    /// When a query last used the list.
    used: Instant,
    /// The clock at that use.
    tick: u64,
    /// What the list takes, once made.
    bytes: usize,
}

/// One list's making, which every query waiting for it shares.
struct Making {
    /// The list went, so its listing stops and its waiters answer `cancelled`.
    discarded: AtomicBool,
    made: Mutex<Option<Result<Arc<FileList>, FilesError>>>,
    changed: Condvar,
}

impl Lists {
    /// No lists yet; each is listed through `launch` within `slots`.
    pub(super) fn new(launch: Arc<dyn Launch>, slots: Arc<Slots>) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                timer: Timer::new(),
            }),
            launch,
            slots,
        }
    }

    /// The list `key` names, made by `request`, once it is made; the list is started when none is
    /// held or the one held lists something else.
    ///
    /// # Errors
    ///
    /// `cancelled` once `abandoned` holds or the list goes, and the listing's failure.
    pub(super) fn get(
        &self,
        key: Key,
        request: &listing::Request,
        abandoned: impl Fn() -> bool,
    ) -> Result<Arc<FileList>, FilesError> {
        let now = Instant::now();
        let (making, start) = self.shared.used(key, request, now);
        if start {
            self.shared.expire_at(key, &making, now + IDLE);
            self.start(key, request.clone(), Arc::clone(&making));
        }
        making.wait(&abandoned)
    }

    /// `search.close-cache`.
    pub(super) fn close(&self, key: Key) {
        self.shared.discard(|held| *held == key);
    }

    /// The scope of `workspace` changed, so its lists name files of the old root.
    pub(super) fn scope_changed(&self, workspace: WorkspaceId) {
        self.shared.discard(|held| held.workspace == workspace);
    }

    pub(super) fn connection_lost(&self, connection: ConnectionId) {
        self.shared.discard(|held| held.connection == connection);
    }

    pub(super) fn drain(&self) {
        self.shared.discard(|_| true);
    }

    /// Makes the list of `key` on a thread of its own.
    fn start(&self, key: Key, request: listing::Request, making: Arc<Making>) {
        let shared = Arc::downgrade(&self.shared);
        let (launch, slots) = (Arc::clone(&self.launch), Arc::clone(&self.slots));
        let worker = Arc::clone(&making);
        let started = thread::Builder::new()
            .name("orchbee-files-list".into())
            .spawn(move || {
                let made = listing::list(&request, launch.as_ref(), &slots, &worker.discarded)
                    .map(Arc::new);
                if let Some(shared) = shared.upgrade() {
                    shared.made(key, &worker, made.as_ref().ok());
                }
                worker.finish(made);
            });
        if started.is_err() {
            self.shared.made(key, &making, None);
            making.finish(Err(FilesError::new(ErrorCode::Failed)));
        }
    }
}

impl Shared {
    /// A query's use of the list `key` names at `now`: the making to wait for, and whether it is
    /// new and must start.
    fn used(&self, key: Key, request: &listing::Request, now: Instant) -> (Arc<Making>, bool) {
        let mut state = self.lock();
        state.clock += 1;
        let tick = state.clock;
        if let Some(entry) = state.lists.get_mut(&key)
            && entry.request == *request
        {
            entry.used = now;
            entry.tick = tick;
            return (Arc::clone(&entry.making), false);
        }
        let mut discarded: Vec<Entry> = state.lists.remove(&key).into_iter().collect();
        while state
            .lists
            .keys()
            .filter(|held| held.workspace == key.workspace)
            .count()
            >= MAXIMUM_PER_WORKSPACE
        {
            let Some(oldest) = state.oldest(|held, _| held.workspace == key.workspace) else {
                break;
            };
            discarded.extend(state.lists.remove(&oldest));
        }
        let making = Arc::new(Making::new());
        state.lists.insert(
            key,
            Entry {
                request: request.clone(),
                making: Arc::clone(&making),
                used: now,
                tick,
                bytes: 0,
            },
        );
        drop(state);
        for entry in discarded {
            entry.making.discard();
        }
        (making, true)
    }

    /// The listing of `key` by `making` ended with `made`, or failed: a made list counts against
    /// the process's bound, which evicts the least recently used others, and a failed one is
    /// forgotten.
    fn made(&self, key: Key, making: &Arc<Making>, made: Option<&Arc<FileList>>) {
        let mut state = self.lock();
        let current = state
            .lists
            .get(&key)
            .is_some_and(|entry| Arc::ptr_eq(&entry.making, making));
        if !current {
            return;
        }
        let Some(list) = made else {
            state.lists.remove(&key);
            return;
        };
        if let Some(entry) = state.lists.get_mut(&key) {
            entry.bytes = list.bytes();
        }
        let mut discarded = Vec::new();
        while state.lists.values().map(|entry| entry.bytes).sum::<usize>() > MAXIMUM_BYTES {
            let Some(oldest) = state.oldest(|held, entry| *held != key && entry.bytes > 0) else {
                break;
            };
            discarded.extend(state.lists.remove(&oldest));
        }
        drop(state);
        for entry in discarded {
            entry.making.discard();
        }
    }

    /// Looks at the list `making` makes for `key` at `at`, to drop it once it went unused for
    /// 10 minutes. Without a timer thread a list lives until it is evicted or its cache, scope or
    /// connection ends.
    fn expire_at(self: &Arc<Self>, key: Key, making: &Arc<Making>, at: Instant) {
        let (shared, making) = (Arc::downgrade(self), Arc::downgrade(making));
        let _ = self.timer.schedule(at, move || {
            if let (Some(shared), Some(making)) = (shared.upgrade(), making.upgrade()) {
                shared.expire(key, &making);
            }
        });
    }

    fn expire(self: &Arc<Self>, key: Key, making: &Arc<Making>) {
        let mut state = self.lock();
        let Some(entry) = state
            .lists
            .get(&key)
            .filter(|entry| Arc::ptr_eq(&entry.making, making))
        else {
            return;
        };
        let due = entry.used + IDLE;
        if due > Instant::now() {
            drop(state);
            self.expire_at(key, making, due);
            return;
        }
        let expired = state.lists.remove(&key);
        drop(state);
        if let Some(entry) = expired {
            entry.making.discard();
        }
    }

    /// Drops every list whose key `matches`.
    fn discard(&self, matches: impl Fn(&Key) -> bool) {
        let mut state = self.lock();
        let keys: Vec<Key> = state
            .lists
            .keys()
            .copied()
            .filter(|key| matches(key))
            .collect();
        let discarded: Vec<Entry> = keys
            .iter()
            .filter_map(|key| state.lists.remove(key))
            .collect();
        drop(state);
        for entry in discarded {
            entry.making.discard();
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    /// The least recently used list among those `eligible` admits.
    fn oldest(&self, eligible: impl Fn(&Key, &Entry) -> bool) -> Option<Key> {
        self.lists
            .iter()
            .filter(|(key, entry)| eligible(key, entry))
            .min_by_key(|(_, entry)| entry.tick)
            .map(|(key, _)| *key)
    }
}

impl Making {
    fn new() -> Self {
        Self {
            discarded: AtomicBool::new(false),
            made: Mutex::new(None),
            changed: Condvar::new(),
        }
    }

    fn finish(&self, made: Result<Arc<FileList>, FilesError>) {
        *self.made.lock().unwrap_or_else(PoisonError::into_inner) = Some(made);
        self.changed.notify_all();
    }

    fn discard(&self) {
        self.discarded.store(true, Ordering::Release);
        self.changed.notify_all();
    }

    /// The list once made; `cancelled` once `abandoned` holds or the list went, even made, since a
    /// list goes when what it lists no longer applies.
    fn wait(&self, abandoned: &impl Fn() -> bool) -> Result<Arc<FileList>, FilesError> {
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if self.discarded.load(Ordering::Acquire) || abandoned() {
                return Err(FilesError::new(ErrorCode::Cancelled));
            }
            if let Some(made) = made.as_ref() {
                return made.clone();
            }
            made = self
                .changed
                .wait_timeout(made, WAIT_SLICE)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

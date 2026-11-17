//! Durable record of live draft Machine IDs held by a hub under `root.machines()` so a hub
//! that starts after an earlier hub was killed can forget any remembered credentials written for
//! unsaved drafts, without ever listing the OS credential store.
//!
//! State updates happen in memory under the hub lock; all disk writes (`drafts.json`) and OS
//! credential-store deletes run strictly off that lock.

use super::state::Shared;
use orchbee_local_runtime::{DataRoot, PrivateCache};
use orchbee_machine_protocol::MachineId;
use orchbee_workbench_client::application::machine_credentials::{Deleted, Failure, Store};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Condvar, Mutex, Weak, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const RECORD: &str = "drafts";
const LEAF: &str = "drafts.json";
const VERSION: u64 = 1;
const MAXIMUM_BYTES: usize = 16 * 1024;
const MAXIMUM_IDS: usize = 256;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u64,
    ids: Vec<String>,
}

struct PendingState {
    generation: u64,
    written: u64,
    ids: Vec<String>,
    stop: bool,
}

/// Off-lock persistence owner for `drafts.json`. Lock order: `io` (only acquired outside the hub's
/// shared lock) -> `pending` (short-lived leaf lock, never held across I/O).
pub(super) struct LedgerPersister {
    io: Mutex<Option<PrivateCache>>,
    pending: Mutex<PendingState>,
    wake: Condvar,
}

impl LedgerPersister {
    fn new(cache: Option<PrivateCache>, initial_ids: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            io: Mutex::new(cache),
            pending: Mutex::new(PendingState {
                generation: 0,
                written: 0,
                ids: initial_ids,
                stop: false,
            }),
            wake: Condvar::new(),
        })
    }

    fn queue(&self, ids: Vec<String>) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pending.generation = pending.generation.saturating_add(1);
        pending.ids = ids;
        self.wake.notify_all();
    }

    /// Writes any queued ledger update to disk. Must NEVER be called with the hub lock held.
    pub(super) fn flush(&self) {
        let mut cache_guard = self
            .io
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            let (generation, ids) = {
                let pending = self
                    .pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if pending.written >= pending.generation {
                    return;
                }
                (pending.generation, pending.ids.clone())
            };
            write_document(cache_guard.as_mut(), &ids);
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending.written = pending.written.max(generation);
            if pending.written >= pending.generation {
                return;
            }
        }
    }

    /// Starts the background writer that drains queued updates off the hub lock.
    pub(super) fn spawn_writer(self: &Arc<Self>) -> Option<WriterHandle> {
        let persister = Arc::clone(self);
        let thread = thread::Builder::new()
            .name("orchbee-hub-draft-persist".into())
            .spawn(move || {
                persister.run_writer();
            })
            .ok()?;
        Some(WriterHandle {
            persister: Arc::clone(self),
            thread: Some(thread),
        })
    }

    fn run_writer(&self) {
        loop {
            let stop = {
                let mut pending = self
                    .pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                while !pending.stop && pending.written >= pending.generation {
                    pending = self
                        .wake
                        .wait(pending)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                pending.stop && pending.written >= pending.generation
            };
            self.flush();
            if stop {
                return;
            }
        }
    }

    fn signal_stop(&self) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pending.stop = true;
        self.wake.notify_all();
    }
}

pub(super) struct WriterHandle {
    persister: Arc<LedgerPersister>,
    thread: Option<JoinHandle<()>>,
}

impl WriterHandle {
    pub(super) fn stop_and_join(&mut self) {
        self.persister.signal_stop();
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
    }
}

impl Drop for WriterHandle {
    fn drop(&mut self) {
        self.persister.signal_stop();
    }
}

/// Tracks draft Machine IDs that may hold remembered answers across kills until promoted or
/// forgotten. Mutated in memory under the hub lock; persistence and OS credential calls happen
/// outside it.
pub(super) struct DraftLedger {
    orphans: BTreeSet<MachineId>,
    sweeping: BTreeSet<MachineId>,
    active: BTreeSet<MachineId>,
    forgetting: BTreeMap<MachineId, u8>,
    persister: Arc<LedgerPersister>,
}

impl DraftLedger {
    /// Opens the ledger in `root.machines()`, loading any IDs left behind by an earlier hub.
    pub(super) fn open(root: &DataRoot) -> Self {
        let cache = PrivateCache::acquire(&root.machines(), RECORD).ok();
        let orphans = cache
            .as_ref()
            .and_then(|cache| cache.read(LEAF, MAXIMUM_BYTES).ok().flatten())
            .map(|bytes| decode(&bytes))
            .unwrap_or_default();
        let initial_ids = orphans.iter().map(|id| id.as_str().to_owned()).collect();
        Self {
            orphans,
            sweeping: BTreeSet::new(),
            active: BTreeSet::new(),
            forgetting: BTreeMap::new(),
            persister: LedgerPersister::new(cache, initial_ids),
        }
    }

    pub(super) fn persister(&self) -> Arc<LedgerPersister> {
        Arc::clone(&self.persister)
    }

    /// Reconciles loaded startup orphans against a cleanly read catalog without blocking: any ID now
    /// saved in the catalog leaves `orphans` immediately; remaining unswept IDs are marked
    /// in-flight and returned so an off-lock worker can delete their OS credentials.
    pub(super) fn reconcile_catalog(
        &mut self,
        saved: impl Fn(&MachineId) -> bool,
    ) -> Vec<MachineId> {
        if self.orphans.is_empty() {
            return Vec::new();
        }
        let ids: Vec<MachineId> = self.orphans.iter().cloned().collect();
        let mut removed_saved = false;
        let mut to_sweep = Vec::new();
        for id in ids {
            if saved(&id) {
                if self.orphans.remove(&id) {
                    removed_saved = true;
                }
            } else if self.sweeping.insert(id.clone()) {
                to_sweep.push(id);
            }
        }
        if removed_saved {
            self.publish();
        }
        to_sweep
    }

    /// Records the off-lock sweep outcome for one orphan draft ID. Failed deletions remain in
    /// `orphans` so a later reload or hub restart retries them.
    pub(super) fn finish_sweep(&mut self, id: &MachineId, succeeded: bool) {
        self.sweeping.remove(id);
        if succeeded && self.orphans.remove(id) {
            self.publish();
        }
    }

    /// Records a newly created draft in memory and queues its persistence before login answers can
    /// be stored under its ID. Callers must flush [`Self::persister`] off lock before starting the
    /// draft's carrier.
    pub(super) fn record_draft(&mut self, id: &MachineId) {
        self.forgetting.remove(id);
        if self.active.insert(id.clone()) {
            self.publish();
        }
    }

    /// Synchronizes live active drafts after a catalog reload: saved drafts leave `active` cleanly,
    /// while any still-in-flight `forgetting` deletes stay queued until finished.
    pub(super) fn sync_live<'a>(&mut self, drafts: impl Iterator<Item = &'a MachineId>) {
        let next: BTreeSet<MachineId> = drafts.cloned().collect();
        if self.active != next {
            self.active = next;
            self.publish();
        }
    }

    /// Moves a tracked draft into `forgetting` while `Effect::Forget` deletes one `SecretKind`.
    /// Saved catalog Machines are not in `active` or `forgetting` and are ignored.
    pub(super) fn begin_forget(&mut self, id: &MachineId) {
        let was_active = self.active.remove(id);
        if was_active || self.forgetting.contains_key(id) {
            let count = self.forgetting.entry(id.clone()).or_insert(0);
            *count = count.saturating_add(1);
            if was_active {
                self.publish();
            }
        }
    }

    /// Marks one `SecretKind` deletion finished for `id`, removing `id` from the queued ledger once
    /// every pending deletion for it settles. Failed deletions keep `id` in the ledger.
    pub(super) fn finish_forget(&mut self, id: &MachineId, completed: bool) {
        let Some(remaining) = self.forgetting.get_mut(id) else {
            return;
        };
        if !completed {
            return;
        }
        if *remaining <= 1 {
            self.forgetting.remove(id);
            self.publish();
        } else {
            *remaining -= 1;
        }
    }

    /// Returns all draft and unresolved orphan IDs still tracked, without clearing them from the
    /// ledger before deletion succeeds.
    pub(super) fn pending_ids(&self) -> Vec<MachineId> {
        self.active
            .iter()
            .chain(self.forgetting.keys())
            .chain(self.orphans.iter())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Drops `succeeded` IDs after their OS credential deletions succeeded; IDs whose deletion failed
    /// or missed the shutdown bound stay on disk for the next hub to sweep.
    pub(super) fn mark_forgotten(&mut self, succeeded: &[MachineId]) {
        let mut changed = false;
        for id in succeeded {
            changed |= self.active.remove(id);
            changed |= self.forgetting.remove(id).is_some();
            changed |= self.orphans.remove(id);
            self.sweeping.remove(id);
        }
        if changed {
            self.publish();
        }
    }

    /// Spawns a background sweep thread that deletes `ids` in the OS credential store outside the
    /// hub lock and updates the ledger after each completion.
    pub(super) fn spawn_sweep(shared: Arc<Shared>, ids: Vec<MachineId>) {
        if ids.is_empty() {
            return;
        }
        let weak = Arc::downgrade(&shared);
        let batch = ids.clone();
        let started = thread::Builder::new()
            .name("orchbee-hub-draft-sweep".into())
            .spawn(move || {
                sweep_worker(weak, batch, forget_in_os_store);
            });
        if started.is_err() {
            let mut hub = shared.lock();
            for id in &ids {
                hub.drafts.finish_sweep(id, false);
            }
        }
    }

    /// Runs best-effort credential deletions for `ids` off lock within `bound`, keeping any failed
    /// or unfinished ID on disk in `drafts.json`.
    pub(super) fn forget_bounded(shared: &Shared, ids: &[MachineId], bound: Duration) {
        forget_bounded_with(shared, ids, bound, forget_in_os_store);
    }

    fn publish(&self) {
        let ids: Vec<String> = self
            .orphans
            .iter()
            .chain(self.active.iter())
            .chain(self.forgetting.keys())
            .map(|id| id.as_str().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.persister.queue(ids);
    }
}

fn sweep_worker(
    weak: Weak<Shared>,
    ids: Vec<MachineId>,
    mut forget: impl FnMut(&MachineId) -> bool,
) {
    let mut persister = None;
    for id in ids {
        let succeeded = forget(&id);
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let mut hub = shared.lock();
        hub.drafts.finish_sweep(&id, succeeded);
        persister.get_or_insert_with(|| hub.drafts.persister());
        drop(hub);
        shared.wake();
    }
    if let Some(persister) = persister {
        persister.flush();
    }
}

fn forget_bounded_with(
    shared: &Shared,
    ids: &[MachineId],
    bound: Duration,
    forget: impl Fn(&MachineId) -> bool + Send + 'static,
) {
    let persister = shared.lock().drafts.persister();
    if ids.is_empty() {
        persister.flush();
        return;
    }
    let (done_tx, done_rx) = mpsc::channel();
    let batch = ids.to_vec();
    let total = batch.len();
    let spawned = thread::Builder::new()
        .name("orchbee-hub-shutdown-forget".into())
        .spawn(move || {
            for id in batch {
                if forget(&id) && done_tx.send(id).is_err() {
                    break;
                }
            }
        });
    let mut succeeded = Vec::new();
    if spawned.is_ok() {
        let deadline = Instant::now() + bound;
        while succeeded.len() < total {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match done_rx.recv_timeout(remaining) {
                Ok(id) => succeeded.push(id),
                Err(_) => break,
            }
        }
    }
    if !succeeded.is_empty() {
        let mut hub = shared.lock();
        hub.drafts.mark_forgotten(&succeeded);
    }
    persister.flush();
}

fn write_document(cache: Option<&mut PrivateCache>, ids: &[String]) {
    let Some(cache) = cache else {
        return;
    };
    if ids.is_empty() {
        drop(cache.remove(LEAF));
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&Document {
        version: VERSION,
        ids: ids.to_vec(),
    }) && bytes.len() <= MAXIMUM_BYTES
    {
        drop(cache.replace(LEAF, &bytes));
    }
}

fn forget_in_os_store(id: &MachineId) -> bool {
    match Store::select().forget(id) {
        Ok(Deleted::Removed | Deleted::Absent) | Err(Failure::Unavailable) => true,
        Err(Failure::Failed) => false,
    }
}

fn decode(bytes: &[u8]) -> BTreeSet<MachineId> {
    let Ok(document) = serde_json::from_slice::<Document>(bytes) else {
        return BTreeSet::new();
    };
    if document.version != VERSION || document.ids.len() > MAXIMUM_IDS {
        return BTreeSet::new();
    }
    document
        .ids
        .into_iter()
        .filter_map(|raw| MachineId::new(&raw).ok())
        .collect()
}

#[cfg(test)]
mod tests;

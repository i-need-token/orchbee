//! Keeps earlier layouts over time for manual reopening. After each committed checkpoint save it
//! records that checkpoint as a history entry when its layout differs from the newest entry and
//! 15 minutes or more have passed since that entry, keeping the newest 48. It owns this policy
//! and an in-memory index of kept layouts, so a save never reads storage back; storage effects
//! go through the narrow store port, and the checkpoint save path itself is unchanged.

use orchbee_server::{RetainedLayout, WorkbenchCheckpoint};
use orchbee_workbench_protocol::{LayoutHistoryEntry, MAXIMUM_LAYOUT_HISTORY_ENTRIES, Snapshot};
use std::{
    io,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// The shortest time between two entries.
pub(crate) const INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Where entries persist. Entries are complete checkpoint payloads named by their save time in
/// Unix milliseconds; `record` also removes the oldest beyond `keep`.
pub(crate) trait LayoutHistoryStore: Send + Sync {
    /// Every kept entry, newest first; `None` marks an unreadable one.
    fn entries(&self) -> io::Result<Vec<(u64, Option<Vec<u8>>)>>;
    fn entry(&self, saved_at: u64) -> io::Result<Option<Vec<u8>>>;
    fn record(&self, saved_at: u64, payload: &[u8], keep: usize) -> io::Result<()>;
}

#[cfg(windows)]
impl LayoutHistoryStore for Mutex<orchbee_local_runtime::SessionStore> {
    fn entries(&self) -> io::Result<Vec<(u64, Option<Vec<u8>>)>> {
        Ok(self
            .lock()
            .map_err(|_| io::Error::other("checkpoint store failed"))?
            .read_layout_history()?
            .into_iter()
            .map(|entry| (entry.saved_at, entry.payload))
            .collect())
    }

    fn entry(&self, saved_at: u64) -> io::Result<Option<Vec<u8>>> {
        self.lock()
            .map_err(|_| io::Error::other("checkpoint store failed"))?
            .read_layout_history_entry(saved_at)
    }

    fn record(&self, saved_at: u64, payload: &[u8], keep: usize) -> io::Result<()> {
        self.lock()
            .map_err(|_| io::Error::other("checkpoint store failed"))?
            .record_layout_history(saved_at, payload, keep)
    }
}

struct Kept {
    saved_at: u64,
    layout: RetainedLayout,
}

pub(crate) struct LayoutHistory {
    store: Arc<dyn LayoutHistoryStore>,
    /// Readable entries, newest first.
    kept: Mutex<Vec<Kept>>,
}

impl LayoutHistory {
    /// Indexes the stored entries once. An entry whose payload no longer parses is left out of
    /// the index; it still counts toward the kept total until newer entries replace it.
    pub(crate) fn load(store: Arc<dyn LayoutHistoryStore>) -> io::Result<Self> {
        let kept = store
            .entries()?
            .into_iter()
            .filter_map(|(saved_at, payload)| {
                let layout = WorkbenchCheckpoint::parse(&payload?).ok()?.layout().ok()?;
                Some(Kept { saved_at, layout })
            })
            .collect();
        Ok(Self {
            store,
            kept: Mutex::new(kept),
        })
    }

    /// Called after a committed checkpoint save of `payload` at `now`. Returns whether an entry
    /// was written. The caller reports a failure as a diagnostic; it never fails the save.
    pub(crate) fn after_save(&self, payload: &[u8], now: SystemTime) -> io::Result<bool> {
        let now = unix_milliseconds(now)?;
        let mut kept = self
            .kept
            .lock()
            .map_err(|_| io::Error::other("layout history failed"))?;
        // The interval is checked first, so a save inside it parses nothing.
        if kept
            .first()
            .is_some_and(|newest| !elapsed(newest.saved_at, now))
        {
            return Ok(false);
        }
        let layout = WorkbenchCheckpoint::parse(payload)
            .and_then(|checkpoint| checkpoint.layout())
            .map_err(io::Error::other)?;
        if kept.first().is_some_and(|newest| newest.layout == layout) {
            return Ok(false);
        }
        self.store
            .record(now, payload, MAXIMUM_LAYOUT_HISTORY_ENTRIES)?;
        kept.insert(
            0,
            Kept {
                saved_at: now,
                layout,
            },
        );
        kept.truncate(MAXIMUM_LAYOUT_HISTORY_ENTRIES);
        Ok(true)
    }

    /// Every kept entry, newest first, compared with `current` by identity.
    pub(crate) fn summaries(&self, current: &Snapshot) -> io::Result<Vec<LayoutHistoryEntry>> {
        Ok(self
            .kept
            .lock()
            .map_err(|_| io::Error::other("layout history failed"))?
            .iter()
            .map(|kept| {
                let summary = kept.layout.summary(current);
                LayoutHistoryEntry {
                    saved_at: kept.saved_at,
                    workspaces: summary.workspaces,
                    tabs: summary.tabs,
                    workspaces_not_open: summary.workspaces_not_open,
                    tabs_not_open: summary.tabs_not_open,
                }
            })
            .collect())
    }

    /// The kept entry saved at `saved_at`, read from storage, or `None` when none is kept.
    pub(crate) fn entry(&self, saved_at: u64) -> io::Result<Option<WorkbenchCheckpoint>> {
        let indexed = self
            .kept
            .lock()
            .map_err(|_| io::Error::other("layout history failed"))?
            .iter()
            .any(|kept| kept.saved_at == saved_at);
        if !indexed {
            return Ok(None);
        }
        Ok(self
            .store
            .entry(saved_at)?
            .and_then(|payload| WorkbenchCheckpoint::parse(&payload).ok()))
    }
}

/// A clock behind the newest entry never makes a new one due, and save times stay increasing.
fn elapsed(newest: u64, now: u64) -> bool {
    now.checked_sub(newest)
        .is_some_and(|gap| u128::from(gap) >= INTERVAL.as_millis())
}

fn unix_milliseconds(now: SystemTime) -> io::Result<u64> {
    now.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| u64::try_from(since.as_millis()).ok())
        .filter(|ms| *ms > 0 && *ms <= orchbee_workbench_protocol::MAXIMUM_LAYOUT_HISTORY_SAVED_AT)
        .ok_or_else(|| io::Error::other("clock outside the layout history range"))
}

#[cfg(test)]
mod tests;

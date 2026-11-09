//! Connection-owned projection of exact physical OSC titles and its change sequence, read by the
//! Server by cursor. Never Agent semantic state, and never a decision about which title a Client
//! shows: the Server projects, paces and retains what it reads.
use orchbee_capability_node_protocol::{
    HostTerminalIdentity, MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES, MAXIMUM_TERMINAL_TITLE_CHANGES,
    TerminalTitleChangePage, TerminalTitleChangesOutcome, TerminalTitleObservation,
    TerminalTitleRow, TerminalTitlesChanged,
};
use orchbee_terminal_protocol::TerminalMetadataChanged;
use std::{
    collections::BTreeMap,
    io,
    num::NonZeroU64,
    ops::Bound::{Excluded, Unbounded},
};
const MAXIMUM_ROWS: usize = crate::terminal_authorization::MAXIMUM_RETAINED_TERMINAL_BINDINGS;
const MAXIMUM_BYTES: usize = 64 * 1024 * 1024;
/// Removals retained for readers that still hold the removed terminal. Dropping an older one
/// raises the resumable floor, so such a reader is reset instead of keeping a stale title, and
/// moves every older row to a new change, so a fresh read's next page always resumes above it.
const MAXIMUM_REMOVED: usize = MAXIMUM_ROWS;

struct Row {
    identity: HostTerminalIdentity,
    revision: NonZeroU64,
    title: Option<String>,
    change: u64,
}

/// Change-read bookkeeping for the kind-35 note: a note is due when the projection holds changes
/// after the cursor of the last answered read, and none is outstanding.
#[derive(Default)]
struct Note {
    answered: u64,
    outstanding: bool,
}

/// Every terminal key appears in `changes` once, at its latest change, and lives in exactly one
/// of `rows` or `removed`; every change is above `floor`. One Host connection serves one Worker
/// generation, so `newest` is that generation's change sequence.
#[derive(Default)]
pub(super) struct Titles {
    rows: BTreeMap<[u8; 16], Row>,
    removed: BTreeMap<[u8; 16], (HostTerminalIdentity, u64)>,
    changes: BTreeMap<u64, [u8; 16]>,
    bytes: usize,
    newest: u64,
    /// The oldest nonzero cursor a read can resume; an older one is reset. `u64::MAX` after the
    /// sequence is exhausted, when no nonzero cursor is resumable.
    floor: u64,
    note: Note,
}
impl Titles {
    pub(super) fn observe(&mut self, event: TerminalMetadataChanged) -> io::Result<()> {
        let identity = crate::terminal_host_launch_adapter::capability_identity(event.identity)
            .map_err(io::Error::other)?;
        let key = identity.terminal_id().as_bytes();
        let previous = self.rows.get(&key);
        if let Some(row) = previous {
            if row.identity != identity
                || event.revision < row.revision
                || (event.revision == row.revision && event.title != row.title)
            {
                return Err(io::Error::other(
                    "invalid physical terminal title revision or identity",
                ));
            }
            if event.revision == row.revision {
                return Ok(());
            }
        }
        let n = event.title.as_ref().map_or(0, String::len);
        if n > orchbee_capability_node_protocol::MAXIMUM_TERMINAL_TITLE_BYTES {
            return Err(io::Error::other("physical terminal title is too large"));
        }
        let old = previous
            .and_then(|row| row.title.as_ref())
            .map_or(0, String::len);
        let bytes = self
            .bytes
            .checked_sub(old)
            .and_then(|v| v.checked_add(n))
            .filter(|v| *v <= MAXIMUM_BYTES);
        if (previous.is_none() && self.rows.len() >= MAXIMUM_ROWS) || bytes.is_none() {
            return Err(io::Error::other(
                "physical terminal title capacity exhausted",
            ));
        }
        let change = self
            .newest
            .checked_add(1)
            .ok_or_else(|| io::Error::other("physical terminal title sequence exhausted"))?;
        self.forget_change(key);
        self.removed.remove(&key);
        self.rows.insert(
            key,
            Row {
                identity,
                revision: event.revision,
                title: event.title,
                change,
            },
        );
        self.changes.insert(change, key);
        self.newest = change;
        self.bytes = bytes.expect("checked title bound");
        Ok(())
    }
    #[cfg(any(windows, test))]
    pub(super) fn read(&self, identity: HostTerminalIdentity) -> TerminalTitleObservation {
        match self
            .rows
            .get(&identity.terminal_id().as_bytes())
            .filter(|row| row.identity == identity)
        {
            Some(row) => TerminalTitleObservation::Observed {
                revision: row.revision,
                title: row.title.clone(),
            },
            None => TerminalTitleObservation::Unavailable,
        }
    }
    /// Stops observing a confirmed-disposed terminal and records the removal as a change. This
    /// cannot fail: when the sequence is exhausted, no nonzero cursor stays resumable.
    pub(super) fn remove(&mut self, identity: HostTerminalIdentity) {
        let key = identity.terminal_id().as_bytes();
        if !self
            .rows
            .get(&key)
            .is_some_and(|row| row.identity == identity)
        {
            return;
        }
        self.forget_change(key);
        if let Some(row) = self.rows.remove(&key) {
            self.bytes -= row.title.as_ref().map_or(0, String::len);
        }
        let Some(change) = self.newest.checked_add(1) else {
            self.floor = u64::MAX;
            return;
        };
        self.removed.insert(key, (identity, change));
        self.changes.insert(change, key);
        self.newest = change;
        if self.removed.len() > MAXIMUM_REMOVED
            && let Some((oldest, key)) = self
                .changes
                .iter()
                .find(|(_, key)| self.removed.contains_key(*key))
                .map(|(change, key)| (*change, *key))
        {
            self.changes.remove(&oldest);
            self.removed.remove(&key);
            self.floor = self.floor.max(oldest);
            self.restamp_below_floor();
        }
    }
    /// Moves the rows older than the floor to new changes, which an incremental reader receives
    /// again unchanged. Without this, the next page of a fresh read would resume below the floor
    /// and be reset forever.
    fn restamp_below_floor(&mut self) {
        let older: Vec<(u64, [u8; 16])> = self
            .changes
            .range(..self.floor)
            .map(|(change, key)| (*change, *key))
            .collect();
        for (change, key) in older {
            let Some(next) = self.newest.checked_add(1) else {
                self.floor = u64::MAX;
                return;
            };
            self.changes.remove(&change);
            self.changes.insert(next, key);
            if let Some(row) = self.rows.get_mut(&key) {
                row.change = next;
            }
            self.newest = next;
        }
    }
    /// Answers one change read after `after` within the page bounds, and rearms the note.
    pub(super) fn changes(&mut self, after: u64) -> TerminalTitleChangesOutcome {
        if after != 0 && !self.resumable(after) {
            self.note = Note::default();
            return TerminalTitleChangesOutcome::Reset;
        }
        let fresh = after == 0;
        let mut rows = Vec::new();
        let mut bytes = 0;
        let mut next = after;
        let mut complete = true;
        for (&change, key) in self.changes.range((Excluded(after), Unbounded)) {
            let row = match (self.rows.get(key), self.removed.get(key)) {
                (Some(row), _) => TerminalTitleRow {
                    identity: row.identity,
                    observation: TerminalTitleObservation::Observed {
                        revision: row.revision,
                        title: row.title.clone(),
                    },
                },
                // A fresh reader holds nothing a removal could retract.
                (None, Some((identity, _))) if !fresh => TerminalTitleRow {
                    identity: *identity,
                    observation: TerminalTitleObservation::Unavailable,
                },
                _ => {
                    next = change;
                    continue;
                }
            };
            let n = match &row.observation {
                TerminalTitleObservation::Observed {
                    title: Some(title), ..
                } => title.len(),
                _ => 0,
            };
            if rows.len() == MAXIMUM_TERMINAL_TITLE_CHANGES
                || bytes + n > MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES
            {
                complete = false;
                break;
            }
            bytes += n;
            rows.push(row);
            next = change;
        }
        if complete {
            next = self.newest;
        }
        // An incomplete page already tells the reader to continue, so it needs no note.
        self.note = Note {
            answered: next,
            outstanding: !complete,
        };
        TerminalTitleChangesOutcome::Page(TerminalTitleChangePage {
            rows,
            next,
            complete,
        })
    }
    /// The note to write between requests, or `None` while one is outstanding or the last
    /// answered read already reached the newest change.
    pub(super) fn take_note(&mut self) -> Option<TerminalTitlesChanged> {
        if self.note.outstanding || self.newest <= self.note.answered {
            return None;
        }
        self.note.outstanding = true;
        NonZeroU64::new(self.newest).map(|through| TerminalTitlesChanged { through })
    }
    fn resumable(&self, after: u64) -> bool {
        after >= self.floor && after <= self.newest && self.floor != u64::MAX
    }
    fn forget_change(&mut self, key: [u8; 16]) {
        let change = self
            .rows
            .get(&key)
            .map(|row| row.change)
            .or_else(|| self.removed.get(&key).map(|(_, change)| *change));
        if let Some(change) = change {
            self.changes.remove(&change);
        }
    }
}

#[cfg(test)]
mod tests;

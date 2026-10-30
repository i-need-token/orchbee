//! Volatile, content-validated reading choices across viewer and context replacement.
//! This owner retains no viewer, controller, pending command, selection, or paintable window;
//! `history_restore` captures and restores choices through the ordinary viewport gates.

use crate::{
    terminal_content::{ContentSource, SnapshotMember},
    workbench_client::{
        state::{PaneRef, WorkbenchClientState},
        terminal_viewport::ReadingPosition,
    },
};

const MAXIMUM_WITNESS_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub(super) enum Choice {
    Live,
    History(ReadingPosition),
    Unavailable,
}

#[derive(Clone)]
pub(super) struct Saved {
    pub(super) source: ContentSource,
    pub(super) choice: Choice,
}

/// A bounded set of row witnesses, with small Live/unavailable choices for canonical members.
#[derive(Clone, Default)]
pub(super) struct Bank {
    entries: Vec<Saved>,
}

impl Bank {
    pub(super) fn prune(&mut self, state: &WorkbenchClientState) {
        self.entries.retain(|saved| {
            saved
                .source
                .snapshot_member(state.cache().canonical_snapshot())
                .is_some()
        });
    }

    pub(super) fn inherit_older(&mut self, older: Self, state: &WorkbenchClientState) {
        self.prune(state);
        let newer = std::mem::take(&mut self.entries);
        for saved in older.entries.into_iter().chain(newer) {
            if saved
                .source
                .snapshot_member(state.cache().canonical_snapshot())
                .is_some()
            {
                self.set(saved.source, saved.choice);
            }
        }
    }

    pub(super) fn get(&self, member: &SnapshotMember) -> Option<&Saved> {
        self.entries
            .iter()
            .find(|saved| &saved.source.member == member)
    }

    pub(super) fn set(&mut self, source: ContentSource, choice: Choice) {
        self.entries
            .retain(|saved| saved.source.member != source.member);
        self.entries.push(Saved { source, choice });
        let mut bytes = self.entries.iter().map(witness_bytes).sum::<usize>();
        for saved in &mut self.entries {
            if bytes <= MAXIMUM_WITNESS_BYTES {
                break;
            }
            let released = witness_bytes(saved);
            if released > 0 {
                saved.choice = Choice::Unavailable;
                bytes -= released;
            }
        }
    }

    pub(super) fn live(&mut self, pane: &PaneRef) {
        if let Some(saved) = self
            .entries
            .iter_mut()
            .find(|saved| &saved.source.pane == pane)
        {
            saved.choice = Choice::Live;
        }
    }
}

fn witness_bytes(saved: &Saved) -> usize {
    let Choice::History(position) = &saved.choice else {
        return 0;
    };
    position.witness.iter().fold(0_usize, |bytes, row| {
        row.cells.iter().fold(
            bytes
                .saturating_add(std::mem::size_of_val(row))
                .saturating_add(
                    row.cells.capacity()
                        * std::mem::size_of::<orchbee_terminal_protocol::TerminalTextCell>(),
                ),
            |bytes, cell| {
                bytes
                    .saturating_add(cell.text.len())
                    .saturating_add(2 * std::mem::size_of::<usize>())
            },
        )
    })
}

#[derive(Default)]
pub(super) struct Coordinator {
    pub(super) bank: Bank,
    examined: Vec<PaneRef>,
}

impl Coordinator {
    pub(super) fn projections_cleared(&mut self) {
        self.examined.clear();
    }

    /// Whether this pane incarnation already had its saved choice examined.
    pub(super) fn examined(&self, key: &PaneRef) -> bool {
        self.examined.contains(key)
    }

    pub(super) fn mark_examined(&mut self, key: PaneRef) {
        self.examined.push(key);
    }

    /// Forgets examinations of pane incarnations that left the projections.
    pub(super) fn retain_examined(&mut self, mut keep: impl FnMut(&PaneRef) -> bool) {
        self.examined.retain(|key| keep(key));
    }
}

#[cfg(test)]
mod tests;

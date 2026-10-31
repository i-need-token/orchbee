//! Volatile, content-validated reading choices across viewer and context replacement.
//! This owner retains no viewer, controller, pending command, or selection; its only paintable
//! content is a render-only image of each bookmark's last committed window. `history_restore`
//! captures and restores choices through the ordinary viewport gates.

use crate::terminal_content::{ContentSource, SnapshotMember, TerminalScreen};
use crate::terminal_viewport::{ReadingPosition, ViewportWindow};
use crate::workbench_state::{PaneRef, WorkbenchClientState};
use orchbee_terminal_protocol::TerminalTextFrame;
use std::sync::Arc;

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
    /// The committed window last painted at this bookmark. Render-only: it stands in for the
    /// live tail until restoration settles and grants no input, selection, or acknowledgement.
    image: Option<Arc<ViewportWindow>>,
}

impl Saved {
    /// The bookmark a fresh live projection may restore: same Host terminal, epoch, generation,
    /// primary screen, columns and rows, with at least the history the bookmark needs.
    pub(super) fn restorable(
        &self,
        source: &ContentSource,
        frame: &TerminalTextFrame,
    ) -> Option<&ReadingPosition> {
        let Choice::History(position) = &self.choice else {
            return None;
        };
        (self.source.host_terminal == source.host_terminal
            && self.source.rows.host_epoch == source.rows.host_epoch
            && position.generation == source.rows.generation
            && source.screen == TerminalScreen::Primary
            && position.columns == source.rows.columns
            && position.viewport_rows == frame.geometry.rows
            && source.rows.total_rows >= position.minimum_total)
            .then_some(position)
    }

    pub(super) fn image(&self) -> Option<&Arc<ViewportWindow>> {
        self.image.as_ref()
    }
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
                self.store(saved);
            }
        }
    }

    pub(super) fn get(&self, member: &SnapshotMember) -> Option<&Saved> {
        self.entries
            .iter()
            .find(|saved| &saved.source.member == member)
    }

    /// Records a choice. A History choice keeps the member's last painted image.
    pub(super) fn set(&mut self, source: ContentSource, choice: Choice) {
        let image = matches!(choice, Choice::History(_))
            .then(|| {
                self.get(&source.member)
                    .and_then(|saved| saved.image.clone())
            })
            .flatten();
        self.store(Saved {
            source,
            choice,
            image,
        });
    }

    /// Records a History choice with the committed window painted at it. An unchanged window
    /// keeps its shared copy, so repaint does not clone rows.
    pub(super) fn set_viewed(
        &mut self,
        source: ContentSource,
        position: ReadingPosition,
        window: &ViewportWindow,
    ) {
        let image = self
            .get(&source.member)
            .and_then(|saved| saved.image.clone())
            .filter(|image| image.route == window.route)
            .unwrap_or_else(|| Arc::new(copy_window(window)));
        self.store(Saved {
            source,
            choice: Choice::History(position),
            image: Some(image),
        });
    }

    fn store(&mut self, saved: Saved) {
        self.entries
            .retain(|old| old.source.member != saved.source.member);
        self.entries.push(saved);
        let mut bytes = self.entries.iter().map(witness_bytes).sum::<usize>();
        for saved in &mut self.entries {
            if bytes <= MAXIMUM_WITNESS_BYTES {
                break;
            }
            let released = witness_bytes(saved);
            if released > 0 {
                saved.choice = Choice::Unavailable;
                saved.image = None;
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
            saved.image = None;
        }
    }
}

fn copy_window(window: &ViewportWindow) -> ViewportWindow {
    ViewportWindow {
        route: window.route,
        foreground: window.foreground,
        background: window.background,
        graphics_present: window.graphics_present,
        rows: window.rows.clone(),
    }
}

/// Witness rows and the retained image share the bank's byte budget.
fn witness_bytes(saved: &Saved) -> usize {
    let Choice::History(position) = &saved.choice else {
        return 0;
    };
    let image = saved
        .image
        .as_ref()
        .map_or(&[][..], |image| &image.rows[..]);
    position
        .witness
        .iter()
        .chain(image)
        .fold(0_usize, |bytes, row| {
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
    /// The image each restoring incarnation shows until its window commits or recovery starts.
    restoring: Vec<(PaneRef, Arc<ViewportWindow>)>,
}

impl Coordinator {
    pub(super) fn projections_cleared(&mut self) {
        self.examined.clear();
        self.restoring.clear();
    }

    pub(super) fn begin_restore(&mut self, key: PaneRef, image: Option<Arc<ViewportWindow>>) {
        self.restoring.retain(|(old, _)| old != &key);
        if let Some(image) = image {
            self.restoring.push((key, image));
        }
    }

    /// Releases images whose restoration settled; only a still-resuming incarnation keeps one.
    pub(super) fn retain_restoring(&mut self, mut keep: impl FnMut(&PaneRef) -> bool) {
        self.restoring.retain(|(key, _)| keep(key));
    }

    pub(super) fn restoring_image(&self, key: &PaneRef) -> Option<&ViewportWindow> {
        self.restoring
            .iter()
            .find(|(old, _)| old == key)
            .map(|(_, image)| image.as_ref())
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
        self.restoring.retain(|(key, _)| keep(key));
    }
}

#[cfg(test)]
mod tests;

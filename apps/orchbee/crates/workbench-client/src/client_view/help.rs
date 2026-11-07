//! Client-owned shortcut-help incarnation and exact successful-frame authority.

use crate::terminal_text::line_editor::{LengthLimit, LineEditor, LineKey};

/// The shortcut filter's scalar budget.
const MAXIMUM_FILTER_SCALARS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelpEpisodeToken(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HelpDrawCandidate {
    episode: HelpEpisodeToken,
    page: usize,
    revision: u64,
}

impl HelpDrawCandidate {
    pub(crate) const fn page(self) -> usize {
        self.page
    }

    /// Test fixture for a complete help panel whose viewport height was not measured.
    #[cfg(test)]
    pub(crate) const fn presented(
        self,
        effective_page: usize,
        last: usize,
    ) -> Option<HelpDrawSource> {
        self.presented_rows(effective_page, last, 0)
    }

    /// Constructs the exact source for a complete help panel. The renderer may
    /// clamp the first visible line after geometry changes, but it cannot
    /// publish a different filter revision or an invalid scroll boundary.
    pub(crate) const fn presented_rows(
        self,
        effective_page: usize,
        last: usize,
        rows: usize,
    ) -> Option<HelpDrawSource> {
        let expected_page = if self.page < last { self.page } else { last };
        if effective_page == expected_page {
            Some(HelpDrawSource::Presented {
                episode: self.episode,
                page: effective_page,
                last,
                rows,
                revision: self.revision,
            })
        } else {
            None
        }
    }

    /// A refused surface proves only the complete recovery control for the
    /// requested scroll offset. It deliberately carries no invented boundary.
    pub(crate) const fn refused(self) -> HelpDrawSource {
        HelpDrawSource::Refused {
            episode: self.episode,
            page: self.page,
            revision: self.revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpDrawSource {
    Presented {
        episode: HelpEpisodeToken,
        page: usize,
        last: usize,
        rows: usize,
        revision: u64,
    },
    Refused {
        episode: HelpEpisodeToken,
        page: usize,
        revision: u64,
    },
}

impl HelpDrawSource {
    pub(crate) const fn episode(self) -> HelpEpisodeToken {
        match self {
            Self::Presented { episode, .. } | Self::Refused { episode, .. } => episode,
        }
    }

    pub(crate) const fn page(self) -> usize {
        match self {
            Self::Presented { page, .. } | Self::Refused { page, .. } => page,
        }
    }

    pub(crate) const fn revision(self) -> u64 {
        match self {
            Self::Presented { revision, .. } | Self::Refused { revision, .. } => revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpControl {
    ToggleAll,
    Close,
    Previous,
    Next,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawnHelpControl {
    source: HelpDrawSource,
    control: HelpControl,
}

impl DrawnHelpControl {
    pub(crate) const fn new(source: HelpDrawSource, control: HelpControl) -> Self {
        Self { source, control }
    }

    pub(crate) const fn source(self) -> HelpDrawSource {
        self.source
    }

    pub(crate) const fn control(self) -> HelpControl {
        self.control
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Activation {
    ToggleAll,
    Close,
    Page(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct State {
    episode: u64,
    /// First rendered line in the continuous result list (the old name stays
    /// on the draw protocol to preserve its narrow pointer contract).
    page: usize,
    revision: u64,
    filter: LineEditor,
    expanded: bool,
    page_rows: usize,
    shown: Option<HelpDrawSource>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            episode: 0,
            page: 0,
            revision: 0,
            filter: empty_filter(),
            expanded: false,
            page_rows: 0,
            shown: None,
        }
    }
}

fn empty_filter() -> LineEditor {
    LineEditor::new(String::new(), LengthLimit::Scalars(MAXIMUM_FILTER_SCALARS))
}

impl State {
    pub(super) fn open(&mut self) -> bool {
        let Some(episode) = self.episode.checked_add(1) else {
            return false;
        };
        self.episode = episode;
        self.page = 0;
        self.revision = 0;
        self.filter = empty_filter();
        self.expanded = false;
        self.shown = None;
        true
    }

    pub(super) fn close(&mut self) {
        self.page = 0;
        self.filter = empty_filter();
        self.expanded = false;
        self.shown = None;
    }

    pub(super) const fn page(&self) -> usize {
        self.page
    }

    pub(super) const fn candidate(&self) -> HelpDrawCandidate {
        HelpDrawCandidate {
            episode: HelpEpisodeToken(self.episode),
            page: self.page,
            revision: self.revision,
        }
    }

    pub(super) fn request_page(&mut self, page: usize, bound: usize) {
        self.page = page.min(bound);
        self.bump_revision();
        self.shown = None;
    }

    pub(super) const fn expanded(&self) -> bool {
        self.expanded
    }
    pub(super) fn toggle_all(&mut self) {
        self.expanded = !self.expanded;
        self.page = 0;
        self.bump_revision();
        self.shown = None;
    }
    /// One page is the last presented viewport height. Before any draw has measured it, keep the
    /// established eight-line step instead of degrading paging into single-line scrolling.
    pub(super) fn page_step(&self) -> usize {
        if self.page_rows == 0 {
            8
        } else {
            self.page_rows
        }
    }

    pub(super) fn filter(&self) -> &str {
        self.filter.text()
    }

    /// The filter cursor's byte offset; always a grapheme boundary of the filter.
    pub(super) fn filter_cursor(&self) -> usize {
        self.filter.cursor()
    }

    pub(super) fn end_page(&self) -> Option<usize> {
        match self.shown {
            Some(HelpDrawSource::Presented { last, .. }) => Some(last),
            Some(HelpDrawSource::Refused { .. }) | None => None,
        }
    }

    pub(super) fn append_filter(&mut self, character: char) -> bool {
        if character.is_control() || self.filter.insert(&character.to_string()).is_err() {
            return false;
        }
        self.filter_changed();
        true
    }

    pub(super) fn backspace_filter(&mut self) -> bool {
        let changed = self.filter.backspace();
        if changed {
            self.filter_changed();
        }
        changed
    }

    pub(super) fn delete_filter(&mut self) -> bool {
        let changed = self.filter.delete();
        if changed {
            self.filter_changed();
        }
        changed
    }

    /// Moves the filter cursor one grapheme; only the query row repaints.
    pub(super) fn move_filter_cursor(&mut self, forward: bool) -> bool {
        let moved = if forward {
            self.filter.right()
        } else {
            self.filter.left()
        };
        if moved {
            self.filter_cursor_moved();
        }
        moved
    }

    /// Applies one shared readline key; an over-budget yank changes nothing.
    pub(super) fn edit_filter(&mut self, key: LineKey) -> bool {
        let changed = self.filter.apply(key) == Ok(true);
        if changed && key.edits_text() {
            self.filter_changed();
        } else if changed {
            self.filter_cursor_moved();
        }
        changed
    }

    /// A text edit restarts the result list at its top and revokes the presented frame.
    fn filter_changed(&mut self) {
        self.page = 0;
        self.bump_revision();
        self.shown = None;
    }

    fn filter_cursor_moved(&mut self) {
        self.bump_revision();
        self.shown = None;
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(super) fn settle(&mut self, source: Option<HelpDrawSource>, active: bool) {
        self.shown = None;
        if !active {
            return;
        }
        let Some(source) = source else {
            return;
        };
        if source.episode() != HelpEpisodeToken(self.episode) || source.revision() != self.revision
        {
            return;
        }
        match source {
            HelpDrawSource::Presented {
                page, last, rows, ..
            } if page == self.page.min(last) && page <= last => {
                self.page = page;
                self.page_rows = rows;
                self.shown = Some(source);
            }
            HelpDrawSource::Refused { page, .. } if page == self.page => {
                self.shown = Some(source);
            }
            HelpDrawSource::Presented { .. } | HelpDrawSource::Refused { .. } => {}
        }
    }

    pub(super) fn activate(&self, target: &DrawnHelpControl) -> Option<Activation> {
        let source = target.source();
        if self.shown != Some(source)
            || source.episode() != HelpEpisodeToken(self.episode)
            || source.page() != self.page
            || source.revision() != self.revision
        {
            return None;
        }
        match target.control() {
            HelpControl::Close => Some(Activation::Close),
            HelpControl::ToggleAll if matches!(source, HelpDrawSource::Presented { .. }) => {
                Some(Activation::ToggleAll)
            }
            HelpControl::Previous if matches!(source, HelpDrawSource::Presented { page, .. } if page > 0) => {
                Some(Activation::Page(self.page.saturating_sub(self.page_step())))
            }
            HelpControl::Next if matches!(source, HelpDrawSource::Presented { page, last, .. } if page < last) => {
                Some(Activation::Page(
                    self.page
                        .saturating_add(self.page_step())
                        .min(self.end_page()?),
                ))
            }
            HelpControl::Previous | HelpControl::Next | HelpControl::ToggleAll => None,
        }
    }

    pub(super) fn scroll(&self, source: &HelpDrawSource, up: bool) -> Option<usize> {
        if self.shown.as_ref() != Some(source)
            || source.episode() != HelpEpisodeToken(self.episode)
            || source.page() != self.page
            || source.revision() != self.revision
        {
            return None;
        }
        let HelpDrawSource::Presented { page, last, .. } = *source else {
            return None;
        };
        let next = if up {
            page.saturating_sub(1)
        } else {
            page.saturating_add(1).min(last)
        };
        (next != page).then_some(next)
    }
}

#[cfg(test)]
mod tests;

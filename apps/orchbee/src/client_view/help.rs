//! Client-owned shortcut-help incarnation and exact successful-frame authority.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HelpEpisodeToken(u64);

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

    /// Constructs the exact source for a complete help panel. The renderer may
    /// clamp the first visible line after geometry changes, but it cannot
    /// publish a different filter revision or an invalid scroll boundary.
    pub(crate) const fn presented(
        self,
        effective_page: usize,
        last: usize,
    ) -> Option<HelpDrawSource> {
        let expected_page = if self.page < last { self.page } else { last };
        if effective_page == expected_page {
            Some(HelpDrawSource::Presented {
                episode: self.episode,
                page: effective_page,
                last,
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
pub(crate) enum HelpDrawSource {
    Presented {
        episode: HelpEpisodeToken,
        page: usize,
        last: usize,
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
pub(crate) enum HelpControl {
    Close,
    Previous,
    Next,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DrawnHelpControl {
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
    Close,
    Page(usize),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct State {
    episode: u64,
    /// First rendered line in the continuous result list (the old name stays
    /// on the draw protocol to preserve its narrow pointer contract).
    page: usize,
    revision: u64,
    filter: String,
    shown: Option<HelpDrawSource>,
}

impl State {
    pub(super) fn open(&mut self) -> bool {
        let Some(episode) = self.episode.checked_add(1) else {
            return false;
        };
        self.episode = episode;
        self.page = 0;
        self.revision = 0;
        self.filter.clear();
        self.shown = None;
        true
    }

    pub(super) fn close(&mut self) {
        self.page = 0;
        self.filter.clear();
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

    pub(super) fn filter(&self) -> &str {
        &self.filter
    }

    pub(super) fn end_page(&self) -> Option<usize> {
        match self.shown {
            Some(HelpDrawSource::Presented { last, .. }) => Some(last),
            Some(HelpDrawSource::Refused { .. }) | None => None,
        }
    }

    pub(super) fn append_filter(&mut self, character: char) -> bool {
        if character.is_control() || self.filter.chars().count() >= 128 {
            return false;
        }
        self.filter.push(character);
        self.page = 0;
        self.bump_revision();
        self.shown = None;
        true
    }

    pub(super) fn backspace_filter(&mut self) -> bool {
        let Some(grapheme_width) = self.filter.graphemes(true).next_back().map(str::len) else {
            return false;
        };
        self.filter.truncate(self.filter.len() - grapheme_width);
        self.page = 0;
        self.bump_revision();
        self.shown = None;
        true
    }

    pub(super) fn clear_filter(&mut self) -> bool {
        if self.filter.is_empty() {
            return false;
        }
        self.filter.clear();
        self.page = 0;
        self.bump_revision();
        self.shown = None;
        true
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
            HelpDrawSource::Presented { page, last, .. }
                if page == self.page.min(last) && page <= last =>
            {
                self.page = page;
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
            HelpControl::Previous if matches!(source, HelpDrawSource::Presented { page, .. } if page > 0) => {
                Some(Activation::Page(self.page - 1))
            }
            HelpControl::Next if matches!(source, HelpDrawSource::Presented { page, last, .. } if page < last) => {
                Some(Activation::Page(self.page + 1))
            }
            HelpControl::Previous | HelpControl::Next => None,
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

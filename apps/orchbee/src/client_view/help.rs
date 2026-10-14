//! Client-owned shortcut-help incarnation and exact successful-frame authority.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HelpEpisodeToken(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HelpDrawCandidate {
    episode: HelpEpisodeToken,
    page: usize,
}

impl HelpDrawCandidate {
    pub(crate) const fn page(self) -> usize {
        self.page
    }

    /// Constructs the exact source for a complete help panel. The renderer may
    /// clamp a requested page after geometry changes, but it cannot publish a
    /// different page or an invalid last-page boundary.
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
            })
        } else {
            None
        }
    }

    /// A refused surface proves only the complete recovery control for the
    /// requested page. It deliberately carries no invented page boundary.
    pub(crate) const fn refused(self) -> HelpDrawSource {
        HelpDrawSource::Refused {
            episode: self.episode,
            page: self.page,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HelpDrawSource {
    Presented {
        episode: HelpEpisodeToken,
        page: usize,
        last: usize,
    },
    Refused {
        episode: HelpEpisodeToken,
        page: usize,
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
    page: usize,
    shown: Option<HelpDrawSource>,
}

impl State {
    pub(super) fn open(&mut self) -> bool {
        let Some(episode) = self.episode.checked_add(1) else {
            return false;
        };
        self.episode = episode;
        self.page = 0;
        self.shown = None;
        true
    }

    pub(super) fn close(&mut self) {
        self.page = 0;
        self.shown = None;
    }

    pub(super) const fn page(&self) -> usize {
        self.page
    }

    pub(super) const fn candidate(&self) -> HelpDrawCandidate {
        HelpDrawCandidate {
            episode: HelpEpisodeToken(self.episode),
            page: self.page,
        }
    }

    pub(super) fn request_page(&mut self, page: usize, bound: usize) {
        self.page = page.min(bound);
        self.shown = None;
    }

    pub(super) fn settle(&mut self, source: Option<HelpDrawSource>, active: bool) {
        self.shown = None;
        if !active {
            return;
        }
        let Some(source) = source else {
            return;
        };
        if source.episode() != HelpEpisodeToken(self.episode) {
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

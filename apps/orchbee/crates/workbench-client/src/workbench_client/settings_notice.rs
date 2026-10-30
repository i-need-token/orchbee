//! The Settings outcome notice: success notices expire after a bound, and failures stay until
//! the next outcome replaces them or the surface clears them.

use crate::localization::SettingsNotice as Notice;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct SettingsNotice {
    shown: Option<(Notice, Option<Instant>)>,
}

impl SettingsNotice {
    pub(super) fn clear(&mut self) {
        self.shown = None;
    }

    /// The notice still shown at `now`.
    pub(super) fn current(&self, now: Instant) -> Option<Notice> {
        self.shown
            .filter(|(_, until)| until.is_none_or(|until| now < until))
            .map(|(notice, _)| notice)
    }

    /// Shows `notice`; only `Saved` and `NoChange` expire.
    pub(super) fn note(&mut self, notice: Notice, now: Instant) {
        self.shown = Some((
            notice,
            if !matches!(notice, Notice::Saved | Notice::NoChange) {
                None
            } else {
                now.checked_add(Duration::from_secs(5))
            },
        ));
    }

    /// Drops an expired notice and reports whether it did.
    pub(super) fn expire(&mut self, now: Instant) -> bool {
        let expired = self
            .shown
            .is_some_and(|(_, until)| until.is_some_and(|until| now >= until));
        if expired {
            self.shown = None;
        }
        expired
    }
}

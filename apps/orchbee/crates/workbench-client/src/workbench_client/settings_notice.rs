//! The Settings outcome notice. Edits apply without a success notice, so every notice reports a
//! refusal or a failure and stays until the next outcome replaces it or the surface clears it.

use crate::localization::SettingsNotice as Notice;

#[derive(Default)]
pub(super) struct SettingsNotice {
    shown: Option<Notice>,
}

impl SettingsNotice {
    pub(super) fn clear(&mut self) {
        self.shown = None;
    }

    pub(super) const fn current(&self) -> Option<Notice> {
        self.shown
    }

    pub(super) fn note(&mut self, notice: Notice) {
        self.shown = Some(notice);
    }
}

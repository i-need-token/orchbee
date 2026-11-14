//! The status line that reports how a forget of a remote Machine's saved password ended. The
//! action itself is an item of the Machine's action menu, and Ctrl+G.

use crate::localization::{Locale, RememberText};
use crate::machine_discovery::Forgotten;
use crate::machine_picker::Picker;
use crate::tui_theme::Role;

impl Picker {
    /// How the last forget ended, for the status line.
    pub(super) fn forget_status(&self, locale: Locale) -> Option<(&'static str, Role)> {
        let (text, role) = match self.forget_notice? {
            Forgotten::Removed => (RememberText::Forgotten, Role::StatusSuccess),
            Forgotten::NothingSaved => (RememberText::NothingSaved, Role::TextSecondary),
            Forgotten::Failed => (RememberText::ForgetFailed, Role::StatusWarning),
        };
        Some((locale.machine_remember_text(text), role))
    }
}

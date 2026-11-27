//! The About category's version and update control, which hold no setting and never reach the
//! Server's settings; Automatic updates beside them is an ordinary setting row. What the Updates row
//! shows mirrors the release update state the TUI owns (`crate::release_update`), and a press only
//! records what it asks for, which the TUI takes.
use crate::release_update::{UpdateCheck, UpdateRequest};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct About {
    /// The version the Update control offers, while it is Ready.
    update: Option<String>,
    check: UpdateCheck,
    /// What the last press asked for, until the TUI takes it.
    request: Option<UpdateRequest>,
}

impl About {
    pub(crate) fn update(&self) -> Option<&str> {
        self.update.as_deref()
    }

    pub(crate) const fn check(&self) -> &UpdateCheck {
        &self.check
    }

    /// Whether a press on the Updates row can do anything: switch to an offered version, or ask
    /// for a check while one is possible and none is under way.
    pub(super) fn actionable(&self) -> bool {
        self.update.is_some()
            || !(matches!(self.check, UpdateCheck::Unavailable(_)) || self.check.running())
    }

    /// Whether the row shows as available; a check under way stays readable, not disabled.
    pub(super) const fn enabled(&self) -> bool {
        self.update.is_some() || !matches!(self.check, UpdateCheck::Unavailable(_))
    }

    /// A press on the Updates row.
    pub(super) fn press(&mut self) -> bool {
        if !self.actionable() {
            return false;
        }
        self.request = Some(if self.update.is_some() {
            UpdateRequest::Switch
        } else {
            UpdateRequest::Check
        });
        true
    }

    /// Follows the update state; answers whether it changed.
    pub(super) fn observe(&mut self, update: Option<&str>, check: UpdateCheck) -> bool {
        let changed = self.update.as_deref() != update || self.check != check;
        self.update = update.map(str::to_owned);
        self.check = check;
        changed
    }

    pub(super) const fn take_request(&mut self) -> Option<UpdateRequest> {
        self.request.take()
    }
}

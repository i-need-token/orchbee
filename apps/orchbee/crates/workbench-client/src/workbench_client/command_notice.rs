//! The Workbench command notice: one bounded notice about a Workbench command, and the context
//! that names its subject.
//!
//! Client-local notices belong to `ClientView`; Settings and link notices take precedence in the
//! view model and keep their own owners.

use crate::localization::{NoticeContext, WorkbenchClientNotice};
use std::time::Instant;

#[derive(Default)]
pub(super) struct CommandNotice {
    shown: Option<(WorkbenchClientNotice, Instant)>,
    /// Never cleared: the view pairs it with a notice only while that notice names it.
    context: Option<NoticeContext>,
}

impl CommandNotice {
    /// Replaces the notice; `None` clears it, including when a deadline cannot be represented.
    pub(super) fn set(&mut self, shown: Option<(WorkbenchClientNotice, Instant)>) {
        self.shown = shown;
    }

    /// Clears the notice only when `matches` admits it.
    pub(super) fn clear_if(&mut self, matches: impl FnOnce(&WorkbenchClientNotice) -> bool) {
        if self
            .shown
            .as_ref()
            .is_some_and(|(notice, _)| matches(notice))
        {
            self.shown = None;
        }
    }

    /// Drops an expired notice and reports whether it did.
    pub(super) fn expire(&mut self, now: Instant) -> bool {
        if self.shown.is_some_and(|(_, until)| now >= until) {
            self.shown = None;
            true
        } else {
            false
        }
    }

    /// The notice still shown at `now`.
    pub(super) fn current(&self, now: Instant) -> Option<WorkbenchClientNotice> {
        self.shown
            .filter(|(_, until)| now < *until)
            .map(|(notice, _)| notice)
    }

    pub(super) fn set_context(&mut self, context: NoticeContext) {
        self.context = Some(context);
    }

    pub(super) const fn context(&self) -> Option<&NoticeContext> {
        self.context.as_ref()
    }
}

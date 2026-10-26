//! The shared reminder policy consumes exact physical command facts once, even while disabled.
use orchbee_terminal_protocol::TerminalCommandCompletion;
use orchbee_workbench_protocol::{CompletionReminderMode, UserPreferencesValues};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Cursor {
    sequence: u64,
    notified: Option<Instant>,
}
#[derive(Default)]
pub(super) struct Observer {
    cursors: BTreeMap<String, Cursor>,
}
impl Observer {
    /// Some means a local reminder; its bool admits the rate-limited audible notification.
    pub(super) fn observe(
        &mut self,
        identity: String,
        completion: TerminalCommandCompletion,
        focused: bool,
        preferences: &UserPreferencesValues,
        wall_millis: u64,
        now: Instant,
    ) -> Option<bool> {
        // Retain consumed identities for the session lifetime, refusing new ones at the cap.
        if self.cursors.len() >= 4096 && !self.cursors.contains_key(&identity) {
            return None;
        }
        let cursor = self.cursors.entry(identity).or_default();
        if completion.sequence.get() <= cursor.sequence {
            return None;
        }
        cursor.sequence = completion.sequence.get();
        let completed = completion.completed_at_milliseconds.get();
        if completed > wall_millis.saturating_add(5000)
            || wall_millis.saturating_sub(completed) > 60_000
            || completion.duration_milliseconds
                < u64::from(preferences.completion_reminder_minimum_seconds) * 1000
            || preferences.completion_reminder_mode == CompletionReminderMode::Never
            || (preferences.completion_reminder_mode == CompletionReminderMode::Unfocused
                && focused)
        {
            return None;
        }
        let notify = cursor.notified.is_none_or(|previous| {
            now.saturating_duration_since(previous) >= Duration::from_secs(30)
        });
        if notify {
            cursor.notified = Some(now);
        }
        Some(notify && preferences.sound_enabled)
    }
}

pub(crate) struct Reminder {
    pub(crate) title: String,
    pub(crate) duration_milliseconds: u64,
    pub(crate) exit_code: u8,
}
#[derive(Default)]
pub(super) struct State {
    pub(super) observer: Observer,
    pub(super) visible: Option<(Reminder, Instant)>,
}
impl State {
    pub(super) fn tick(&mut self, now: Instant) -> bool {
        if self
            .visible
            .as_ref()
            .is_some_and(|(_, until)| now >= *until)
        {
            self.visible = None;
            return true;
        }
        false
    }
    pub(super) fn reminder(&self, now: Instant) -> Option<&Reminder> {
        self.visible
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(reminder, _)| reminder)
    }
}

#[cfg(test)]
mod tests;

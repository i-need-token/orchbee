//! Join a validated frame fact to its live pane; rendering never observes clocks or consumes events.
use super::{NativeProjectionStatus, NativeUi, WorkbenchClientView};
use crate::{
    terminal_text::{sanitize_terminal_text, truncate_display},
    workbench_client::{
        completion_reminders::Reminder, state::ClientNotice, terminal_effects::Payload,
    },
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl WorkbenchClientView<'_> {
    pub(crate) const fn completion_reminder(&self) -> Option<&Reminder> {
        self.completion_reminder
    }
}
impl NativeUi {
    pub(super) fn drive_completion_reminders(&mut self) -> bool {
        let now = self.clock.now();
        let mut changed = self.completion_reminders.tick(now);
        let Some(wall) = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        else {
            return changed;
        };
        let focus = self.state.as_ref().and_then(|state| state.focus_pane_ref());
        let focused_identity = self
            .projections
            .iter()
            .find(|projection| Some(&projection.key) == focus.as_ref())
            .and_then(|projection| {
                projection
                    .value
                    .host_terminal_id()
                    .map(|terminal| (*projection.value.epoch(), terminal))
            });
        for projection in &self.projections {
            if projection.status != NativeProjectionStatus::FrameAvailable {
                continue;
            }
            let Some(frame) = projection.value.frame() else {
                continue;
            };
            let Some(completion) = frame.command_completion else {
                continue;
            };
            let Some(terminal) = projection.value.host_terminal_id() else {
                continue;
            };
            let focused = self.outer_focused.unwrap_or(true)
                && focused_identity == Some((*projection.value.epoch(), terminal));
            let identity = format!("{}:{terminal}", projection.value.epoch().to_text());
            let Some(sound) = self.completion_reminders.observer.observe(
                identity,
                completion,
                focused,
                &self.preferences,
                wall,
                now,
            ) else {
                continue;
            };
            self.completion_reminders.visible = Some((
                Reminder {
                    title: truncate_display(
                        &sanitize_terminal_text(
                            frame
                                .title
                                .as_deref()
                                .filter(|title| !title.is_empty())
                                .unwrap_or(terminal),
                        ),
                        64,
                        "…",
                    ),
                    duration_milliseconds: completion.duration_milliseconds,
                    exit_code: completion.exit_code,
                },
                now + Duration::from_secs(5),
            ));
            changed = true;
            if sound
                && !self.effects.enqueue_program(
                    projection.key.clone(),
                    *projection.value.attachment(),
                    *projection.value.epoch(),
                    Payload::Bell,
                )
                && let Some(state) = &mut self.state
            {
                state.set_notice(ClientNotice::BellFailed);
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests;

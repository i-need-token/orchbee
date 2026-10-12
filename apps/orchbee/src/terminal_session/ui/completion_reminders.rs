//! Join a validated frame fact to its live pane; rendering never observes clocks or consumes events.
use super::{NativeProjectionStatus, NativeSessionView, NativeUi};
use crate::{
    terminal_session::{
        completion_reminders::Reminder, state::SessionNotice, terminal_effects::Payload,
    },
    terminal_text::{sanitize_terminal_text, truncate_display},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl NativeSessionView<'_> {
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
                state.set_notice(SessionNotice::BellFailed);
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_session::ui::{move_membership_tests::ready_ui, viewport_tests};
    use orchbee_terminal_protocol::TerminalCommandCompletion;
    use orchbee_workbench_protocol::{CompletionReminderMode, UserPreferencesValues};
    use std::num::NonZeroU64;

    #[test]
    fn installed_physical_frame_drives_one_localized_reminder_and_one_sound_effect() {
        let (mut ui, _, _, _) = ready_ui();
        viewport_tests::install_projection(&mut ui, Some((3, false, 24)), 0);
        ui.apply_user_preferences(&UserPreferencesValues {
            completion_reminder_mode: CompletionReminderMode::Always,
            sound_enabled: true,
            ..UserPreferencesValues::default()
        });
        let mut frame = viewport_tests::frame(&ui.projections[0].value, 3, false, 24);
        frame.title = Some("task\u{1b}]52;c;secret\u{7}".into());
        frame.command_completion = Some(TerminalCommandCompletion {
            sequence: NonZeroU64::MIN,
            duration_milliseconds: 5500,
            completed_at_milliseconds: NonZeroU64::new(
                u64::try_from(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                )
                .unwrap(),
            )
            .unwrap(),
            exit_code: 7,
        });
        ui.projections[0].value.install_frame_for_test(frame);
        assert!(ui.drive_completion_reminders());
        let view = ui.view();
        let reminder = view.completion_reminder().unwrap();
        assert_eq!(reminder.title, "task");
        assert!(
            crate::localization::Locale::EnUs
                .command_completion_reminder(reminder)
                .contains("5.500 s · exit 7")
        );
        assert!(
            crate::localization::Locale::ZhCn
                .command_completion_reminder(reminder)
                .contains("退出码 7")
        );
        assert!(ui.effects.dispatch().is_some());
        assert!(!ui.drive_completion_reminders());
        assert!(ui.effects.dispatch().is_none());
        ui.apply_user_preferences(&UserPreferencesValues::default());
        assert!(!ui.drive_completion_reminders());
    }
}

use super::*;
use crate::terminal_session::ui::tests::{move_membership::ready_ui, viewport};
use orchbee_terminal_protocol::TerminalCommandCompletion;
use orchbee_workbench_protocol::{CompletionReminderMode, UserPreferencesValues};
use std::num::NonZeroU64;

#[test]
fn installed_physical_frame_drives_one_localized_reminder_and_one_sound_effect() {
    let (mut ui, _, _, _) = ready_ui();
    viewport::install_projection(&mut ui, Some((3, false, 24)), 0);
    ui.apply_user_preferences(&UserPreferencesValues {
        completion_reminder_mode: CompletionReminderMode::Always,
        sound_enabled: true,
        ..UserPreferencesValues::default()
    });
    let mut frame = viewport::frame(&ui.projections[0].value, 3, false, 24);
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

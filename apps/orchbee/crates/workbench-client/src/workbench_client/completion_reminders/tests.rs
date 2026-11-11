use super::*;
use std::num::NonZeroU64;

fn fact(sequence: u64, duration: u64, wall: u64) -> TerminalCommandCompletion {
    TerminalCommandCompletion {
        sequence: NonZeroU64::new(sequence).unwrap(),
        duration_milliseconds: duration,
        completed_at_milliseconds: NonZeroU64::new(wall).unwrap(),
        exit_code: 0,
    }
}

#[test]
fn completion_preferences_match_duration_focus_freshness_and_consumed_sequence_rules() {
    let now = Instant::now();
    for mode in [
        CompletionReminderMode::Never,
        CompletionReminderMode::Unfocused,
        CompletionReminderMode::Always,
    ] {
        for focused in [false, true] {
            for duration in [4999, 5000] {
                for wall in [1, 39_999, 40_000, 100_000, 105_000, 105_001] {
                    let mut observer = Observer::default();
                    let preferences = UserSettingsValues {
                        completion_reminder_mode: mode,
                        sound_enabled: true,
                        ..UserSettingsValues::default()
                    };
                    let completion = fact(1, duration, wall);
                    let actual = observer.observe(
                        "physical".into(),
                        completion,
                        focused,
                        &preferences,
                        100_000,
                        now,
                    );
                    let eligible = mode != CompletionReminderMode::Never
                        && (mode != CompletionReminderMode::Unfocused || !focused)
                        && duration >= 5000
                        && (40_000..=105_000).contains(&wall);
                    assert_eq!(actual, eligible.then_some(true));
                    let enabled = UserSettingsValues {
                        completion_reminder_mode: CompletionReminderMode::Always,
                        sound_enabled: true,
                        ..UserSettingsValues::default()
                    };
                    assert_eq!(
                        observer.observe(
                            "physical".into(),
                            completion,
                            false,
                            &enabled,
                            100_000,
                            now
                        ),
                        None,
                        "ignored facts cannot replay after preference changes"
                    );
                }
            }
        }
    }
}

#[test]
fn audio_is_rate_limited_per_physical_terminal_while_each_new_completion_can_remind() {
    let mut observer = Observer::default();
    let now = Instant::now();
    let preferences = UserSettingsValues {
        completion_reminder_mode: CompletionReminderMode::Always,
        sound_enabled: true,
        ..UserSettingsValues::default()
    };
    assert_eq!(
        observer.observe(
            "one".into(),
            fact(1, 6000, 100_000),
            false,
            &preferences,
            100_000,
            now
        ),
        Some(true)
    );
    assert_eq!(
        observer.observe(
            "one".into(),
            fact(2, 6000, 110_000),
            false,
            &preferences,
            110_000,
            now + Duration::from_secs(10)
        ),
        Some(false)
    );
    assert_eq!(
        observer.observe(
            "two".into(),
            fact(1, 6000, 110_000),
            false,
            &preferences,
            110_000,
            now + Duration::from_secs(10)
        ),
        Some(true)
    );
    assert_eq!(
        observer.observe(
            "one".into(),
            fact(3, 6000, 130_000),
            false,
            &preferences,
            130_000,
            now + Duration::from_secs(30)
        ),
        Some(true)
    );
}

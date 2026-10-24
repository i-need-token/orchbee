use super::*;

#[test]
fn manual_and_automatic_defaults_remain_independent() {
    let policy = WindowsNotificationSettings::default();
    assert_eq!(policy.delivery, WindowsNotificationDelivery::Off);
    assert_eq!(policy.agent_delivery, WindowsNotificationDelivery::Tui);
    assert_eq!(policy.delay_seconds, 1);
    assert!(!policy.sound_enabled);
    assert_eq!(
        policy.agent_sound("codex"),
        WindowsAgentSoundSetting::Default
    );
}

#[test]
fn notification_profile_roundtrips_deliveries_delay_position_and_agent_sound() {
    for delivery in ["off", "orchbee", "terminal", "system"] {
        for agent_delivery in ["off", "orchbee", "terminal", "system"] {
            let text = format!(
                "[ui.toast]\ndelivery='{delivery}'\nagent_delivery='{agent_delivery}'\ndelay_seconds=3600\n[ui.toast.overlay]\nposition='top-left'\n[ui.sound]\nenabled=true\n[ui.sound.agents]\ncodex='off'\npi='on'"
            );
            let policy = WindowsNotificationSettings::from_profile(&text).unwrap();
            assert_eq!(policy.position(), "top-left");
            assert_eq!(policy.delay_seconds, 3600);
            assert!(policy.sound_enabled);
            assert_eq!(policy.agent_sound("codex"), WindowsAgentSoundSetting::Off);
            assert_eq!(policy.agent_sound("pi"), WindowsAgentSoundSetting::On);
            assert_eq!(policy.delivery.as_str(), delivery);
            assert_eq!(policy.agent_delivery.as_str(), agent_delivery);
            assert_eq!(
                WindowsNotificationSettings::from_profile(&policy.profile()).unwrap(),
                policy
            );
        }
    }
}

#[test]
fn notification_profile_rejects_unknown_types_bounds_and_agent_ids() {
    for text in [
        "[ui.toast]\ndelivery=1",
        "[ui.toast]\ndelivery='bad'",
        "[ui.toast]\nagent_delivery='bad'",
        "[ui.toast]\ndelivery='herdr'",
        "[ui.toast]\nagent_delivery='herdr'",
        "[ui.toast]\nenabled=true",
        "[ui.toast.unknown]\nposition='top-left'",
        "[ui.toast]\ndelay_seconds=-1",
        "[ui.toast]\ndelay_seconds=3601",
        "[ui.toast]\nunknown=true",
        "[ui.toast.overlay]\nposition='center'",
        "[ui.sound]\nenabled='yes'",
        "[ui.sound]\npath='later.mp3'",
        "[ui.sound.agents]\n'Bad Agent'='off'",
        "[ui.sound.agents]\ncodex='sometimes'",
        "[ui]",
        "[ui.toast]\ndelivery='off'\n[terminal]",
    ] {
        assert!(
            WindowsNotificationSettings::from_profile(text).is_err(),
            "{text}"
        );
    }
    let too_many = (0..=MAXIMUM_AGENT_SOUND_OVERRIDES)
        .map(|index| format!("a{index}='off'"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        WindowsNotificationSettings::from_profile(&format!("[ui.sound.agents]\n{too_many}"))
            .is_err()
    );
    assert!(
        WindowsNotificationSettings::from_profile(
            &"x".repeat(MAXIMUM_NOTIFICATION_PROFILE_BYTES + 1)
        )
        .is_err()
    );
}

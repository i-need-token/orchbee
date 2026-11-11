use super::*;

#[test]
fn manual_and_automatic_defaults_remain_independent() {
    let policy = NotificationSettings::default();
    assert_eq!(policy.delivery, NotificationDelivery::Off);
    assert_eq!(policy.agent_delivery, NotificationDelivery::Tui);
    assert_eq!(policy.delay_seconds, 1);
    assert!(!policy.sound_enabled);
    assert_eq!(policy.agent_sound("codex"), AgentSoundSetting::Default);
}

#[test]
fn notification_profile_roundtrips_deliveries_delay_position_and_agent_sound() {
    for delivery in ["off", "orchbee", "terminal", "system"] {
        for agent_delivery in ["off", "orchbee", "terminal", "system"] {
            let text = format!(
                "[notifications]\ncli_delivery='{delivery}'\nagent_delivery='{agent_delivery}'\ndelay_seconds=3600\nposition='top-left'\nsound=true\n[notifications.sound_by_agent]\ncodex='off'\npi='on'"
            );
            let policy = NotificationSettings::from_profile(&text).unwrap();
            assert_eq!(policy.position(), "top-left");
            assert_eq!(policy.delay_seconds, 3600);
            assert!(policy.sound_enabled);
            assert_eq!(policy.agent_sound("codex"), AgentSoundSetting::Off);
            assert_eq!(policy.agent_sound("pi"), AgentSoundSetting::On);
            assert_eq!(policy.delivery.as_str(), delivery);
            assert_eq!(policy.agent_delivery.as_str(), agent_delivery);
            assert_eq!(
                NotificationSettings::from_profile(&policy.profile()).unwrap(),
                policy
            );
        }
    }
}

#[test]
fn notification_profile_rejects_unknown_types_bounds_and_agent_ids() {
    let too_many = (0..=MAXIMUM_AGENT_SOUND_OVERRIDES)
        .map(|index| format!("a{index}='off'"))
        .collect::<Vec<_>>()
        .join("\n");
    for text in [
        "[notifications]\ncli_delivery=1",
        "[notifications]\ncli_delivery='bad'",
        "[notifications]\nagent_delivery='bad'",
        "[notifications]\ncli_delivery='invalid-provider'",
        "[notifications]\nagent_delivery='invalid-provider'",
        "[notifications]\nenabled=true",
        "[notifications.unknown]\nposition='top-left'",
        "[notifications]\ndelay_seconds=-1",
        "[notifications]\ndelay_seconds=3601",
        "[notifications]\nunknown=true",
        "[notifications]\nposition='center'",
        "[notifications]\nsound='yes'",
        "[notifications]\nsound_path='later.mp3'",
        "[notifications.sound_by_agent]\n'Bad Agent'='off'",
        "[notifications.sound_by_agent]\ncodex='sometimes'",
        &format!("[notifications.sound_by_agent]\n{too_many}"),
        "[notifications]",
        "[notifications]\ncli_delivery='off'\n[terminal]",
        "not = 'toml",
        &"x".repeat(MAXIMUM_NOTIFICATION_PROFILE_BYTES + 1),
    ] {
        let error = NotificationSettings::from_profile(text).unwrap_err();
        assert_eq!(error.field, "notifications", "{text}");
    }
}

use super::*;
#[test]
fn notification_policy_roundtrip_precedence_alias_and_strict_rejection() {
    assert_eq!(
        WindowsNotificationSettings::default().delivery,
        WindowsNotificationDelivery::Off
    );
    for delivery in ["off", "orchbee", "herdr", "terminal", "system"] {
        for position in ["top-left", "top-right", "bottom-left", "bottom-right"] {
            let text = format!(
                "[ui.toast]\nenabled=true\ndelivery='{delivery}'\n[ui.toast.herdr]\nposition='{position}'"
            );
            let policy = WindowsNotificationSettings::from_profile(&text).unwrap();
            assert_eq!(policy.position(), position);
            assert_eq!(
                policy.delivery.as_str(),
                if delivery == "herdr" {
                    "orchbee"
                } else {
                    delivery
                }
            );
            assert_eq!(
                WindowsNotificationSettings::from_profile(&policy.profile()).unwrap(),
                policy
            );
        }
    }
    assert_eq!(
        WindowsNotificationSettings::from_profile("[ui.toast]\nenabled=true")
            .unwrap()
            .delivery,
        WindowsNotificationDelivery::Tui
    );
    for text in [
        "[ui.toast]\ndelivery=1",
        "[ui.toast]\ndelivery='bad'",
        "[ui.toast]\nunknown=true",
        "[ui.toast.herdr]\nposition='center'",
        "[ui.toast]\ndelivery='off'\n[terminal]",
    ] {
        assert!(
            WindowsNotificationSettings::from_profile(text).is_err(),
            "{text}"
        );
    }
}

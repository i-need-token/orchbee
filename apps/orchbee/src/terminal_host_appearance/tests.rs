use super::{ColorReply, ColorTarget, HostAppearance, OscReplyFrame, OscStep, Probe, parse_rgb};
use std::time::{Duration, Instant};

#[test]
fn query_requests_both_default_colors_without_changing_terminal_state() {
    assert_eq!(super::Query.sequence(), "\x1b]10;?\x1b\\\x1b]11;?\x1b\\");
}

#[test]
fn rgb_forms_scale_to_rgb8_and_reject_malformed_components() {
    assert_eq!(parse_rgb(b"rgb:0000/8080/ffff"), Some([0, 128, 255]));
    assert_eq!(parse_rgb(b"rgb:00/80/ff"), Some([0, 128, 255]));
    assert_eq!(parse_rgb(b"#102030"), Some([16, 32, 48]));
    for invalid in [
        b"rgb:ffff/ffff".as_slice(),
        b"rgb:ffff//ffff",
        b"rgb:gggg/0000/0000",
        b"rgb:00000/0000/0000",
        b"#12345",
    ] {
        assert_eq!(parse_rgb(invalid), None, "{invalid:?}");
    }
}

#[test]
fn framed_reply_is_bounded_and_requires_a_complete_terminator() {
    let now = Instant::now();
    let mut frame = OscReplyFrame::new(now);
    for byte in b"11" {
        assert_eq!(frame.push(*byte), OscStep::Pending);
    }
    assert_eq!(frame.push(b';'), OscStep::Pending);
    for byte in b"rgb:1010/2020/3030" {
        assert_eq!(frame.push(*byte), OscStep::Pending);
    }
    assert_eq!(frame.push(0x1b), OscStep::Pending);
    assert_eq!(
        frame.push(b'\\'),
        OscStep::Complete(ColorReply {
            target: ColorTarget::Background,
            rgb: Some([16, 32, 48]),
        })
    );

    let mut malformed = OscReplyFrame::new(now);
    for byte in b"11" {
        assert_eq!(malformed.push(*byte), OscStep::Pending);
    }
    assert_eq!(malformed.push(b';'), OscStep::Pending);
    for byte in b"not-rgb" {
        assert_eq!(malformed.push(*byte), OscStep::Pending);
    }
    assert_eq!(
        malformed.push(0x07),
        OscStep::Complete(ColorReply {
            target: ColorTarget::Background,
            rgb: None,
        })
    );

    let mut oversized = OscReplyFrame::new(now);
    for byte in b"11" {
        assert_eq!(oversized.push(byte), OscStep::Pending);
    }
    assert_eq!(oversized.push(b';'), OscStep::Pending);
    for byte in std::iter::repeat_n(b'a', 128) {
        assert_eq!(oversized.push(byte), OscStep::Pending);
    }
    assert_eq!(
        oversized.push(0x07),
        OscStep::Complete(ColorReply {
            target: ColorTarget::Background,
            rgb: None,
        })
    );
}

#[test]
fn dark_and_light_backgrounds_classify_and_foreground_only_waits_for_timeout() {
    let now = Instant::now();
    let mut dark = Probe::default();
    assert!(dark.begin(now).is_some());
    assert_eq!(
        dark.observe(
            ColorReply {
                target: ColorTarget::Background,
                rgb: Some([18, 24, 32]),
            },
            now + Duration::from_millis(10),
        ),
        Some(HostAppearance::Dark)
    );
    assert!(dark.timeout(now + Duration::from_secs(1)).is_none());
    assert!(dark.settle_fallback(HostAppearance::Light).is_none());
    assert_eq!(dark.appearance(), Some(HostAppearance::Dark));

    let mut light = Probe::default();
    light.begin(now);
    assert_eq!(
        light.observe(
            ColorReply {
                target: ColorTarget::Background,
                rgb: Some([245, 245, 245]),
            },
            now + Duration::from_millis(10),
        ),
        Some(HostAppearance::Light)
    );
    assert!(light.timeout(now + Duration::from_secs(1)).is_none());
    assert!(light.settle_fallback(HostAppearance::Dark).is_none());
    assert_eq!(light.appearance(), Some(HostAppearance::Light));

    let mut foreground_only = Probe::default();
    foreground_only.begin(now);
    assert_eq!(
        foreground_only.observe(
            ColorReply {
                target: ColorTarget::Foreground,
                rgb: Some([255, 255, 255]),
            },
            now + Duration::from_millis(10),
        ),
        None
    );
    assert_eq!(
        foreground_only.timeout(now + Duration::from_millis(351)),
        Some(super::Timeout)
    );
    assert!(foreground_only.appearance().is_none());
    assert_eq!(
        foreground_only.settle_fallback(HostAppearance::Unknown),
        Some(HostAppearance::Unknown)
    );
    assert_eq!(foreground_only.appearance(), Some(HostAppearance::Unknown));
}

#[test]
fn no_reply_timeout_accepts_fallback_and_late_color_cannot_change_it() {
    let now = Instant::now();
    let mut probe = Probe::default();
    assert!(probe.begin(now).is_some());
    assert!(probe.begin(now).is_none());
    assert!(probe.timeout(now + Duration::from_millis(349)).is_none());
    assert_eq!(
        probe.timeout(now + Duration::from_millis(350)),
        Some(super::Timeout)
    );
    assert_eq!(probe.appearance(), None);
    assert_eq!(
        probe.settle_fallback(HostAppearance::Dark),
        Some(HostAppearance::Dark)
    );
    assert_eq!(
        probe.observe(
            ColorReply {
                target: ColorTarget::Background,
                rgb: Some([0, 0, 0]),
            },
            now + Duration::from_millis(400),
        ),
        None
    );
    assert_eq!(probe.appearance(), Some(HostAppearance::Dark));
}

#[test]
fn malformed_background_reply_waits_for_deadline_and_keeps_fallback_eligible() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now);
    assert_eq!(
        probe.observe(
            ColorReply {
                target: ColorTarget::Background,
                rgb: None,
            },
            now + Duration::from_millis(10),
        ),
        None
    );
    assert_eq!(
        probe.timeout(now + Duration::from_millis(351)),
        Some(super::Timeout)
    );
    assert_eq!(
        probe.settle_fallback(HostAppearance::Light),
        Some(HostAppearance::Light)
    );
    assert_eq!(probe.appearance(), Some(HostAppearance::Light));
}

#[test]
fn late_reply_does_not_change_the_deadline_fallback_decision() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now);
    assert_eq!(
        probe.observe(
            ColorReply {
                target: ColorTarget::Background,
                rgb: Some([0, 0, 0]),
            },
            now + Duration::from_millis(351),
        ),
        None
    );
    assert_eq!(
        probe.timeout(now + Duration::from_millis(351)),
        Some(super::Timeout)
    );
    assert_eq!(
        probe.settle_fallback(HostAppearance::Light),
        Some(HostAppearance::Light)
    );
}

#[test]
fn a_partial_reply_at_the_deadline_keeps_registry_fallback_eligible() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now);
    assert_eq!(
        probe.timeout(now + Duration::from_millis(351)),
        Some(super::Timeout)
    );
    assert_eq!(
        probe.settle_fallback(HostAppearance::Unknown),
        Some(HostAppearance::Unknown)
    );
}

#[test]
fn windows_registry_fallback_maps_only_boolean_values() {
    assert_eq!(
        super::from_windows_apps_use_light_theme(Some(true)),
        HostAppearance::Light
    );
    assert_eq!(
        super::from_windows_apps_use_light_theme(Some(false)),
        HostAppearance::Dark
    );
    assert_eq!(
        super::from_windows_apps_use_light_theme(None),
        HostAppearance::Unknown
    );
}

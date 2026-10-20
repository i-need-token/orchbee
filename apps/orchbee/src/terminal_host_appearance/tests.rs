use super::{ColorReply, ColorTarget, HostAppearance, OscReplyFrame, OscStep, Probe, parse_rgb};
use std::time::{Duration, Instant};

#[test]
fn query_requests_both_default_colors_without_changing_terminal_state() {
    assert_eq!(super::Query.sequence(), "\x1b]10;?\x1b\\\x1b]11;?\x1b\\");
}

#[test]
fn both_reply_orders_capture_outer_colors_once_within_the_original_window() {
    let now = Instant::now();
    let foreground = ColorReply {
        target: ColorTarget::Foreground,
        rgb: Some([0xcc, 0xcc, 0xcc]),
    };
    let background = ColorReply {
        target: ColorTarget::Background,
        rgb: Some([0x0c, 0x0c, 0x0c]),
    };
    for replies in [[foreground, background], [background, foreground]] {
        let mut probe = Probe::default();
        probe.begin(now);
        for reply in replies {
            probe.observe(reply, now + Duration::from_millis(10));
        }
        assert_eq!(
            probe.colors(),
            super::HostColors {
                foreground: Some(0xcccccc),
                background: Some(0x0c0c0c),
            }
        );
        for target in [ColorTarget::Foreground, ColorTarget::Background] {
            assert_eq!(
                probe.observe(
                    ColorReply {
                        target,
                        rgb: Some([255, 255, 255]),
                    },
                    now + Duration::from_millis(20),
                ),
                None
            );
        }
        assert_eq!(probe.colors().background, Some(0x0c0c0c));
        assert_eq!(probe.colors().foreground, Some(0xcccccc));
        assert!(probe.timeout(now + Duration::from_secs(1)).is_none());
    }
}

#[test]
fn settled_background_does_not_admit_a_late_foreground() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now);
    probe.observe(
        ColorReply {
            target: ColorTarget::Background,
            rgb: Some([12, 12, 12]),
        },
        now,
    );
    assert_eq!(
        probe.observe(
            ColorReply {
                target: ColorTarget::Foreground,
                rgb: Some([204, 204, 204]),
            },
            now + Duration::from_millis(350),
        ),
        None
    );
    assert_eq!(probe.colors().foreground, None);
    assert_eq!(probe.colors().background, Some(0x0c0c0c));
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
        assert_eq!(oversized.push(*byte), OscStep::Pending);
    }
    assert_eq!(oversized.push(b';'), OscStep::Pending);
    let mut over_bound = None;
    for byte in std::iter::repeat_n(b'a', 128) {
        let step = oversized.push(byte);
        if step != OscStep::Pending {
            over_bound = Some(step);
            break;
        }
    }
    assert_eq!(
        over_bound,
        Some(OscStep::OverBound(ColorTarget::Background))
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
    assert_eq!(
        foreground_only.settle_fallback(HostAppearance::Unknown),
        Some(HostAppearance::Unknown)
    );
    assert!(
        foreground_only
            .settle_fallback(HostAppearance::Light)
            .is_none()
    );
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
    assert!(probe.settle_fallback(HostAppearance::Light).is_none());
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
    assert!(probe.settle_fallback(HostAppearance::Dark).is_none());
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
fn a_complete_late_background_reply_is_consumed_but_cannot_replace_fallback() {
    let start = Instant::now();
    let mut probe = Probe::default();
    probe.begin(start);
    assert_eq!(
        probe.timeout(start + Duration::from_millis(350)),
        Some(super::Timeout)
    );
    assert_eq!(
        probe.settle_fallback(HostAppearance::Dark),
        Some(HostAppearance::Dark)
    );

    let mut frame = OscReplyFrame::new(start + Duration::from_millis(300));
    let mut late = None;
    for byte in b"11;#fefefe\x07" {
        if let OscStep::Complete(reply) = frame.push(*byte) {
            late = Some(reply);
        }
    }
    let late = late.expect("the complete OSC frame remains recognizable");
    assert_eq!(late.rgb, Some([254, 254, 254]));
    assert_eq!(
        probe.observe(late, start + Duration::from_millis(400)),
        None
    );
    assert!(probe.settle_fallback(HostAppearance::Light).is_none());
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

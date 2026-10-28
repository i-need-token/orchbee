use super::*;

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

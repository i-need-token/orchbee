use super::*;
#[test]
fn timed_out_and_oversized_pastes_never_submit_fragments_or_detach() {
    let now = Instant::now();
    let mut input = Input::default();
    assert!(
        input
            .read(b"\x1b[200~partial\x02q", now, 24, false)
            .is_empty()
    );
    assert_eq!(
        input.tick(now + Duration::from_millis(60), 24, false),
        [Action::Notice]
    );
    assert!(input.read(&[b'x'; 256], now, 24, false).is_empty());
    assert_eq!(
        input.read(b"\x02q\x1b[201~ok", now, 24, false),
        [Action::Input(b"ok".to_vec())]
    );
    for size in [256 * 1024 + 1, 4 * 1024 * 1024 + 100] {
        let mut paste = b"\x1b[200~".to_vec();
        paste.extend(std::iter::repeat_n(b'x', size));
        paste.extend_from_slice(b"\x02q\x1b[201~after");
        assert_eq!(
            input.read(&paste, now, 24, false),
            [Action::Notice, Action::Input(b"after".to_vec())]
        );
    }
}

#[test]
fn disabled_mouse_and_focus_do_not_consume_detach_prefix() {
    let now = Instant::now();
    let mut input = Input::new(false);
    assert_eq!(
        input.read(b"\x02\x1b[<0;1;1M\x1b[<64;1;1M\x1b[Iq", now, 24, false),
        [Action::Focus(true), Action::Detach]
    );
}

#[test]
fn fragmented_unicode_and_invalid_paste_keep_input_boundaries() {
    let now = Instant::now();
    let mut input = Input::default();
    assert!(input.read(&[0xe4, 0xb8], now, 24, false).is_empty());
    assert_eq!(
        input.read(&[0xad], now, 24, false),
        [Action::Input("中".as_bytes().to_vec())]
    );
    assert_eq!(
        input.read(b"\x1b[200~\xff\x02q\x1b[201~x", now, 24, false),
        [Action::Notice, Action::Input(vec![b'x'])]
    );
}

#[test]
fn prefix_is_lossless_across_fragments_and_paste_never_detaches() {
    let now = Instant::now();
    let mut input = Input::default();
    assert_eq!(
        input.read(b"before\x02", now, 24, false),
        [Action::Input(b"before".to_vec())]
    );
    assert_eq!(input.read(b"qdiscard", now, 24, false), [Action::Detach]);
    let mut input = Input::default();
    assert_eq!(
        input.read(b"\x02\x02x\x02y", now, 24, false),
        [Action::Input(b"\x02x\x02y".to_vec())]
    );
    assert!(input.read(b"\x02\x1b[200~\x02q", now, 24, false).is_empty());
    assert_eq!(
        input.read(b"\x1b[201~", now, 24, false),
        [Action::Input(vec![2]), Action::Paste("\x02q".to_owned())]
    );
}
#[test]
fn shell_pages_mouse_and_application_keys_have_distinct_ownership() {
    let now = Instant::now();
    let mut input = Input::default();
    assert_eq!(
        input.read(b"\x1b[A\x1b[11~", now, 24, true),
        [Action::Input(b"\x1bOA\x1bOP".to_vec())]
    );
    assert!(matches!(
        &input.read(b"\x1b[5~", now, 24, false)[..],
        [Action::Scroll(Scroll {
            lines: 23,
            source: ScrollSource::PageKey,
            ..
        })]
    ));
    assert!(matches!(
        &input.read(b"\x1b[<68;2;3M", now, 24, false)[..],
        [Action::Scroll(Scroll {
            modifiers: 1,
            column: Some(1),
            row: Some(2),
            lines: 3,
            ..
        })]
    ));
    assert!(input.read(b"\x1b", now, 24, false).is_empty());
    assert_eq!(
        input.tick(now + Duration::from_millis(60), 24, false),
        [Action::Input(vec![27])]
    );
}

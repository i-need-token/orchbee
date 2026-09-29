use std::time::{Duration, Instant};

use super::*;
use crate::terminal_input::{MouseAction, MouseButton};

fn decode(bytes: &[u8]) -> Vec<DecodedItem> {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    let mut items = Vec::new();
    for byte in bytes {
        items.extend(decoder.push(*byte, now, false));
    }
    items
}

fn decode_split(first: &[u8], rest: &[&[u8]]) -> Vec<DecodedItem> {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    let mut items = Vec::new();
    for byte in first {
        items.extend(decoder.push(*byte, now, false));
    }
    for chunk in rest {
        for byte in chunk.iter() {
            items.extend(decoder.push(*byte, now, false));
        }
    }
    items
}

fn press(code: KeyCode) -> DecodedKind {
    DecodedKind::Key(KeyEvent::press(code, Modifiers::NONE))
}

#[test]
fn plain_and_control_bytes_decode_to_typed_events() {
    let items = decode(b"a\x03\x0d\x7f");
    assert_eq!(
        items.iter().map(|item| &item.kind).collect::<Vec<_>>(),
        [
            press(KeyCode::Char('a')),
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('c'), Modifiers::CTRL)),
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL)),
            press(KeyCode::Backspace),
        ]
        .iter()
        .collect::<Vec<_>>()
    );
    let ctrl_c = KeyEvent::press(KeyCode::Char('c'), Modifiers::CTRL);
    assert_eq!(items[1].kind, DecodedKind::Key(ctrl_c));
}

#[test]
fn utf8_scalar_is_one_item_and_owns_its_bytes() {
    let items = decode("中".as_bytes());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Char('中')));
    assert_eq!(items[0].raw.as_ref(), "中".as_bytes());
}

#[test]
fn utf8_split_across_every_boundary_still_decodes_as_one_scalar() {
    let zh = "中".as_bytes();
    let emoji = "😀".as_bytes();
    for (scalar, splits) in [(zh, vec![1, 2]), (emoji, vec![1, 2, 3])] {
        for cut in splits {
            let items = decode_split(&scalar[..cut], &[&scalar[cut..]]);
            assert_eq!(items.len(), 1, "cut {cut}");
            assert_eq!(items[0].raw.as_ref(), scalar);
        }
    }
    // Every byte in its own read.
    let items = decode_split(&emoji[..1], &[&emoji[1..2], &emoji[2..3], &emoji[3..]]);
    assert_eq!(items.len(), 1);
}

#[test]
fn arrow_csi_decodes_as_one_item_at_every_split() {
    for cut in 1..b"\x1b[D".len() {
        let items = decode_split(&b"\x1b[D"[..cut], &[&b"\x1b[D"[cut..]]);
        assert_eq!(items.len(), 1, "cut {cut}");
        assert_eq!(items[0].kind, press(KeyCode::Left));
        assert_eq!(items[0].raw.as_ref(), b"\x1b[D");
    }
}

#[test]
fn ss3_and_modified_csi_decode_modifiers() {
    let items = decode(b"\x1bOA");
    assert_eq!(items[0].kind, press(KeyCode::Up));
    let items = decode(b"\x1b[1;5D");
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(KeyCode::Left, Modifiers::CTRL))
    );
    let items = decode(b"\x1b[3;2~");
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(
            KeyCode::Delete,
            Modifiers::NONE.with_shift()
        ))
    );
}

#[test]
fn kitty_u_decodes_modifiers_and_event_kinds() {
    let items = decode(b"\x1b[97;5u");
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(KeyCode::Char('a'), Modifiers::CTRL))
    );
    let items = decode(b"\x1b[97;5:3u");
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent {
            code: KeyCode::Char('a'),
            mods: Modifiers::CTRL,
            kind: KeyKind::Release,
        })
    );
}

#[test]
fn function_keys_use_actual_xterm_numbers_and_preserve_modified_event_kinds() {
    for (number, key) in [
        (11, 1),
        (12, 2),
        (13, 3),
        (14, 4),
        (15, 5),
        (17, 6),
        (18, 7),
        (19, 8),
        (20, 9),
        (21, 10),
        (23, 11),
        (24, 12),
    ] {
        let raw = format!("\x1b[{number}~");
        assert_eq!(decode(raw.as_bytes())[0].kind, press(KeyCode::F(key)));
        for (kind, suffix) in [
            (KeyKind::Press, 1),
            (KeyKind::Repeat, 2),
            (KeyKind::Release, 3),
        ] {
            let raw = format!("\x1b[{number};5:{suffix}~");
            assert_eq!(
                decode(raw.as_bytes())[0].kind,
                DecodedKind::Key(KeyEvent {
                    code: KeyCode::F(key),
                    mods: Modifiers::CTRL,
                    kind
                })
            );
        }
    }
    for (letter, key) in [('P', 1), ('Q', 2), ('R', 3), ('S', 4)] {
        let raw = format!("\x1b[1;3:3{letter}");
        assert_eq!(
            decode(raw.as_bytes())[0].kind,
            DecodedKind::Key(KeyEvent {
                code: KeyCode::F(key),
                mods: Modifiers::NONE.with_alt(),
                kind: KeyKind::Release
            })
        );
    }
    for key in 1..=24 {
        let raw = format!("\x1b[{};1u", 57363 + u32::from(key));
        assert_eq!(decode(raw.as_bytes())[0].kind, press(KeyCode::F(key)));
    }
}

#[test]
fn malformed_function_parameters_and_unsupported_modifiers_never_alias_bindings() {
    for raw in [
        "\x1b[16~",
        "\x1b[22~",
        "\x1b[1;0P",
        "\x1b[1;5:9P",
        "\x1b[15;17~",
        "\x1b[15;33~",
        "\x1b[57364;17u",
        "\x1b[57364;1:9u",
    ] {
        let item = &decode(raw.as_bytes())[0];
        assert_eq!(item.kind, DecodedKind::Nondecision, "{raw:?}");
        assert_eq!(item.raw.as_ref(), raw.as_bytes());
    }
}

#[test]
fn alt_prefixed_bytes_decode_as_alt_events() {
    let items = decode(b"\x1ba");
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(
            KeyCode::Char('a'),
            Modifiers::NONE.with_alt()
        ))
    );
    assert_eq!(items[0].raw.as_ref(), b"\x1ba");
    let items = decode("\x1b中".as_bytes());
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(
            KeyCode::Char('中'),
            Modifiers::NONE.with_alt()
        ))
    );
}

#[test]
fn lone_escape_resolves_only_after_the_injected_deadline() {
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    let start = Instant::now();
    assert!(decoder.push(0x1b, start, false).is_empty());
    assert!(decoder.tick(start + Duration::from_millis(49)).is_empty());
    let items = decoder.tick(start + Duration::from_millis(50));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Esc));
    assert_eq!(items[0].raw.as_ref(), b"\x1b");
}

#[test]
fn escape_before_a_complete_sequence_never_yields_a_lone_esc() {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    assert!(decoder.push(0x1b, now, false).is_empty());
    assert!(decoder.push(b'[', now, false).is_empty());
    let items = decoder.push(b'D', now, false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Left));
}

#[test]
fn double_escape_yields_esc_then_restarts_the_wait() {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    assert!(decoder.push(0x1b, now, false).is_empty());
    let items = decoder.push(0x1b, now, false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Esc));
    let items = decoder.tick(now + Duration::from_millis(60));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Esc));
}

#[test]
fn unknown_complete_sequence_is_one_nondecision() {
    let items = decode(b"\x1b[?25h");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[?25h");
}

#[test]
fn malformed_utf8_is_a_nondecision_and_the_next_byte_recovers() {
    let items = decode(&[0xe4, b'a']);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), &[0xe4]);
    assert_eq!(items[1].kind, press(KeyCode::Char('a')));
}

#[test]
fn paste_run_is_one_atomic_item_and_payload_never_becomes_keys() {
    let items = decode(b"\x1b[200~ab\x02\x1b[201~");
    assert_eq!(items.len(), 1);
    let DecodedKind::Paste { text } = &items[0].kind else {
        panic!("expected paste, got {:?}", items[0].kind);
    };
    assert_eq!(text.as_deref(), Some("ab\x02"));
    assert_eq!(items[0].raw.as_ref(), b"\x1b[200~ab\x02\x1b[201~");
}

#[test]
fn paste_split_across_reads_stays_atomic() {
    let paste = b"\x1b[200~payload\x1b[201~";
    for cut in 1..paste.len() {
        let items = decode_split(&paste[..cut], &[&paste[cut..]]);
        assert_eq!(items.len(), 1, "cut {cut}");
        assert!(matches!(items[0].kind, DecodedKind::Paste { .. }));
        assert_eq!(items[0].raw.as_ref(), paste);
    }
}

#[test]
fn stray_paste_terminator_is_a_nondecision() {
    let items = decode(b"\x1b[201~");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
}

#[test]
fn a_csi_over_the_byte_bound_drains_until_its_final_byte() {
    let now = Instant::now();
    let mut decoder = LegacyDecoder::default();
    let mut items = Vec::new();
    items.extend(decoder.push(0x1b, now, false));
    items.extend(decoder.push(b'[', now, false));
    // 2 (ESC+[) + 62 digits reaches the 64-byte run bound.
    for _ in 0..62 {
        items.extend(decoder.push(b'9', now, false));
    }
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.len(), 64);
    // The run is still open: over-bound tail bytes stay owned — even a
    // `Ctrl+B` tail cannot surface as a fresh binding — until a CSI final
    // byte releases the frame.
    for byte in [0x02, b'9', b' '] {
        assert!(decoder.push(byte, now, false).is_empty());
    }
    let items = decoder.push(b'q', now, false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), &[0x02, b'9', b' ', b'q']);
    // Only input after the reliable boundary is fresh again.
    let items = decoder.push(b'a', now, false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Char('a')));
}

#[test]
fn a_truncated_csi_releases_at_the_deadline_and_drains_its_tail() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    assert!(decoder.push(0x1b, start, false).is_empty());
    assert!(decoder.push(b'[', start, false).is_empty());
    assert!(decoder.push(b'3', start, false).is_empty());
    // Before the deadline the frame still waits for its final byte.
    assert!(decoder.tick(start + Duration::from_millis(49)).is_empty());
    // At the deadline the collected bytes emit once as non-decision and the
    // tail drains: late bytes cannot become fresh bindings.
    let items = decoder.tick(start + Duration::from_millis(50));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[3");
    assert!(
        decoder
            .push(b'1', start + Duration::from_millis(60), false)
            .is_empty()
    );
    // The late final byte ends the drained frame; only later input is fresh.
    let items = decoder.push(b'm', start + Duration::from_millis(60), false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), b"1m");
    let items = decoder.push(b'z', start + Duration::from_millis(60), false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Char('z')));
}

#[test]
fn an_esc_inside_a_csi_aborts_it_and_refeeds_as_fresh_input() {
    let items = decode(b"\x1b[3\x1b[31m");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[3");
    // The aborting ESC opens a new frame; its bytes never degrade into
    // printable residue.
    assert_eq!(items[1].kind, DecodedKind::Nondecision);
    assert_eq!(items[1].raw.as_ref(), b"\x1b[31m");
}

#[test]
fn an_ss3_modifier_param_is_decoded_not_dropped() {
    let items = decode(b"\x1bO5A");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(KeyCode::Up, Modifiers::CTRL))
    );
    // An invalid param form is a non-decision, not a silent unmodified key.
    let items = decode(b"\x1bO;A");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
}

#[test]
fn a_utf8_continuation_is_never_a_paste_terminator() {
    let items = decode(b"\x1b[200~ab\xc2\x9b201~\x02q\x1b[201~");
    assert_eq!(items.len(), 1);
    let DecodedKind::Paste { text } = &items[0].kind else {
        panic!("expected paste, got {:?}", items[0].kind);
    };
    assert_eq!(text.as_deref(), Some("ab\u{9b}201~\x02q"));
    assert_eq!(
        items[0].raw.as_ref(),
        b"\x1b[200~ab\xc2\x9b201~\x02q\x1b[201~"
    );
}

#[test]
fn a_c1_paste_uses_the_c1_terminator() {
    let items = decode(b"\x9b200~ab\x9b201~z");
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0].kind, DecodedKind::Paste { .. }));
    assert_eq!(items[0].raw.as_ref(), b"\x9b200~ab\x9b201~");
    assert_eq!(items[1].kind, press(KeyCode::Char('z')));
}

#[test]
fn a_paste_terminator_split_at_the_raw_bound_still_completes() {
    // Every possible split of the terminator across the raw bound: the
    // in-flight match state survives the cap drain instead of losing the
    // already-emitted prefix bytes.
    for split in 0..=6usize {
        let mut decoder = LegacyDecoder::default();
        let now = Instant::now();
        let mut bytes = b"\x1b[200~".to_vec();
        let term = b"\x1b[201~";
        let payload = 4 * 1024 * 1024 - 6 - split;
        bytes.extend(std::iter::repeat_n(b'a', payload));
        bytes.extend_from_slice(&term[..split]);
        assert_eq!(bytes.len(), 4 * 1024 * 1024);
        let mut items = Vec::new();
        for byte in &bytes {
            items.extend(decoder.push(*byte, now, false));
        }
        for byte in &term[split..] {
            items.extend(decoder.push(*byte, now, false));
        }
        if split == 6 {
            // The terminator ending exactly at the bound still emits the
            // atomic paste item, never a drain.
            assert_eq!(items.len(), 1);
            assert!(matches!(items[0].kind, DecodedKind::Paste { .. }));
        } else {
            // The run emitted once at the bound and drained; the straddled
            // terminator completes it.
            assert!(items.iter().all(|i| i.kind == DecodedKind::Nondecision
                || matches!(i.kind, DecodedKind::Paste { .. })));
        }
        let z = decoder.push(b'z', now, false);
        assert_eq!(z.len(), 1, "split {split}");
        assert_eq!(z[0].kind, press(KeyCode::Char('z')));
    }
}

#[test]
fn a_timed_out_paste_still_recognizes_its_late_terminator() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    // The terminator's first bytes sit in the emitted run's tail: the
    // in-flight match state must survive the drain boundary.
    for byte in b"\x1b[200~ab\x1b[20" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }
    // The payload wait expired: collected bytes emit and the run drains.
    let items = decoder.tick(start + Duration::from_millis(50));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[200~ab\x1b[20");
    // A late terminator — including one split across the drain — still ends
    // the run; only bytes after it are fresh.
    let items = decoder.push(b'1', start + Duration::from_millis(60), false);
    assert!(items.is_empty());
    let items = decoder.push(b'~', start + Duration::from_millis(60), false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    let z = decoder.push(b'z', start + Duration::from_millis(60), false);
    assert_eq!(z.len(), 1);
    assert_eq!(z[0].kind, press(KeyCode::Char('z')));
}

#[test]
fn a_c1_paste_terminator_split_at_the_raw_bound_still_completes() {
    // The C1-notation terminator (`9b201~`, five bytes) straddling the raw
    // bound keeps its in-flight match through the drain, same as the ESC
    // form — and the payload `\x9b` continuation is still never a lead.
    for split in [0usize, 2, 4, 5] {
        let mut decoder = LegacyDecoder::default();
        let now = Instant::now();
        let mut bytes = b"\x9b200~".to_vec();
        let term = b"\x9b201~";
        // A `\xc2\x9b` payload pair ahead of the boundary keeps the UTF-8
        // continuation distinction exercised on the cap path too.
        let payload = 4 * 1024 * 1024 - 5 - split;
        bytes.extend(std::iter::repeat_n(b'a', payload - 2));
        bytes.extend_from_slice(b"\xc2\x9b");
        bytes.extend_from_slice(&term[..split]);
        assert_eq!(bytes.len(), 4 * 1024 * 1024);
        let mut items = Vec::new();
        for byte in &bytes {
            items.extend(decoder.push(*byte, now, false));
        }
        for byte in &term[split..] {
            items.extend(decoder.push(*byte, now, false));
        }
        if split == 5 {
            assert_eq!(items.len(), 1);
            assert!(matches!(items[0].kind, DecodedKind::Paste { .. }));
        } else {
            assert!(items.iter().all(|i| i.kind == DecodedKind::Nondecision
                || matches!(i.kind, DecodedKind::Paste { .. })));
        }
        let z = decoder.push(b'z', now, false);
        assert_eq!(z.len(), 1, "split {split}");
        assert_eq!(z[0].kind, press(KeyCode::Char('z')));
    }
}

#[test]
fn a_c1_paste_terminator_is_not_a_utf8_continuation() {
    // A `\x9b` continuation byte inside a valid UTF-8 payload is data: the
    // run ends only at the real C1 terminator.
    let items = decode(b"\x9b200~ab\xc2\x9b201~\x02q\x9b201~z");
    assert_eq!(items.len(), 2);
    let DecodedKind::Paste { text } = &items[0].kind else {
        panic!("expected paste, got {:?}", items[0].kind);
    };
    assert_eq!(text.as_deref(), Some("ab\u{9b}201~\x02q"));
    assert_eq!(
        items[0].raw.as_ref(),
        b"\x9b200~ab\xc2\x9b201~\x02q\x9b201~"
    );
    assert_eq!(items[1].kind, press(KeyCode::Char('z')));
}

#[test]
fn ss3_uses_c1_8f_and_8d_is_never_a_sequence_lead() {
    let items = decode(&[0x8f, b'A']);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Up));
    let items = decode(&[0x8f, b'5', b'A']);
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(KeyCode::Up, Modifiers::CTRL))
    );
    // 0x8d is RI, not SS3: it is one non-decision byte and `A` stays a key.
    let items = decode(&[0x8d, b'A']);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.as_ref(), &[0x8d]);
    assert_eq!(items[1].kind, press(KeyCode::Char('A')));
}

#[test]
fn an_owned_frame_consumed_in_a_captured_packet_stays_owned_across_reads() {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    // The decision byte `y` and the frame lead arrive in the captured
    // packet; the decoder is fed the packet bytes with `owned` provenance.
    assert_eq!(decoder.push(b'y', now, true).len(), 1);
    assert!(decoder.push(0x1b, now, true).is_empty());
    // The frame completes in a later, uncaptured read — still owned.
    assert!(decoder.push(b'[', now, false).is_empty());
    let items = decoder.push(b'A', now, false);
    assert_eq!(items.len(), 1);
    assert!(items[0].owned);
    assert_eq!(items[0].kind, press(KeyCode::Up));
    // A genuinely later key is not owned.
    let items = decoder.push(b'z', now, false);
    assert_eq!(items.len(), 1);
    assert!(!items[0].owned);
}

// ---- Host mouse reports: SGR and X10 at the decoder layer ----

#[test]
fn a_valid_sgr_report_decodes_to_one_atomic_mouse_item() {
    let items = decode(b"\x1b[<0;5;9M");
    assert_eq!(items.len(), 1);
    let DecodedKind::Mouse(event) = items[0].kind else {
        panic!("expected a mouse item, got {:?}", items[0].kind)
    };
    assert_eq!(event.action, MouseAction::Press);
    assert_eq!(event.button, MouseButton::Left);
    // Wire coordinates are 1-based; decoded cells are 0-based.
    assert_eq!((event.column, event.row), (4, 8));
    assert_eq!(items[0].raw.as_ref(), b"\x1b[<0;5;9M");
}

#[test]
fn a_valid_x10_report_decodes_atomically_across_split_feeds() {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    // Head and body arrive split; mid-frame pushes emit nothing.
    for byte in [0x1b, b'[', b'M', 0x20] {
        assert!(decoder.push(byte, now, false).is_empty());
    }
    let mut items = decoder.push(0x25, now, false);
    items.extend(decoder.push(0x29, now, false));
    assert_eq!(items.len(), 1);
    let DecodedKind::Mouse(event) = items[0].kind else {
        panic!("expected a mouse item, got {:?}", items[0].kind)
    };
    assert_eq!(event.action, MouseAction::Press);
    assert_eq!(event.button, MouseButton::Left);
    assert_eq!((event.column, event.row), (4, 8));
    assert_eq!(items[0].raw.as_ref(), &[0x1b, b'[', b'M', 0x20, 0x25, 0x29]);
}

#[test]
fn a_timed_out_x10_frame_drops_and_drains_its_bounded_tail() {
    let mut decoder = LegacyDecoder::default();
    let start = Instant::now();
    for byte in [0x1b, b'[', b'M', 0x20] {
        assert!(decoder.push(byte, start, false).is_empty());
    }
    // Nothing resolves before the frame's deadline.
    assert!(decoder.tick(start + Duration::from_millis(49)).is_empty());
    // At the deadline the partial frame is dropped pointer protocol.
    let items = decoder.tick(start + Duration::from_millis(50));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), &[0x1b, b'[', b'M', 0x20]);
    // The frame's two remaining body bytes drain owned — never fresh keys.
    assert!(
        decoder
            .push(b'x', start + Duration::from_millis(60), false)
            .is_empty()
    );
    let items = decoder.push(b'y', start + Duration::from_millis(60), false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), b"xy");
    // And the decoder is live for real keys again.
    let items = decoder.push(b'z', start + Duration::from_millis(60), false);
    assert_eq!(items[0].kind, press(KeyCode::Char('z')));
}

#[test]
fn an_esc_aborts_an_x10_frame_and_refeeds_as_fresh_input() {
    let items = decode(&[0x1b, b'[', b'M', 0x20, 0x1b, b'[', b'A']);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), &[0x1b, b'[', b'M', 0x20]);
    assert_eq!(items[1].kind, press(KeyCode::Up));
}

#[test]
fn a_c1_lead_aborts_an_x10_frame_and_refeeds() {
    let items = decode(&[0x1b, b'[', b'M', 0x20, 0x9b, b'A']);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[1].kind, press(KeyCode::Up));
}

#[test]
fn a_malformed_x10_body_is_ignored_not_keys() {
    // A coordinate byte under the 1-based wire offset fails the body.
    let items = decode(&[0x1b, b'[', b'M', 0x20, 0x20, 0x29]);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), &[0x1b, b'[', b'M', 0x20, 0x20, 0x29]);
}

#[test]
fn malformed_sgr_reports_are_ignored_atomically() {
    // Coordinate zero is outside the 1-based domain.
    let items = decode(b"\x1b[<0;0;1M");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[<0;0;1M");
    // A parameter field over the five-digit bound.
    let items = decode(b"\x1b[<0;111111;1M");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[<0;111111;1M");
    // A parameter-range non-digit field and an empty field.
    let items = decode(b"\x1b[<0;?;1M");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    let items = decode(b"\x1b[<0;;1M");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    // An overlong field on the release final.
    let items = decode(b"\x1b[<999999;1;1m");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
}

#[test]
fn an_aborted_sgr_mouse_partial_is_dropped_and_the_abort_refeeds() {
    let items = decode(b"\x1b[<0;\x1b[A");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[<0;");
    assert_eq!(items[1].kind, press(KeyCode::Up));
}

#[test]
fn a_timed_out_sgr_mouse_partial_drains_as_ignored() {
    let mut decoder = LegacyDecoder::default();
    let start = Instant::now();
    for byte in b"\x1b[<0;" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }
    let items = decoder.tick(start + Duration::from_millis(50));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), b"\x1b[<0;");
    // The late remainder drains under pointer ownership until a final byte.
    assert!(
        decoder
            .push(b'0', start + Duration::from_millis(60), false)
            .is_empty()
    );
    let items = decoder.push(b'M', start + Duration::from_millis(60), false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), b"0M");
    let items = decoder.push(b'z', start + Duration::from_millis(60), false);
    assert_eq!(items[0].kind, press(KeyCode::Char('z')));
}

#[test]
fn an_overbound_mouse_shaped_run_drains_as_ignored_but_plain_csi_passes() {
    let now = Instant::now();
    // `\x1b[<` + 61 digits reaches the 64-byte run bound mid-field.
    let mut decoder = LegacyDecoder::default();
    let mut run = b"\x1b[<".to_vec();
    run.extend(std::iter::repeat_n(b'9', 61));
    let mut items = Vec::new();
    for byte in &run {
        items.extend(decoder.push(*byte, now, false));
    }
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.len(), 64);
    // The tail keeps draining as pointer protocol until a final byte.
    assert!(decoder.push(b'9', now, false).is_empty());
    let items = decoder.push(b'~', now, false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Ignored);
    assert_eq!(items[0].raw.as_ref(), b"9~");

    // A plain over-bound CSI stays a passthrough non-decision — the drain
    // is identical, only the emitted kind differs.
    let mut decoder = LegacyDecoder::default();
    let mut run = b"\x1b[".to_vec();
    run.extend(std::iter::repeat_n(b'9', 62));
    let mut items = Vec::new();
    for byte in &run {
        items.extend(decoder.push(*byte, now, false));
    }
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.len(), 64);
    let items = decoder.push(b'~', now, false);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
}

#[test]
fn malformed_mouse_bytes_never_swallow_neighboring_keys_or_paste() {
    let items = decode(b"a\x1b[<0;0;1Mb\x1b[200~hi\x1b[201~c");
    assert_eq!(items.len(), 5);
    assert_eq!(items[0].kind, press(KeyCode::Char('a')));
    assert_eq!(items[1].kind, DecodedKind::Ignored);
    assert_eq!(items[1].raw.as_ref(), b"\x1b[<0;0;1M");
    assert_eq!(items[2].kind, press(KeyCode::Char('b')));
    let DecodedKind::Paste { text } = &items[3].kind else {
        panic!("expected a paste item, got {:?}", items[3].kind)
    };
    assert_eq!(text.as_deref(), Some("hi"));
    assert_eq!(items[4].kind, press(KeyCode::Char('c')));
}

#[test]
fn focus_reports_survive_every_packet_split_and_paste_keeps_them_as_data() {
    for bytes in [b"\x1b[I\x1b[O".as_slice(), b"\x9bI\x9bO".as_slice()] {
        for split in 0..=bytes.len() {
            let items = decode_split(&bytes[..split], &[&bytes[split..]]);
            assert_eq!(
                items.iter().map(|item| &item.kind).collect::<Vec<_>>(),
                [&DecodedKind::Focus(true), &DecodedKind::Focus(false)]
            );
        }
    }
    let items = decode(b"\x1b[200~a\x1b[Ob\x1b[Ic\x1b[201~");
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0].kind, DecodedKind::Paste { .. }));
    assert_eq!(&*items[0].raw, b"\x1b[200~a\x1b[Ob\x1b[Ic\x1b[201~");
    assert!(
        decode(b"\x1b[1I\x1b[1O")
            .iter()
            .all(|item| !matches!(item.kind, DecodedKind::Focus(_)))
    );
}

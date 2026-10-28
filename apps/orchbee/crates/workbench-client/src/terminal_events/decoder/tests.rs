use std::time::{Duration, Instant};

use super::*;
use crate::terminal_events::{MouseAction, MouseButton};

fn decode(bytes: &[u8]) -> Vec<DecodedItem> {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    let mut items = Vec::new();
    for byte in bytes {
        items.extend(decoder.push(*byte, now, false));
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
    for scalar in ['中', '😀'] {
        let text = scalar.to_string();
        let items = decode(text.as_bytes());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, press(KeyCode::Char(scalar)));
        assert_eq!(items[0].raw.as_ref(), text.as_bytes());
    }
}

#[test]
fn arrow_csi_decodes_as_one_item() {
    let items = decode(b"\x1b[D");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Left));
    assert_eq!(items[0].raw.as_ref(), b"\x1b[D");
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
    assert_eq!(text.as_str(), "ab\x02");
    assert_eq!(items[0].raw.as_ref(), b"\x1b[200~ab\x02\x1b[201~");
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
    assert_eq!(text.as_str(), "ab\u{9b}201~\x02q");
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
fn rejected_paste_at_every_raw_cap_terminator_split_releases_only_fresh_input() {
    for (start, term) in [
        (b"\x1b[200~".as_slice(), b"\x1b[201~".as_slice()),
        (b"\x9b200~".as_slice(), b"\x9b201~".as_slice()),
    ] {
        for split in 0..=term.len() {
            let mut decoder = LegacyDecoder::default();
            let now = Instant::now();
            let owned = split % 2 == 0;
            for (index, &byte) in start.iter().enumerate() {
                assert!(decoder.push(byte, now, owned && index == 0).is_empty());
            }
            let super::State::Paste { raw, .. } = &mut decoder.state else {
                panic!("expected paste after opener");
            };
            // ASCII padding leaves terminator and UTF-8 state unchanged. Prepare
            // the near-cap prefix once, then feed every boundary byte normally.
            raw.resize(4 * 1024 * 1024 - split - 2, b'a');
            let mut items = Vec::new();
            for &byte in b"\xc2\x9b".iter().chain(&term[..split]) {
                items.extend(decoder.push(byte, now, false));
            }
            assert_eq!(items.len(), 1, "{start:?}, split {split}");
            assert_eq!(items[0].kind, DecodedKind::RejectedPaste);
            assert!(items[0].raw.is_empty());
            assert_eq!(items[0].owned, owned);
            for &byte in &term[split..] {
                assert!(decoder.push(byte, now, false).is_empty());
            }
            let fresh = decoder.push(b'z', now, false);
            assert_eq!(fresh.len(), 1);
            assert_eq!(fresh[0].kind, press(KeyCode::Char('z')));
            assert!(!fresh[0].owned);
        }
    }
}

#[test]
fn rejected_paste_at_every_timeout_terminator_split_emits_nothing_more() {
    for (start, term) in [
        (b"\x1b[200~".as_slice(), b"\x1b[201~".as_slice()),
        (b"\x9b200~".as_slice(), b"\x9b201~".as_slice()),
    ] {
        for split in 0..term.len() {
            let now = Instant::now();
            let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
            for &byte in start.iter().chain(b"payload\x02q").chain(&term[..split]) {
                assert!(decoder.push(byte, now, true).is_empty());
            }
            let items = decoder.tick(now + Duration::from_millis(50));
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].kind, DecodedKind::RejectedPaste);
            assert!(items[0].raw.is_empty());
            assert!(items[0].owned);
            assert!(decoder.tick(now + Duration::from_millis(60)).is_empty());
            for &byte in &term[split..] {
                assert!(
                    decoder
                        .push(byte, now + Duration::from_millis(70), false)
                        .is_empty()
                );
            }
            let fresh = decoder.push(b'z', now + Duration::from_millis(70), false);
            assert_eq!(fresh.len(), 1);
            assert_eq!(fresh[0].kind, press(KeyCode::Char('z')));
            assert!(!fresh[0].owned);
        }
    }
}

#[test]
fn rejected_paste_discards_invalid_utf8_and_text_overflow_without_retaining_raw_bytes() {
    for payload in [vec![0xff, 0x02, b'q'], vec![b'x'; 256 * 1024 + 1]] {
        for (start, term) in [
            (b"\x1b[200~".as_slice(), b"\x1b[201~".as_slice()),
            (b"\x9b200~".as_slice(), b"\x9b201~".as_slice()),
        ] {
            let mut bytes = start.to_vec();
            bytes.extend_from_slice(&payload);
            bytes.extend_from_slice(term);
            bytes.push(b'z');
            let items = decode(&bytes);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].kind, DecodedKind::RejectedPaste);
            assert!(items[0].raw.is_empty());
            assert_eq!(items[1].kind, press(KeyCode::Char('z')));
        }
    }
    for size in [0, 256 * 1024] {
        let text = "x".repeat(size);
        let raw = format!("\x1b[200~{text}\x1b[201~");
        let items = decode(raw.as_bytes());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, DecodedKind::Paste { text });
        assert_eq!(items[0].raw.as_ref(), raw.as_bytes());
    }
}

#[test]
fn rejected_c1_paste_keeps_utf8_continuations_and_other_notation_inside_the_tail() {
    let now = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    for &byte in b"\x9b200~\xe4" {
        assert!(decoder.push(byte, now, false).is_empty());
    }
    let rejected = decoder.tick(now + Duration::from_millis(50));
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].kind, DecodedKind::RejectedPaste);
    assert!(rejected[0].raw.is_empty());
    for &byte in b"\x9b201~\x1b[201~\x02q" {
        assert!(
            decoder
                .push(byte, now + Duration::from_millis(60), false)
                .is_empty()
        );
    }
    decoder.capture_pending();
    for &byte in b"\x9b201~" {
        assert!(
            decoder
                .push(byte, now + Duration::from_millis(70), false)
                .is_empty()
        );
    }
    let fresh = decoder.push(b'z', now + Duration::from_millis(70), false);
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].kind, press(KeyCode::Char('z')));
    assert!(!fresh[0].owned);
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
    assert_eq!(text.as_str(), "ab\u{9b}201~\x02q");
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
    assert_eq!(text.as_str(), "hi");
    assert_eq!(items[4].kind, press(KeyCode::Char('c')));
}

#[test]
fn focus_reports_decode_in_both_forms_and_paste_keeps_them_as_data() {
    for bytes in [b"\x1b[I\x1b[O".as_slice(), b"\x9bI\x9bO".as_slice()] {
        let items = decode(bytes);
        assert_eq!(
            items.iter().map(|item| &item.kind).collect::<Vec<_>>(),
            [&DecodedKind::Focus(true), &DecodedKind::Focus(false)]
        );
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

#[test]
fn pointer_admission_replies_decode_strictly_in_escape_and_c1_forms() {
    use crate::terminal_events::HostReply;
    use crate::terminal_events::pointer::{HostReply as PointerReply, ModeReportState};

    for (status, state) in [
        (b'0', ModeReportState::Unsupported),
        (b'1', ModeReportState::Set),
        (b'2', ModeReportState::Reset),
        (b'3', ModeReportState::PermanentlySet),
        (b'4', ModeReportState::PermanentlyReset),
    ] {
        for prefix in [b"\x1b[".as_slice(), b"\x9b".as_slice()] {
            let mut sequence = prefix.to_vec();
            sequence.extend_from_slice(b"?1003;");
            sequence.push(status);
            sequence.extend_from_slice(b"$y");
            let items = decode(&sequence);
            assert_eq!(items.len(), 1);
            assert_eq!(
                items[0].kind,
                DecodedKind::HostReply(HostReply::Pointer(PointerReply::Mode1003(state)))
            );
        }
    }

    for sequence in [b"\x1b[?1;2c".as_slice(), b"\x9b?64;1;2;6;9c"] {
        assert_eq!(
            decode(sequence)[0].kind,
            DecodedKind::HostReply(HostReply::Pointer(PointerReply::PrimaryDeviceAttributes))
        );
    }
}

#[test]
fn appearance_replies_are_typed_and_never_reach_key_input() {
    use crate::terminal_events::HostReply;
    use crate::terminal_events::appearance::{ColorReply, ColorTarget};

    let items = decode(b"a\x1b]10;rgb:ffff/ffff/ffff\x1b\\\x1b]11;rgb:1010/2020/3030\x1b\\q");
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].kind, press(KeyCode::Char('a')));
    assert_eq!(
        items[1].kind,
        DecodedKind::HostReply(HostReply::Appearance(ColorReply {
            target: ColorTarget::Foreground,
            rgb: Some([255, 255, 255]),
        }))
    );
    assert_eq!(
        items[2].kind,
        DecodedKind::HostReply(HostReply::Appearance(ColorReply {
            target: ColorTarget::Background,
            rgb: Some([16, 32, 48]),
        }))
    );
    assert_eq!(items[3].kind, press(KeyCode::Char('q')));
}

#[test]
fn malformed_and_oversized_appearance_replies_are_consumed_atomically() {
    use crate::terminal_events::HostReply;
    use crate::terminal_events::appearance::{ColorReply, ColorTarget};

    let malformed = decode(b"\x1b]11;not-rgb\x07x");
    assert_eq!(malformed.len(), 2);
    assert_eq!(
        malformed[0].kind,
        DecodedKind::HostReply(HostReply::Appearance(ColorReply {
            target: ColorTarget::Background,
            rgb: None,
        }))
    );
    assert_eq!(malformed[1].kind, press(KeyCode::Char('x')));

    let mut oversized = b"\x1b]11;".to_vec();
    oversized.extend(std::iter::repeat_n(b'a', 128));
    oversized.extend_from_slice(b"\x07y");
    let oversized = decode(&oversized);
    assert_eq!(oversized.len(), 2);
    assert_eq!(
        oversized[0].kind,
        DecodedKind::HostReply(HostReply::Appearance(ColorReply {
            target: ColorTarget::Background,
            rgb: None,
        }))
    );
    assert_eq!(oversized[1].kind, press(KeyCode::Char('y')));
}

#[test]
fn timed_out_appearance_reply_drains_late_tail_before_resuming_keys() {
    let now = Instant::now();
    let mut decoder = LegacyDecoder::default();
    for byte in b"\x1b]11" {
        assert!(decoder.push(*byte, now, false).is_empty());
    }
    assert!(decoder.push(b';', now, false).is_empty());
    for byte in b"rgb:" {
        assert!(decoder.push(*byte, now, false).is_empty());
    }
    assert!(decoder.tick(now + Duration::from_secs(2)).is_empty());
    for byte in b"ffff/ffff/ffff\x1b\\" {
        assert!(
            decoder
                .push(*byte, now + Duration::from_secs(2), false)
                .is_empty()
        );
    }
    assert_eq!(
        decoder.push(b'z', now + Duration::from_secs(2), false)[0].kind,
        press(KeyCode::Char('z'))
    );
}

#[test]
fn appearance_drain_preserves_a_split_st_after_frame_timeout() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::default();
    for byte in b"\x1b]11;rgb:1010\x1b" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }

    assert!(decoder.tick(start + Duration::from_secs(1)).is_empty());
    assert!(
        decoder
            .push(b'\\', start + Duration::from_millis(1_001), false)
            .is_empty()
    );
    assert_eq!(
        decoder.push(b'z', start + Duration::from_millis(1_002), false)[0].kind,
        press(KeyCode::Char('z'))
    );
}

#[test]
fn a_truncated_appearance_tail_releases_after_idle_and_resumes_keys() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::default();
    for byte in b"\x1b]11;rgb:1010" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }

    assert!(decoder.tick(start + Duration::from_secs(1)).is_empty());
    assert!(
        decoder
            .tick(start + Duration::from_millis(1_051))
            .is_empty()
    );
    assert_eq!(
        decoder.push(b'z', start + Duration::from_millis(1_052), false)[0].kind,
        press(KeyCode::Char('z'))
    );
}

#[test]
fn an_oversized_appearance_tail_releases_at_its_byte_cap() {
    use crate::terminal_events::HostReply;
    use crate::terminal_events::appearance::{ColorReply, ColorTarget};

    let start = Instant::now();
    let mut decoder = LegacyDecoder::default();
    for byte in b"\x1b]11;" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }
    let mut overflow = Vec::new();
    for byte in std::iter::repeat_n(b'a', 97) {
        overflow.extend(decoder.push(byte, start, false));
    }
    assert_eq!(
        overflow.last().map(|item| &item.kind),
        Some(&DecodedKind::HostReply(HostReply::Appearance(ColorReply {
            target: ColorTarget::Background,
            rgb: None,
        })))
    );

    let mut resumed = false;
    for _ in 0..256 {
        if decoder
            .push(b'x', start, false)
            .iter()
            .any(|item| item.kind == press(KeyCode::Char('x')))
        {
            resumed = true;
            break;
        }
    }
    assert!(
        resumed,
        "ordinary keys resume after the bounded OSC tail cap"
    );
    assert_eq!(
        decoder.push(b'z', start, false)[0].kind,
        press(KeyCode::Char('z'))
    );
}

#[test]
fn an_appearance_drain_has_a_hard_deadline_even_while_bytes_keep_arriving() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::default();
    for byte in b"\x1b]11;" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }
    for byte in std::iter::repeat_n(b'a', 97) {
        decoder.push(byte, start, false);
    }

    for tick in 1..=24 {
        let now = start + Duration::from_millis(tick * 40);
        assert!(decoder.push(b'x', now, false).is_empty());
    }
    assert!(
        decoder
            .tick(start + Duration::from_millis(1_001))
            .is_empty()
    );
    assert_eq!(
        decoder.push(b'z', start + Duration::from_millis(1_002), false)[0].kind,
        press(KeyCode::Char('z'))
    );
}

#[test]
fn appearance_frame_deadline_is_enforced_per_byte_without_ticks() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::default();
    for byte in b"\x1b]11;" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }

    // Repeated ESC bytes retain the frame's pending terminator prefix without
    // increasing its payload count. The next byte at the one-second boundary
    // must move it into the bounded drain even though no idle tick ran.
    for packet in 1..=25 {
        assert!(
            decoder
                .push(b'\x1b', start + Duration::from_millis(packet * 40), false)
                .is_empty()
        );
    }
    assert!(
        decoder
            .push(b'\\', start + Duration::from_millis(1_001), false)
            .is_empty()
    );
    assert_eq!(
        decoder.push(b'z', start + Duration::from_millis(1_002), false)[0].kind,
        press(KeyCode::Char('z'))
    );

    // A C1 OSC prefix has no short ESC ambiguity window, but its one-second
    // frame deadline still releases its buffered non-target selector.
    let c1_start = start + Duration::from_secs(2);
    assert!(decoder.push(0x9d, c1_start, false).is_empty());
    assert!(decoder.push(b'1', c1_start, false).is_empty());
    let resumed = decoder.push(b'1', c1_start + Duration::from_secs(1), false);
    assert_eq!(
        resumed
            .iter()
            .filter(|item| item.kind == press(KeyCode::Char('1')))
            .count(),
        2
    );
}

#[test]
fn an_unrecognized_alt_right_bracket_keeps_its_escape_deadline() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::default();
    assert!(decoder.push(0x1b, start, false).is_empty());
    assert!(
        decoder
            .push(b']', start + Duration::from_millis(1), false)
            .is_empty()
    );
    let items = decoder.push(b'1', start + Duration::from_millis(51), false);
    assert_eq!(
        items.iter().map(|item| &item.kind).collect::<Vec<_>>(),
        [
            &DecodedKind::Key(KeyEvent::press(
                KeyCode::Char(']'),
                Modifiers::NONE.with_alt(),
            )),
            &press(KeyCode::Char('1')),
        ]
    );
    assert_eq!(
        decoder.push(b'z', start + Duration::from_millis(52), false)[0].kind,
        press(KeyCode::Char('z'))
    );
}

#[test]
fn unrelated_alt_right_bracket_retains_its_existing_key_meaning() {
    let items = decode(b"\x1b]x");
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].kind,
        DecodedKind::Key(KeyEvent::press(
            KeyCode::Char(']'),
            Modifiers::NONE.with_alt(),
        ))
    );
    assert_eq!(items[1].kind, press(KeyCode::Char('x')));
}

#[test]
fn malformed_pointer_admission_metadata_is_consumed_without_neighbor_leaks() {
    for sequence in [
        b"\x1b[?1002;2$y".as_slice(),
        b"\x1b[?1003;5$y",
        b"\x1b[?1003;02$y",
        b"\x1b[?1003;;2$y",
        b"\x1b[?c",
        b"\x1b[?1;;2c",
        b"\x1b[?1;:c",
    ] {
        let items = decode(sequence);
        assert_eq!(items.len(), 1, "{sequence:?}");
        assert_eq!(items[0].kind, DecodedKind::Ignored, "{sequence:?}");
    }

    let items = decode(b"a\x1b[?1003;2$yb");
    assert_eq!(items[0].kind, press(KeyCode::Char('a')));
    assert!(matches!(items[1].kind, DecodedKind::HostReply(_)));
    assert_eq!(items[2].kind, press(KeyCode::Char('b')));

    let items = decode(b"\x1b[200~\x1b[?1003;2$y\x1b[?1;2c\x1b[201~");
    assert_eq!(items.len(), 1);
    let DecodedKind::Paste { text } = &items[0].kind else {
        panic!("expected literal paste")
    };
    assert_eq!(text, "\x1b[?1003;2$y\x1b[?1;2c");
}

#[test]
fn private_csi_without_a_reply_final_keeps_bounded_passthrough() {
    let start = Instant::now();
    let mut decoder = LegacyDecoder::default();
    for byte in b"\x1b[?1003;" {
        assert!(decoder.push(*byte, start, false).is_empty());
    }
    let mut items = decoder.tick(start + Duration::from_secs(1));
    for byte in b"2$" {
        items.extend(decoder.push(*byte, start + Duration::from_secs(1), false));
    }
    items.extend(decoder.push(b'y', start + Duration::from_secs(1), false));
    assert!(
        items
            .iter()
            .all(|item| item.kind == DecodedKind::Nondecision)
    );
    assert_eq!(
        items
            .iter()
            .flat_map(|item| item.raw.iter().copied())
            .collect::<Vec<_>>(),
        b"\x1b[?1003;2$y"
    );
    assert_eq!(
        decoder.push(b'z', start + Duration::from_secs(1), false)[0].kind,
        press(KeyCode::Char('z'))
    );

    let mut decoder = LegacyDecoder::default();
    let mut run = b"\x1b[?".to_vec();
    run.extend(std::iter::repeat_n(b'9', 61));
    let mut items = Vec::new();
    for byte in run {
        items.extend(decoder.push(byte, start, false));
    }
    items.extend(decoder.push(b'9', start, false));
    items.extend(decoder.push(b'c', start, false));
    assert!(
        items
            .iter()
            .all(|item| item.kind == DecodedKind::Nondecision)
    );
    let mut expected = b"\x1b[?".to_vec();
    expected.extend(std::iter::repeat_n(b'9', 62));
    expected.push(b'c');
    assert_eq!(
        items
            .iter()
            .flat_map(|item| item.raw.iter().copied())
            .collect::<Vec<_>>(),
        expected
    );
}

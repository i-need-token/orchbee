use std::time::{Duration, Instant};

use super::*;

fn decode(bytes: &[u8]) -> Vec<DecodedItem> {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    let mut items = Vec::new();
    for byte in bytes {
        items.extend(decoder.push(*byte, now));
    }
    items
}

fn decode_split(first: &[u8], rest: &[&[u8]]) -> Vec<DecodedItem> {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    let mut items = Vec::new();
    for byte in first {
        items.extend(decoder.push(*byte, now));
    }
    for chunk in rest {
        for byte in chunk.iter() {
            items.extend(decoder.push(*byte, now));
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
    assert!(decoder.push(0x1b, start).is_empty());
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
    assert!(decoder.push(0x1b, now).is_empty());
    assert!(decoder.push(b'[', now).is_empty());
    let items = decoder.push(b'D', now);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Left));
}

#[test]
fn double_escape_yields_esc_then_restarts_the_wait() {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    assert!(decoder.push(0x1b, now).is_empty());
    let items = decoder.push(0x1b, now);
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
fn truncated_csi_waits_bounded_by_bytes_then_drains() {
    let now = Instant::now();
    let mut decoder = LegacyDecoder::default();
    let mut items = Vec::new();
    items.extend(decoder.push(0x1b, now));
    items.extend(decoder.push(b'[', now));
    // 2 (ESC+[) + 62 digits reaches the 64-byte run bound.
    for _ in 0..62 {
        items.extend(decoder.push(b'9', now));
    }
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, DecodedKind::Nondecision);
    assert_eq!(items[0].raw.len(), 64);
    // Bytes after the drain are fresh ground input again.
    let items = decoder.push(b'a', now);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, press(KeyCode::Char('a')));
}

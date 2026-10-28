use super::*;
use crate::terminal_events::{DecodedKind, HostReply, LegacyDecoder};

fn framed(body: &[u8], timeout: bool) -> Option<Reply> {
    let now = Instant::now();
    let wait = Duration::from_millis(50);
    let mut reply = ReplyString::new(now, wait);
    for byte in body {
        assert!(reply.push(*byte, now).is_none());
    }
    if timeout {
        reply.tick(now + wait);
    }
    let finish = now + if timeout { wait } else { Duration::ZERO };
    assert!(reply.push(0x1b, finish).is_none());
    reply.push(b'\\', finish).expect("terminator")
}

#[test]
fn bounded_graphics_strings_reject_malformed_oversized_and_late_replies() {
    assert_eq!(
        framed(b"i=4294967295,p=1;OK", false),
        Some(Reply::Kitty {
            placement_id: Some(1),
            image_id: u32::MAX,
            success: true
        })
    );
    assert_eq!(
        framed(b"i=31;ENOENT: unavailable", false),
        Some(Reply::Kitty {
            placement_id: None,
            image_id: 31,
            success: false
        })
    );
    for body in [
        b"i=0;OK".as_slice(),
        b"i=1,i=2;OK",
        b"i=4294967296;OK",
        b"i=1;",
        b"I=1;OK",
        b"i=+1;OK",
        b"i=1;O\x01K",
    ] {
        assert_eq!(framed(body, false), None, "{body:?}");
    }
    let exact = [b"i=1;".as_slice(), &vec![b'E'; REPLY_BOUND - 4]].concat();
    assert_eq!(
        framed(&exact, false),
        Some(Reply::Kitty {
            placement_id: None,
            image_id: 1,
            success: false
        })
    );
    assert!(framed(&[exact, vec![b'E']].concat(), false).is_none());
    assert!(framed(b"i=31;OK", true).is_none());
    assert_eq!(cell_reply(b"6;16;8t"), Some(Reply::Cell(Some((8, 16)))));
    for body in [
        b"6;0;8t".as_slice(),
        b"6;16;65536t",
        b"6;16;8;1t",
        b"6;+16;8t",
    ] {
        assert_eq!(cell_reply(body), Some(Reply::Cell(None)));
    }
    assert_eq!(cell_reply(b"4;384;640t"), None);
}

fn decode(bytes: &[u8]) -> Vec<DecodedKind> {
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    let now = Instant::now();
    bytes
        .iter()
        .flat_map(|byte| decoder.push(*byte, now, false))
        .map(|item| item.kind)
        .collect()
}

#[test]
fn replies_are_metadata_but_reply_shaped_paste_and_alt_underscore_remain_input() {
    use crate::terminal_events::keys::{KeyCode, KeyEvent, Modifiers};
    assert_eq!(
        decode(b"a\x1b[6;16;8t\x1b_Gi=31;OK\x1b\\b"),
        vec![
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('a'), Modifiers::NONE)),
            DecodedKind::HostReply(HostReply::Graphics(Reply::Cell(Some((8, 16))))),
            DecodedKind::HostReply(HostReply::Graphics(Reply::Kitty {
                placement_id: None,
                image_id: 31,
                success: true
            })),
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::NONE)),
        ]
    );
    let payload = "x\x1b[6;16;8t\x1b_Gi=31;OK\x1b\\y";
    assert_eq!(
        decode(format!("\x1b[200~{payload}\x1b[201~").as_bytes()),
        vec![DecodedKind::Paste {
            text: payload.into()
        }]
    );
    assert_eq!(
        decode(b"\x1b_x"),
        vec![
            DecodedKind::Key(KeyEvent::press(
                KeyCode::Char('_'),
                Modifiers::NONE.with_alt()
            )),
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('x'), Modifiers::NONE)),
        ]
    );
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    let now = Instant::now();
    for byte in b"\x1b_Gi=31;" {
        assert!(decoder.push(*byte, now, false).is_empty());
    }
    assert!(decoder.tick(now + Duration::from_millis(60)).is_empty());
    let late = b"OK\x1b\\x"
        .iter()
        .flat_map(|byte| decoder.push(*byte, now + Duration::from_millis(61), false))
        .map(|item| item.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        late,
        vec![
            DecodedKind::Ignored,
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('x'), Modifiers::NONE))
        ]
    );
}

#[test]
fn malformed_cell_report_residue_never_becomes_a_pane_input_packet() {
    let oversized = [b"\x1b[6;".as_slice(), &vec![b'9'; 2048], b";8tx"].concat();
    let items = decode(&oversized);
    assert!(
        items[..items.len() - 1]
            .iter()
            .all(|kind| *kind == DecodedKind::Ignored)
    );
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    let now = Instant::now();
    for byte in b"\x1b[6;16;" {
        assert!(decoder.push(*byte, now, false).is_empty());
    }
    assert!(
        decoder
            .tick(now + Duration::from_millis(60))
            .iter()
            .all(|item| item.kind == DecodedKind::Ignored)
    );
    let items = b"8t"
        .iter()
        .flat_map(|byte| decoder.push(*byte, now + Duration::from_millis(70), false))
        .collect::<Vec<_>>();
    assert!(items.iter().all(|item| item.kind == DecodedKind::Ignored));
}

//! Probe ownership, real reply framing and literal input preservation witnesses.
use super::*;
use crate::terminal_input::{DecodedKind, LegacyDecoder};

fn dimensions(columns: u16, rows: u16) -> WindowDimensions {
    WindowDimensions {
        columns,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[test]
fn current_host_measurements_never_advertise_an_unused_pixel_renderer() {
    let now = Instant::now();
    let mut probe = Probe::default();
    assert!(probe.query((80, 24), now).is_none());
    probe.activate(31);
    let query = probe.query((80, 24), now).expect("startup query");
    assert_eq!(
        query.sequence(),
        "\x1b[16t\x1b_Ga=q,t=d,f=32,s=1,v=1,i=31;AAAAAA==\x1b\\"
    );
    assert!(probe.query((80, 24), now).is_none());
    probe.reply(
        Reply::Kitty {
            placement_id: None,
            image_id: 32,
            success: true,
        },
        now,
    );
    probe.reply(
        Reply::Kitty {
            image_id: 31,
            placement_id: Some(1),
            success: true,
        },
        now,
    );
    assert!(
        !probe.facts().kitty,
        "placement acknowledgement cannot answer the probe"
    );
    assert!(!probe.facts().kitty);
    probe.reply(Reply::Cell(Some((8, 16))), now + Duration::from_millis(10));
    probe.reply(
        Reply::Kitty {
            placement_id: None,
            image_id: 31,
            success: true,
        },
        now + Duration::from_millis(20),
    );
    let facts = probe.facts();
    assert!(facts.kitty);
    let caps = facts.cell_capabilities();
    assert_eq!(caps.transport, PaneGraphicsTransport::CellRaster);
    assert_eq!(
        caps.cell_pixels,
        Some(PaneGraphicsCellPixels {
            width: 8,
            height: 16
        })
    );
    assert!(!caps.pixel_mouse);
    let measured = facts
        .measurement
        .expect("real pixels")
        .apply(dimensions(80, 24));
    assert_eq!((measured.pixel_width, measured.pixel_height), (640, 384));
    assert_eq!(
        facts.measurement.unwrap().apply(dimensions(81, 24)),
        dimensions(81, 24)
    );
    assert_eq!(
        probe.query((81, 24), now + Duration::from_millis(30)),
        Some(Query {
            cell: true,
            image_id: None
        })
    );
    assert!(probe.facts().measurement.is_none());
    probe.reply(
        Reply::Cell(Some((u16::MAX, 16))),
        now + Duration::from_millis(40),
    );
    assert!(probe.facts().measurement.is_none());
    assert!(
        probe
            .query((81, 24), now + Duration::from_millis(41))
            .is_none()
    );
}

#[test]
fn resize_and_timeout_keep_the_uncorrelated_reply_debt_until_its_late_response() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.activate(77);
    assert!(probe.query((80, 24), now).is_some());
    assert!(
        probe
            .query((100, 30), now + Duration::from_millis(10))
            .is_none()
    );
    probe.reply(Reply::Cell(Some((8, 16))), now + Duration::from_millis(20));
    assert!(probe.facts().measurement.is_none());
    assert_eq!(
        probe.query((100, 30), now + Duration::from_millis(21)),
        Some(Query {
            cell: true,
            image_id: None
        })
    );
    probe.reply(Reply::Cell(Some((9, 18))), now + Duration::from_millis(22));
    assert_eq!(
        probe
            .facts()
            .measurement
            .unwrap()
            .apply(dimensions(100, 30))
            .pixel_width,
        900
    );
    probe.reply(
        Reply::Kitty {
            placement_id: None,
            image_id: 77,
            success: true,
        },
        now + QUERY_DEADLINE,
    );
    assert!(!probe.facts().kitty);
    assert!(
        probe
            .query((101, 30), now + Duration::from_secs(1))
            .is_some()
    );
    assert!(
        probe
            .query((102, 30), now + Duration::from_secs(5))
            .is_none()
    );
    assert!(
        probe
            .query((103, 30), now + Duration::from_secs(500))
            .is_none()
    );
    probe.reply(Reply::Cell(Some((10, 20))), now + Duration::from_secs(501));
    assert!(probe.facts().measurement.is_none());
    assert!(
        probe
            .query((103, 30), now + Duration::from_secs(502))
            .is_some()
    );
    probe.reply(Reply::Cell(None), now + Duration::from_secs(502));
    assert!(probe.facts().measurement.is_none());
    assert!(
        probe
            .query((103, 30), now + Duration::from_secs(502))
            .is_none()
    );
}

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
    use crate::client_view::{KeyCode, KeyEvent, Modifiers};
    assert_eq!(
        decode(b"a\x1b[6;16;8t\x1b_Gi=31;OK\x1b\\b"),
        vec![
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('a'), Modifiers::NONE)),
            DecodedKind::HostReply(Reply::Cell(Some((8, 16)))),
            DecodedKind::HostReply(Reply::Kitty {
                placement_id: None,
                image_id: 31,
                success: true
            }),
            DecodedKind::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::NONE)),
        ]
    );
    let payload = "x\x1b[6;16;8t\x1b_Gi=31;OK\x1b\\y";
    assert_eq!(
        decode(format!("\x1b[200~{payload}\x1b[201~").as_bytes()),
        vec![DecodedKind::Paste {
            text: Some(payload.into())
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

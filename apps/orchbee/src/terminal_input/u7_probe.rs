//! U7 packet-12 provenance probe — decoder layer. Drives the real
//! `LegacyDecoder` (composed U5 `401d5c20`) directly: provenance of frames
//! that open inside a captured packet, abort/refeed ownership, UTF-8 timeout
//! tails, and the `PasteEnd` terminator across drain/UTF-8 boundaries.

use super::decoder::{DecodedItem, DecodedKind, LegacyDecoder};
use crate::client_view::KeyCode;
use std::time::{Duration, Instant};

fn feed(decoder: &mut LegacyDecoder, bytes: &[u8], now: Instant, owned: bool) -> Vec<DecodedItem> {
    let mut items = Vec::new();
    for &byte in bytes {
        items.extend(decoder.push(byte, now, owned));
    }
    items
}

fn kinds(items: &[DecodedItem]) -> Vec<String> {
    items
        .iter()
        .map(|item| match &item.kind {
            DecodedKind::Key(event) => format!("key:{:?}/{:?}", event.code, event.mods),
            DecodedKind::Paste { text } => format!("paste:{:?}", text),
            DecodedKind::Mouse(event) => format!("mouse:{:?}", event),
            DecodedKind::Ignored => format!("ignored:{:?}", &item.raw[..]),
            DecodedKind::Nondecision => format!("nondecision:{:?}", &item.raw[..]),
        })
        .collect()
}

fn esc_item(items: &[DecodedItem]) -> Option<&DecodedItem> {
    items
        .iter()
        .find(|item| matches!(&item.kind, DecodedKind::Key(event) if event.code == KeyCode::Esc))
}

/// A frame opened inside a captured packet keeps `owned` provenance when it
/// completes in a later read: CSI, SS3, ESC-led, and paste variants all emit
/// owned items that the dispatcher must consume rather than forward or decide.
#[test]
fn frames_opened_in_captured_packet_emit_owned_on_later_completion() {
    let t0 = Instant::now();
    for (head, tail, want) in [
        (&b"\x9b"[..], &b"A"[..], "key"),
        (&b"\x8f"[..], &b"A"[..], "key"),
        (&b"\x1b["[..], &b"D"[..], "key"),
        (&b"\x1b[200~"[..], &b"ab\x1b[201~"[..], "paste"),
    ] {
        let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
        let opened = feed(&mut decoder, head, t0, true);
        let completed = feed(&mut decoder, tail, t0 + Duration::from_millis(5), false);
        let all: Vec<DecodedItem> = opened.into_iter().chain(completed).collect();
        assert!(
            all.iter().all(|item| item.owned),
            "{head:?}+{tail:?} produced unowned items: {:?}",
            kinds(&all)
        );
        assert!(
            all.iter().any(|item| match want {
                "key" => matches!(item.kind, DecodedKind::Key(_)),
                "paste" => matches!(item.kind, DecodedKind::Paste { .. }),
                _ => false,
            }),
            "{head:?}+{tail:?} missing {want} item: {:?}",
            kinds(&all)
        );
    }
}

/// CSI abort refeed: the abort byte is reprocessed at Ground under its OWN
/// provenance — an unowned byte aborting an owned sequence opens an unowned
/// frame; an owned byte aborting an unowned sequence stays owned.
#[test]
fn abort_refeed_uses_the_abort_bytes_own_provenance() {
    let t0 = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    feed(&mut decoder, b"\x9b", t0, true);
    let abort = feed(&mut decoder, b"\x1b", t0 + Duration::from_millis(5), false);
    assert!(
        abort
            .iter()
            .all(|item| matches!(item.kind, DecodedKind::Nondecision) && item.owned),
        "aborted frame emits owned nondecision: {:?}",
        kinds(&abort)
    );
    let ticked = decoder.tick(t0 + Duration::from_millis(60));
    let esc = esc_item(&ticked).expect("refed ESC must resolve to an Esc key");
    assert!(!esc.owned, "unowned abort byte must not inherit owned");

    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    feed(&mut decoder, b"\x9b", t0, false);
    feed(&mut decoder, b"\x1b", t0 + Duration::from_millis(5), true);
    let ticked = decoder.tick(t0 + Duration::from_millis(60));
    let esc = esc_item(&ticked).expect("refed ESC must resolve to an Esc key");
    assert!(esc.owned, "owned abort byte must keep owned");
}

/// A second ESC arriving in a NEW packet re-arms the disambiguation: that new
/// Esc state must carry the second byte's provenance, not the old frame's.
/// Owned-then-unowned `ESC ESC` must still emit a REAL unowned Esc key.
#[test]
fn esc_esc_rearm_uses_the_new_bytes_provenance() {
    let t0 = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    feed(&mut decoder, b"\x1b", t0, true);
    let second = feed(&mut decoder, b"\x1b", t0 + Duration::from_millis(5), false);
    assert!(
        esc_item(&second).is_some_and(|item| item.owned),
        "first ESC completes owned: {:?}",
        kinds(&second)
    );
    let ticked = decoder.tick(t0 + Duration::from_millis(60));
    let esc = esc_item(&ticked).expect("second ESC must resolve to an Esc key");
    assert!(
        !esc.owned,
        "the re-armed ESC belongs to the new packet's byte, not the old frame"
    );
}

/// A truncated UTF-8 scalar whose wait expires must treat its late tail the
/// way every other truncated frame does (owned drain), not drop to bare
/// Ground where a late `0x9b`/`0x8f` continuation opens a fresh unowned
/// Csi/Ss3 frame and a plain continuation becomes stray forwarded input.
#[test]
fn utf8_timeout_tail_keeps_frame_ownership() {
    let t0 = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    feed(&mut decoder, &[0xe4], t0, true);
    let expired = decoder.tick(t0 + Duration::from_millis(60));
    assert!(
        expired.iter().all(|item| item.owned),
        "expired scalar emits owned residue: {:?}",
        kinds(&expired)
    );
    let tail1 = feed(&mut decoder, &[0x9b], t0 + Duration::from_millis(61), false);
    assert!(
        tail1.iter().all(|item| item.owned),
        "late 0x9b continuation must stay owned, got {:?}",
        kinds(&tail1)
    );
    let tail2 = feed(&mut decoder, &[0x8f], t0 + Duration::from_millis(62), false);
    assert!(
        tail2.iter().all(|item| item.owned),
        "late 0x8f continuation must stay owned, got {:?}",
        kinds(&tail2)
    );
}

/// `PasteEnd` utf8 suppression: a `0x9b` that genuinely continues a scalar is
/// payload, never a C1 terminator lead (U5's decoder12 fix) — `C2 9B` inside
/// a C1-opened paste must not terminate; the real `\x9b201~` must.
#[test]
fn c1_paste_continuation_0x9b_is_payload_real_terminator_completes() {
    let t0 = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    assert!(feed(&mut decoder, b"\x9b200~", t0, false).is_empty());
    let mid = feed(
        &mut decoder,
        &[0xc2, 0x9b],
        t0 + Duration::from_millis(1),
        false,
    );
    assert!(
        mid.is_empty(),
        "C2 9B continuation must not terminate the paste: {:?}",
        kinds(&mid)
    );
    let end = feed(
        &mut decoder,
        b"\x9b201~",
        t0 + Duration::from_millis(2),
        false,
    );
    assert!(
        end.iter()
            .any(|item| matches!(&item.kind, DecodedKind::Paste { text } if text.is_some())),
        "real C1 terminator must complete the paste: {:?}",
        kinds(&end)
    );
    let after = feed(&mut decoder, b"q", t0 + Duration::from_millis(3), false);
    assert!(
        after.iter().any(
            |item| matches!(&item.kind, DecodedKind::Key(event) if event.code == KeyCode::Char('q'))
        ),
        "post-terminator byte decodes fresh: {:?}",
        kinds(&after)
    );
}

/// The suppression window must end where the scalar dies: `\xe4` followed by a
/// NON-continuation byte (`A`) is already malformed — a `\x9b201~` that
/// follows is a real terminator, not continuation payload. If position-count
/// suppression swallows it, the paste never terminates and later input is
/// eaten as payload.
#[test]
fn c1_paste_terminator_after_broken_scalar_must_still_match() {
    let t0 = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    assert!(feed(&mut decoder, b"\x9b200~", t0, false).is_empty());
    feed(
        &mut decoder,
        &[0xe4, b'A'],
        t0 + Duration::from_millis(1),
        false,
    );
    let end = feed(
        &mut decoder,
        b"\x9b201~",
        t0 + Duration::from_millis(2),
        false,
    );
    assert!(
        end.iter()
            .any(|item| matches!(item.kind, DecodedKind::Paste { .. })),
        "terminator after a dead scalar must complete the paste: {:?}",
        kinds(&end)
    );
    let after = feed(&mut decoder, b"q", t0 + Duration::from_millis(3), false);
    assert!(
        after
            .iter()
            .any(|item| matches!(&item.kind, DecodedKind::Key(_))),
        "input after the terminator must decode, not be swallowed: {:?}",
        kinds(&after)
    );
}

/// A paste terminator straddling the timeout-drain boundary still completes:
/// the matcher lives in `PasteEnd`, so `\x9b20` before the deadline and `1~`
/// after the drain begins must still close the run.
#[test]
fn paste_terminator_completes_across_timeout_drain() {
    let t0 = Instant::now();
    let mut decoder = LegacyDecoder::new(Duration::from_millis(50));
    assert!(feed(&mut decoder, b"\x9b200~", t0, false).is_empty());
    feed(
        &mut decoder,
        b"xy\x9b20",
        t0 + Duration::from_millis(1),
        false,
    );
    let expired = decoder.tick(t0 + Duration::from_millis(60));
    assert!(
        !expired.is_empty(),
        "expired paste emits its collected run as nondecision"
    );
    let rest = feed(&mut decoder, b"1~", t0 + Duration::from_millis(61), false);
    let after = feed(&mut decoder, b"q", t0 + Duration::from_millis(62), false);
    assert!(
        after.iter().any(
            |item| matches!(&item.kind, DecodedKind::Key(event) if event.code == KeyCode::Char('q'))
        ),
        "the byte after a drain-boundary terminator decodes fresh: {:?}",
        kinds(&rest)
            .into_iter()
            .chain(kinds(&after))
            .collect::<Vec<_>>()
    );
}

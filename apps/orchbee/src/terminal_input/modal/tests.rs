use super::{ModalCapacity, ModalDecoder, ModalInputEvent as Event};
use std::time::{Duration, Instant};

fn feed(decoder: &mut ModalDecoder, bytes: &[u8], now: Instant, remaining: usize) -> Vec<Event> {
    bytes
        .iter()
        .flat_map(|byte| {
            let (captured, events) = decoder.push(*byte, now, remaining);
            assert!(captured, "unexpected release of byte {byte:#x}");
            events
        })
        .collect()
}

fn appended(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Append(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn directory_paste_limit_counts_utf8_bytes_and_rejects_the_whole_paste() {
    let now = Instant::now();
    for (text, capacity, accepted) in [
        (format!("/{}abc", "雪".repeat(1364)), 4096, true),
        (format!("/{}abcd", "雪".repeat(1364)), 4096, false),
        ("😀".to_owned(), 3, false),
        ("😀".to_owned(), 4, true),
    ] {
        let mut decoder = ModalDecoder::default();
        decoder.start_with_capacity(ModalCapacity::Bytes4096);
        let packet = format!("\x1b[200~{text}\x1b[201~");
        let mut events = Vec::new();
        for chunk in packet.as_bytes().chunks(7) {
            events.extend(feed(&mut decoder, chunk, now, capacity));
        }
        assert_eq!(appended(&events), if accepted { text.as_str() } else { "" });
        assert_eq!(events.contains(&Event::Fault), !accepted);
        assert!(!events.contains(&Event::Submit));
    }
}

#[test]
fn directory_paste_leaves_literal_nul_and_path_validation_to_the_state_owner() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start_with_capacity(ModalCapacity::Bytes4096);
    let events = feed(&mut decoder, b"\x1b[200~relative\0path\x1b[201~", now, 4096);
    assert_eq!(appended(&events), "relative\0path");
    assert!(!events.contains(&Event::Fault));
    assert_eq!(appended(&feed(&mut decoder, b"\0", now, 4096)), "\0");
}

#[test]
fn scalar_fragments_are_exact_and_invalid_input_cannot_be_repaired_by_its_boundary_key() {
    let now = Instant::now();
    for text in ["é", "雪", "😀", "e\u{301}"] {
        for split in 0..=text.len() {
            let mut decoder = ModalDecoder::default();
            assert!(decoder.start());
            let mut events = feed(&mut decoder, &text.as_bytes()[..split], now, 512);
            events.extend(feed(&mut decoder, &text.as_bytes()[split..], now, 512));
            assert_eq!(appended(&events), text);
            assert!(!events.contains(&Event::Fault));
        }
    }
    for bytes in [
        b"\xc0\xaf".as_slice(),
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"\xe2\x08",
    ] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let events = feed(&mut decoder, bytes, now, 512);
        assert!(events.contains(&Event::Fault));
        assert!(appended(&events).is_empty());
        assert!(!events.contains(&Event::Backspace));
        assert_eq!(feed(&mut decoder, b"\x08", now, 512), [Event::Backspace]);
    }
}

#[test]
fn paste_is_atomic_across_every_split_and_keeps_literal_controls_and_nested_openers() {
    let now = Instant::now();
    let text = "雪\r\n\x02x\x1b[200~\x1b[20X😀";
    let bytes = format!("\x1b[200~{text}\x1b[201~");
    for split in 0..=bytes.len() {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let mut events = feed(&mut decoder, &bytes.as_bytes()[..split], now, 512);
        if split < bytes.len() {
            assert!(appended(&events).is_empty());
        }
        events.extend(feed(&mut decoder, &bytes.as_bytes()[split..], now, 512));
        assert_eq!(appended(&events), text);
        assert!(!events.iter().any(|event| matches!(
            event,
            Event::Submit | Event::Cancel | Event::Backspace | Event::Clear | Event::Fault
        )));
    }
    let mut decoder = ModalDecoder::default();
    decoder.start();
    assert_eq!(feed(&mut decoder, b"\r", now, 512), [Event::Submit]);
    assert!(feed(&mut decoder, b"\n", now, 512).is_empty());
}

#[test]
fn paste_overflow_and_invalid_utf8_discard_every_pending_scalar_until_the_terminator() {
    let now = Instant::now();
    for count in [512, 513] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let text = "😀".repeat(count);
        let bytes = format!("\x1b[200~{text}");
        let prefix = feed(&mut decoder, bytes.as_bytes(), now, 512);
        assert!(appended(&prefix).is_empty());
        let end = feed(&mut decoder, b"\x1b[201~", now, 512);
        if count == 512 {
            assert_eq!(appended(&end), text);
        } else {
            assert!(prefix.contains(&Event::Fault));
            assert!(appended(&end).is_empty());
        }
    }
    let mut decoder = ModalDecoder::default();
    decoder.start();
    let events = feed(
        &mut decoder,
        b"\x1b[200~ok\xff\x15\r\n\x02q\x1b[201~",
        now,
        512,
    );
    assert!(events.contains(&Event::Fault));
    assert!(appended(&events).is_empty());
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Clear | Event::Submit | Event::Cancel))
    );
    assert_eq!(feed(&mut decoder, b"\x15", now, 512), [Event::Clear]);
}

#[test]
fn csi_ss3_bounds_and_alt_utf8_never_insert_navigation_bytes() {
    let now = Instant::now();
    for introducer in ['[', 'O'] {
        for length in [31, 32, 33] {
            let mut decoder = ModalDecoder::default();
            decoder.start();
            let sequence = format!("\x1b{introducer}{}A", "1".repeat(length - 3));
            let events = feed(&mut decoder, sequence.as_bytes(), now, 512);
            assert!(appended(&events).is_empty());
            assert_eq!(events.contains(&Event::Fault), length > 32);
            assert_eq!(appended(&feed(&mut decoder, b"x", now, 512)), "x");
        }
    }
    let mut decoder = ModalDecoder::default();
    decoder.start();
    let events = feed(&mut decoder, "\x1bé\x1b[ 1A".as_bytes(), now, 512);
    assert!(appended(&events).is_empty());
    assert!(events.contains(&Event::Fault));
}

#[test]
fn cancellation_during_a_recognized_paste_opener_keeps_the_complete_payload_owned() {
    let now = Instant::now();
    let opener = b"\x1b[200~";
    for split in 2..=opener.len() {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        feed(&mut decoder, &opener[..split], now, 512);
        decoder.cancel();
        let later = now + Duration::from_secs(60);
        assert!(decoder.tick(later).is_empty());
        assert!(decoder.owns_input());
        feed(&mut decoder, &opener[split..], later, 512);
        for byte in b"\r\n\x02xy\x1b[201" {
            feed(&mut decoder, &[*byte], later + Duration::from_secs(60), 512);
        }
        let final_time = later + Duration::from_secs(120);
        feed(&mut decoder, b"~", final_time, 512);
        // The frame ended inside this packet, so its tail stays owned.
        assert!(decoder.owns_input());
        assert!(decoder.push(b'z', final_time, 512).0);
        // The next packet is free.
        decoder.begin_packet(final_time);
        assert!(!decoder.owns_input());
        assert!(!decoder.push(b'z', final_time, 512).0);
    }
}

#[test]
fn the_released_guard_holds_the_decision_packet_tail_and_ends_only_at_the_boundary() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel();
    // The tail guard owns the rest of the decision packet regardless of
    // how much time passes; no clock ends it.
    feed(&mut decoder, b"xy", now + Duration::from_secs(3600), 512);
    decoder.tick(now + Duration::from_secs(7200));
    assert!(decoder.owns_input());
    assert!(
        decoder
            .begin_packet(now + Duration::from_secs(7200))
            .is_empty()
    );
    assert!(!decoder.owns_input());
    assert!(!decoder.push(b'z', now + Duration::from_secs(7200), 512).0);
    // A frame started inside the released packet outlives the boundary.
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel();
    feed(&mut decoder, b"\x1b[", now, 512);
    decoder.begin_packet(now + Duration::from_secs(1));
    assert!(decoder.owns_input());
    feed(
        &mut decoder,
        b"200~\x02xy",
        now + Duration::from_secs(60),
        512,
    );
    assert!(decoder.owns_input());
    feed(
        &mut decoder,
        b"\x1b[201~",
        now + Duration::from_secs(120),
        512,
    );
    assert!(decoder.push(b't', now + Duration::from_secs(120), 512).0);
    decoder.begin_packet(now + Duration::from_secs(180));
    assert!(!decoder.owns_input());
    assert!(!decoder.push(b'z', now + Duration::from_secs(180), 512).0);
}

#[test]
fn escape_classification_and_completed_handoff_have_the_accepted_late_opener_limit() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    feed(&mut decoder, b"\x1b", now, 512);
    assert!(decoder.tick(now + Duration::from_millis(49)).is_empty());
    assert_eq!(
        decoder.tick(now + Duration::from_millis(50)),
        [Event::Cancel]
    );
    // The released guard owns this packet's tail; a late opener in a new
    // packet is free after the boundary.
    assert!(decoder.push(b'[', now + Duration::from_millis(60), 512).0);
    decoder.begin_packet(now + Duration::from_millis(60));
    assert!(
        !decoder.push(b'[', now + Duration::from_millis(60), 512).0,
        "unrecognized late opener is new input after the packet boundary"
    );
    assert!(!decoder.owns_input());
    decoder.start();
    feed(&mut decoder, b"\x1b", now + Duration::from_secs(1), 512);
    let events = feed(&mut decoder, b"[", now + Duration::from_millis(1050), 512);
    assert_eq!(events, [Event::Cancel]);
    assert!(decoder.owns_input());
}

#[test]
fn recognized_utf8_and_csi_cancellation_drain_across_arbitrary_gaps() {
    let now = Instant::now();
    for (prefix, tail) in [
        (b"\xe9\x9b".as_slice(), b"\xaa".as_slice()),
        (b"\x1b[1;", b"2A"),
    ] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        feed(&mut decoder, prefix, now, 512);
        decoder.cancel();
        let later = now + Duration::from_secs(3600);
        decoder.tick(later);
        assert!(decoder.owns_input());
        assert!(feed(&mut decoder, tail, later, 512).is_empty());
        // The completed frame leaves the rest of its packet owned.
        assert!(decoder.owns_input());
        assert!(decoder.push(b'z', later, 512).0);
        decoder.begin_packet(later + Duration::from_secs(3600));
        assert!(!decoder.owns_input());
        assert!(!decoder.push(b'z', later + Duration::from_secs(3600), 512).0);
    }
}

#[test]
fn a_scalar_lead_in_the_released_packet_keeps_its_continuations_owned() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel();
    feed(&mut decoder, b"\xe4", now, 512);
    decoder.begin_packet(now + Duration::from_secs(3600));
    // The UTF-8 frame owns the boundary, not a clock.
    assert!(decoder.owns_input());
    feed(&mut decoder, b"\xb8", now + Duration::from_secs(3600), 512);
    feed(&mut decoder, b"\xad", now + Duration::from_secs(3600), 512);
    assert!(decoder.push(b'x', now + Duration::from_secs(3600), 512).0);
    decoder.begin_packet(now + Duration::from_secs(7200));
    assert!(!decoder.owns_input());
    assert!(!decoder.push(b'z', now + Duration::from_secs(7200), 512).0);
}

#[test]
fn escape_disambiguation_owns_the_resolver_but_releases_at_the_boundary() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel();
    feed(&mut decoder, b"\x1b", now, 512);
    // An unexpired ESC wait still owns the next packet's resolver byte.
    decoder.begin_packet(now + Duration::from_millis(10));
    assert!(decoder.owns_input());
    assert!(decoder.push(b'x', now + Duration::from_millis(10), 512).0);
    // After the frame resolved, the rest of that packet is still owned.
    assert!(decoder.owns_input());
    // Once the wait has already expired, the boundary frees the packet.
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel();
    feed(&mut decoder, b"\x1b", now, 512);
    decoder.begin_packet(now + Duration::from_millis(60));
    assert!(!decoder.owns_input());
    assert!(!decoder.push(b'x', now + Duration::from_millis(60), 512).0);
}

#[test]
fn a_c1_lead_in_the_released_packet_opens_a_sequence_that_outlives_the_boundary() {
    let now = Instant::now();
    for lead in [0x9b, 0x8f] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        decoder.cancel();
        feed(&mut decoder, &[lead], now, 512);
        decoder.begin_packet(now + Duration::from_secs(1));
        assert!(decoder.owns_input());
        feed(&mut decoder, b"1;5", now + Duration::from_secs(1), 512);
        feed(&mut decoder, b"D", now + Duration::from_secs(1), 512);
        decoder.begin_packet(now + Duration::from_secs(2));
        assert!(!decoder.owns_input());
        assert!(!decoder.push(b'z', now + Duration::from_secs(2), 512).0);
    }
    // A C1 paste opener drains the payload rather than leaking it.
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel();
    feed(&mut decoder, b"\x9b200~xy", now, 512);
    decoder.begin_packet(now + Duration::from_secs(1));
    assert!(decoder.owns_input());
    feed(&mut decoder, b"\x9b201~", now + Duration::from_secs(1), 512);
    decoder.begin_packet(now + Duration::from_secs(2));
    assert!(!decoder.push(b'z', now + Duration::from_secs(2), 512).0);
}

#[test]
fn checked_clock_overflow_cancels_once_and_permanently_suppresses_forwarding() {
    let mut limit = Instant::now();
    for bit in (0..64).rev() {
        if let Some(next) = limit.checked_add(Duration::from_secs(1_u64 << bit)) {
            limit = next;
        }
    }
    for bit in (0..10).rev() {
        if let Some(next) = limit.checked_add(Duration::from_millis(1_u64 << bit)) {
            limit = next;
        }
    }
    assert!(limit.checked_add(Duration::from_millis(50)).is_none());
    let mut decoder = ModalDecoder::default();
    decoder.start();
    let events = feed(&mut decoder, b"\x1b", limit, 512);
    assert!(events.contains(&Event::Cancel));
    assert!(events.contains(&Event::InputFailure));
    assert!(feed(&mut decoder, b"\x02q\x1b[201~plain", limit, 512).is_empty());
    assert!(decoder.tick(limit).is_empty());
    assert!(decoder.owns_input());
    // A packet boundary never revives a suppressed decoder.
    assert!(decoder.begin_packet(limit).is_empty());
    assert!(decoder.owns_input());
    assert!(decoder.push(b'z', limit, 512).0);
}

#[test]
fn external_cancel_of_pending_escape_keeps_the_disambiguation_then_releases_at_the_boundary() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    feed(&mut decoder, b"\x1b", now, 512);
    decoder.cancel();
    // The pending ESC still resolves by its own deadline, then the tail
    // guard holds until a packet boundary — not a second clock.
    decoder.tick(now + Duration::from_millis(49));
    assert!(decoder.owns_input());
    decoder.tick(now + Duration::from_millis(50));
    assert!(decoder.owns_input());
    decoder.tick(now + Duration::from_millis(3_600_000));
    assert!(decoder.owns_input());
    decoder.begin_packet(now + Duration::from_millis(3_600_000));
    assert!(!decoder.owns_input());
    // An ESC opened inside the released packet still disambiguates the
    // next packet's first byte.
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel();
    feed(&mut decoder, b"\x1b", now, 512);
    decoder.begin_packet(now + Duration::from_millis(10));
    assert!(decoder.owns_input());
    feed(&mut decoder, b"[", now + Duration::from_millis(10), 512);
    assert!(decoder.owns_input());
}

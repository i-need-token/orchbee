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
            Event::Append(text) | Event::Paste(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn pasted(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Paste(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn mouse_reports_keep_one_typed_identity_across_every_split() {
    use crate::client_view::Modifiers;
    use crate::terminal_input::{MouseAction, MouseButton, MouseEvent};
    let now = Instant::now();
    for report in [
        b"\x1b[<0;12;8M".as_slice(),
        b"\x9b<0;12;8M",
        b"\x1b[M ,(",
        b"\x9bM ,(",
    ] {
        for split in 0..=report.len() {
            let mut decoder = ModalDecoder::default();
            decoder.start();
            let mut events = feed(&mut decoder, &report[..split], now, 512);
            // The continuation was ready while rendering occupied the owner.
            // No idle poll occurred, so elapsed wall time cannot split a report.
            let later = now + Duration::from_millis(60);
            decoder.begin_packet(later);
            events.extend(feed(&mut decoder, &report[split..], later, 512));
            let mice: Vec<_> = events
                .iter()
                .filter_map(|event| {
                    if let Event::Mouse(mouse) = event {
                        Some(*mouse)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                mice,
                [MouseEvent {
                    action: MouseAction::Press,
                    button: MouseButton::Left,
                    mods: Modifiers::NONE,
                    column: 11,
                    row: 7
                }],
                "{report:?} split {split}"
            );
            assert!(appended(&events).is_empty());
            assert!(!events.contains(&Event::Submit));
        }
    }
}

#[test]
fn mouse_reports_in_paste_and_released_tails_never_become_gestures() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    let events = feed(
        &mut decoder,
        b"\x1b[200~\x1b[<0;12;8M\x1b[M ,(\x1b[201~",
        now,
        512,
    );
    assert_eq!(pasted(&events), "\x1b[<0;12;8M\x1b[M ,(");
    assert!(!events.iter().any(|event| matches!(event, Event::Mouse(_))));
    for report in [b"\x1b[<0;12;8M".as_slice(), b"\x1b[M ,("] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let mut events = feed(&mut decoder, &report[..3], now, 512);
        events.extend(decoder.cancel(now));
        events.extend(feed(
            &mut decoder,
            &report[3..],
            now + Duration::from_millis(60),
            512,
        ));
        assert!(!events.iter().any(|event| matches!(event, Event::Mouse(_))));
        assert!(appended(&events).is_empty());
    }
}

#[test]
fn malformed_sgr_is_consumed_and_cannot_insert_coordinates_or_choose() {
    let now = Instant::now();
    for report in [
        b"\x1b[<0;0;8M".as_slice(),
        b"\x1b[<0;999999;8M",
        b"\x1b[<0;12;8;9M",
    ] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let events = feed(&mut decoder, report, now, 512);
        assert!(appended(&events).is_empty());
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Mouse(_) | Event::Submit | Event::Cancel))
        );
        assert_eq!(appended(&feed(&mut decoder, b"x", now, 512)), "x");
    }
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
        assert_eq!(pasted(&events), if accepted { text.as_str() } else { "" });
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == Event::Fault)
                .count(),
            usize::from(!accepted)
        );
        assert!(!events.contains(&Event::Submit));
    }
}

#[test]
fn directory_paste_leaves_literal_nul_and_path_validation_to_the_state_owner() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start_with_capacity(ModalCapacity::Bytes4096);
    let events = feed(&mut decoder, b"\x1b[200~relative\0path\x1b[201~", now, 4096);
    assert_eq!(pasted(&events), "relative\0path");
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
            assert!(pasted(&events).is_empty());
        }
        events.extend(feed(&mut decoder, &bytes.as_bytes()[split..], now, 512));
        assert_eq!(pasted(&events), text);
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
fn paste_framing_keeps_c1_continuations_and_controls_in_one_payload_across_every_split() {
    let now = Instant::now();
    // Each UTF-8 width ends in 0x9b; the following ASCII resembles a C1 end.
    for character in ['\u{9b}', '\u{269b}', '\u{1f69b}'] {
        let text = format!("{character}201~\r");
        let bytes = [b"\x9b200~".as_slice(), text.as_bytes(), b"\x9b201~"].concat();
        for split in 0..=bytes.len() {
            let mut decoder = ModalDecoder::default();
            decoder.start();
            let mut events = feed(&mut decoder, &bytes[..split], now, 512);
            decoder.begin_packet(now);
            events.extend(feed(&mut decoder, &bytes[split..], now, 512));
            events.retain(|event| !matches!(event, Event::Completeness(_)));
            assert_eq!(events, [Event::Paste(text.clone())], "split {split}");
            assert_eq!(
                feed(&mut decoder, b"x", now, 512),
                [Event::Append("x".into())]
            );
        }
    }
}

#[test]
fn paste_framing_drains_c1_continuations_and_controls_after_timeout() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    feed(&mut decoder, b"\x9b200~\xc2", now, 512);
    let later = now + Duration::from_millis(50);
    assert!(decoder.tick(later).is_empty());
    assert!(feed(&mut decoder, b"\x9b201~\r\x02q", later, 512).is_empty());
    let mut events = feed(&mut decoder, b"\x9b201~", later, 512);
    events.retain(|event| !matches!(event, Event::Completeness(_)));
    assert_eq!(events, [Event::Fault]);
    assert_eq!(
        feed(&mut decoder, b"x", later, 512),
        [Event::Append("x".into())]
    );
}

#[test]
fn paste_framing_reports_each_rejection_once_without_delaying_immediate_faults() {
    let now = Instant::now();
    for (payload, remaining, immediate_faults) in [
        (b"ok\xff\x15\r\n\x02q".as_slice(), 512, 1),
        (b"ab".as_slice(), 1, 1),
        (b"\xc2".as_slice(), 512, 0),
    ] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let mut events = feed(&mut decoder, b"\x1b[200~", now, remaining);
        events.extend(feed(&mut decoder, payload, now, remaining));
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == Event::Fault)
                .count(),
            immediate_faults
        );
        events.extend(feed(&mut decoder, b"\x1b[201~", now, remaining));
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == Event::Completeness(true))
                .count(),
            1
        );
        events.retain(|event| !matches!(event, Event::Completeness(_)));
        assert_eq!(events, [Event::Fault], "payload {payload:?}");
        assert_eq!(
            feed(&mut decoder, b"x", now, 512),
            [Event::Append("x".into())]
        );
    }
}

#[test]
fn paste_framing_cancellation_suppresses_faults_without_releasing_the_tail() {
    let now = Instant::now();
    for cancel_after_timeout in [false, true] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        feed(&mut decoder, b"\x9b200~\xc2", now, 512);
        let later = now + Duration::from_millis(50);
        if !cancel_after_timeout {
            assert!(decoder.cancel(now).is_empty());
        }
        assert!(decoder.tick(later).is_empty());
        assert!(decoder.cancel(later).is_empty());
        assert!(!decoder.start());
        assert!(feed(&mut decoder, b"\x9b201~\r\x02q", later, 512).is_empty());
        decoder.begin_packet(later);
        assert!(decoder.owns_input());
        assert!(feed(&mut decoder, b"\x9b201~tail", later, 512).is_empty());
        decoder.begin_packet(later);
        assert!(!decoder.owns_input());
        assert_eq!(decoder.push(b'x', later, 512), (false, Vec::new()));
    }

    let mut decoder = ModalDecoder::default();
    decoder.start();
    let mut events = feed(&mut decoder, b"\x9b200~\xff", now, 512);
    events.retain(|event| !matches!(event, Event::Completeness(_)));
    assert_eq!(events, [Event::Fault]);
    assert!(decoder.cancel(now).is_empty());
    assert!(feed(&mut decoder, b"\x9b201~\rtail", now, 512).is_empty());
    decoder.begin_packet(now);
    assert_eq!(decoder.push(b'x', now, 512), (false, Vec::new()));
}

#[test]
fn paste_framing_broken_scalars_cannot_hide_a_later_real_c1_terminator() {
    let now = Instant::now();
    for lead in [0xc2, 0xe4, 0xf0] {
        for timed_out in [false, true] {
            let mut decoder = ModalDecoder::default();
            decoder.start();
            feed(&mut decoder, b"\x9b200~", now, 512);
            feed(&mut decoder, &[lead], now, 512);
            let later = now + Duration::from_millis(50);
            if timed_out {
                assert!(decoder.tick(later).is_empty());
            }
            let mut events = feed(&mut decoder, b"a\x9b201~", later, 512);
            events.retain(|event| !matches!(event, Event::Completeness(_)));
            assert_eq!(events, [Event::Fault]);
            assert_eq!(
                feed(&mut decoder, b"x", later, 512),
                [Event::Append("x".into())]
            );
        }
    }
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
        assert!(pasted(&prefix).is_empty());
        let end = feed(&mut decoder, b"\x1b[201~", now, 512);
        if count == 512 {
            assert_eq!(pasted(&end), text);
        } else {
            assert!(prefix.contains(&Event::Fault));
            assert!(pasted(&end).is_empty());
        }
        assert_eq!(
            prefix
                .iter()
                .chain(&end)
                .filter(|event| **event == Event::Fault)
                .count(),
            usize::from(count > 512)
        );
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
    assert!(pasted(&events).is_empty());
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
        decoder.cancel(now);
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
    decoder.cancel(now);
    // The tail guard owns the rest of the decision packet regardless of
    // how much time passes; no clock ends it.
    feed(&mut decoder, b"xy", now + Duration::from_secs(3600), 512);
    decoder.tick(now + Duration::from_secs(7200));
    assert!(decoder.owns_input());
    decoder.begin_packet(now + Duration::from_secs(7200));
    assert!(!decoder.owns_input());
    assert!(!decoder.push(b'z', now + Duration::from_secs(7200), 512).0);
    // A frame started inside the released packet outlives the boundary.
    let mut decoder = ModalDecoder::default();
    decoder.start();
    decoder.cancel(now);
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
    assert_eq!(
        decoder.tick(now + Duration::from_millis(1050)),
        [Event::Cancel]
    );
    assert!(feed(&mut decoder, b"[", now + Duration::from_millis(1050), 512).is_empty());
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
        decoder.cancel(now);
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
    decoder.cancel(now);
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
    decoder.cancel(now);
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
    decoder.cancel(now);
    feed(&mut decoder, b"\x1b", now, 512);
    decoder.tick(now + Duration::from_millis(60));
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
        decoder.cancel(now);
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
    decoder.cancel(now);
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
    decoder.begin_packet(limit);
    assert!(decoder.owns_input());
    assert!(decoder.push(b'z', limit, 512).0);
}

#[test]
fn external_cancel_of_pending_escape_keeps_the_disambiguation_then_releases_at_the_boundary() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    feed(&mut decoder, b"\x1b", now, 512);
    decoder.cancel(now);
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
    decoder.cancel(now);
    feed(&mut decoder, b"\x1b", now, 512);
    decoder.begin_packet(now + Duration::from_millis(10));
    assert!(decoder.owns_input());
    feed(&mut decoder, b"[", now + Duration::from_millis(10), 512);
    assert!(decoder.owns_input());
}

#[test]
fn unmodified_navigation_delete_and_word_keys_emit_edit_events() {
    let now = Instant::now();
    for (bytes, expected) in [
        (b"\x1b[D".as_slice(), Event::Left),
        (b"\x1b[C".as_slice(), Event::Right),
        (b"\x1b[H".as_slice(), Event::Home),
        (b"\x1b[F".as_slice(), Event::End),
        (b"\x1b[1~".as_slice(), Event::Home),
        (b"\x1b[7~".as_slice(), Event::Home),
        (b"\x1b[4~".as_slice(), Event::End),
        (b"\x1b[8~".as_slice(), Event::End),
        (b"\x1b[3~".as_slice(), Event::Delete),
        (b"\x1bOD".as_slice(), Event::Left),
        (b"\x1bOC".as_slice(), Event::Right),
        (b"\x1bOH".as_slice(), Event::Home),
        (b"\x1bOF".as_slice(), Event::End),
        (b"\x9bD".as_slice(), Event::Left),
        (b"\x9bC".as_slice(), Event::Right),
        (b"\x9bH".as_slice(), Event::Home),
        (b"\x9bF".as_slice(), Event::End),
        (b"\x9b1~".as_slice(), Event::Home),
        (b"\x9b3~".as_slice(), Event::Delete),
        (b"\x8fD".as_slice(), Event::Left),
        (b"\x8fH".as_slice(), Event::Home),
        (b"\x01".as_slice(), Event::Home),
        (b"\x05".as_slice(), Event::End),
        (b"\x17".as_slice(), Event::WordBackspace),
    ] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let events = feed(&mut decoder, bytes, now, 512);
        assert!(
            events.contains(&expected),
            "{bytes:?} should emit {expected:?}, got {events:?}"
        );
        assert!(
            !events.contains(&Event::Cancel),
            "{bytes:?} must not cancel"
        );
        assert!(!events.contains(&Event::Fault), "{bytes:?} must not fault");
        assert!(decoder.owns_input());
    }
}

#[test]
fn modified_and_unbound_sequences_emit_no_edit_event_and_do_not_cancel() {
    let now = Instant::now();
    for bytes in [
        b"\x1b[1;5D".as_slice(),
        b"\x1b[1;2H".as_slice(),
        b"\x1b[3;5~".as_slice(),
        b"\x1b[1;5~".as_slice(),
        b"\x1b[A".as_slice(),
        b"\x1b[B".as_slice(),
        b"\x1b[2~".as_slice(),
        b"\x1b[5~".as_slice(),
        b"\x1b[6~".as_slice(),
        b"\x1bOA".as_slice(),
        b"\x1bOB".as_slice(),
        b"\x1b[Z".as_slice(),
        b"\x9bA".as_slice(),
        b"\x1b[201~".as_slice(),
    ] {
        let mut decoder = ModalDecoder::default();
        decoder.start();
        let events = feed(&mut decoder, bytes, now, 512);
        assert!(
            events
                .iter()
                .all(|event| matches!(event, Event::Completeness(_))),
            "{bytes:?} must only bracket the frame, got {events:?}"
        );
        assert!(decoder.owns_input(), "{bytes:?} must stay owned");
        // The next ordinary key still edits: the sequence was consumed, not
        // released or cancelled.
        assert_eq!(
            feed(&mut decoder, b"x", now, 512),
            [Event::Append("x".to_owned())]
        );
    }
}

#[test]
fn paste_emits_one_atomic_event_distinct_from_typed_text() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    decoder.start();
    let events = feed(&mut decoder, b"\x1b[200~a\x02b\x1b[201~", now, 512);
    assert_eq!(
        events,
        [
            Event::Completeness(false),
            Event::Completeness(true),
            Event::Paste("a\x02b".to_owned()),
        ]
    );
}

#[test]
fn focus_metadata_does_not_edit_or_submit_a_modal_field() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    assert!(decoder.start_with_capacity(ModalCapacity::Scalars512));
    let events = feed(&mut decoder, b"a\x1b[Ob\x9bIc", now, 512);
    assert_eq!(appended(&events), "abc");
    let focus: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Focus(value) => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(focus, [false, true]);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Submit | Event::Cancel))
    );
    let events = feed(&mut decoder, b"\x1b[200~a\x1b[Ob\x1b[201~", now, 512);
    assert!(!events.iter().any(|event| matches!(event, Event::Focus(_))));
}

#[test]
fn focus_loss_after_modal_submit_survives_the_captured_packet_tail() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    assert!(decoder.start_with_capacity(ModalCapacity::Scalars512));
    let events = feed(&mut decoder, b"a\r", now, 512);
    assert!(events.contains(&Event::Submit));
    // The dispatcher releases the modal lease after the owner accepts Submit.
    let _release = decoder.cancel(now);
    let tail = feed(&mut decoder, b"\x1b[Oignored", now, 512);
    assert!(tail.contains(&Event::Focus(false)));
    assert!(appended(&tail).is_empty());
}

#[test]
fn graphics_replies_inside_native_text_capture_never_edit_the_draft() {
    use crate::terminal_graphics::Reply;
    use crate::terminal_input::HostReply;
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    assert!(decoder.start());
    let mut events = Vec::new();
    for chunk in b"a\x1b[6;16;8t\x1b_Gi=31;OK\x1b\\b".chunks(3) {
        events.extend(feed(&mut decoder, chunk, now, 512));
    }
    assert_eq!(appended(&events), "ab");
    assert!(
        events.contains(&Event::HostReply(HostReply::Graphics(Reply::Cell(Some((
            8, 16
        ))))))
    );
    assert!(
        events.contains(&Event::HostReply(HostReply::Graphics(Reply::Kitty {
            placement_id: None,
            image_id: 31,
            success: true
        })))
    );
    assert!(!events.contains(&Event::Fault));
    let paste = feed(
        &mut decoder,
        b"\x1b[200~\x1b[6;16;8t\x1b_Gi=31;OK\x1b\\\x1b[201~",
        now,
        512,
    );
    assert!(
        !paste
            .iter()
            .any(|event| matches!(event, Event::HostReply(_)))
    );
}

#[test]
fn late_graphics_strings_stay_captured_until_their_terminator_after_modal_cancel() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    assert!(decoder.start());
    feed(&mut decoder, b"\x1b_Gi=31;", now, 512);
    decoder.cancel(now + Duration::from_millis(1));
    decoder.begin_packet(now + Duration::from_millis(80));
    let events = feed(
        &mut decoder,
        b"OK\x1b\\z",
        now + Duration::from_millis(80),
        512,
    );
    assert!(appended(&events).is_empty());
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::HostReply(_)))
    );
    decoder.begin_packet(now + Duration::from_millis(90));
    assert!(!decoder.push(b'x', now + Duration::from_millis(90), 512).0);
}

#[test]
fn malformed_or_timed_out_cell_metadata_does_not_fault_or_edit_a_native_field() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    assert!(decoder.start());
    let oversized = [b"\x1b[6;".as_slice(), &vec![b'9'; 2048], b";8tx"].concat();
    let events = feed(&mut decoder, &oversized, now, 512);
    assert_eq!(appended(&events), "x");
    assert!(!events.contains(&Event::Fault));
    feed(&mut decoder, b"\x1b[6;16;", now, 512);
    assert!(
        !decoder
            .tick(now + Duration::from_millis(60))
            .contains(&Event::Fault)
    );
    let events = feed(&mut decoder, b"8ty", now + Duration::from_millis(70), 512);
    assert_eq!(appended(&events), "y");
    assert!(!events.contains(&Event::Fault));
}

#[test]
fn pointer_admission_replies_are_metadata_inside_native_text_capture() {
    use crate::terminal_input::HostReply;
    use crate::terminal_pointer_admission::{HostReply as PointerReply, ModeReportState};

    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    assert!(decoder.start());
    let mut events = Vec::new();
    for chunk in b"a\x1b[?1003;2$y\x9b?1;2cb".chunks(2) {
        events.extend(feed(&mut decoder, chunk, now, 512));
    }
    assert_eq!(appended(&events), "ab");
    assert!(events.contains(&Event::HostReply(HostReply::Pointer(
        PointerReply::Mode1003(ModeReportState::Reset)
    ))));
    assert!(events.contains(&Event::HostReply(HostReply::Pointer(
        PointerReply::PrimaryDeviceAttributes
    ))));
    assert!(!events.contains(&Event::Fault));

    let literal = "\u{1b}[?1003;2$y\u{1b}[?1;2c";
    let paste = feed(
        &mut decoder,
        format!("\x1b[200~{literal}\x1b[201~").as_bytes(),
        now,
        512,
    );
    assert_eq!(pasted(&paste), literal);
    assert!(
        !paste
            .iter()
            .any(|event| matches!(event, Event::HostReply(_)))
    );
}

#[test]
fn only_reply_framed_private_csi_is_metadata_inside_a_modal() {
    let now = Instant::now();
    let mut decoder = ModalDecoder::default();
    assert!(decoder.start());
    let reply_shaped = feed(&mut decoder, b"a\x1b[?1003;5$y\x1b[?1;:cb", now, 512);
    assert_eq!(appended(&reply_shaped), "ab");
    assert!(!reply_shaped.contains(&Event::Fault));
    assert!(
        !reply_shaped
            .iter()
            .any(|event| matches!(event, Event::HostReply(_)))
    );

    let oversized = [b"\x1b[?".as_slice(), &[b'9'; 64], b"cx"].concat();
    let events = feed(&mut decoder, &oversized, now, 512);
    assert_eq!(appended(&events), "x");
    assert!(events.contains(&Event::Fault));

    feed(&mut decoder, b"\x1b[?1003;", now, 512);
    assert!(
        decoder
            .tick(now + Duration::from_millis(60))
            .contains(&Event::Fault)
    );
    let events = feed(&mut decoder, b"2$yz", now + Duration::from_millis(70), 512);
    assert_eq!(appended(&events), "z");
    assert!(!events.contains(&Event::Fault));
}

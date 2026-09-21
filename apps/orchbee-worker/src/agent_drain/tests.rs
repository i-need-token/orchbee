use std::num::NonZeroU64;

use orchbee_capability_node_protocol::{
    AgentDrainEntry, HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity,
    MAXIMUM_SIGNAL_PAYLOAD_BYTES, REPLY_ENTRY_BYTES,
};

use super::{AgentDrain, MAXIMUM_QUEUED_ENTRIES, MAXIMUM_QUEUED_ENTRY_BYTES};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("test value")
}

fn identity(byte: u8) -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([byte; 16]).expect("id"),
        HostEpoch::from_bytes([byte; 16]).expect("epoch"),
        HostTerminalId::from_bytes([byte; 16]).expect("terminal"),
    )
}

fn append(drain: &mut AgentDrain, payload_bytes: usize) -> TestResult {
    drain.append(identity(9), "x".repeat(payload_bytes))?;
    Ok(())
}

fn event(entry: &AgentDrainEntry) -> Option<u64> {
    match entry {
        AgentDrainEntry::Event { sequence, .. } => Some(sequence.get()),
        _ => None,
    }
}

fn gap(entry: &AgentDrainEntry) -> Option<(u64, u64)> {
    match entry {
        AgentDrainEntry::Gap { low, high } => Some((low.get(), *high)),
        _ => None,
    }
}

#[test]
fn append_mints_monotone_sequences_and_empty_reply_reports_tail() -> TestResult {
    let mut drain = AgentDrain::new();
    let (reply, covered) = drain.respond(nz(1), 0)?;
    assert!(reply.entries().is_empty());
    assert_eq!(reply.queue_tail(), 0);
    assert!(covered.is_none());

    append(&mut drain, 4)?;
    append(&mut drain, 4)?;
    let (reply, covered) = drain.respond(nz(2), 0)?;
    assert_eq!(reply.queue_tail(), 2);
    assert_eq!(covered, Some(2));
    assert_eq!(event(&reply.entries()[0]), Some(1));
    assert_eq!(event(&reply.entries()[1]), Some(2));
    Ok(())
}

/// Real entry-count bound: at 1024 live entries the next admission evicts the
/// oldest EVENT; the merged leading GAP consumes its own slot, so admitting
/// one event needs two evictions — one for the GAP slot, one for the event.
#[test]
fn count_bound_evicts_with_gap_slot_accounted() -> TestResult {
    let mut drain = AgentDrain::new();
    for _ in 0..MAXIMUM_QUEUED_ENTRIES {
        append(&mut drain, 1)?;
    }
    append(&mut drain, 1)?;
    let (entries, minted, _, _) = drain.snapshot();
    assert_eq!(entries.len(), MAXIMUM_QUEUED_ENTRIES);
    assert_eq!(minted, 1025);
    assert_eq!(gap(&entries[0]), Some((1, 2)));
    assert_eq!(event(&entries[1]), Some(3));
    Ok(())
}

/// Real byte bound: maximum-size events fill the 4 MiB budget; eviction must
/// also reserve the 17 bytes of the leading GAP it is about to merge in.
#[test]
fn byte_bound_evicts_with_gap_bytes_accounted() -> TestResult {
    let mut drain = AgentDrain::new();
    let event_bytes =
        MAXIMUM_SIGNAL_PAYLOAD_BYTES + orchbee_capability_node_protocol::EVENT_ENTRY_OVERHEAD_BYTES;
    let fit = MAXIMUM_QUEUED_ENTRY_BYTES / event_bytes;
    for _ in 0..fit {
        append(&mut drain, MAXIMUM_SIGNAL_PAYLOAD_BYTES)?;
    }
    append(&mut drain, MAXIMUM_SIGNAL_PAYLOAD_BYTES)?;
    let (entries, minted, _, _) = drain.snapshot();
    let bytes: usize = entries.iter().map(AgentDrainEntry::encoded_bytes).sum();
    assert!(bytes <= MAXIMUM_QUEUED_ENTRY_BYTES);
    assert_eq!(minted as usize, fit + 1);
    assert_eq!(gap(&entries[0]), Some((1, 1)));
    Ok(())
}

/// An offered-but-unconfirmed event may be evicted; the leading GAP covers it
/// and the offered watermark is left untouched for the commit path to move.
#[test]
fn offered_unconfirmed_eviction_is_covered_by_gap() -> TestResult {
    let mut drain = AgentDrain::new();
    for _ in 0..MAXIMUM_QUEUED_ENTRIES {
        append(&mut drain, 1)?;
    }
    let (reply, covered) = drain.respond(nz(1), 0)?;
    // The reply page is byte-bounded, so only a prefix of the 1024 queued
    // events is offered; the covered watermark is the page's last sequence.
    let covered_hi = covered.expect("covered");
    assert!(covered_hi < MAXIMUM_QUEUED_ENTRIES as u64);
    assert_eq!(Some(covered_hi), reply.entries().last().map(|e| e.end()));
    assert!(reply.entries().len() <= 480);
    drain.commit_offered(covered_hi);

    append(&mut drain, 1)?;
    let (entries, _, offered, confirmed) = drain.snapshot();
    // Both evicted events were already offered but unconfirmed; the merged
    // GAP covers them while the offered watermark stays at the commit.
    assert_eq!(offered, covered_hi);
    assert_eq!(confirmed, 0);
    assert_eq!(gap(&entries[0]), Some((1, 2)));
    Ok(())
}

/// The offered commit uses the watermark captured at request time: events
/// appended between capture and physical commit are not retroactively offered.
#[test]
fn commit_uses_captured_watermark_not_current_tail() -> TestResult {
    let mut drain = AgentDrain::new();
    append(&mut drain, 1)?;
    let (reply, covered) = drain.respond(nz(1), 0)?;
    assert_eq!(covered, Some(1));
    append(&mut drain, 1)?;
    drain.commit_offered(covered.expect("covered"));
    let (_, minted, offered, _) = drain.snapshot();
    assert_eq!(minted, 2);
    assert_eq!(offered, 1);
    drop(reply);
    Ok(())
}

#[test]
fn interior_ack_trims_events_and_clips_gap() -> TestResult {
    let mut drain = AgentDrain::new();
    for _ in 0..MAXIMUM_QUEUED_ENTRIES + 2 {
        append(&mut drain, 1)?;
    }
    // Queue now starts with GAP(1,2) then EVENTs 3..=1026. Each page is
    // byte-bounded, so offer and confirm advance in interior steps: every
    // round acknowledges the previously offered prefix, trimming it and the
    // leading GAP once the ACK moves past it.
    let mut token = 1;
    let mut ack = 0;
    loop {
        let (reply, covered) = drain.respond(nz(token), ack)?;
        let covered_hi = covered.expect("nonempty page must cover");
        assert!(!reply.entries().is_empty());
        drain.commit_offered(covered_hi);
        if covered_hi == (MAXIMUM_QUEUED_ENTRIES + 2) as u64 {
            break;
        }
        assert!(covered_hi > ack);
        ack = covered_hi;
        token += 1;
    }
    // ack 1026 = offered: trims everything including the clipped GAP.
    let (reply, covered) = drain.respond(nz(token + 1), (MAXIMUM_QUEUED_ENTRIES + 2) as u64)?;
    assert!(reply.entries().is_empty());
    assert!(covered.is_none());
    let (_, _, _, confirmed) = drain.snapshot();
    assert_eq!(confirmed, (MAXIMUM_QUEUED_ENTRIES + 2) as u64);
    Ok(())
}

#[test]
fn partial_gap_clip_keeps_uncovered_tail() -> TestResult {
    let mut drain = AgentDrain::new();
    for _ in 0..MAXIMUM_QUEUED_ENTRIES + 2 {
        append(&mut drain, 1)?;
    }
    let (_, covered) = drain.respond(nz(1), 0)?;
    drain.commit_offered(covered.expect("covered"));
    // Capture covered a bounded page; ack exactly its high bound.
    let ack = covered.expect("c");
    let (reply, covered) = drain.respond(nz(2), ack)?;
    let (entries, _, _, confirmed) = drain.snapshot();
    assert_eq!(confirmed, ack);
    // The page covered at most 480 entries ending at ack; the remaining queue
    // begins at ack + 1 — as an EVENT (the GAP was fully consumed) or a GAP.
    let first = entries.first().expect("entries remain");
    assert_eq!(first.start(), ack + 1);
    assert!(reply.entries().len() <= 480);
    let _ = covered;
    Ok(())
}

/// The REAL interior-GAP-ACK trace (fills the report-43 F2 hole, input44
/// B1): a genuine leading GAP 1..=4 produced by real eviction at the 1024
/// bound, a committed offered watermark at the page end, then an
/// acknowledgment landing strictly inside the GAP (ack=1 covers seq1 but
/// leaves seqs 2..=4 unconfirmed). The leading GAP must clip to (2,4) in
/// place; the fresh page must begin at the clipped head and the following
/// EVENT coverage must continue at seq 5.
#[test]
fn interior_gap_ack_clips_leading_gap_and_keeps_tail() -> TestResult {
    let mut drain = AgentDrain::new();
    // 1027 one-byte admissions -> [GAP(1,4), EVENT 5..=1027], minted 1027.
    for _ in 0..(MAXIMUM_QUEUED_ENTRIES + 3) {
        append(&mut drain, 1)?;
    }
    let (entries, minted, _, _) = drain.snapshot();
    assert_eq!(gap(&entries[0]), Some((1, 4)));
    assert_eq!(minted, 1027);
    drop(entries);
    // Offer the first page and commit its captured watermark.
    let (_reply, covered) = drain.respond(nz(1), 0)?;
    drain.commit_offered(covered.expect("nonempty page"));
    // Interior ack strictly inside the leading GAP.
    let (reply, covered) = drain.respond(nz(2), 1)?;
    let (entries, _, offered, confirmed) = drain.snapshot();
    assert_eq!(confirmed, 1);
    assert_eq!(offered, covered.expect("committed"));
    // Head clipped in place: GAP(2,4), one slot, same high bound.
    assert_eq!(gap(&entries[0]), Some((2, 4)));
    assert_eq!(entries.len(), MAXIMUM_QUEUED_ENTRIES); // 1 GAP + 1023 EVENTs
    assert_eq!(event(&entries[1]), Some(5)); // coverage continues
    // The fresh page leads with the clipped GAP, not a new one.
    assert_eq!(gap(&reply.entries()[0]), Some((2, 4)));
    assert_eq!(reply.acknowledged_echo(), 1);
    assert_eq!(reply.queue_tail(), 1027);
    // Budget-identical page: GAP(17) + 135 EVENTs(60B) -> covers ..=139.
    let page_bytes: usize = reply
        .entries()
        .iter()
        .map(AgentDrainEntry::encoded_bytes)
        .sum();
    assert!(page_bytes <= REPLY_ENTRY_BYTES);
    assert_eq!(covered, Some(139));
    // Nothing after the commit may be offered; re-commit is monotone.
    drain.commit_offered(139);
    let (_, _, offered, _) = drain.snapshot();
    assert_eq!(offered, 139);
    Ok(())
}

/// At minted_hi == u64::MAX the next append fails closed and never wraps
/// the sequence space (input44 B2, exercised through the test-only minted
/// seeding seam).
#[test]
fn mint_at_max_fails_closed_without_wrap() -> TestResult {
    let mut drain = AgentDrain::new();
    drain.seed_minted_for_test(u64::MAX - 1);
    append(&mut drain, 1)?; // mints u64::MAX
    assert!(append(&mut drain, 1).is_err()); // next mint fails closed
    let (_, minted, _, _) = drain.snapshot();
    assert_eq!(minted, u64::MAX); // never wraps to 0
    Ok(())
}

#[test]
fn stale_ack_is_noop_and_future_ack_is_fatal() -> TestResult {
    let mut drain = AgentDrain::new();
    append(&mut drain, 1)?;
    let (_, covered) = drain.respond(nz(1), 0)?;
    drain.commit_offered(covered.expect("covered"));

    // ack beyond offered is fatal.
    assert!(drain.respond(nz(2), 2).is_err());
    // stale ack at or below confirmed is a legal no-op; the unconfirmed
    // sequence 1 stays queued and is offered again, never dropped.
    let (reply, _) = drain.respond(nz(3), 0)?;
    assert_eq!(reply.acknowledged_echo(), 0);
    assert!(matches!(
        reply.entries().first(),
        Some(AgentDrainEntry::Event { sequence, .. }) if sequence.get() == 1
    ));
    Ok(())
}

/// One page never splits a record: two maximum-size events cost 8310 bytes
/// which exceeds the 8165 budget, so the second event stays queued for the
/// next request.
#[test]
fn page_respects_reply_entry_budget() -> TestResult {
    let mut drain = AgentDrain::new();
    append(&mut drain, MAXIMUM_SIGNAL_PAYLOAD_BYTES)?;
    append(&mut drain, MAXIMUM_SIGNAL_PAYLOAD_BYTES)?;
    append(&mut drain, 8)?;
    let (reply, covered) = drain.respond(nz(1), 0)?;
    let page_bytes: usize = reply
        .entries()
        .iter()
        .map(AgentDrainEntry::encoded_bytes)
        .sum();
    assert!(page_bytes <= REPLY_ENTRY_BYTES);
    assert_eq!(reply.entries().len(), 1);
    assert_eq!(covered, Some(1));
    drain.commit_offered(1);
    let (reply, covered) = drain.respond(nz(2), 1)?;
    assert_eq!(reply.entries().len(), 2);
    assert_eq!(covered, Some(3));
    Ok(())
}

#[test]
fn oversized_payload_rejects() {
    let mut drain = AgentDrain::new();
    assert!(
        drain
            .append(identity(9), "x".repeat(MAXIMUM_SIGNAL_PAYLOAD_BYTES + 1))
            .is_err()
    );
}

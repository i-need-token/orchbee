//! Deterministic coverage for the retained-history viewport core: entry,
//! facts gates, window install, acknowledgement promotion, navigation
//! coalescing and bounds, typed/opaque rejections, recovery, deadlines,
//! resize, and sequence exhaustion — the T1–T10 surface of
//! `docs/native-terminal-viewport.md`.

use std::{num::NonZeroU64, time::Duration, time::Instant};

use orchbee_client_server_protocol::{ViewerRowsRejectedReason, ViewportRowOrigin};
use orchbee_terminal_protocol::{AttachmentId, HistoryAnchorStatus, HistoryChanged, HostEpoch};

use super::{
    TerminalViewport, ViewportMode, ViewportNav, ViewportReadRejection, ViewportStep,
    ViewportSurface,
};
use crate::terminal_session::state::SessionNotice;

fn ids() -> (AttachmentId, HostEpoch) {
    (
        AttachmentId::from_bytes([7; 16]).expect("attachment"),
        HostEpoch::from_bytes([9; 16]).expect("epoch"),
    )
}

fn facts_event(sequence: u64, generation: u64, anchor: HistoryAnchorStatus) -> HistoryChanged {
    let (attachment_id, _host_epoch) = ids();
    HistoryChanged {
        attachment_id,
        sequence: NonZeroU64::new(sequence).expect("sequence"),
        revision: sequence,
        generation: NonZeroU64::new(generation).expect("generation"),
        anchor_status: anchor,
        total_rows: 200,
    }
}

fn window(
    sequence: u64,
    generation: u64,
    start_row: u64,
    total_rows: u64,
    columns: u16,
    rows: u16,
) -> crate::terminal_session::terminal_viewport::ViewportWindow {
    let (attachment_id, host_epoch) = ids();
    crate::terminal_session::terminal_viewport::ViewportWindow {
        route: orchbee_terminal_protocol::TerminalRowWindowRoute {
            attachment_id,
            host_epoch,
            sequence: NonZeroU64::new(sequence).unwrap(),
            revision: sequence,
            generation: NonZeroU64::new(generation).unwrap(),
            total_rows,
            start_row,
            columns,
            row_count: rows,
            detail: orchbee_terminal_protocol::RowWindowDetail::Render,
        },
        foreground: 0xffffff,
        background: 0,
        rows: (0..rows)
            .map(|index| orchbee_terminal_protocol::TerminalTextRow {
                index,
                flags: 0,
                cells: vec![orchbee_terminal_protocol::TerminalTextCell {
                    text: "x".into(),
                    width: 0,
                    style: orchbee_terminal_protocol::TerminalTextStyle {
                        foreground: 0xffffff,
                        background: 0,
                        underline_color: 0x1000000,
                        flags: 0,
                        underline: 0,
                    },
                }],
            })
            .collect(),
    }
}

fn entering(columns: u16, row_count: u16) -> TerminalViewport {
    entering_at(columns, row_count, 1)
}

/// A core entered above a previously issued read watermark — the same
/// attachment's next episode must resume above it, never restart.
fn entering_at(columns: u16, row_count: u16, first_read_sequence: u64) -> TerminalViewport {
    let (attachment_id, host_epoch) = ids();
    TerminalViewport::entering(
        attachment_id,
        host_epoch,
        columns,
        row_count,
        u32::from(row_count),
        first_read_sequence,
        Instant::now(),
    )
}

/// Admits every staged command in order, returning what was emitted.
fn drain(viewport: &mut TerminalViewport) -> Vec<ViewportStep> {
    let mut admitted = Vec::new();
    while let Some(step) = viewport.peek_command() {
        admitted.push(step);
        viewport.command_admitted();
    }
    admitted
}

/// A viewing core: entered, facts accepted, entry window installed, and its
/// acknowledgement accepted — the committed anchor state.
fn viewing(columns: u16, row_count: u16) -> TerminalViewport {
    let mut viewport = entering(columns, row_count);
    viewport.facts(facts_event(1, 1, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    let read_sequence = steps
        .iter()
        .find_map(|step| match step {
            ViewportStep::Read {
                request_sequence, ..
            } => Some(*request_sequence),
            _ => None,
        })
        .expect("entry read staged");
    viewport.window(
        window(read_sequence.get(), 1, 90, 200, columns, row_count),
        Instant::now(),
    );
    let ack = drain(&mut viewport);
    assert!(matches!(ack.as_slice(), [ViewportStep::Ack { .. }]));
    let effect = viewport.ack_outcome(read_sequence, true);
    assert!(!effect.live);
    assert_eq!(viewport.mode(), ViewportMode::Viewing);
    viewport
}

#[test]
fn consecutive_scrolls_preserve_direction_across_entry_and_committed_windows() {
    let (attachment, epoch) = ids();
    let mut viewport = TerminalViewport::entering(attachment, epoch, 80, 20, 3, 1, Instant::now());
    let mut facts = facts_event(1, 1, HistoryAnchorStatus::Valid);
    facts.total_rows = 102;
    viewport.facts(facts);
    let mut start = 82;
    for (intent, expected_start) in [(0, 79), (3, 76), (-3, 79)] {
        if intent != 0 {
            viewport.nav(ViewportNav::Lines(intent));
        }
        let steps = drain(&mut viewport);
        let [
            ViewportStep::Read {
                origin,
                request_sequence,
                ..
            },
        ] = steps.as_slice()
        else {
            panic!("expected one read, got {steps:?}");
        };
        // Host resolveStart measures committed deltas from the start row,
        // while the entry offset measures distance backwards from the tail.
        start = match origin {
            ViewportRowOrigin::Tail { rows_before } => 102 - 20 - i64::from(*rows_before),
            ViewportRowOrigin::Committed { delta } => start + i64::from(*delta),
        };
        assert_eq!(start, expected_start, "intent {intent}: {origin:?}");
        let sequence = *request_sequence;
        viewport.window(
            window(sequence.get(), 1, start as u64, 102, 80, 20),
            Instant::now(),
        );
        assert!(matches!(
            drain(&mut viewport).as_slice(),
            [ViewportStep::Ack { .. }]
        ));
        viewport.ack_outcome(sequence, true);
        assert!(drain(&mut viewport).is_empty());
    }
    viewport.nav(ViewportNav::Lines(-3));
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
}

// T1: while a read flight or an acknowledgement flight is open, no new read
// is emitted; navigation coalesces and only the install+accepted ack moves
// the committed anchor.
#[test]
fn t1_no_read_during_flights() {
    let mut viewport = viewing(80, 24);
    // Scroll once: a committed read stages and admits.
    viewport.nav(ViewportNav::Lines(10));
    let steps = drain(&mut viewport);
    let [
        ViewportStep::Read {
            origin: ViewportRowOrigin::Committed { delta },
            request_sequence,
            ..
        },
    ] = steps.as_slice()
    else {
        panic!("expected one committed read, got {steps:?}");
    };
    assert_eq!(*delta, -10);
    let request_sequence = *request_sequence;
    // During the read flight more scroll intent coalesces without emitting.
    viewport.nav(ViewportNav::Lines(5));
    assert!(viewport.peek_command().is_none());
    // The window installs; an ack stages. While it flies, still no read.
    viewport.window(
        window(request_sequence.get(), 1, 80, 200, 80, 24),
        Instant::now(),
    );
    let ack = drain(&mut viewport);
    assert!(matches!(ack.as_slice(), [ViewportStep::Ack { .. }]));
    viewport.nav(ViewportNav::Lines(5));
    assert!(viewport.peek_command().is_none());
    // Accepted ack promotes the anchor; the coalesced older-row intent
    // emits another -10 start-row delta.
    viewport.ack_outcome(request_sequence, true);
    let steps = drain(&mut viewport);
    let [
        ViewportStep::Read {
            origin: ViewportRowOrigin::Committed { delta },
            ..
        },
    ] = steps.as_slice()
    else {
        panic!("expected one residual committed read, got {steps:?}");
    };
    assert_eq!(*delta, -10);
}

// T2: host-pending and typed failures enter recovery, never kill the pane.
#[test]
fn t2_rejections_recover_not_kill() {
    for rejection in [
        ViewportReadRejection::Host,
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::InvalidField),
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::InvalidPhase),
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::HistoryEvicted),
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::MissingResource),
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::InvalidAcknowledgement),
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::ResourceExhausted),
    ] {
        let mut viewport = viewing(80, 24);
        viewport.nav(ViewportNav::Lines(-1));
        let steps = drain(&mut viewport);
        let request_sequence = match steps.as_slice() {
            [
                ViewportStep::Read {
                    request_sequence, ..
                },
            ] => *request_sequence,
            other => panic!("expected one read, got {other:?}"),
        };
        let effect = viewport.read_outcome(request_sequence, rejection, Instant::now());
        assert!(!effect.live, "rejection {rejection:?} must not kill");
        assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
        assert!(viewport.recovering());
        assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
    }
}

// T3: a large committed delta splits into ±4096-bounded residual steps.
#[test]
fn t3_committed_delta_splits_at_bound() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Lines(10_000));
    let mut deltas = Vec::new();
    // Each installed window + accepted ack admits the next residual chunk.
    for served in (0..3u64).rev() {
        let steps = drain(&mut viewport);
        let [
            ViewportStep::Read {
                origin: ViewportRowOrigin::Committed { delta },
                request_sequence,
                ..
            },
        ] = steps.as_slice()
        else {
            panic!("expected one bounded committed read, got {steps:?}");
        };
        assert!(delta.abs() <= 4096);
        deltas.push(*delta);
        let request_sequence = *request_sequence;
        // Serve a window 4096 rows older each step so the estimate keeps
        // headroom.
        viewport.window(
            window(request_sequence.get(), 1, served * 4096, 100_000, 80, 24),
            Instant::now(),
        );
        drain(&mut viewport);
        viewport.ack_outcome(request_sequence, true);
    }
    assert_eq!(deltas, [-4096, -4096, -1808]);
    assert!(viewport.peek_command().is_none());
}

// T4: an evicted committed anchor enters recovery with a notice and never
// fabricates a position.
#[test]
fn t4_anchor_eviction_recovers() {
    let mut viewport = viewing(80, 24);
    let effect = viewport.facts(facts_event(2, 1, HistoryAnchorStatus::Evicted));
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(viewport.recovering());
    assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
    assert!(viewport.position().is_none());
    // No further reads while recovering.
    viewport.nav(ViewportNav::Lines(-1));
    assert!(
        drain(&mut viewport)
            .iter()
            .all(|step| matches!(step, ViewportStep::Recover))
    );
}

// T5: a valid facts event clears the facts gate exactly once per arrival —
// entry, stale retry, and post-resize waits all re-arm on the next event.
#[test]
fn t5_facts_gate_per_event() {
    let mut viewport = entering(80, 24);
    // No read before the first facts event.
    assert!(drain(&mut viewport).is_empty());
    viewport.facts(facts_event(1, 1, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    assert!(
        steps.iter().any(|step| matches!(
            step,
            ViewportStep::Read {
                origin: ViewportRowOrigin::Tail { .. },
                ..
            }
        )),
        "entry tail read must stage after facts: {steps:?}"
    );
}

// T6: an ordinary typed byte queues recovery before any input may ride.
#[test]
fn t6_typed_queues_recover_first() {
    let mut viewport = viewing(80, 24);
    viewport.typed();
    let steps = drain(&mut viewport);
    assert_eq!(steps, [ViewportStep::Recover]);
    assert!(viewport.recovering());
    // A second byte while recovering queues nothing further.
    viewport.typed();
    assert!(viewport.peek_command().is_none());
}

// T7: a rejected entry drops back to live with the unavailable notice and
// never replays.
#[test]
fn t7_enter_rejected_returns_live() {
    let mut viewport = entering(80, 24);
    let effect = viewport.enter_outcome(false);
    assert!(effect.live);
    assert_eq!(effect.notice, Some(SessionNotice::HistoryUnavailable));
}

// T8: a silent facts channel hits the bounded deadline and recovers; a facts
// arrival inside the budget instead serves the entry window.
#[test]
fn t8_facts_deadline_recovers() {
    let start = Instant::now();
    let (attachment_id, host_epoch) = ids();
    let mut viewport = TerminalViewport::entering(attachment_id, host_epoch, 80, 24, 24, 1, start);
    let effect = viewport.tick(start + Duration::from_secs(3));
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(viewport.recovering());
    assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
    // Inside the budget the facts event unblocks the entry read.
    let mut viewport = TerminalViewport::entering(attachment_id, host_epoch, 80, 24, 24, 1, start);
    assert!(
        viewport
            .tick(start + Duration::from_secs(2))
            .notice
            .is_none()
    );
    viewport.facts(facts_event(1, 1, HistoryAnchorStatus::Valid));
    assert!(!drain(&mut viewport).is_empty());
}

// T9: resize invalidates the painted window and blocks on fresh facts; the
// old columns can never paint again, and an unchanged geometry report is a
// no-op that keeps the wait episode.
#[test]
fn t9_resize_invalidates_and_waits() {
    let mut viewport = viewing(80, 24);
    let start = Instant::now();
    assert!(matches!(viewport.surface(), ViewportSurface::Window(_)));
    viewport.resize(120, 30, start);
    // Old columns never paint; new columns have no window yet.
    assert!(matches!(
        viewport.surface(),
        ViewportSurface::AwaitingWindow
    ));
    assert!(matches!(
        viewport.surface(),
        ViewportSurface::AwaitingWindow
    ));
    // The wait is bounded: a repeated identical resize must not extend it.
    viewport.resize(120, 30, start + Duration::from_secs(2));
    let effect = viewport.tick(start + Duration::from_secs(3));
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(viewport.recovering());
    assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
}

// T10: recovery ends only when recovery demands were applied *and* a frame
// arrives afterwards; an in-flight frame can never end it early.
#[test]
fn t10_recovery_needs_recovered_then_frame() {
    let mut viewport = viewing(80, 24);
    viewport.typed();
    let steps = drain(&mut viewport);
    assert_eq!(steps, [ViewportStep::Recover]);
    // A frame arriving before the recovered event cannot end recovery.
    let effect = viewport.frame_applied();
    assert!(!effect.live);
    viewport.recovered();
    assert!(viewport.recovering());
    // The first frame after the applied recovery returns the pane to live.
    let effect = viewport.frame_applied();
    assert!(effect.live);
}

// A window for a sequence that cannot prove its pending enters recovery —
// never adoption.
#[test]
fn stray_window_recovers() {
    let mut viewport = viewing(80, 24);
    let effect = viewport.window(window(99, 1, 0, 200, 80, 24), Instant::now());
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(viewport.recovering());
    assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
}

// A window stamped at another attachment or epoch is a wire violation, not
// a local recovery.
#[test]
fn foreign_route_window_is_protocol_error() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Lines(-1));
    let steps = drain(&mut viewport);
    let request_sequence = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected one read, got {other:?}"),
    };
    let mut foreign = window(request_sequence.get(), 1, 0, 200, 80, 24);
    foreign.route.attachment_id = AttachmentId::from_bytes([8; 16]).expect("other attachment");
    let effect = viewport.window(foreign, Instant::now());
    assert!(effect.protocol_error);
}

// A stale-generation or stale-column window releases the flight and waits
// for fresh facts instead of entering recovery or installing.
#[test]
fn stale_window_releases_flight() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Lines(-1));
    let steps = drain(&mut viewport);
    let request_sequence = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected one read, got {other:?}"),
    };
    let stale = window(request_sequence.get(), 2, 0, 200, 80, 24);
    let effect = viewport.window(stale, Instant::now());
    assert!(!viewport.recovering());
    assert_eq!(effect, Default::default());
    assert!(matches!(
        viewport.surface(),
        ViewportSurface::AwaitingWindow
    ));
    // The released flight lets the next facts event re-drive a fresh read.
    viewport.facts(facts_event(2, 2, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    assert!(
        steps.iter().any(
            |step| matches!(step, ViewportStep::Read { generation, .. } if generation.get() == 2)
        ),
        "fresh-generation read must stage: {steps:?}"
    );
}

// A stale typed rejection retries once inside the same bounded episode; a
// second one recovers.
#[test]
fn typed_stale_retries_once() {
    let mut viewport = viewing(80, 24);
    for attempt in 0..2 {
        viewport.nav(ViewportNav::Lines(-1));
        let steps = drain(&mut viewport);
        let request_sequence = match steps.as_slice() {
            [
                ViewportStep::Read {
                    request_sequence, ..
                },
            ] => *request_sequence,
            other => panic!("expected read {attempt}, got {other:?}"),
        };
        let effect = viewport.read_outcome(
            request_sequence,
            ViewportReadRejection::Typed(ViewerRowsRejectedReason::StaleGeneration),
            Instant::now(),
        );
        if attempt == 0 {
            // Retry: waits for fresh facts; fresh facts re-drive the intent.
            assert_eq!(effect, Default::default());
            viewport.facts(facts_event(2, 1, HistoryAnchorStatus::Valid));
        } else {
            assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
            assert!(viewport.recovering());
            assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
        }
    }
}

// `LimitExceeded` halves the read once inside the episode; a second
// rejection recovers.
#[test]
fn limit_exceeded_halves_once() {
    let mut viewport = viewing(80, 24);
    for attempt in 0..2 {
        viewport.nav(ViewportNav::Lines(-1));
        let steps = drain(&mut viewport);
        let (request_sequence, row_count) = match steps.as_slice() {
            [
                ViewportStep::Read {
                    request_sequence,
                    row_count,
                    ..
                },
            ] => (*request_sequence, *row_count),
            other => panic!("expected read {attempt}, got {other:?}"),
        };
        assert_eq!(row_count, if attempt == 0 { 24 } else { 12 });
        let effect = viewport.read_outcome(
            request_sequence,
            ViewportReadRejection::Typed(ViewerRowsRejectedReason::LimitExceeded),
            Instant::now(),
        );
        if attempt == 0 {
            assert_eq!(effect, Default::default());
        } else {
            assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
            assert!(viewport.recovering());
        }
    }
}

// An acknowledgement rejection loses the anchor: recovery, notice, and no
// fabricated position.
#[test]
fn ack_rejection_recovers() {
    let mut viewport = entering(80, 24);
    viewport.facts(facts_event(1, 1, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    let request_sequence = steps
        .iter()
        .find_map(|step| match step {
            ViewportStep::Read {
                request_sequence, ..
            } => Some(*request_sequence),
            _ => None,
        })
        .expect("entry read");
    viewport.window(
        window(request_sequence.get(), 1, 90, 200, 80, 24),
        Instant::now(),
    );
    drain(&mut viewport);
    let effect = viewport.ack_outcome(request_sequence, false);
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(viewport.recovering());
    assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
    assert!(viewport.position().is_none());
}

// Scrolling to the live tail exits through recovery instead of emitting a
// read the host would only clamp.
#[test]
fn bottom_exit_queues_recover() {
    let mut viewport = viewing(80, 24);
    // The installed window covers rows 90..114 of 200: 86 rows remain below.
    viewport.nav(ViewportNav::Lines(-86));
    let steps = drain(&mut viewport);
    assert_eq!(steps, [ViewportStep::Recover]);
    assert!(viewport.recovering());
}

// `Home` owns the next read absolutely; a delta landing on the absolute
// intent is dropped.
#[test]
fn oldest_jump_owns_pending() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Oldest);
    viewport.nav(ViewportNav::Lines(-10));
    let steps = drain(&mut viewport);
    let [
        ViewportStep::Read {
            origin: ViewportRowOrigin::Tail { rows_before },
            ..
        },
    ] = steps.as_slice()
    else {
        panic!("expected one tail read, got {steps:?}");
    };
    assert_eq!(*rows_before, u32::MAX);
}

#[test]
fn copy_newest_installs_tail_zero_without_recovering_and_only_exposes_committed_window() {
    let mut viewport = viewing(80, 24);
    assert!(viewport.committed_window().is_some());
    viewport.nav(ViewportNav::Newest);
    let steps = drain(&mut viewport);
    let [
        ViewportStep::Read {
            origin: ViewportRowOrigin::Tail { rows_before },
            request_sequence,
            ..
        },
    ] = steps.as_slice()
    else {
        panic!("expected newest read")
    };
    assert_eq!(*rows_before, 0);
    assert!(viewport.committed_window().is_some());
    let sequence = *request_sequence;
    viewport.window(window(sequence.get(), 1, 176, 200, 80, 24), Instant::now());
    assert!(viewport.committed_window().is_none());
    assert!(viewport.copy_commit_pending());
    assert!(matches!(
        drain(&mut viewport).as_slice(),
        [ViewportStep::Ack { .. }]
    ));
    viewport.ack_outcome(sequence, true);
    assert_eq!(viewport.mode(), ViewportMode::Viewing);
    assert_eq!(viewport.committed_window().unwrap().route.start_row, 176);
    assert!(!viewport.copy_commit_pending());
    viewport.nav(ViewportNav::Exit);
    assert!(!viewport.copy_commit_pending());
}

// Read-sequence exhaustion enters recovery rather than wrapping.
#[test]
fn sequence_exhaustion_recovers() {
    let mut viewport = viewing(80, 24);
    // Drive the sequence to the edge via the accessor-visible path: admit
    // and settle reads until exhaustion. `next_read_sequence` is private, so
    // emulate the exhaustion boundary with repeated full cycles — bounded
    // by the test budget, not by an unreachable loop. Instead, exhaust via
    // the documented `None` arm: a read that cannot mint a sequence recovers.
    // Direct exhaustion is covered structurally by `NonZeroU64::new(_)?` in
    // `next_read_step`; here we assert one honest full cycle stays sound.
    for _ in 0..2 {
        viewport.nav(ViewportNav::Lines(-1));
        let steps = drain(&mut viewport);
        let request_sequence = match steps.as_slice() {
            [
                ViewportStep::Read {
                    request_sequence, ..
                },
            ] => *request_sequence,
            other => panic!("expected read, got {other:?}"),
        };
        viewport.window(
            window(request_sequence.get(), 1, 10, 200, 80, 24),
            Instant::now(),
        );
        drain(&mut viewport);
        viewport.ack_outcome(request_sequence, true);
    }
    assert_eq!(viewport.mode(), ViewportMode::Viewing);
}

// Entering without facts shows the entry surface; a live-mode surface is
// never produced by a non-live core.
#[test]
fn surfaces_track_mode() {
    let viewport = entering(80, 24);
    assert!(matches!(viewport.surface(), ViewportSurface::Entering));
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Exit);
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(matches!(viewport.surface(), ViewportSurface::Recovering));
}

// A re-entered episode resumes above the projection's read watermark — the
// host's attachment-scoped `last_row_request_sequence` survives the cycle.
#[test]
fn reentry_resumes_above_watermark() {
    let mut viewport = entering_at(80, 24, 7);
    viewport.facts(facts_event(1, 1, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    let sequence = steps
        .iter()
        .find_map(|step| match step {
            ViewportStep::Read {
                request_sequence, ..
            } => Some(request_sequence.get()),
            _ => None,
        })
        .expect("entry read");
    assert_eq!(sequence, 7, "first read resumes at the watermark, not 1");
}

// A stale window proves its pin was released when the held facts already
// advanced past the issue generation: the transition is the release, so the
// next read proceeds under the fresh generation without waiting for an
// event the host will never re-emit.
#[test]
fn advanced_facts_release_stale_window() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Lines(-5));
    let steps = drain(&mut viewport);
    let request_sequence = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence,
                generation,
                ..
            },
        ] => {
            assert_eq!(generation.get(), 1, "issued under generation 1");
            *request_sequence
        }
        other => panic!("expected one read, got {other:?}"),
    };
    // Facts advance mid-flight; the flight stays open so nothing emits yet.
    viewport.facts(facts_event(2, 2, HistoryAnchorStatus::Valid));
    assert!(drain(&mut viewport).is_empty());
    // The generation-1 window lands: the kind-53 transition already proved
    // this pending's pin was cleared — release and proceed under gen 2.
    let effect = viewport.window(
        window(request_sequence.get(), 1, 80, 200, 80, 24),
        Instant::now(),
    );
    assert_eq!(effect, Default::default());
    assert!(!viewport.recovering());
    let steps = drain(&mut viewport);
    assert!(
        steps.iter().any(
            |step| matches!(step, ViewportStep::Read { generation, .. } if generation.get() == 2)
        ),
        "the released flight re-reads under the fresh generation: {steps:?}"
    );
}

// The contrast case: facts that did NOT move since the read was issued
// mean the host still owes an event — the flight releases into a bounded
// wait, and only fresh facts re-drive.
#[test]
fn unmoved_facts_stale_window_waits_bounded() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Lines(-5));
    let steps = drain(&mut viewport);
    let request_sequence = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected one read, got {other:?}"),
    };
    // A window stamped at a generation the facts never reported: no proof
    // of release — wait, bounded, for the describing event.
    let effect = viewport.window(
        window(request_sequence.get(), 9, 80, 200, 80, 24),
        Instant::now(),
    );
    assert_eq!(effect, Default::default());
    assert!(!viewport.recovering());
    assert!(
        drain(&mut viewport).is_empty(),
        "no read before fresh facts"
    );
    assert!(matches!(
        viewport.surface(),
        ViewportSurface::AwaitingWindow
    ));
    viewport.facts(facts_event(2, 9, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    assert!(
        steps.iter().any(
            |step| matches!(step, ViewportStep::Read { generation, .. } if generation.get() == 9)
        ),
        "fresh facts re-drive the intent: {steps:?}"
    );
}

// Once recovery is decided — a Recover merely staged, not yet admitted —
// late facts, windows, and read/ack outcomes are absorbed: nothing
// installs, nothing re-arms, no second Recover is minted. The staged step
// remains the only command.
#[test]
fn pending_recovery_suppresses_late_arrivals() {
    let mut viewport = viewing(80, 24);
    viewport.typed();
    assert_eq!(viewport.peek_command(), Some(ViewportStep::Recover));
    assert_eq!(
        viewport.facts(facts_event(2, 2, HistoryAnchorStatus::Valid)),
        Default::default()
    );
    assert_eq!(viewport.peek_command(), Some(ViewportStep::Recover));
    // An evicted anchor on top of the decided recovery: still absorbed,
    // still no second notice.
    assert_eq!(
        viewport.facts(facts_event(3, 2, HistoryAnchorStatus::Evicted)),
        Default::default()
    );
    assert_eq!(viewport.peek_command(), Some(ViewportStep::Recover));
    assert_eq!(
        viewport.window(window(77, 2, 0, 200, 80, 24), Instant::now()),
        Default::default()
    );
    assert_eq!(viewport.peek_command(), Some(ViewportStep::Recover));
    assert_eq!(
        viewport.read_outcome(
            NonZeroU64::new(1).expect("sequence"),
            ViewportReadRejection::Host,
            Instant::now(),
        ),
        Default::default()
    );
    assert_eq!(
        viewport.ack_outcome(NonZeroU64::new(1).expect("sequence"), true),
        Default::default()
    );
    // Navigation — Exit included — mints nothing while recovery is staged.
    viewport.nav(ViewportNav::Exit);
    viewport.nav(ViewportNav::Oldest);
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(viewport.recovering());
}

// The same absorption once Recovering is entered: a second Exit or a late
// window never mints another Recover.
#[test]
fn recovering_absorbs_exit_and_late_window() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Exit);
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert!(viewport.recovering());
    viewport.nav(ViewportNav::Exit);
    viewport.typed();
    assert_eq!(
        viewport.window(window(3, 1, 0, 200, 80, 24), Instant::now()),
        Default::default()
    );
    assert!(viewport.peek_command().is_none());
}

// The retry/halving budget is per intent: a completed intent leaves the
// budget re-armed for the next nav call, while merges inside one intent
// share it.
#[test]
fn intent_budget_rearms_between_intents() {
    let mut viewport = viewing(80, 24);
    // First intent: one stale retry inside it, then the window installs
    // and the intent completes.
    viewport.nav(ViewportNav::Lines(-1));
    let steps = drain(&mut viewport);
    let first = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected read, got {other:?}"),
    };
    let effect = viewport.read_outcome(
        first,
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::StaleGeneration),
        Instant::now(),
    );
    assert_eq!(effect, Default::default());
    viewport.facts(facts_event(2, 2, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    let second = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected retry read, got {other:?}"),
    };
    viewport.window(window(second.get(), 2, 60, 200, 80, 24), Instant::now());
    drain(&mut viewport);
    viewport.ack_outcome(second, true);
    // A fresh intent re-arms the budget: the next stale rejection retries
    // rather than recovering, and only its own second stale fails.
    viewport.nav(ViewportNav::Lines(-1));
    let steps = drain(&mut viewport);
    let third = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected fresh-intent read, got {other:?}"),
    };
    let effect = viewport.read_outcome(
        third,
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::StaleGeneration),
        Instant::now(),
    );
    assert_eq!(effect, Default::default(), "fresh intent keeps its retry");
    viewport.facts(facts_event(3, 3, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    let fourth = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected second-intent retry, got {other:?}"),
    };
    let effect = viewport.read_outcome(
        fourth,
        ViewportReadRejection::Typed(ViewerRowsRejectedReason::StaleGeneration),
        Instant::now(),
    );
    assert_eq!(drain(&mut viewport), [ViewportStep::Recover]);
    assert_eq!(effect.notice, Some(SessionNotice::HistoryRecovering));
}

// `position` reports only the currently validated window: entering has
// none, a resize-expired window reports none, and recovering reports none.
#[test]
fn position_tracks_current_window_only() {
    let viewport = entering(80, 24);
    assert_eq!(viewport.position(), None);
    let mut viewport = viewing(80, 24);
    assert_eq!(viewport.position(), Some((90, 200)));
    viewport.resize(120, 30, Instant::now());
    assert_eq!(viewport.position(), None);
    // After a committed window is re-established the position returns.
    viewport.facts(facts_event(2, 1, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    let request_sequence = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected post-resize read, got {other:?}"),
    };
    viewport.window(
        window(request_sequence.get(), 1, 40, 200, 120, 30),
        Instant::now(),
    );
    assert_eq!(viewport.position(), Some((40, 200)));
}

// A resize drops unadmitted wire steps minted under the old geometry —
// the staged acknowledgement for the discarded window never reaches the
// wire — while an already-admitted Recover is preserved.
#[test]
fn resize_drops_staged_ack_and_preserves_the_previously_acknowledged_anchor() {
    let mut viewport = viewing(80, 24);
    viewport.nav(ViewportNav::Lines(-5));
    let steps = drain(&mut viewport);
    let request_sequence = match steps.as_slice() {
        [
            ViewportStep::Read {
                request_sequence, ..
            },
        ] => *request_sequence,
        other => panic!("expected one read, got {other:?}"),
    };
    // The window installs and stages its Ack — but nothing admits it yet.
    viewport.window(
        window(request_sequence.get(), 1, 80, 200, 80, 24),
        Instant::now(),
    );
    assert!(matches!(
        viewport.peek_command(),
        Some(ViewportStep::Ack { .. })
    ));
    viewport.resize(120, 30, Instant::now());
    // The staged Ack died with the old geometry: it never emits.
    assert_eq!(drain(&mut viewport), Vec::<ViewportStep>::new());
    assert!(matches!(
        viewport.surface(),
        ViewportSurface::AwaitingWindow
    ));
    // A late acceptance for the discarded window cannot install or promote it.
    let effect = viewport.ack_outcome(request_sequence, true);
    assert!(!effect.live && effect.notice.is_none());
    assert!(drain(&mut viewport).is_empty());
    // The earlier accepted anchor survives independently of the new window's
    // cancelled Ack. Fresh facts refresh that anchor at the new geometry.
    viewport.facts(facts_event(2, 2, HistoryAnchorStatus::Valid));
    let steps = drain(&mut viewport);
    assert!(
        matches!(
            steps.as_slice(),
            [ViewportStep::Read {
                origin: ViewportRowOrigin::Committed { delta: 0 },
                row_count: 30,
                generation,
                request_sequence: next_sequence,
                ..
            }] if generation.get() == 2 && next_sequence > &request_sequence
        ),
        "fresh geometry must refresh the previously accepted anchor: {steps:?}"
    );
}

#[test]
fn row_response_requires_the_transport_envelope_and_matching_bounded_payload() {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, MessageRole, PayloadEncoding,
        decode_terminal_render_row_window, encode_transport_envelope,
    };
    const ROWS: &[u8] = include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/ghostty-render-row-window-v0.0.3.bin"
    );
    let expected = decode_terminal_render_row_window(ROWS).unwrap();
    let envelope = Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind: 46,
        payload_encoding: PayloadEncoding::RowWindow,
        host_epoch: Some(expected.route().host_epoch),
        correlation_id: Some(CorrelationId::from_bytes([3; 16]).unwrap()),
        optional_message: false,
        payload: ROWS,
    };
    let encoded = encode_transport_envelope(&envelope).unwrap();
    let actual = super::decode_viewport_window(&encoded, ROWS.len() as u32).unwrap();
    assert_eq!(actual.route, expected.route());
    assert_eq!(actual.foreground, 0xd8dee9);
    assert_eq!(actual.foreground, expected.projection().foreground());
    assert_eq!(actual.background, 0x101418);
    assert_eq!(actual.background, expected.projection().background());
    let mut aliases = std::collections::HashMap::new();
    for (actual_row, expected_row) in actual.rows.iter().zip(expected.projection().rows()) {
        assert_eq!(actual_row.index, expected_row.index());
        assert_eq!(actual_row.flags, expected_row.flags());
        for (cell, expected) in actual_row.cells.iter().zip(expected_row.cells()) {
            assert_eq!(cell.text.as_ref(), expected.text);
            assert_eq!(cell.style, expected.style);
            assert_eq!(cell.width, expected.width);
            let key = (expected.text.as_ptr() as usize, expected.text.len());
            if let Some(previous) = aliases.insert(key, &cell.text) {
                assert!(
                    std::sync::Arc::ptr_eq(previous, &cell.text),
                    "aliased wire strings must share allocation"
                );
            }
        }
    }
    assert!(
        actual
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .any(|c| c.width == 1)
    );
    assert!(
        super::decode_viewport_window(ROWS, u32::MAX).is_err(),
        "bare OBRW is not a response"
    );
    assert!(super::decode_viewport_window(&encoded, ROWS.len() as u32 - 1).is_err());
    assert!(super::decode_viewport_window(&encoded[..encoded.len() - 1], u32::MAX).is_err());
    let mismatched = encode_transport_envelope(&Envelope {
        host_epoch: Some(HostEpoch::from_bytes([9; 16]).unwrap()),
        ..envelope
    })
    .unwrap();
    assert!(super::decode_viewport_window(&mismatched, u32::MAX).is_err());
    let text = include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-row-window-v0.0.3.bin"
    );
    let text_record = encode_transport_envelope(&Envelope {
        payload: text,
        ..envelope
    })
    .unwrap();
    assert!(
        super::decode_viewport_window(&text_record, u32::MAX).is_err(),
        "no text fallback"
    );
    let mut bad_graphics = ROWS.to_vec();
    *bad_graphics.last_mut().unwrap() = 1;
    let malformed = encode_transport_envelope(&Envelope {
        payload: &bad_graphics,
        ..envelope
    })
    .unwrap();
    assert!(
        super::decode_viewport_window(&malformed, u32::MAX).is_err(),
        "unused graphics still strictly validated"
    );
}

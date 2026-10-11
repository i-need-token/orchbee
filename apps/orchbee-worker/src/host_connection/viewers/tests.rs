use super::*;
use orchbee_capability_node_protocol::{HostTerminalId, WorkerGeneration, WorkerId};
use orchbee_terminal_protocol::{
    Envelope, HistoryAnchorStatus, HistoryChanged, ProjectionBlockReason, ProjectionBlocked,
    SelectionChanged, SelectionChangedState, SelectionControl, SelectionExportChunk,
    SelectionExportControl, SelectionExportFinished, SelectionExportStatus, TerminalBell,
    TerminalClipboardWriteRequest, TerminalEffectEvent, decode_terminal_text_frame,
    encode_history_control, encode_projection_demand_control, encode_selection_control,
    encode_selection_export_chunk, encode_selection_export_control, encode_terminal_effect_event,
    encode_transport_envelope,
};
use std::{error::Error, num::NonZeroU64};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn delivery_requires_exact_feedback_and_does_not_stall_another_attachment() -> TestResult {
    let mut viewers = HostViewers::default();
    let first = request(1)?;
    let second = request(2)?;
    viewers.admit(&first).map_err(|e| format!("{e:?}"))?;
    viewers.admit(&second).map_err(|e| format!("{e:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let first_frame = frame(1, 1, epoch)?;
    viewers.receive(first_frame.clone(), epoch)?;
    let feedback = TerminalViewerRequest {
        action: TerminalViewerAction::FrameApplied {
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 5,
        },
        ..first
    };
    assert_eq!(
        viewers.feedback(&feedback),
        Err(ViewerRejection::FeedbackMismatch)
    );
    assert_eq!(
        viewers.poll(first.attachment_id),
        TerminalViewerOutcome::Record(first_frame)
    );
    let wrong = TerminalViewerRequest {
        action: TerminalViewerAction::FrameApplied {
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 6,
        },
        ..first
    };
    assert_eq!(
        viewers.feedback(&wrong),
        Err(ViewerRejection::FeedbackMismatch)
    );
    let other = frame(2, 1, epoch)?;
    viewers.receive(other.clone(), epoch)?;
    assert_eq!(
        viewers.poll(second.attachment_id),
        TerminalViewerOutcome::Record(other)
    );
    viewers.feedback(&feedback).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        viewers.feedback(&feedback),
        Err(ViewerRejection::FeedbackMismatch)
    );
    viewers.receive(frame(1, 2, epoch)?, epoch)?;
    // A full slot closes only this viewer, preserving the other flight.
    viewers.receive(frame(1, 3, epoch)?, epoch)?;
    assert_eq!(
        viewers.poll(first.attachment_id),
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    assert_eq!(viewers.closed_ids(), vec![[1; 16]]);
    assert_eq!(
        viewers.poll(second.attachment_id),
        TerminalViewerOutcome::Empty
    );
    Ok(())
}

#[test]
fn snapshot_recovery_and_detach_are_attachment_scoped_and_capacity_is_reclaimed() -> TestResult {
    let mut viewers = HostViewers::default();
    for id in 1..=24 {
        viewers.admit(&request(id)?).map_err(|e| format!("{e:?}"))?;
    }
    assert_eq!(
        viewers.admit(&request(25)?),
        Err(ViewerRejection::CapacityExceeded)
    );
    assert_eq!(
        viewers.admit(&request(1)?),
        Err(ViewerRejection::AttachmentAlreadyExists)
    );
    let epoch = HostEpoch::from_bytes([3; 16])?;
    viewers.receive(frame(1, 1, epoch)?, epoch)?;
    viewers.poll(request(1)?.attachment_id);
    viewers
        .feedback(&TerminalViewerRequest {
            action: TerminalViewerAction::NeedSnapshot {
                sequence: NonZeroU64::MIN,
                reason: orchbee_capability_node_protocol::ViewerSnapshotReason::DecoderRejected,
            },
            ..request(1)?
        })
        .map_err(|e| format!("{e:?}"))?;
    let mut delta = frame(1, 2, epoch)?;
    delta[20..22].copy_from_slice(&42_u16.to_le_bytes());
    delta[64 + 6] = 2;
    delta[64 + 56..64 + 64].copy_from_slice(&4_u64.to_le_bytes());
    assert!(viewers.receive(delta, epoch).is_err());
    viewers.receive(frame(1, 2, epoch)?, epoch)?;
    viewers.remove(request(1)?.attachment_id);
    viewers.admit(&request(25)?).map_err(|e| format!("{e:?}"))?;
    assert!(viewers.receive(frame(1, 3, epoch)?, epoch).is_err());
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    Ok(())
}

#[test]
fn blocked_projection_preserves_bytes_and_recovers_only_with_a_later_snapshot() -> TestResult {
    let mut viewers = HostViewers::default();
    viewers.admit(&request(1)?).map_err(|e| format!("{e:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let blocked = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id: AttachmentId::from_bytes([1; 16])?,
            revision: 4,
            reason: ProjectionBlockReason::ProjectionTooLarge,
        },
    ))?;
    let record = envelope(
        blocked.kind,
        PayloadEncoding::ControlJson,
        &blocked.payload,
        epoch,
    )?;
    viewers.receive(record.clone(), epoch)?;
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(record)
    );
    let mut stale_recovery = frame(1, 1, epoch)?;
    stale_recovery[64 + 64..64 + 72].copy_from_slice(&4_u64.to_le_bytes());
    assert!(viewers.receive(stale_recovery, epoch).is_err());
    viewers.receive(frame(1, 1, epoch)?, epoch)?;
    assert!(viewers.receive(frame(2, 1, epoch)?, epoch).is_err());
    viewers
        .admit(&request(2)?)
        .map_err(|error| format!("{error:?}"))?;
    let pending = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id: AttachmentId::from_bytes([2; 16])?,
            revision: 4,
            reason: ProjectionBlockReason::ProjectionTooLarge,
        },
    ))?;
    viewers.receive(
        envelope(
            pending.kind,
            PayloadEncoding::ControlJson,
            &pending.payload,
            epoch,
        )?,
        epoch,
    )?;
    // Recovery cannot overwrite an unread blocked notification.
    viewers.receive(frame(2, 1, epoch)?, epoch)?;
    assert_eq!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    assert_eq!(viewers.closed_ids(), vec![[2; 16]]);
    Ok(())
}

#[test]
fn aggregate_pressure_retires_only_the_addressed_viewer_and_releases_exactly_once() -> TestResult {
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let first = frame(1, 1, epoch)?;
    let record_bytes = first.len();
    let mut viewers = HostViewers::with_retained_record_limit(record_bytes);
    viewers.admit(&request(1)?).map_err(|e| format!("{e:?}"))?;
    viewers.admit(&request(2)?).map_err(|e| format!("{e:?}"))?;
    // Equality retains: the ceiling equals this record's logical length.
    viewers.receive(first.clone(), epoch)?;
    assert_eq!(viewers.retained_record_bytes, record_bytes);
    // Crossing the aggregate retires only the addressed viewer.
    viewers.receive(frame(2, 1, epoch)?, epoch)?;
    assert_eq!(viewers.retained_record_bytes, record_bytes);
    assert_eq!(viewers.closed_ids(), vec![[2; 16]]);
    assert_eq!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    // A closed viewer's late frames are discarded without charging the budget.
    viewers.receive(frame(2, 2, epoch)?, epoch)?;
    assert_eq!(viewers.retained_record_bytes, record_bytes);
    // The sibling record survives untouched and releases exactly once on poll.
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(first)
    );
    assert_eq!(viewers.retained_record_bytes, 0);
    // Removing the retired viewer releases nothing twice.
    viewers.remove(request(2)?.attachment_id);
    assert_eq!(viewers.retained_record_bytes, 0);
    Ok(())
}

#[test]
fn both_record_kinds_charge_the_logical_budget_and_remove_releases_once() -> TestResult {
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let blocked = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id: AttachmentId::from_bytes([1; 16])?,
            revision: 4,
            reason: ProjectionBlockReason::ProjectionTooLarge,
        },
    ))?;
    let blocked_record = envelope(
        blocked.kind,
        PayloadEncoding::ControlJson,
        &blocked.payload,
        epoch,
    )?;
    let frame_record = frame(2, 1, epoch)?;
    let third = frame(3, 1, epoch)?;
    let limit = blocked_record.len() + frame_record.len() + third.len() - 1;
    let mut viewers = HostViewers::with_retained_record_limit(limit);
    for id in 1..=3_u8 {
        viewers.admit(&request(id)?).map_err(|e| format!("{e:?}"))?;
    }
    viewers.receive(blocked_record.clone(), epoch)?;
    assert_eq!(viewers.retained_record_bytes, blocked_record.len());
    viewers.receive(frame_record.clone(), epoch)?;
    assert_eq!(
        viewers.retained_record_bytes,
        blocked_record.len() + frame_record.len()
    );
    // The third record would cross the aggregate: only viewer 3 retires.
    viewers.receive(third, epoch)?;
    assert_eq!(viewers.closed_ids(), vec![[3; 16]]);
    assert_eq!(
        viewers.retained_record_bytes,
        blocked_record.len() + frame_record.len()
    );
    // Removal releases a retained record exactly once.
    viewers.remove(request(1)?.attachment_id);
    assert_eq!(viewers.retained_record_bytes, frame_record.len());
    viewers.remove(request(1)?.attachment_id);
    assert_eq!(viewers.retained_record_bytes, frame_record.len());
    assert_eq!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Record(frame_record)
    );
    assert_eq!(viewers.retained_record_bytes, 0);
    Ok(())
}

#[test]
fn malformed_or_flow_invalid_records_still_fail_under_pressure() -> TestResult {
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let first = frame(1, 1, epoch)?;
    let mut viewers = HostViewers::with_retained_record_limit(first.len());
    viewers.admit(&request(1)?).map_err(|e| format!("{e:?}"))?;
    viewers.admit(&request(2)?).map_err(|e| format!("{e:?}"))?;
    viewers.receive(first.clone(), epoch)?;
    // A malformed record still fails the connection rather than retiring quietly.
    let mut malformed = frame(2, 1, epoch)?;
    malformed[0] = b'X';
    assert!(viewers.receive(malformed, epoch).is_err());
    // A flow-invalid frame (delta for a snapshot-needing viewer) still fails.
    let mut delta = frame(2, 1, epoch)?;
    delta[20..22].copy_from_slice(&42_u16.to_le_bytes());
    delta[64 + 6] = 2;
    delta[64 + 56..64 + 64].copy_from_slice(&4_u64.to_le_bytes());
    assert!(viewers.receive(delta, epoch).is_err());
    // Neither failure used cache pressure as a bypass.
    assert_eq!(viewers.retained_record_bytes, first.len());
    assert!(viewers.closed_ids().is_empty());
    Ok(())
}

#[test]
fn retained_budget_boundary_maps_to_the_actual_128_mib_ceiling() -> TestResult {
    assert_eq!(MAXIMUM_RETAINED_VIEWER_RECORD_BYTES, 128 * 1024 * 1024);
    const MIB: usize = 1024 * 1024;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let mut viewers = HostViewers::default();
    for id in 1..=24_u8 {
        viewers.admit(&request(id)?).map_err(|e| format!("{e:?}"))?;
    }
    let mut retained = 0;
    for id in 1..=22_u8 {
        let record = frame_sized(id, 1, epoch, 5 * MIB)?;
        retained += record.len();
        viewers.receive(record, epoch)?;
    }
    assert_eq!(viewers.retained_record_bytes, retained);
    // Viewer 23 lands exactly on the real ceiling and is retained.
    let exact = MAXIMUM_RETAINED_VIEWER_RECORD_BYTES - retained;
    viewers.receive(frame_sized(23, 1, epoch, exact)?, epoch)?;
    assert_eq!(
        viewers.retained_record_bytes,
        MAXIMUM_RETAINED_VIEWER_RECORD_BYTES
    );
    // Viewer 24's smallest valid record crosses it: only viewer 24 retires.
    viewers.receive(frame(24, 1, epoch)?, epoch)?;
    assert_eq!(viewers.closed_ids(), vec![[24; 16]]);
    assert_eq!(
        viewers.retained_record_bytes,
        MAXIMUM_RETAINED_VIEWER_RECORD_BYTES
    );
    assert!(matches!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(_)
    ));
    assert_eq!(
        viewers.retained_record_bytes,
        MAXIMUM_RETAINED_VIEWER_RECORD_BYTES - 5 * MIB
    );
    Ok(())
}

#[test]
fn history_events_poll_during_a_pending_frame_flight_without_disturbing_it() -> TestResult {
    let mut viewers = HostViewers::default();
    viewers.admit(&request(1)?).map_err(|e| format!("{e:?}"))?;
    viewers.admit(&request(2)?).map_err(|e| format!("{e:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let snapshot = frame(1, 1, epoch)?;
    viewers.receive(snapshot.clone(), epoch)?;
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(snapshot)
    );
    // The frame flight is now pending feedback, yet the event still delivers.
    let event = history(1, 1, epoch)?;
    viewers.receive(event.clone(), epoch)?;
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(event)
    );
    // The pending flight is untouched: the exact feedback still settles it.
    viewers
        .feedback(&TerminalViewerRequest {
            action: TerminalViewerAction::FrameApplied {
                sequence: NonZeroU64::MIN,
                generation: NonZeroU64::MIN,
                revision: 5,
            },
            ..request(1)?
        })
        .map_err(|e| format!("{e:?}"))?;
    // The settled flight frees the record slot: the next frame delivers
    // normally and the sibling viewer is unaffected throughout.
    let next = frame(1, 2, epoch)?;
    viewers.receive(next.clone(), epoch)?;
    viewers.receive(frame(2, 1, epoch)?, epoch)?;
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(next)
    );
    assert!(matches!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Record(_)
    ));
    assert!(viewers.closed_ids().is_empty());
    Ok(())
}

#[test]
fn poll_arbitration_prefers_the_record_once_then_serves_the_waiting_fifo() -> TestResult {
    let mut viewers = HostViewers::default();
    viewers.admit(&request(1)?).map_err(|e| format!("{e:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let first = history(1, 1, epoch)?;
    let second = history(1, 2, epoch)?;
    viewers.receive(first.clone(), epoch)?;
    viewers.receive(second.clone(), epoch)?;
    // The record slot is preferred initially: a queued event waits while the
    // frame takes the first poll, then the FIFO drains in arrival order.
    let snapshot = frame(1, 1, epoch)?;
    viewers.receive(snapshot.clone(), epoch)?;
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(snapshot)
    );
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(first)
    );
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(second)
    );
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Empty
    );
    Ok(())
}

#[test]
fn a_full_event_fifo_closes_only_that_viewer_and_a_closed_viewer_discards() -> TestResult {
    let mut viewers = HostViewers::default();
    viewers.admit(&request(1)?).map_err(|e| format!("{e:?}"))?;
    viewers.admit(&request(2)?).map_err(|e| format!("{e:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    for sequence in 1..=MAXIMUM_VIEWER_EVENTS as u64 {
        viewers.receive(history(1, sequence, epoch)?, epoch)?;
    }
    // The next event saturates the bounded FIFO: only this viewer retires,
    // and the queued bytes leave with it.
    viewers.receive(history(1, 9, epoch)?, epoch)?;
    assert_eq!(viewers.closed_ids(), vec![[1; 16]]);
    assert_eq!(viewers.retained_record_bytes, 0);
    // A closed viewer discards late events without charging the budget.
    viewers.receive(history(1, 10, epoch)?, epoch)?;
    assert_eq!(viewers.retained_record_bytes, 0);
    let sibling = history(2, 1, epoch)?;
    viewers.receive(sibling.clone(), epoch)?;
    assert_eq!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Record(sibling)
    );
    // An event for an unknown attachment remains a connection failure.
    assert!(viewers.receive(history(9, 1, epoch)?, epoch).is_err());
    Ok(())
}

#[test]
fn history_events_share_the_byte_budget_and_remove_releases_exactly_once() -> TestResult {
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let frame_record = frame(1, 1, epoch)?;
    let first = history(1, 1, epoch)?;
    let second = history(1, 2, epoch)?;
    let limit = frame_record.len() + first.len();
    let mut viewers = HostViewers::with_retained_record_limit(limit);
    viewers.admit(&request(1)?).map_err(|e| format!("{e:?}"))?;
    viewers.admit(&request(2)?).map_err(|e| format!("{e:?}"))?;
    viewers.receive(frame_record.clone(), epoch)?;
    viewers.receive(first, epoch)?;
    assert_eq!(viewers.retained_record_bytes, limit);
    // Crossing the aggregate with an event retires only the addressed viewer
    // and releases both its frame slot and its queued events exactly once.
    viewers.receive(second, epoch)?;
    assert_eq!(viewers.closed_ids(), vec![[1; 16]]);
    assert_eq!(viewers.retained_record_bytes, 0);
    let event = history(2, 1, epoch)?;
    let event_bytes = event.len();
    viewers.receive(event.clone(), epoch)?;
    assert_eq!(viewers.retained_record_bytes, event_bytes);
    viewers.remove(request(2)?.attachment_id);
    assert_eq!(viewers.retained_record_bytes, 0);
    viewers.remove(request(2)?.attachment_id);
    assert_eq!(viewers.retained_record_bytes, 0);
    Ok(())
}

#[test]
fn pane_effect_streams_keep_independent_sequences_and_poll_only_from_the_exact_viewer() -> TestResult
{
    let mut viewers = HostViewers::default();
    viewers
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    viewers
        .admit(&request(2)?)
        .map_err(|error| format!("{error:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let bell = bell(1, 1, epoch)?;
    // Bell and pane-program clipboard writes are separate Host streams. The
    // same positive sequence is valid once in each stream and their records
    // still leave through one arrival-ordered viewer FIFO.
    let clipboard = clipboard(1, 1, "selected text", epoch)?;

    viewers.receive(bell.clone(), epoch)?;
    viewers.receive(clipboard.clone(), epoch)?;

    assert_eq!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Empty
    );
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(bell)
    );
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(clipboard)
    );
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Empty
    );
    Ok(())
}

#[test]
fn pane_effect_replays_regressions_and_unknown_attachments_fail_closed() -> TestResult {
    let epoch = HostEpoch::from_bytes([3; 16])?;

    let mut bells = HostViewers::default();
    bells
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    let first_bell = bell(1, 2, epoch)?;
    bells.receive(first_bell.clone(), epoch)?;
    assert!(bells.receive(bell(1, 2, epoch)?, epoch).is_err());
    assert!(bells.receive(bell(1, 1, epoch)?, epoch).is_err());
    assert_eq!(
        bells.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(first_bell)
    );
    assert_eq!(
        bells.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Empty
    );

    let mut clipboard_writes = HostViewers::default();
    clipboard_writes
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    let first_write = clipboard(1, 3, "new", epoch)?;
    clipboard_writes.receive(first_write.clone(), epoch)?;
    assert!(
        clipboard_writes
            .receive(clipboard(1, 2, "stale", epoch)?, epoch)
            .is_err()
    );
    assert_eq!(
        clipboard_writes.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Record(first_write)
    );
    assert_eq!(
        clipboard_writes.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Empty
    );

    let mut exact = HostViewers::default();
    exact
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    assert!(exact.receive(bell(9, 1, epoch)?, epoch).is_err());
    assert_eq!(
        exact.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Empty
    );
    Ok(())
}

#[test]
fn pane_effects_share_fifo_capacity_and_closed_viewers_discard_late_records() -> TestResult {
    let mut viewers = HostViewers::default();
    viewers
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    viewers
        .admit(&request(2)?)
        .map_err(|error| format!("{error:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let queued = [
        bell(1, 1, epoch)?,
        clipboard(1, 1, "first", epoch)?,
        history(1, 1, epoch)?,
        history(1, 2, epoch)?,
    ];
    let retained: usize = queued.iter().map(Vec::len).sum();
    for record in queued {
        viewers.receive(record, epoch)?;
    }
    assert_eq!(viewers.retained_record_bytes(), retained);

    // One more pane effect closes only the saturated viewer and releases all
    // four heterogeneous records exactly once.
    viewers.receive(bell(1, 2, epoch)?, epoch)?;
    assert_eq!(viewers.closed_ids(), vec![[1; 16]]);
    assert_eq!(viewers.retained_record_bytes(), 0);
    viewers.receive(clipboard(1, 2, "late", epoch)?, epoch)?;
    assert_eq!(viewers.retained_record_bytes(), 0);

    let sibling = clipboard(2, 1, "sibling", epoch)?;
    viewers.receive(sibling.clone(), epoch)?;
    assert_eq!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Record(sibling)
    );
    assert_eq!(viewers.retained_record_bytes(), 0);
    Ok(())
}

#[test]
fn pane_effects_share_the_aggregate_byte_budget_and_remove_releases_once() -> TestResult {
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let frame_record = frame(1, 1, epoch)?;
    let bell_record = bell(1, 1, epoch)?;
    let clipboard_record = clipboard(1, 1, "crosses budget", epoch)?;
    let limit = frame_record.len() + bell_record.len();
    let mut viewers = HostViewers::with_retained_record_limit(limit);
    viewers
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    viewers
        .admit(&request(2)?)
        .map_err(|error| format!("{error:?}"))?;
    viewers.receive(frame_record, epoch)?;
    viewers.receive(bell_record, epoch)?;
    assert_eq!(viewers.retained_record_bytes(), limit);

    viewers.receive(clipboard_record, epoch)?;
    assert_eq!(viewers.closed_ids(), vec![[1; 16]]);
    assert_eq!(viewers.retained_record_bytes(), 0);

    let sibling = bell(2, 1, epoch)?;
    let sibling_bytes = sibling.len();
    viewers.receive(sibling, epoch)?;
    assert_eq!(viewers.retained_record_bytes(), sibling_bytes);
    viewers.remove(request(2)?.attachment_id);
    assert_eq!(viewers.retained_record_bytes(), 0);
    viewers.remove(request(2)?.attachment_id);
    assert_eq!(viewers.retained_record_bytes(), 0);
    Ok(())
}

#[test]
fn selection_events_keep_opaque_bytes_and_arrival_order_for_the_exact_viewer() -> TestResult {
    let mut viewers = HostViewers::default();
    viewers
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    viewers
        .admit(&request(2)?)
        .map_err(|error| format!("{error:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let records = [
        selection_changed(1, epoch)?,
        selection_export_chunk(1, 7, 0, &[0x00, 0xff, 0x80, 0x61], epoch)?,
        selection_export_finished(1, 7, 4, epoch)?,
    ];
    let retained: usize = records.iter().map(Vec::len).sum();

    for record in &records {
        viewers.receive(record.clone(), epoch)?;
    }

    assert_eq!(viewers.retained_record_bytes(), retained);
    assert_eq!(
        viewers.poll(request(2)?.attachment_id),
        TerminalViewerOutcome::Empty
    );
    for record in records {
        assert_eq!(
            viewers.poll(request(1)?.attachment_id),
            TerminalViewerOutcome::Record(record)
        );
    }
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Empty
    );
    assert_eq!(viewers.retained_record_bytes(), 0);
    Ok(())
}

#[test]
fn selection_events_reject_unknown_attachments_without_touching_a_live_viewer() -> TestResult {
    let mut viewers = HostViewers::default();
    viewers
        .admit(&request(1)?)
        .map_err(|error| format!("{error:?}"))?;
    let epoch = HostEpoch::from_bytes([3; 16])?;
    let foreign = [
        selection_changed(9, epoch)?,
        selection_export_chunk(9, 1, 0, b"foreign", epoch)?,
        selection_export_finished(9, 1, 7, epoch)?,
    ];

    for record in foreign {
        assert!(viewers.receive(record, epoch).is_err());
    }
    assert_eq!(
        viewers.poll(request(1)?.attachment_id),
        TerminalViewerOutcome::Empty
    );
    assert_eq!(viewers.retained_record_bytes(), 0);
    Ok(())
}

fn history(id: u8, sequence: u64, epoch: HostEpoch) -> Result<Vec<u8>, Box<dyn Error>> {
    let control = encode_history_control(&HistoryControl::HistoryChanged(HistoryChanged {
        anchor_status: HistoryAnchorStatus::Valid,
        attachment_id: AttachmentId::from_bytes([id; 16])?,
        generation: NonZeroU64::MIN,
        revision: 5,
        sequence: NonZeroU64::new(sequence).ok_or("zero history sequence")?,
        total_rows: 100,
    }))?;
    envelope(
        control.kind,
        PayloadEncoding::ControlJson,
        &control.payload,
        epoch,
    )
}

fn effect(event: TerminalEffectEvent, epoch: HostEpoch) -> Result<Vec<u8>, Box<dyn Error>> {
    let encoded = encode_terminal_effect_event(&event)?;
    envelope(
        encoded.kind,
        PayloadEncoding::ControlJson,
        &encoded.payload,
        epoch,
    )
}

fn bell(id: u8, sequence: u64, epoch: HostEpoch) -> Result<Vec<u8>, Box<dyn Error>> {
    effect(
        TerminalEffectEvent::Bell(TerminalBell {
            attachment_id: AttachmentId::from_bytes([id; 16])?,
            sequence: NonZeroU64::new(sequence).ok_or("zero bell sequence")?,
        }),
        epoch,
    )
}

fn clipboard(
    id: u8,
    sequence: u64,
    text: &str,
    epoch: HostEpoch,
) -> Result<Vec<u8>, Box<dyn Error>> {
    effect(
        TerminalEffectEvent::ClipboardWriteRequest(TerminalClipboardWriteRequest {
            attachment_id: AttachmentId::from_bytes([id; 16])?,
            sequence: NonZeroU64::new(sequence).ok_or("zero clipboard sequence")?,
            text: text.to_owned(),
        }),
        epoch,
    )
}

fn selection_changed(id: u8, epoch: HostEpoch) -> Result<Vec<u8>, Box<dyn Error>> {
    let encoded = encode_selection_control(&SelectionControl::ChangedEvent(SelectionChanged {
        attachment_id: AttachmentId::from_bytes([id; 16])?,
        state: SelectionChangedState::None,
    }))?;
    envelope(
        encoded.kind,
        PayloadEncoding::ControlJson,
        &encoded.payload,
        epoch,
    )
}

fn selection_export_chunk(
    id: u8,
    export_sequence: u64,
    chunk_sequence: u32,
    bytes: &[u8],
    epoch: HostEpoch,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let encoded = encode_selection_export_chunk(&SelectionExportChunk {
        attachment_id: AttachmentId::from_bytes([id; 16])?,
        bytes: bytes.to_vec(),
        chunk_sequence,
        export_sequence: NonZeroU64::new(export_sequence).ok_or("zero export sequence")?,
        final_chunk: true,
    })?;
    envelope(
        encoded.kind,
        PayloadEncoding::RawBytes,
        &encoded.payload,
        epoch,
    )
}

fn selection_export_finished(
    id: u8,
    export_sequence: u64,
    byte_length: u32,
    epoch: HostEpoch,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let encoded = encode_selection_export_control(&SelectionExportControl::FinishedEvent(
        SelectionExportFinished {
            attachment_id: AttachmentId::from_bytes([id; 16])?,
            byte_length,
            export_sequence: NonZeroU64::new(export_sequence).ok_or("zero export sequence")?,
            status: SelectionExportStatus::Completed,
        },
    ))?;
    envelope(
        encoded.kind,
        PayloadEncoding::ControlJson,
        &encoded.payload,
        epoch,
    )
}

fn frame_sized(
    id: u8,
    sequence: u64,
    epoch: HostEpoch,
    record_bytes: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut payload = include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    )
    .to_vec();
    let probe = envelope(41, PayloadEncoding::TerminalFrame, &payload, epoch)?;
    let payload_bytes = record_bytes - (probe.len() - payload.len());
    // Grow through a real optional extension record: the canonical full
    // decoder validates the whole frame and skips extension contents.
    let extension_bytes = payload_bytes - payload.len() - 8;
    let extensions = u16::from_le_bytes(payload[132..134].try_into()?) + 1;
    payload[132..134].copy_from_slice(&extensions.to_le_bytes());
    payload.extend_from_slice(&0_u16.to_le_bytes());
    payload.extend_from_slice(&0_u16.to_le_bytes());
    payload.extend_from_slice(&(extension_bytes as u32).to_le_bytes());
    payload.resize(payload_bytes, 0);
    payload[6] = 1;
    let payload_len = payload.len() as u32;
    payload[8..12].copy_from_slice(&payload_len.to_le_bytes());
    payload[56..64].fill(255);
    payload[16..32].copy_from_slice(epoch.as_bytes());
    payload[32..48].fill(id);
    payload[48..56].copy_from_slice(&sequence.to_le_bytes());
    payload[64..72].copy_from_slice(&5_u64.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    decode_terminal_text_frame(&payload)?;
    let record = envelope(41, PayloadEncoding::TerminalFrame, &payload, epoch)?;
    assert_eq!(record.len(), record_bytes);
    Ok(record)
}

fn request(id: u8) -> Result<TerminalViewerRequest, Box<dyn Error>> {
    Ok(TerminalViewerRequest {
        request_id: OperationId::from_bytes([20 + id; 16])?,
        attachment_id: OperationId::from_bytes([id; 16])?,
        worker_id: WorkerId::from_bytes([1; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Attach {
            host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes([3; 16])?,
            terminal_id: HostTerminalId::from_bytes([4; 16])?,
        },
    })
}

fn frame(id: u8, sequence: u64, epoch: HostEpoch) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut payload = include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    )
    .to_vec();
    payload[6] = 1;
    payload[56..64].fill(255);
    payload[16..32].copy_from_slice(epoch.as_bytes());
    payload[32..48].fill(id);
    payload[48..56].copy_from_slice(&sequence.to_le_bytes());
    payload[64..72].copy_from_slice(&5_u64.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    envelope(41, PayloadEncoding::TerminalFrame, &payload, epoch)
}
fn envelope(
    kind: u16,
    payload_encoding: PayloadEncoding,
    payload: &[u8],
    epoch: HostEpoch,
) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(epoch),
        kind,
        optional_message: false,
        payload,
        payload_encoding,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })?)
}

#[test]
fn carrier_record_limit_matches_the_host_frame_contract() {
    assert_eq!(
        orchbee_capability_node_protocol::MAXIMUM_VIEWER_RECORD_BYTES,
        orchbee_terminal_protocol::TRANSPORT_ENVELOPE_HEADER_BYTES
            + orchbee_terminal_protocol::MAXIMUM_TERMINAL_FRAME_BYTES
    );
}

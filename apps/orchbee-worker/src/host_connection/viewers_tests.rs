use super::*;
use orchbee_capability_node_protocol::{HostTerminalId, WorkerGeneration, WorkerId};
use orchbee_terminal_protocol::{
    Envelope, ProjectionBlockReason, ProjectionBlocked, decode_terminal_text_frame,
    encode_projection_demand_control, encode_transport_envelope,
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

fn frame_sized(
    id: u8,
    sequence: u64,
    epoch: HostEpoch,
    record_bytes: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut payload =
        include_bytes!("../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin")
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
    let mut payload =
        include_bytes!("../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin")
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

use super::*;
use orchbee_capability_node_protocol::{HostTerminalId, WorkerGeneration, WorkerId};
use orchbee_terminal_protocol::{
    Envelope, ProjectionBlockReason, ProjectionBlocked, encode_projection_demand_control,
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
    for id in 1..=8 {
        viewers.admit(&request(id)?).map_err(|e| format!("{e:?}"))?;
    }
    assert_eq!(
        viewers.admit(&request(9)?),
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
    viewers.admit(&request(9)?).map_err(|e| format!("{e:?}"))?;
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

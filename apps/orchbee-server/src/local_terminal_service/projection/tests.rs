use super::*;
use orchbee_terminal_protocol::{
    Envelope, ProjectionBlockReason, ProjectionBlocked, encode_projection_demand_control,
    encode_transport_envelope,
};
use std::{error::Error, num::NonZeroU64};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn records_and_feedback_cannot_cross_attachment_or_host_scope() -> TestResult {
    let mut projection = projection()?;
    let good = frame(1, 5)?;
    for range in [40..56, 64 + 16..64 + 32, 64 + 32..64 + 48] {
        let mut wrong = good.clone();
        wrong[range].fill(9);
        assert!(projection.receive(&wrong).is_err());
    }
    projection
        .receive(&good)
        .map_err(|_| "valid frame rejected")?;
    let feedback = TerminalViewerAction::FrameApplied {
        attachment_id: orchbee_client_server_protocol::ViewerId::parse(
            &projection.attachment_id.to_text(),
        )?,
        sequence: NonZeroU64::MIN,
        generation: NonZeroU64::MIN,
        revision: 5,
    };
    assert!(projection.accepts_feedback(&feedback));
    assert!(
        !projection.accepts_feedback(&TerminalViewerAction::FrameApplied {
            attachment_id: orchbee_client_server_protocol::ViewerId::parse(
                &projection.attachment_id.to_text()
            )?,
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 6
        })
    );
    assert!(
        !projection.accepts_feedback(&TerminalViewerAction::NeedSnapshot {
            attachment_id: orchbee_client_server_protocol::ViewerId::parse(
                &projection.attachment_id.to_text()
            )?,
            sequence: NonZeroU64::new(2).ok_or("zero sequence")?,
            reason: orchbee_client_server_protocol::ViewerSnapshotReason::StoreReset
        })
    );
    assert!(projection.receive(&frame(2, 6)?).is_err());
    projection.complete_feedback(&feedback);
    assert!(!projection.accepts_feedback(&feedback));
    assert!(projection.receive(&good).is_err());
    projection
        .receive(&frame(2, 6)?)
        .map_err(|_| "next frame rejected")?;
    Ok(())
}

#[test]
fn blocked_and_requested_snapshot_recovery_require_the_correct_snapshot() -> TestResult {
    let mut projection = projection()?;
    let blocked = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id: projection.attachment_id,
            reason: ProjectionBlockReason::ProjectionTooLarge,
            revision: 4,
        },
    ))?;
    let bytes = envelope(blocked.kind, PayloadEncoding::ControlJson, &blocked.payload)?;
    projection
        .receive(&bytes)
        .map_err(|_| "blocked record rejected")?;
    assert!(projection.flight.is_none());
    assert!(projection.receive(&frame(1, 4)?).is_err());
    projection
        .receive(&frame(1, 5)?)
        .map_err(|_| "recovery rejected")?;
    let feedback = TerminalViewerAction::NeedSnapshot {
        attachment_id: orchbee_client_server_protocol::ViewerId::parse(
            &projection.attachment_id.to_text(),
        )?,
        sequence: NonZeroU64::MIN,
        reason: orchbee_client_server_protocol::ViewerSnapshotReason::DecoderRejected,
    };
    assert!(projection.accepts_feedback(&feedback));
    projection.complete_feedback(&feedback);
    let mut delta = frame(2, 6)?;
    delta[20..22].copy_from_slice(&42_u16.to_le_bytes());
    delta[64 + 6] = 2;
    delta[64 + 56..64 + 64].copy_from_slice(&5_u64.to_le_bytes());
    assert!(projection.receive(&delta).is_err());
    projection
        .receive(&frame(2, 6)?)
        .map_err(|_| "requested snapshot rejected")?;
    Ok(())
}

fn projection() -> Result<ViewerProjection, Box<dyn Error>> {
    Ok(ViewerProjection::new(
        AttachmentId::from_bytes([2; 16])?,
        HostEpoch::from_bytes([1; 16])?,
    ))
}
fn frame(sequence: u64, revision: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut payload =
        include_bytes!("../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin").to_vec();
    payload[6] = 1;
    payload[16..32].fill(1);
    payload[32..48].fill(2);
    payload[48..56].copy_from_slice(&sequence.to_le_bytes());
    payload[56..64].fill(255);
    payload[64..72].copy_from_slice(&revision.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    envelope(41, PayloadEncoding::TerminalFrame, &payload)
}
fn envelope(
    kind: u16,
    payload_encoding: PayloadEncoding,
    payload: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::from_bytes([1; 16])?),
        kind,
        optional_message: false,
        payload,
        payload_encoding,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })?)
}

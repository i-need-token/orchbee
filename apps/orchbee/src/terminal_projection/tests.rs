use super::*;
use orchbee_terminal_protocol::{
    Envelope, ProjectionBlockReason, ProjectionBlocked, encode_projection_demand_control,
    encode_transport_envelope,
};
const SNAPSHOT: &[u8] =
    include_bytes!("../../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin");
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn record(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: if bytes[6] == 1 { 41 } else { 42 },
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(inspect_terminal_frame_route(bytes)?.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: bytes,
    })?)
}

fn geometry() -> Result<Geometry, Box<dyn std::error::Error>> {
    let geometry = decode_terminal_text_frame(SNAPSHOT)?.geometry;
    Ok(Geometry::new(
        geometry.columns,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    )?)
}

fn snapshot_at(sequence: u64, revision: u64) -> Vec<u8> {
    let mut snapshot = SNAPSHOT.to_vec();
    snapshot[48..56].copy_from_slice(&sequence.to_le_bytes());
    snapshot[64..72].copy_from_slice(&revision.to_le_bytes());
    snapshot
}

fn blocked_record(
    attachment_id: AttachmentId,
    epoch: HostEpoch,
    revision: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let blocked = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id,
            reason: ProjectionBlockReason::ProjectionTooLarge,
            revision,
        },
    ))?;
    Ok(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: blocked.role,
        kind: blocked.kind,
        payload_encoding: PayloadEncoding::ControlJson,
        host_epoch: Some(epoch),
        correlation_id: None,
        optional_message: false,
        payload: &blocked.payload,
    })?)
}

#[test]
fn required_snapshot_geometry_checks_all_four_fields() -> TestResult {
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let expected = geometry()?;
    let mismatches = [
        Geometry::new(
            expected.columns().checked_add(1).ok_or("fixture columns")?,
            expected.rows(),
            expected.cell_pixel_width(),
            expected.cell_pixel_height(),
        )?,
        Geometry::new(
            expected.columns(),
            expected.rows().checked_add(1).ok_or("fixture rows")?,
            expected.cell_pixel_width(),
            expected.cell_pixel_height(),
        )?,
        Geometry::new(
            expected.columns(),
            expected.rows(),
            expected
                .cell_pixel_width()
                .checked_add(1)
                .ok_or("fixture cell pixel width")?,
            expected.cell_pixel_height(),
        )?,
        Geometry::new(
            expected.columns(),
            expected.rows(),
            expected.cell_pixel_width(),
            expected
                .cell_pixel_height()
                .checked_add(1)
                .ok_or("fixture cell pixel height")?,
        )?,
    ];
    for mismatch in mismatches {
        let mut projection = TerminalProjection::new(route.attachment_id, route.host_epoch);
        assert!(matches!(
            projection.receive_for_geometry(&record(SNAPSHOT)?, Some(mismatch)),
            Ok(Some(Feedback::NeedSnapshot {
                sequence,
                reason: Reason::DimensionsMismatch,
            })) if sequence == route.sequence
        ));
        assert!(projection.frame().is_none());
    }

    let mut controlling = TerminalProjection::new(route.attachment_id, route.host_epoch);
    assert!(matches!(
        controlling.receive_for_geometry(&record(SNAPSHOT)?, Some(expected)),
        Ok(Some(Feedback::FrameApplied { .. }))
    ));
    let mut read_only = TerminalProjection::new(route.attachment_id, route.host_epoch);
    assert!(matches!(
        read_only.receive_for_geometry(&record(SNAPSHOT)?, None),
        Ok(Some(Feedback::FrameApplied { .. }))
    ));
    Ok(())
}

#[test]
fn mismatch_and_clear_preserve_monotonic_fences_and_allow_fresh_recovery() -> TestResult {
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let expected = geometry()?;
    let mismatched = Geometry::new(
        expected.columns().checked_add(1).ok_or("fixture columns")?,
        expected.rows(),
        expected.cell_pixel_width(),
        expected.cell_pixel_height(),
    )?;
    let next_sequence = route
        .sequence
        .get()
        .checked_add(1)
        .ok_or("fixture sequence")?;
    let recovery_sequence = next_sequence.checked_add(1).ok_or("fixture sequence")?;
    let stale_sequence = recovery_sequence.checked_add(1).ok_or("fixture sequence")?;
    let next_revision = route.revision.checked_add(1).ok_or("fixture revision")?;
    let recovery_revision = next_revision.checked_add(1).ok_or("fixture revision")?;
    let blocked_revision = recovery_revision.checked_add(1).ok_or("fixture revision")?;
    let mut projection = TerminalProjection::new(route.attachment_id, route.host_epoch);

    projection
        .receive_for_geometry(&record(SNAPSHOT)?, Some(expected))
        .map_err(|_| "initial snapshot")?;
    let mismatch = snapshot_at(next_sequence, next_revision);
    assert!(matches!(
        projection.receive_for_geometry(&record(&mismatch)?, Some(mismatched)),
        Ok(Some(Feedback::NeedSnapshot {
            sequence,
            reason: Reason::DimensionsMismatch,
        })) if sequence.get() == next_sequence
    ));
    assert!(projection.frame().is_none());
    assert_eq!(projection.last_sequence, next_sequence);
    assert_eq!(projection.last_revision, route.revision);
    assert!(
        projection
            .receive_for_geometry(&record(&mismatch)?, Some(expected))
            .is_err()
    );

    let recovery = snapshot_at(recovery_sequence, recovery_revision);
    assert!(matches!(
        projection.receive_for_geometry(&record(&recovery)?, Some(expected)),
        Ok(Some(Feedback::FrameApplied { .. }))
    ));
    projection.clear_frame();
    assert!(projection.frame().is_none());
    assert_eq!(projection.last_sequence, recovery_sequence);
    assert_eq!(projection.last_revision, recovery_revision);
    assert!(
        projection
            .receive_for_geometry(&record(&recovery)?, Some(expected))
            .is_err()
    );

    let stale = snapshot_at(stale_sequence, next_revision);
    assert!(matches!(
        projection.receive_for_geometry(&record(&stale)?, Some(expected)),
        Ok(Some(Feedback::NeedSnapshot {
            reason: Reason::BaseMismatch,
            ..
        }))
    ));
    let blocked = blocked_record(route.attachment_id, route.host_epoch, blocked_revision)?;
    assert_eq!(projection.receive(&blocked), Ok(None));
    projection.clear_frame();
    assert_eq!(projection.blocked_revision, Some(blocked_revision));
    assert!(projection.receive(&blocked).is_err());
    Ok(())
}
#[test]
fn snapshot_delta_gap_and_decoder_failure_have_atomic_feedback() -> TestResult {
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let mut projection = TerminalProjection::new(route.attachment_id, route.host_epoch);
    assert!(matches!(
        projection.receive(&record(SNAPSHOT)?),
        Ok(Some(Feedback::FrameApplied { .. }))
    ));
    assert!(projection.frame().is_some());
    let mut delta = SNAPSHOT.to_vec();
    delta[6] = 2;
    delta[48..56].copy_from_slice(&(route.sequence.get() + 1).to_le_bytes());
    delta[56..64].copy_from_slice(&route.revision.to_le_bytes());
    delta[64..72].copy_from_slice(&(route.revision + 1).to_le_bytes());
    assert!(matches!(
        projection.receive(&record(&delta)?),
        Ok(Some(Feedback::FrameApplied { .. }))
    ));
    let revision = projection.frame().ok_or("frame")?.route.revision;
    delta[48..56].copy_from_slice(&(route.sequence.get() + 3).to_le_bytes());
    delta[56..64].copy_from_slice(&revision.to_le_bytes());
    delta[64..72].copy_from_slice(&(revision + 1).to_le_bytes());
    assert!(matches!(
        projection.receive(&record(&delta)?),
        Ok(Some(Feedback::NeedSnapshot {
            reason: Reason::SequenceGap,
            ..
        }))
    ));
    assert!(projection.frame().is_none());
    let mut snapshot = SNAPSHOT.to_vec();
    snapshot[48..56].copy_from_slice(&(route.sequence.get() + 4).to_le_bytes());
    snapshot[64..72].copy_from_slice(&revision.to_le_bytes());
    assert!(matches!(
        projection.receive(&record(&snapshot)?),
        Ok(Some(Feedback::FrameApplied { .. }))
    ));
    snapshot[48..56].copy_from_slice(&(route.sequence.get() + 5).to_le_bytes());
    snapshot[153] = 1;
    assert!(matches!(
        projection.receive(&record(&snapshot)?),
        Ok(Some(Feedback::NeedSnapshot {
            reason: Reason::DecoderRejected,
            ..
        }))
    ));
    assert!(projection.frame().is_none());
    assert!(projection.receive(&record(&snapshot)?).is_err());
    Ok(())
}
#[test]
fn scope_and_delta_generation_mismatch_never_commit() -> TestResult {
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let mut projection = TerminalProjection::new(route.attachment_id, route.host_epoch);
    let mut wrong = SNAPSHOT.to_vec();
    wrong[32] ^= 1;
    assert!(projection.receive(&record(&wrong)?).is_err());
    assert!(projection.frame().is_none());
    projection
        .receive(&record(SNAPSHOT)?)
        .map_err(|_| "snapshot")?;
    let mut next = decode_terminal_text_frame(SNAPSHOT)?;
    next.route.base_revision = Some(route.revision);
    next.route.generation =
        std::num::NonZeroU64::new(route.generation.get() + 1).ok_or("generation")?;
    assert_eq!(projection.apply(next), Err(Reason::GenerationMismatch));
    assert_eq!(projection.frame().ok_or("frame")?.route, route);
    Ok(())
}

#[test]
fn snapshot_cannot_move_an_existing_projection_backwards() -> TestResult {
    let route = inspect_terminal_frame_route(SNAPSHOT)?;
    let mut projection = TerminalProjection::new(route.attachment_id, route.host_epoch);
    let mut first = decode_terminal_text_frame(SNAPSHOT)?;
    first.route.revision = 12;
    projection.apply(first).map_err(|_| "initial snapshot")?;
    let mut stale = decode_terminal_text_frame(SNAPSHOT)?;
    stale.route.revision = 11;
    assert_eq!(projection.apply(stale), Err(Reason::BaseMismatch));
    let saved = projection.frame.take();
    let mut stale_after_reset = decode_terminal_text_frame(SNAPSHOT)?;
    stale_after_reset.route.revision = 11;
    assert_eq!(
        projection.apply(stale_after_reset),
        Err(Reason::BaseMismatch)
    );
    projection.frame = saved;
    assert_eq!(projection.frame().ok_or("frame")?.route.revision, 12);
    Ok(())
}

use super::*;

#[test]
fn equal_frame_counters_produce_feedback_for_each_exact_attachment() -> TestResult {
    let first_route = inspect_terminal_frame_route(SNAPSHOT)?;
    let mut second_bytes = SNAPSHOT.to_vec();
    second_bytes[32] ^= 1;
    let second_route = inspect_terminal_frame_route(&second_bytes)?;
    assert_ne!(first_route.attachment_id, second_route.attachment_id);
    assert_eq!(first_route.sequence, second_route.sequence);
    assert_eq!(first_route.generation, second_route.generation);
    assert_eq!(first_route.revision, second_route.revision);

    let mut first = TerminalProjection::new(first_route.attachment_id, first_route.host_epoch);
    let mut second = TerminalProjection::new(second_route.attachment_id, second_route.host_epoch);
    assert!(first.receive(&record(&second_bytes)?).is_err());
    assert!(second.receive(&record(SNAPSHOT)?).is_err());
    assert!(first.frame().is_none());
    assert!(second.frame().is_none());

    for (projection, bytes, route) in [
        (&mut first, SNAPSHOT, first_route),
        (&mut second, second_bytes.as_slice(), second_route),
    ] {
        assert_eq!(
            projection.receive(&record(bytes)?),
            Ok(Some(Feedback::FrameApplied {
                attachment_id: viewer_id(route),
                sequence: route.sequence,
                generation: route.generation,
                revision: route.revision,
            })),
        );
    }

    let mut later_second = snapshot_at(
        second_route
            .sequence
            .get()
            .checked_add(1)
            .ok_or("sequence")?,
        second_route.revision.checked_add(1).ok_or("revision")?,
    );
    later_second[32] ^= 1;
    let later_route = inspect_terminal_frame_route(&later_second)?;
    let expected = geometry()?;
    let mismatched = Geometry::new(
        expected.columns().checked_add(1).ok_or("columns")?,
        expected.rows(),
        expected.cell_pixel_width(),
        expected.cell_pixel_height(),
    )?;
    assert_eq!(
        second.receive_for_geometry(&record(&later_second)?, Some(mismatched)),
        Ok(Some(Feedback::NeedSnapshot {
            attachment_id: viewer_id(second_route),
            sequence: later_route.sequence,
            reason: Reason::DimensionsMismatch,
        })),
    );
    assert!(second.frame().is_none());
    assert_eq!(first.frame().ok_or("first frame")?.route, first_route);
    Ok(())
}

use super::*;
use orchbee_workbench_protocol::WorkbenchTabInsertIndex;

fn indexed(
    authority: &WorkbenchAuthority,
    request: u128,
    source: usize,
    boundary: usize,
) -> WorkbenchCommand {
    let mut request = command(authority, request, source, WorkbenchTabMoveDirection::First);
    let WorkbenchAction::TabMove {
        terminal_context_id,
        tab_id,
        ..
    } = request.action
    else {
        unreachable!()
    };
    request.action = WorkbenchAction::TabMoveIndex {
        terminal_context_id,
        tab_id,
        insert_index: WorkbenchTabInsertIndex::new(boundary).unwrap(),
    };
    request
}

#[test]
fn every_insertion_boundary_matches_an_independent_remove_insert_oracle() {
    for count in 1..=8 {
        for source in 0..count {
            for boundary in 0..=count + 1 {
                let (mut authority, grant) = authority(count);
                authority.last_identity = u128::MAX;
                let before = authority.projection.snapshot().clone();
                let request = indexed(&authority, 10, source, boundary);
                let result = submit(&mut authority, &grant, request.clone());
                let mut expected = before.clone();
                if boundary > count {
                    assert!(matches!(
                        result.outcome,
                        WorkbenchCommandOutcome::Rejected {
                            reason: WorkbenchRejectionReason::TabMoveIndexOutOfBounds,
                            ..
                        }
                    ));
                } else {
                    // Construct around the original boundary rather than using the
                    // production destination arithmetic or rotate operation.
                    let original = &before.terminal_contexts[0].tabs;
                    let mut ordered = Vec::new();
                    for gap in 0..=count {
                        if gap == boundary {
                            ordered.push(original[source].clone());
                        }
                        if gap < count && gap != source {
                            ordered.push(original[gap].clone());
                        }
                    }
                    let changed = ordered != *original;
                    expected.terminal_contexts[0].tabs = ordered;
                    expected.revision =
                        WorkbenchRevision::new(before.revision.value() + u64::from(changed));
                    assert_eq!(
                        matches!(result.outcome, WorkbenchCommandOutcome::TabMoved { .. }),
                        changed
                    );
                    assert_eq!(
                        matches!(result.outcome, WorkbenchCommandOutcome::Unchanged { .. }),
                        !changed
                    );
                }
                assert_eq!(
                    authority.projection.snapshot(),
                    &expected,
                    "{count}/{source}/{boundary}"
                );
                assert_eq!(authority.last_identity, u128::MAX);
                assert_eq!(submit(&mut authority, &grant, request), result);
                assert_eq!(authority.ledger.len(), 1);
            }
        }
    }
}

#[test]
fn maximum_boundary_append_and_historical_replay_preserve_identity() {
    let (mut authority, grant) = authority(256);
    let request = indexed(&authority, 10, 0, 256);
    let before = authority.projection.snapshot().clone();
    let result = submit(&mut authority, &grant, request.clone());
    let mut expected = before;
    let first = expected.terminal_contexts[0].tabs.remove(0);
    expected.terminal_contexts[0].tabs.push(first);
    expected.revision = WorkbenchRevision::new(257);
    assert_eq!(authority.projection.snapshot(), &expected);
    let later = indexed(&authority, 11, 1, 256);
    assert!(matches!(
        submit(&mut authority, &grant, later).outcome,
        WorkbenchCommandOutcome::TabMoved { .. }
    ));
    let committed = authority.projection.snapshot().clone();
    assert_eq!(submit(&mut authority, &grant, request.clone()), result);
    let mut conflict = request.clone();
    let WorkbenchAction::TabMoveIndex { insert_index, .. } = &mut conflict.action else {
        unreachable!()
    };
    *insert_index = WorkbenchTabInsertIndex::new(255).unwrap();
    assert!(matches!(
        submit(&mut authority, &grant, conflict).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    let mut stale = request;
    stale.request_id = WorkbenchRequestId::parse(&identity(12)).unwrap();
    assert!(matches!(
        submit(&mut authority, &grant, stale).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &committed);
}

#[test]
fn bounds_target_noop_revision_and_ledger_precedence_are_atomic() {
    for (source, boundary, expected) in [
        (9, 3, Some(WorkbenchRejectionReason::TabUnavailable)),
        (
            0,
            3,
            Some(WorkbenchRejectionReason::TabMoveIndexOutOfBounds),
        ),
        (0, 0, None),
        (0, 1, None),
        (0, 2, Some(WorkbenchRejectionReason::RevisionExhausted)),
    ] {
        let (mut authority, grant) = authority(2);
        let mut snapshot = authority.projection.snapshot().clone();
        snapshot.revision = WorkbenchRevision::new(u64::MAX);
        authority.projection = WorkbenchProjectionStore::load(
            &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
        )
        .unwrap();
        let request = indexed(&authority, 10, source, boundary);
        authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
        assert_eq!(
            submit(&mut authority, &grant, request.clone()).outcome,
            unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
        );
        assert!(authority.ledger.is_empty());
        assert_eq!(authority.projection.snapshot(), &snapshot);
        authority.ledger_bytes = 0;
        let result = submit(&mut authority, &grant, request.clone());
        match expected {
            Some(expected) => assert!(
                matches!(result.outcome, WorkbenchCommandOutcome::Rejected { reason, revision } if reason == expected && revision == snapshot.revision)
            ),
            None => assert_eq!(
                result.outcome,
                WorkbenchCommandOutcome::Unchanged {
                    revision: snapshot.revision
                }
            ),
        }
        assert_eq!(submit(&mut authority, &grant, request), result);
        assert_eq!(authority.projection.snapshot(), &snapshot);
    }
}

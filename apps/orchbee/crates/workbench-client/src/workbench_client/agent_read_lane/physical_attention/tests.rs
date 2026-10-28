use super::*;
#[test]
fn physical_batches_require_a_complete_revision_and_reject_duplicate_membership() {
    let row = TerminalAgentStateRow {
        presentation_id: TerminalPresentationId::parse(&"1".repeat(32)).unwrap(),
        snapshot: None,
    };
    let mut round = RoundBuilder::default();
    assert!(RoundBuilder::default().finish().is_err());
    round
        .accept(WorkbenchRevision::new(1), vec![row.clone()])
        .unwrap();
    assert_eq!(
        round.accept(WorkbenchRevision::new(2), vec![]),
        Err(LaneFailure::Unavailable)
    );
    assert_eq!(
        round.accept(WorkbenchRevision::new(1), vec![row]),
        Err(LaneFailure::Transport)
    );
    let mut empty = RoundBuilder::default();
    empty.accept(WorkbenchRevision::new(0), vec![]).unwrap();
    assert!(empty.finish().unwrap().states.is_empty());
}

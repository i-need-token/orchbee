use super::*;
#[test]
fn closed_projection_requires_exact_scope_minimum_revision_and_target_absence() {
    let current = projected(2, 10);
    let expected = ClosedTabExpectation::new(
        current.session_id().clone(),
        current.terminal_context_id().unwrap().clone(),
        current.tabs()[0].tab_id().clone(),
        WorkbenchRevision::new(12),
    );
    let mut absent = current.clone();
    absent.revision = WorkbenchRevision::new(12);
    absent.tabs.remove(0);
    assert_eq!(
        validate_closed_tab(&current, &expected, absent.clone()),
        Ok(absent.clone())
    );
    absent.revision = WorkbenchRevision::new(13);
    absent.tabs.clear();
    assert_eq!(
        validate_closed_tab(&current, &expected, absent.clone()),
        Ok(absent.clone())
    );
    for revision in [9, 10, 11] {
        absent.revision = WorkbenchRevision::new(revision);
        assert_eq!(
            validate_closed_tab(&current, &expected, absent.clone()),
            Err(ClientProjectionDivergence)
        );
    }
    let present = projected(2, 12);
    assert_eq!(
        validate_closed_tab(&current, &expected, present),
        Err(ClientProjectionDivergence)
    );
    absent.revision = WorkbenchRevision::new(12);
    let wrong_session = ClosedTabExpectation::new(
        session_id(91),
        expected.terminal_context_id().clone(),
        expected.tab_id().clone(),
        expected.minimum_revision(),
    );
    assert_eq!(
        validate_closed_tab(&current, &wrong_session, absent.clone()),
        Err(ClientProjectionDivergence)
    );
    let wrong_context = ClosedTabExpectation::new(
        expected.session_id().clone(),
        context_id(92),
        expected.tab_id().clone(),
        expected.minimum_revision(),
    );
    assert_eq!(
        validate_closed_tab(&current, &wrong_context, absent),
        Err(ClientProjectionDivergence)
    );
}

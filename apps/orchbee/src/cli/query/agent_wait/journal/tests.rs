use super::*;

fn cursor() -> AgentChangesCursor {
    AgentChangesCursor::parse(&format!("v1.{}.{}", "A".repeat(59), "A".repeat(43))).unwrap()
}

fn change(revision: u64, id: &str, kind: &str) -> AgentChange {
    let mut status = json!({"kind":kind});
    if kind == "working" {
        status = json!({"kind":"working", "activity":null, "phase":"working"});
    }
    serde_json::from_value(json!({"revision":revision,"transition":"session-idle-observed","session":{
        "lifecycle":"live","observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":revision,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,"providerId":"test","sessionId":id,"status":status
    }})).unwrap()
}

fn page(changes: Vec<AgentChange>, head: u64) -> AgentChangesOutcome {
    let revision = changes.last().unwrap().revision;
    AgentChangesOutcome::Changes {
        changes,
        cursor: cursor(),
        observed_through_revision: revision,
        runtime_revision: AgentSafeInteger::new(head),
        has_more: revision.value() < head,
    }
}

fn initialized() -> WaitJournal {
    let mut journal = WaitJournal::default();
    journal
        .accept(
            AgentChangesOutcome::Current {
                cursor: cursor(),
                revision: AgentSafeInteger::new(7),
            },
            &AgentSessionId::parse("s1").unwrap(),
            &[],
            None,
        )
        .unwrap();
    journal
}

#[test]
fn transient_match_preserves_history_and_condition_order_across_pages() {
    let mut journal = initialized();
    let session = AgentSessionId::parse("s1").unwrap();
    let until = [AgentWaitCondition::Succeeded, AgentWaitCondition::Done];
    assert!(
        journal
            .accept(
                page(vec![change(8, "other", "succeeded")], 11),
                &session,
                &until,
                None
            )
            .unwrap()
            .is_none()
    );
    assert!(journal.has_more());
    let (matched, conditions) = journal
        .accept(
            page(
                vec![
                    change(9, "s1", "succeeded"),
                    change(10, "s1", "working"),
                    change(11, "other", "idle"),
                ],
                11,
            ),
            &session,
            &until,
            None,
        )
        .unwrap()
        .unwrap();
    assert_eq!(matched.revision.value(), 9);
    assert_eq!(conditions, until);
    assert!(!journal.has_more());
    assert_eq!(journal.revision.unwrap().value(), 11);
}

#[test]
fn gaps_reset_foreign_cursor_and_late_page_contradiction_never_match() {
    let session = AgentSessionId::parse("s1").unwrap();
    for outcome in [
        page(vec![change(9, "s1", "succeeded")], 9),
        page(
            vec![change(8, "s1", "succeeded"), change(10, "other", "idle")],
            10,
        ),
        AgentChangesOutcome::ResetRequired {
            cursor: cursor(),
            revision: AgentSafeInteger::new(900),
        },
        AgentChangesOutcome::Rejected {
            reason: AgentChangesRejectionReason::CursorInvalid,
        },
        AgentChangesOutcome::Current {
            cursor: cursor(),
            revision: AgentSafeInteger::new(8),
        },
    ] {
        let mut journal = initialized();
        assert!(
            journal
                .accept(outcome, &session, &[AgentWaitCondition::Done], None)
                .is_err()
        );
        assert_eq!(journal.revision.unwrap().value(), 7);
    }
    let mut journal = initialized();
    assert!(
        journal
            .accept(
                page(vec![change(8, "s1", "succeeded")], 8),
                &session,
                &[AgentWaitCondition::Done],
                Some(AgentSafeInteger::new(8))
            )
            .unwrap()
            .is_none()
    );
}

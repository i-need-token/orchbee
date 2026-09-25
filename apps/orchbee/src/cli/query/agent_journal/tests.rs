use super::*;
use serde_json::json;

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

fn initialized() -> AgentJournal {
    let mut journal = AgentJournal::default();
    journal
        .accept(AgentChangesOutcome::Current {
            cursor: cursor(),
            revision: AgentSafeInteger::new(7),
        })
        .unwrap();
    journal
}
#[test]
fn contiguous_pages_preserve_every_transient_event_in_order() {
    let mut journal = initialized();
    let first = journal
        .accept(page(vec![change(8, "other", "succeeded")], 11))
        .unwrap();
    assert_eq!(first.len(), 1);
    assert!(journal.has_more());
    let changes = journal
        .accept(page(
            vec![
                change(9, "s1", "succeeded"),
                change(10, "s1", "working"),
                change(11, "other", "idle"),
            ],
            11,
        ))
        .unwrap();
    assert_eq!(
        changes
            .iter()
            .map(|c| c.revision.value())
            .collect::<Vec<_>>(),
        [9, 10, 11]
    );
    assert!(!journal.has_more());
    assert_eq!(journal.observed_through().unwrap().value(), 11);
}
#[test]
fn incomplete_history_and_invalid_pages_never_advance_the_checkpoint() {
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
        assert!(journal.accept(outcome).is_err());
        assert_eq!(journal.observed_through().unwrap().value(), 7);
    }
}

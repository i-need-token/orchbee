use super::*;
#[cfg(windows)]
use crate::cli_outcome::AgentClientVersionMatch;

fn summary(id: usize, presentation: &str) -> AgentSessionSummary {
    serde_json::from_value(json!({
        "executionLocation":{"executionLocationId":"local"}, "lifecycle":"live",
        "observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":2,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,
        "presentationAttachments":[{"generation":"generation-1","navigable":true,"presentationId":presentation,"visibility":"visible"}],
        "providerId":"codex","sessionId":format!("session-{id}"),"status":{"kind":"succeeded"}
    })).unwrap()
}

/// The Agent projection of `projection`; only Windows admits it.
#[cfg(windows)]
pub(in crate::cli) fn document(
    projection: &Projection,
    snapshot: &Snapshot,
    selected: Option<&ClientViewRecord>,
) -> Document {
    let rows = membership(projection, snapshot).unwrap();
    let presentations = rows.values().flatten().collect::<BTreeSet<_>>();
    let sessions = presentations
        .into_iter()
        .enumerate()
        .map(|(i, presentation)| Session {
            summary: summary(i, presentation),
            client: selected.map(|record| AgentClientViewDoc {
                client_id: record.client_id.clone(),
                operation_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!(
                    "{:032x}",
                    i + 100
                ))
                .unwrap(),
                phase: record.view.phase,
                outer_focused: None,
                focused: false,
                attention_revision: AgentSafeInteger::new(9),
                state: AgentClientVersionMatch::Current,
                state_change_revision: Some(AgentSafeInteger::new(8)),
                seen: Some(false),
                status: Some("done"),
            }),
        })
        .collect();
    Document {
        revision: AgentSafeInteger::new(9),
        requests: vec![ObapRequestId::parse(&format!("{:032x}", 99)).unwrap()],
        sessions,
        rows,
    }
}

#[test]
fn priority_and_uncertain_seen_do_not_invent_completion() {
    let ranks = ["unknown", "idle", "working", "done", "error", "blocked"];
    for (i, left) in ranks.iter().enumerate() {
        for (j, right) in ranks.iter().enumerate() {
            assert_eq!(
                aggregate([Some(*left), Some(*right)].into_iter()),
                Some(ranks[i.max(j)])
            );
        }
        assert_eq!(
            aggregate([None, Some(*left)].into_iter()),
            (i >= 3).then_some(*left)
        );
    }
    assert_eq!(aggregate([].into_iter()), Some("unknown"));
    let mut session = Session {
        summary: summary(1, "00000000000000000000000000000001"),
        client: None,
    };
    assert_eq!(status(&session), Some("done"));
    session.summary.status =
        serde_json::from_value(json!({"kind":"working","activity":null,"phase":"working"}))
            .unwrap();
    assert_eq!(status(&session), Some("working"));
    session.summary.observation.freshness = AgentObservationFreshness::Stale;
    assert_eq!(status(&session), Some("unknown"));
}

fn cursor(n: usize) -> AgentListCursor {
    AgentListCursor::parse(&format!("v1.{n:059}.{}", "b".repeat(43))).unwrap()
}
fn push(
    pages: &mut Pages,
    revision: u64,
    start: usize,
    count: usize,
    next: Option<AgentListCursor>,
) -> Result<Option<AgentListCursor>, Failure> {
    pages.push(
        ObapRequestId::parse(&format!("{:032x}", start + 1)).unwrap(),
        AgentListOutcome::Listed {
            revision: AgentSafeInteger::new(revision),
            items: (start..start + count)
                .map(|n| summary(n, "00000000000000000000000000000001"))
                .collect(),
            next_cursor: next,
        },
    )
}
fn reason(error: Failure) -> String {
    error.details.unwrap()["reason"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn complete_pages_reject_truncation_duplicates_revision_drift_and_cursor_loops() {
    for total in [0, 1, 100, 101, 512] {
        let mut pages = Pages::default();
        for offset in (0..total.max(1)).step_by(100) {
            let end = (offset + 100).min(total);
            push(
                &mut pages,
                9,
                offset,
                end - offset,
                (end < total).then(|| cursor(offset)),
            )
            .unwrap();
        }
        assert_eq!(pages.items.len(), total);
    }
    let mut pages = Pages::default();
    push(&mut pages, 9, 0, 100, Some(cursor(1))).unwrap();
    assert_eq!(
        reason(push(&mut pages, 10, 100, 1, None).unwrap_err()),
        "agent-revision-changed"
    );
    for (start, count, next) in [
        (0, 1, None),
        (100, 0, Some(cursor(2))),
        (100, 1, Some(cursor(1))),
        (100, 101, None),
    ] {
        let mut pages = Pages::default();
        push(&mut pages, 9, 0, 100, Some(cursor(1))).unwrap();
        assert_eq!(
            reason(push(&mut pages, 9, start, count, next).unwrap_err()),
            "agent-page-invalid"
        );
    }
    let mut pages = Pages::default();
    for i in 0..5 {
        push(&mut pages, 9, i * 100, 100, Some(cursor(i))).unwrap();
    }
    assert_eq!(
        reason(push(&mut pages, 9, 500, 13, None).unwrap_err()),
        "agent-capacity"
    );
}

#[test]
fn attachments_require_live_navigable_primary_membership_and_deduplicate() {
    let p = "00000000000000000000000000000001";
    let mut session = summary(1, p);
    let primary = BTreeSet::from([p.to_owned()]);
    assert!(matches(&session, &primary));
    assert!(!matches(&session, &BTreeSet::new()));
    session.presentation_attachments[0].navigable = false;
    assert!(!matches(&session, &primary));
    session.presentation_attachments[0].navigable = true;
    session.lifecycle = AgentLifecycle::Ended;
    assert!(!matches(&session, &primary));
    session.lifecycle = AgentLifecycle::Live;
    session
        .presentation_attachments
        .push(session.presentation_attachments[0].clone());
    let doc = Document {
        revision: AgentSafeInteger::new(9),
        requests: vec![],
        sessions: vec![Session {
            summary: session,
            client: None,
        }],
        rows: BTreeMap::from([("row".to_owned(), primary)]),
    };
    assert_eq!(doc.row_sessions("row").len(), 1);
}

#[test]
fn composition_failures_are_closed_and_localized() {
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/orchbee.cli.failure.workbench-query.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for reason in [
        "agent-capacity",
        "agent-page-invalid",
        "agent-revision-changed",
        "workbench-changed",
        "client-changed",
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let out = crate::cli::failure_output(
                failure(reason),
                Some(crate::invocation::OutputFormat::Json),
                locale,
            );
            validator
                .validate(&serde_json::from_slice::<Value>(&out.stdout).unwrap())
                .unwrap();
            let out = crate::cli::failure_output(
                failure(reason),
                Some(crate::invocation::OutputFormat::Text),
                locale,
            );
            assert!(out.stdout.is_empty());
            assert!(!out.stderr.contains("()"));
            assert!(!out.stderr.is_empty());
        }
    }
}

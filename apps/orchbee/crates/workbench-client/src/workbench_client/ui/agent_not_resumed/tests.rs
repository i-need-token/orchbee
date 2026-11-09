//! A resume fact shows once per Client, only while fresh, newest first.
use super::AgentNotResumedNotices;
use orchbee_workbench_protocol::{
    AgentNotResumedReason, PanelNode, Snapshot, WorkbenchProtocolDocument, parse_document,
};

const CORPUS: &str =
    include_str!("../../../../../../../../schemas/workbench/v1/workbench-conformance-v1.json");

/// The corpus snapshot whose bound Presentation carries a fact, with that fact at `at`.
fn snapshot(at: u64) -> Snapshot {
    let corpus: serde_json::Value = serde_json::from_str(CORPUS).unwrap();
    let document = &corpus["documents"]["snapshot-agent-not-resumed-r2"];
    let WorkbenchProtocolDocument::Snapshot(mut snapshot) =
        parse_document(&serde_json::to_vec(document).unwrap()).unwrap()
    else {
        panic!("the corpus document is a snapshot");
    };
    for tab in snapshot
        .terminal_contexts
        .iter_mut()
        .flat_map(|context| &mut context.tabs)
    {
        if let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut tab.root
            && let Some(fact) = primary_presentation.agent_not_resumed.as_mut()
        {
            fact.at = at;
        }
    }
    snapshot
}

#[test]
fn a_fact_shows_once_and_only_while_fresh() {
    let now = 1_800_000_000_000;
    let mut notices = AgentNotResumedNotices::default();
    assert_eq!(
        notices.observe(&snapshot(now - 1_000), now),
        Some((
            "claude-code".to_owned(),
            AgentNotResumedReason::PromptUnconfirmed
        ))
    );
    // Every later snapshot still carries it; it never shows again.
    assert_eq!(notices.observe(&snapshot(now - 1_000), now + 1_000), None);
    // A Client that first reads a fact long after it was published says nothing.
    let mut late = AgentNotResumedNotices::default();
    assert_eq!(late.observe(&snapshot(now - 61_000), now), None);
    // A new give-up on the same Presentation is a new fact.
    assert!(
        notices
            .observe(&snapshot(now + 5_000), now + 5_000)
            .is_some()
    );
}

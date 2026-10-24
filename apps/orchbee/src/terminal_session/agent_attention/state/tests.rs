use super::*;
use orchbee_agent_observation_protocol::*;

const PRESENTATION: &str = "00000000000000000000000000000001";

fn summary(id: &str) -> AgentSessionSummary {
    AgentSessionSummary {
        execution_location: AgentExecutionLocation {
            execution_location_id: AgentExecutionLocationId::parse("local").unwrap(),
        },
        lifecycle: AgentLifecycle::Live,
        observation: AgentObservation {
            fidelity: AgentObservationFidelity::Rich,
            freshness: AgentObservationFreshness::Fresh,
            generation: AgentSafeInteger::new(1),
            sequence: AgentSafeInteger::new(1),
            source: AgentObservationSource::Native,
        },
        parent_session_id: None,
        pending_request_count: AgentSafeInteger::new(0),
        presentation_attachments: vec![AgentPresentationAttachment {
            generation: AgentAttachmentGeneration::parse("generation-1").unwrap(),
            navigable: true,
            presentation_id: AgentPresentationId::parse(PRESENTATION).unwrap(),
            visibility: AgentPresentationVisibility::Visible,
        }],
        provider_id: AgentProviderId::parse("codex").unwrap(),
        session_id: AgentSessionId::parse(id).unwrap(),
        status: AgentSemanticStatus::Idle {},
    }
}

fn round(
    items: Vec<AgentSessionSummary>,
    prior: Option<u64>,
    revision: u64,
    changes: Vec<AgentChange>,
    reset: bool,
) -> AttentionRound {
    AttentionRound {
        items,
        changes,
        prior_revision: prior,
        revision,
        cursor: AgentChangesCursor::parse(&format!("v1.{}.{}", "a".repeat(59), "b".repeat(43)))
            .unwrap(),
        reset,
    }
}

fn change(row: &AgentSessionSummary, revision: u64) -> AgentChange {
    AgentChange {
        revision: AgentSafeInteger::new(revision),
        session: projection(row),
        transition: AgentChangeTransition::SessionIdleObserved,
    }
}

fn paint(state: &AttentionState) -> AttentionPaint {
    state.paint(&BTreeSet::from([PRESENTATION])).unwrap()
}

#[test]
fn editing_a_completed_session_changes_only_the_matching_display_version() {
    let mut state = AttentionState::default();
    let mut row = summary("edited-agent");
    row.status = AgentSemanticStatus::Succeeded {};
    state
        .publish(&round(vec![row.clone()], None, 1, vec![], false))
        .unwrap();
    state.acknowledge(&paint(&state), Some(true), |_| true);
    assert!(!state.completion_edited(
        &row.session_id,
        row.observation.generation,
        row.observation.sequence
    ));
    assert!(state.edit_completed(PRESENTATION));
    assert!(state.completion_edited(
        &row.session_id,
        row.observation.generation,
        row.observation.sequence
    ));
    assert_eq!(
        state.entries[&row.session_id].summary.status,
        AgentSemanticStatus::Succeeded {}
    );
    row.observation.sequence = AgentSafeInteger::new(2);
    state
        .publish(&round(
            vec![row.clone()],
            Some(1),
            2,
            vec![change(&row, 2)],
            false,
        ))
        .unwrap();
    assert!(!state.completion_edited(
        &row.session_id,
        row.observation.generation,
        row.observation.sequence
    ));
}

#[test]
fn attention_rapid_round_trip_is_unseen_and_clients_acknowledge_independently() {
    let row = summary("agent-1");
    let mut first = AttentionState::default();
    let mut second = AttentionState::default();
    let baseline = round(vec![row.clone()], None, 1, vec![], false);
    first.publish(&baseline).unwrap();
    second.publish(&baseline).unwrap();
    assert_eq!(first.unseen_in(PRESENTATION), 0);
    let mut working = row.clone();
    working.status = AgentSemanticStatus::Succeeded {};
    let update = round(
        vec![row.clone()],
        Some(1),
        3,
        vec![change(&working, 2), change(&row, 3)],
        false,
    );
    first.publish(&update).unwrap();
    second.publish(&update).unwrap();
    assert_eq!(first.unseen_in(PRESENTATION), 1);
    let proof = paint(&first);
    assert!(!first.acknowledge(&proof, Some(false), |_| true));
    assert!(!first.acknowledge(&proof, Some(true), |_| false));
    assert!(first.acknowledge(&proof, None, |_| true));
    assert_eq!(first.unseen_in(PRESENTATION), 0);
    assert_eq!(second.unseen_in(PRESENTATION), 1);
    assert_eq!(
        first.entries[&row.session_id].summary.status,
        AgentSemanticStatus::Idle {}
    );
}

#[test]
fn attention_metadata_and_other_agents_do_not_reopen_seen_versions() {
    let row = summary("agent-1");
    let mut state = AttentionState::default();
    state
        .publish(&round(vec![row.clone()], None, 1, vec![], false))
        .unwrap();
    let mut metadata = row.clone();
    metadata.observation.sequence = AgentSafeInteger::new(2);
    metadata.presentation_attachments[0].generation =
        AgentAttachmentGeneration::parse("generation-2").unwrap();
    let mut other = summary("agent-2");
    other.presentation_attachments.clear();
    state
        .publish(&round(
            vec![metadata.clone(), other.clone()],
            Some(1),
            3,
            vec![change(&metadata, 2), change(&other, 3)],
            false,
        ))
        .unwrap();
    assert_eq!(state.unseen_in(PRESENTATION), 0);
    assert!(!state.entries[&other.session_id].seen());
    assert_eq!(state.entries[&row.session_id].version, 1);
}

#[test]
fn attention_stale_paint_reset_and_read_failure_never_acknowledge() {
    let mut row = summary("agent-1");
    let mut state = AttentionState::default();
    state
        .publish(&round(vec![row.clone()], None, 1, vec![], true))
        .unwrap();
    let stale = paint(&state);
    row.observation.generation = AgentSafeInteger::new(2);
    state
        .publish(&round(
            vec![row.clone()],
            Some(1),
            2,
            vec![change(&row, 2)],
            false,
        ))
        .unwrap();
    assert!(!state.acknowledge(&stale, Some(true), |_| true));
    let current = paint(&state);
    assert!(state.unavailable());
    assert!(state.paint(&BTreeSet::from([PRESENTATION])).is_none());
    assert!(!state.acknowledge(&current, Some(true), |_| true));
    state
        .publish(&round(vec![row.clone()], Some(2), 2, vec![], false))
        .unwrap();
    assert!(!state.acknowledge(&current, Some(true), |_| true));
    let proof = paint(&state);
    assert!(state.acknowledge(&proof, Some(true), |_| true));
    state
        .publish(&round(vec![row], None, 0, vec![], true))
        .unwrap();
    assert_eq!(state.unseen_in(PRESENTATION), 1);
}

#[test]
fn attention_bad_rounds_leave_the_previous_snapshot_untouched() {
    let row = summary("agent-1");
    let mut state = AttentionState::default();
    state
        .publish(&round(vec![row.clone()], None, 1, vec![], false))
        .unwrap();
    let entries = state.entries.clone();
    let mut changed = row.clone();
    changed.status = AgentSemanticStatus::Failed {};
    for bad in [
        round(vec![changed.clone()], Some(1), 1, vec![], false),
        round(
            vec![changed.clone()],
            Some(1),
            3,
            vec![change(&changed, 3)],
            false,
        ),
        round(vec![row.clone()], Some(0), 1, vec![], false),
        round(vec![row.clone(), row.clone()], None, 1, vec![], true),
        round(vec![row; 513], None, 1, vec![], true),
    ] {
        assert_eq!(state.publish(&bad), Err(()));
        assert_eq!(state.entries, entries);
        assert_eq!(state.revision, Some(1));
    }
}

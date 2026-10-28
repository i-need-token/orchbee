use super::*;
use crate::machine_agents::{PlacementOrder, Row};
use crate::machines::ProfileId;
use orchbee_workbench_protocol::TerminalTitleObservation;
use orchbee_workbench_protocol::{
    ClientTerminalAgentAttentionEntry, ClientTerminalAgentAttentionVersion, MetadataTokens,
    PanelMetadataEffective,
};
#[test]
fn raw_title_animation_and_clear_never_change_agent_seen_or_semantic_sequence() {
    let mut attention = Attention::default();
    let baseline = super::super::test_support::checkpoint(vec![row()]);
    assert!(attention.project(&Choice::Local, &baseline).unwrap()[0].seen);
    let mut changed = baseline.as_ref().clone();
    let mut rows = changed.rows.to_vec();
    rows[0].sequence = WorkbenchRevision::new(2);
    changed.rows = rows.into();
    changed.report_revision = WorkbenchRevision::new(3);
    assert!(
        !attention
            .project(&Choice::Local, &Arc::new(changed.clone()))
            .unwrap()[0]
            .seen
    );
    for (revision, title) in [(1, Some("⠋ task")), (2, Some("⠙ task")), (3, None)] {
        let mut rows = changed.rows.to_vec();
        rows[0].terminal_title = TerminalTitleObservation::Observed {
            revision: WorkbenchRevision::new(revision),
            title: title.map(str::to_owned),
        };
        changed.rows = rows.into();
        let projected = attention
            .project(&Choice::Local, &Arc::new(changed.clone()))
            .unwrap();
        assert!(!projected[0].seen);
        assert_eq!(projected[0].sequence.value(), 2);
        assert_eq!(projected[0].status(), "idle");
    }
}
fn row() -> Row {
    Row {
        seen: true,
        name: None,
        order: PlacementOrder::default(),
        target: super::super::test_support::target(),
        label: "terminal".into(),
        workspace: String::new(),
        tab_label: None,
        terminal_title: orchbee_workbench_protocol::TerminalTitleObservation::Unavailable {},
        state: PanelAgentState::Idle,
        sequence: WorkbenchRevision::new(1),
        metadata: PanelMetadataEffective::default(),
        tokens: MetadataTokens::default(),
    }
}

#[test]
fn completion_edit_projects_idle_without_mutating_observation_or_new_completion() {
    let mut attention = Attention::default();
    let mut done = row();
    done.state = PanelAgentState::Done;
    let round = super::super::test_support::checkpoint(vec![done.clone()]);
    assert_eq!(
        attention.project(&Choice::Local, &round).unwrap()[0].state,
        PanelAgentState::Done
    );
    let edits = [(done.target.physical.clone(), done.sequence)];
    assert!(attention.observe_edits(&Choice::Local, &edits));
    assert_eq!(
        attention.project(&Choice::Local, &round).unwrap()[0].display_state(),
        orchbee_workbench_protocol::MetadataState::Idle
    );
    assert_eq!(round[0].state, PanelAgentState::Done);
    done.sequence = WorkbenchRevision::new(2);
    let next = super::super::test_support::checkpoint(vec![done]);
    assert_eq!(
        attention.project(&Choice::Local, &next).unwrap()[0].state,
        PanelAgentState::Done
    );
    assert!(!attention.observe_edits(&Choice::Local, &edits));
}
#[test]
fn endpoint_baseline_changed_new_rebound_and_restart_versions_are_distinct() {
    let mut attention = Attention::default();
    let local = Choice::Local;
    let round = super::super::test_support::checkpoint(vec![row()]);
    assert!(attention.project(&local, &round).unwrap()[0].seen);
    let remote = Choice::Remote(ProfileId::parse(&"9".repeat(32)).unwrap());
    assert!(attention.project(&remote, &round).unwrap()[0].seen);
    let mut changed = round.as_ref().clone();
    let mut rows = changed.rows.to_vec();
    rows[0].sequence = WorkbenchRevision::new(2);
    changed.rows = rows.into();
    changed.report_revision = WorkbenchRevision::new(3);
    let next = attention
        .project(&local, &Arc::new(changed.clone()))
        .unwrap();
    assert!(!next[0].seen);
    assert_eq!(next[0].status(), "idle");
    assert!(attention.project(&remote, &round).unwrap()[0].seen);
    let before = attention.clone();
    let mut invalid = changed.clone();
    invalid.report_revision = WorkbenchRevision::new(1);
    assert!(attention.project(&local, &Arc::new(invalid)).is_err());
    assert_eq!(attention, before);
    let binding = next[0].target.physical.clone();
    let mut ack = ClientTerminalAgentAttentionSnapshot {
        report_revision: WorkbenchRevision::new(3),
        outer_focused: Some(true),
        entries: vec![ClientTerminalAgentAttentionEntry {
            binding: binding.clone(),
            state: Some(ClientTerminalAgentAttentionVersion {
                sequence: WorkbenchRevision::new(1),
                seen: true,
            }),
        }],
    };
    assert!(!attention.acknowledge(&local, &ack));
    ack.entries[0].state.as_mut().unwrap().sequence = WorkbenchRevision::new(2);
    assert!(attention.acknowledge(&local, &ack));
    assert!(
        attention
            .project(&local, &Arc::new(changed.clone()))
            .unwrap()[0]
            .seen
    );
    let mut rows = changed.rows.to_vec();
    rows[0].target.physical.generation =
        orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&"9".repeat(32))
            .unwrap();
    changed.rows = rows.into();
    assert!(
        !attention
            .project(&local, &Arc::new(changed.clone()))
            .unwrap()[0]
            .seen
    );
    assert!(!attention.acknowledge(&local, &ack));
    let session = SessionId::parse(&"8".repeat(32)).unwrap();
    changed.session_id = session.clone();
    let mut rows = changed.rows.to_vec();
    rows[0].target.context.session_id = session;
    changed.rows = rows.into();
    assert!(attention.project(&local, &Arc::new(changed)).unwrap()[0].seen);
}
#[test]
fn invalid_complete_checkpoint_preserves_prior_attention_and_missing_members_are_pruned() {
    let mut attention = Attention::default();
    let round = super::super::test_support::checkpoint(vec![row()]);
    attention.project(&Choice::Local, &round).unwrap();
    let before = attention.clone();
    let invalid = super::super::test_support::checkpoint(vec![row(), row()]);
    assert!(attention.project(&Choice::Local, &invalid).is_err());
    assert_eq!(attention, before);
    attention
        .project(&Choice::Local, &super::super::test_support::checkpoint(Vec::new()))
        .unwrap();
    assert!(!attention.project(&Choice::Local, &round).unwrap()[0].seen);
}

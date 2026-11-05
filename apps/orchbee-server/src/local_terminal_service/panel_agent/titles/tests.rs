use super::*;
use orchbee_workbench_protocol::{
    PanelAgentState, SessionId, TerminalAgentStateSnapshot, WorkbenchRequestId,
};
use serde_json::json;
fn binding() -> AgentLaunchBinding {
    serde_json::from_value(json!({"presentationId":"00000000000000000000000000000001","generation":"00000000000000000000000000000002","hostEpoch":"00000000000000000000000000000003","hostTerminalId":"00000000000000000000000000000004"})).unwrap()
}
fn result(binding: AgentLaunchBinding) -> PanelAgentResult {
    PanelAgentResult {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("00000000000000000000000000000005").unwrap(),
        session_id: SessionId::parse("00000000000000000000000000000006").unwrap(),
        outcome: PanelAgentOutcome::States {
            workbench_revision: WorkbenchRevision::new(1),
            report_revision: WorkbenchRevision::new(2),
            presentations: vec![TerminalAgentStateRow {
                presentation_id: TerminalPresentationId::parse(binding.presentation_id.as_str())
                    .unwrap(),
                snapshot: Some(TerminalAgentStateSnapshot {
                    completion: Default::default(),
                    binding,
                    sequence: WorkbenchRevision::new(7),
                    agent: Some("codex".into()),
                    state: PanelAgentState::Working,
                }),
            }],
        },
    }
}
#[test]
fn exact_physical_title_context_rejects_changed_creation_and_partial_membership() {
    let exact = binding();
    assert!(state_context(result(exact.clone()), std::slice::from_ref(&exact)).is_ok());
    let mut changed = exact.clone();
    changed.generation = orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(
        "00000000000000000000000000000009",
    )
    .unwrap();
    assert_eq!(
        state_context(result(changed), std::slice::from_ref(&exact)),
        Err(PanelAgentFailure::BindingChanged)
    );
    let mut incomplete = result(exact.clone());
    if let PanelAgentOutcome::States { presentations, .. } = &mut incomplete.outcome {
        presentations.clear();
    }
    assert_eq!(
        state_context(incomplete, std::slice::from_ref(&exact)),
        Err(PanelAgentFailure::BindingChanged)
    );
    let physical = identities(&[exact]).unwrap()[0];
    assert_eq!(physical.creation_id().as_bytes(), 2_u128.to_be_bytes());
    assert_eq!(physical.host_epoch().as_bytes(), 3_u128.to_be_bytes());
    assert_eq!(physical.terminal_id().as_bytes(), 4_u128.to_be_bytes());
}
#[test]
fn one_physical_title_observation_projects_to_distinct_exact_presentation_aliases() {
    let first = binding();
    let mut second = first.clone();
    second.presentation_id = orchbee_agent_observation_protocol::AgentPresentationId::parse(
        "00000000000000000000000000000007",
    )
    .unwrap();
    let bindings = vec![first, second];
    let physical = identities(&bindings).unwrap();
    assert_eq!(physical[0], physical[1]);
    let rows = vec![node::TerminalTitleRow {
        identity: physical[0],
        observation: node::TerminalTitleObservation::Observed {
            revision: std::num::NonZeroU64::MIN,
            title: Some("⠋ shared".into()),
        },
    }];
    let projected = project_titles(&bindings, &physical, &rows).unwrap();
    assert_eq!(projected.len(), 2);
    assert_ne!(
        projected[0].binding.presentation_id,
        projected[1].binding.presentation_id
    );
    assert_eq!(projected[0].observation, projected[1].observation);
    assert!(project_titles(&bindings, &physical, &[]).is_err());
}

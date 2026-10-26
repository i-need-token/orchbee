use super::*;
use orchbee_workbench_protocol::{PanelAgentView, TerminalAgentObservation};
use serde_json::json;

#[test]
fn physical_client_projection_requires_exact_binding_sequence_and_server_head() {
    let id = "1".repeat(32);
    let binding: AgentLaunchBinding = serde_json::from_value(json!({
        "presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id
    }))
    .unwrap();
    let mut report = TerminalAgentRead {
        source: json!({}),
        report_revision: WorkbenchRevision::new(7),
        observation: Box::new(TerminalAgentObservation {
            binding: binding.clone(),
            sequence: WorkbenchRevision::new(3),
            changes: vec![],
            view: PanelAgentView {
                agent: Some("pi".into()),
                state: PanelAgentState::Idle,
                ..Default::default()
            },
        }),
    };
    let read = |binding: &AgentLaunchBinding, revision: u64, sequence: Option<u64>, seen: bool| {
        Read {
        client: WorkbenchClientId::parse(&id).unwrap(),
        operation: WorkbenchRequestId::parse(&id).unwrap(),
        view: serde_json::from_value(json!({"workbenchRevision":"1","phase":"starting","selection":null})).unwrap(),
        attention: serde_json::from_value(json!({"reportRevision":revision.to_string(),"outerFocused":false,
            "entries":[{"binding":binding,"state":sequence.map(|seq|json!({"sequence":seq.to_string(),"seen":seen}))}]})).unwrap(),
    }
    };
    for (seen, status) in [(true, "idle"), (false, "idle")] {
        let result = read(&binding, 7, Some(3), seen).corroborate(&report);
        assert_eq!((result.seen, result.status), (Some(seen), Some(status)));
        assert_eq!(result.outer_focused, Some(false));
    }
    let mut replacement = binding.clone();
    replacement.host_terminal_id = serde_json::from_value(json!("2".repeat(32))).unwrap();
    for (binding, revision, sequence) in [
        (&binding, 7, None),
        (&binding, 7, Some(2)),
        (&binding, 7, Some(4)),
        (&binding, 8, Some(3)),
        (&replacement, 7, Some(3)),
    ] {
        let result = read(binding, revision, sequence, true).corroborate(&report);
        assert_eq!(
            (result.sequence, result.seen, result.status),
            (None, None, Some("idle"))
        );
    }
    report.observation.view.state = PanelAgentState::Working;
    let result = read(&binding, 7, Some(2), true).corroborate(&report);
    assert_eq!((result.seen, result.status), (None, Some("working")));
}

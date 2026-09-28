use super::*;
use crate::{
    PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, PanelAgentResult, ProtocolVersion,
    SiteId, WorkbenchRequestId,
};
use serde_json::json;

fn snapshot(n: u8) -> TerminalAgentStateSnapshot {
    let id = format!("{n:032x}");
    serde_json::from_value(json!({"binding":{"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id},"sequence":"2","agent":"pi","state":"idle"})).unwrap()
}

#[test]
fn compact_physical_states_require_ordered_exact_presentations_and_real_generations() {
    let a = snapshot(1);
    let b = snapshot(2);
    let ids = vec![
        TerminalPresentationId::parse(a.binding.presentation_id.as_str()).unwrap(),
        TerminalPresentationId::parse(b.binding.presentation_id.as_str()).unwrap(),
    ];
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        site_id: SiteId::parse(&"2".repeat(32)).unwrap(),
        action: PanelAgentAction::States {
            presentation_ids: ids.clone(),
        },
    };
    assert!(request.validate().is_ok());
    let mut result = PanelAgentResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: PanelAgentOutcome::States {
            workbench_revision: WorkbenchRevision::new(3),
            report_revision: WorkbenchRevision::new(5),
            presentations: vec![
                TerminalAgentStateRow {
                    presentation_id: ids[0].clone(),
                    snapshot: Some(a),
                },
                TerminalAgentStateRow {
                    presentation_id: ids[1].clone(),
                    snapshot: None,
                },
            ],
        },
    };
    assert!(result.answers(&request));
    let PanelAgentOutcome::States { presentations, .. } = &mut result.outcome else {
        unreachable!()
    };
    presentations[0].snapshot = Some(b);
    assert!(result.validate().is_err());
    let mut duplicate = request.clone();
    duplicate.action = PanelAgentAction::States {
        presentation_ids: vec![ids[0].clone(), ids[0].clone()],
    };
    assert!(duplicate.validate().is_err());
    let mut value = serde_json::to_value(snapshot(1)).unwrap();
    value["binding"]["generation"] = json!("1");
    let invalid: TerminalAgentStateSnapshot = serde_json::from_value(value).unwrap();
    assert!(!invalid.valid());
    let mut value = serde_json::to_value(TerminalAgentStateRow {
        presentation_id: ids[0].clone(),
        snapshot: None,
    })
    .unwrap();
    value.as_object_mut().unwrap().remove("snapshot");
    assert!(serde_json::from_value::<TerminalAgentStateRow>(value).is_err());
}

#[test]
fn compact_states_bound_unique_requests_and_do_not_admit_history_fields() {
    let ids: Vec<_> = (1..=MAXIMUM_TERMINAL_AGENT_STATE_ROWS + 1)
        .map(|n| TerminalPresentationId::parse(&format!("{n:032x}")).unwrap())
        .collect();
    let mut request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        site_id: SiteId::parse(&"2".repeat(32)).unwrap(),
        action: PanelAgentAction::States {
            presentation_ids: ids,
        },
    };
    assert!(request.validate().is_err());
    let PanelAgentAction::States { presentation_ids } = &mut request.action else {
        unreachable!()
    };
    presentation_ids.pop();
    assert!(request.validate().is_ok());
    let mut value = serde_json::to_value(snapshot(1)).unwrap();
    value["changes"] = json!([]);
    assert!(serde_json::from_value::<TerminalAgentStateSnapshot>(value).is_err());
}

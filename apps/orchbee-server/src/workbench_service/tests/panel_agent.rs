use super::*;
use orchbee_workbench_protocol::{
    PanelAgentAction as Action, PanelAgentFailure as Failure, PanelAgentOutcome as Outcome,
    PanelAgentRequest, SiteId, Snapshot,
};
fn service(terminals: &Arc<LocalTerminalService>, grants: u8) -> WorkbenchService {
    WorkbenchService::new(
        WorkbenchPolicy {
            read: grants & 1 != 0,
            manage_navigation: grants & 2 != 0,
            associate_directory: grants & 4 != 0,
            create_terminal: grants & 8 != 0,
            manage_terminal_lifecycle: grants & 16 != 0,
        },
        Arc::clone(terminals),
    )
}
fn snapshot(service: &WorkbenchService) -> Snapshot {
    let WorkbenchResponse::Snapshot(result) = response(service.execute(
        WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 200)).unwrap(),
        }),
    )) else {
        panic!("snapshot result")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("snapshot")
    };
    snapshot
}

#[test]
fn panel_agent_permissions_are_independent_and_refusals_leave_topology_unchanged() {
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let admin = service(&terminals, 31);
    let before = snapshot(&admin);
    let mut request:PanelAgentRequest=serde_json::from_value(serde_json::json!({"protocolVersion":1,"requestId":"1".repeat(32),"siteId":before.site_id,"action":{"kind":"read","panelIds":[]}})).unwrap();
    let execute = |grants, request: &PanelAgentRequest| {
        let WorkbenchResponse::PanelAgent(reply) = response(
            service(&terminals, grants).execute(WorkbenchRequest::PanelAgent(request.clone())),
        ) else {
            panic!("metadata")
        };
        assert!(reply.answers(request));
        reply.outcome
    };
    for grants in 0..32 {
        assert!(if grants & 1 == 0 {
            execute(grants, &request)
                == Outcome::Failed {
                    reason: Failure::Denied,
                }
        } else {
            matches!(execute(grants,&request),Outcome::Read{panels,..} if panels.is_empty())
        });
    }
    request.action=serde_json::from_value(serde_json::json!({"kind":"report","panelId":"2".repeat(32),"expectedBinding":{"workerId":"3".repeat(32),"terminalId":"4".repeat(32)},"report":{"kind":"state","source":"test","agent":"Team Agent","state":"working"}})).unwrap();
    for grants in 0..32 {
        assert_eq!(
            execute(grants, &request),
            Outcome::Failed {
                reason: if grants & 2 == 0 {
                    Failure::Denied
                } else {
                    Failure::NotFound
                }
            }
        );
    }
    request.site_id = SiteId::parse(&"f".repeat(32)).unwrap();
    assert_eq!(
        execute(31, &request),
        Outcome::Failed {
            reason: Failure::SiteMismatch
        }
    );
    request.site_id = before.site_id.clone();
    if let Action::Report { report, .. } = &mut request.action
        && let orchbee_workbench_protocol::PanelAgentReport::State { agent, .. } = report.as_mut()
    {
        *agent = " ".into();
    }
    assert_eq!(
        execute(31, &request),
        Outcome::Failed {
            reason: Failure::InvalidRequest
        }
    );
    assert_eq!(snapshot(&admin), before);
}

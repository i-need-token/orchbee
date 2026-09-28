use super::*;
use crate::local_client::test_server::FakeServer;
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    PanelAgentResult, PanelMetadataResult, WorkbenchRequest, WorkbenchResponse,
    read_workbench_request, write_workbench_response,
};
use serde_json::json;
use std::{
    io::{self, Read},
    num::NonZeroU64,
    time::Duration,
};

fn scope() -> Scope {
    Scope { site: SiteId::parse("00000000000000000000000000000001").unwrap(), revision: WorkbenchRevision::new(5), panels: vec![(WorkbenchPanelId::parse("00000000000000000000000000000002").unwrap(), serde_json::from_value(json!({"workerId":"00000000000000000000000000000003","terminalId":"00000000000000000000000000000004"})).unwrap())] }
}
use orchbee_workbench_protocol::SiteId;
fn accept(stream: &mut orchbee_local_ipc::LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(1).unwrap(),
        },
    )
    .map_err(io::Error::other)
}
#[test]
fn panel_read_lane_joins_guards_and_refuses_either_revision_fence_or_wrong_binding() {
    // 0 coherent, 1 metadata fence changed, 2 report fence changed, 3 wrong binding,
    // 4 Workbench revision changed, 5 missing requested rows, 6 guard does not match.
    for fault in 0..=6 {
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            accept(&mut stream)?;
            if index == 0 { return Ok(()); }
            let s = scope();
            let mut row_binding = s.panels[0].1.clone();
            if fault == 3 { row_binding.terminal_id = orchbee_workbench_protocol::TerminalId::parse("00000000000000000000000000000009").unwrap(); }
            for step in 0..4 {
                let request = read_workbench_request(&mut stream).map_err(io::Error::other)?;
                let response = if step % 2 == 0 {
                    let WorkbenchRequest::PanelMetadata(request) = request else { panic!("metadata before reports and first fence") };
                    let PanelMetadataAction::Read { panel_ids } = request.action else { panic!("read only") };
                    assert_eq!(panel_ids.len(), usize::from(step == 0));
                    let panels = if step == 0 && fault != 5 { json!([{"panelId":s.panels[0].0,"binding":row_binding,"tokens":{"branch":"dev"},"presentations":[{"source":"custom","agent":if fault==6 {"pi"} else {"grok"},"appliesToSource":"custom","title":{"value":"Approval title","order":"1"},"displayAgent":null,"stateLabels":{}}]}]) } else { json!([]) };
                    let result: PanelMetadataResult = serde_json::from_value(json!({"protocolVersion":1,"requestId":request.request_id,"siteId":request.site_id,"outcome":{"kind":"read","workbenchRevision":if fault==4 {"6"} else {"5"},"metadataRevision":if fault==1 && step==2 {"2"} else {"1"},"panels":panels}})).unwrap();
                    WorkbenchResponse::PanelMetadata(result)
                } else {
                    let WorkbenchRequest::PanelAgent(request) = request else { panic!("report after metadata and last fence") };
                    let PanelAgentAction::Read { panel_ids } = request.action else { panic!("read only") };
                    assert_eq!(panel_ids.len(), usize::from(step == 1));
                    let panels = if step == 1 { json!([{"panelId":s.panels[0].0,"binding":s.panels[0].1,"report":{"agent":"grok","state":"blocked","authoritySource":"custom","message":"approval","session":null}}]) } else { json!([]) };
                    let result: PanelAgentResult = serde_json::from_value(json!({"protocolVersion":1,"requestId":request.request_id,"siteId":request.site_id,"outcome":{"kind":"read","workbenchRevision":"5","reportRevision":if fault==2 && step==3 {"2"} else {"1"},"panels":panels}})).unwrap();
                    WorkbenchResponse::PanelAgent(result)
                };
                write_workbench_response(&mut stream, &response).map_err(io::Error::other)?;
                if (matches!(fault,3..=5) && step==0) || (fault==1 && step==2) { break; }
            }
            assert_eq!(stream.read(&mut [0])?,0); Ok(())
        }).unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let (origin, clock) = (client.origin(), client.operation_clock());
        drop(client);
        let lane = AgentReadLane::spawn(origin, clock.clone()).unwrap();
        assert!(lane.send(LaneCommand::PanelReports {
            epoch: 7,
            scope: scope(),
            deadline: clock.now() + Duration::from_secs(5)
        }));
        let LaneOutcome::PanelReports { epoch, result } =
            lane.outcomes.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("report outcome")
        };
        assert_eq!(epoch, 7);
        if matches!(fault, 0 | 6) {
            let round = result.unwrap();
            let row = &round.rows[&scope().panels[0].0];
            assert_eq!(
                row.report.as_ref().unwrap().state,
                orchbee_workbench_protocol::PanelAgentState::Blocked
            );
            assert_eq!(
                row.metadata.title.as_ref().map(|v| v.as_str()),
                if fault == 6 {
                    None
                } else {
                    Some("Approval title")
                }
            );
        } else {
            assert!(result.is_err(), "fault {fault} admitted");
        }
        lane.close();
        server.finish().unwrap();
    }
}
#[test]
fn report_lane_drop_interrupts_required_read_and_joins() {
    let (tx, rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        accept(&mut stream)?;
        if index == 0 {
            return Ok(());
        }
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::PanelMetadata(_)
        ));
        tx.send(()).unwrap();
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let (origin, clock) = (client.origin(), client.operation_clock());
    drop(client);
    let lane = AgentReadLane::spawn(origin, clock.clone()).unwrap();
    assert!(lane.send(LaneCommand::PanelReports {
        epoch: 1,
        scope: scope(),
        deadline: clock.now() + Duration::from_secs(20)
    }));
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let start = Instant::now();
    drop(lane);
    assert!(start.elapsed() < Duration::from_secs(3));
    server.finish().unwrap();
}

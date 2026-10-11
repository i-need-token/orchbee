use super::*;
use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason as Reason;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentReport, PanelAgentRequest, PanelAgentResult, PanelAgentState,
};
#[test]
fn panel_agent_report_correlation_uncertainty_and_no_replay() -> TestResult {
    for case in 0..9 {
        let request = PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&id(1)).unwrap(),
            site_id: SiteId::parse(&id(2)).unwrap(),
            action: PanelAgentAction::Report {
                panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(3)).unwrap(),
                expected_binding: orchbee_workbench_protocol::PresentationBinding {
                    worker_id: orchbee_workbench_protocol::WorkerId::parse(&id(4)).unwrap(),
                    terminal_id: orchbee_workbench_protocol::TerminalId::parse(&id(5)).unwrap(),
                },
                report: Box::new(PanelAgentReport::State {
                    source: "custom".into(),
                    agent: "custom-agent".into(),
                    state: PanelAgentState::Working,
                    message: None,
                    sequence: None,
                    agent_session_id: None,
                    agent_session_path: None,
                }),
            },
        };
        let expected = request.clone();
        let mut reply = serde_json::json!({"protocolVersion":1,"requestId":id(1),"siteId":id(2),"outcome":{"kind":"reported","panelId":id(3),"workbenchRevision":"5","reportRevision":"1","disposition":"applied","changed":true,"view":{"agent":"custom-agent","state":"working","authoritySource":"custom","message":null,"session":null},"binding":{"workerId":id(4),"terminalId":id(5)}}});
        match case {
            8 => reply["outcome"]["binding"]["terminalId"] = serde_json::json!(id(99)),
            1 => reply["requestId"] = serde_json::json!(id(90)),
            2 => reply["siteId"] = serde_json::json!(id(90)),
            3 => reply["outcome"]["panelId"] = serde_json::json!(id(90)),
            4 => {
                reply["outcome"] = serde_json::json!({"kind":"read","workbenchRevision":"5","reportRevision":"1","panels":[]})
            }
            _ => {}
        }
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 60)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::PanelAgent(expected)
            );
            if case == 6 {
                return Ok(());
            }
            if case == 7 {
                std::thread::sleep(Duration::from_millis(250));
            } else if case == 5 {
                stream.write_all(&[0; 12])?;
            } else {
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::PanelAgent(
                        serde_json::from_value::<PanelAgentResult>(reply).unwrap(),
                    ),
                )
                .map_err(io::Error::other)?;
            }
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte)?, 0, "no replay or further frames");
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let early = client
            .panel_agent(
                request.clone(),
                client.operation_clock().now() - Duration::from_millis(1),
                None,
            )
            .unwrap_err();
        assert!(!early.submitted);
        assert!(matches!(
            early.reason,
            Reason::Deadline | Reason::NotSubmitted
        ));
        assert!(client.generation().is_some());
        let deadline = client.operation_clock().now()
            + if case == 7 {
                Duration::from_millis(60)
            } else {
                SERVER_GUARD_TIMEOUT
            };
        let result = client.panel_agent(request, deadline, None);
        if case == 0 {
            assert!(result.is_ok());
            assert!(client.generation().is_some());
        } else {
            let error = result.unwrap_err();
            assert!(error.submitted);
            assert_eq!(
                error.reason,
                match case {
                    5 => Reason::Protocol,
                    6 => Reason::Read,
                    7 => Reason::ResponseDeadline,
                    _ => Reason::Correlation,
                }
            );
            assert!(client.generation().is_none());
        }
        drop(client);
        server.finish()?;
    }
    Ok(())
}

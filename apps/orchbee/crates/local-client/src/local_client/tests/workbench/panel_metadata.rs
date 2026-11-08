use super::*;
use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason as Reason;
use orchbee_workbench_protocol::{
    MetadataSource, MetadataTokenValue, PanelMetadataAction, PanelMetadataRequest,
    PanelMetadataResult,
};
#[test]
fn panel_metadata_report_correlation_uncertainty_and_no_replay() -> TestResult {
    for case in 0..9 {
        let request = PanelMetadataRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&id(1)).unwrap(),
            session_id: SessionId::parse(&id(2)).unwrap(),
            action: PanelMetadataAction::Report {
                panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(3)).unwrap(),
                source: MetadataSource::parse("hook").unwrap(),
                expected_binding: orchbee_workbench_protocol::PresentationBinding {
                    worker_id: orchbee_workbench_protocol::WorkerId::parse(&id(4)).unwrap(),
                    terminal_id: orchbee_workbench_protocol::TerminalId::parse(&id(5)).unwrap(),
                },
                patch: orchbee_workbench_protocol::PanelMetadataPatch {
                    title: Some(MetadataTokenValue::parse("title").unwrap()),
                    ..Default::default()
                }
                .into(),
                sequence: None,
                ttl_milliseconds: Some(1000),
            },
        };
        let expected = request.clone();
        let mut reply = serde_json::json!({"protocolVersion":1,"requestId":id(1),"sessionId":id(2),"outcome":{"kind":"reported","panelId":id(3),"workbenchRevision":"5","metadataRevision":"1","disposition":"applied","binding":{"workerId":id(4),"terminalId":id(5)}}});
        match case {
            8 => reply["outcome"]["binding"]["terminalId"] = serde_json::json!(id(99)),
            1 => reply["requestId"] = serde_json::json!(id(90)),
            2 => reply["sessionId"] = serde_json::json!(id(90)),
            3 => reply["outcome"]["panelId"] = serde_json::json!(id(90)),
            4 => {
                reply["outcome"] = serde_json::json!({"kind":"read","workbenchRevision":"5","metadataRevision":"1","panels":[]})
            }
            _ => {}
        }
        let server = FakeServer::start(move |mut stream| {
            handshake::expect_hello(&mut stream)?;
            admit(&mut stream, 60)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::PanelMetadata(expected)
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
                    &WorkbenchResponse::PanelMetadata(
                        serde_json::from_value::<PanelMetadataResult>(reply).unwrap(),
                    ),
                )
                .map_err(io::Error::other)?;
            }
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte)?, 0, "no replay or further frames");
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint())?;
        let early = client
            .panel_metadata(
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
        let result = client.panel_metadata(request, deadline, None);
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

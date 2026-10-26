use super::*;
use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason as Reason;
use orchbee_workbench_protocol::{AgentViewAction, AgentViewRequest, AgentViewResult};
#[test]
fn agent_view_correlation_deadline_and_post_handoff_failure_never_replay() -> TestResult {
    for case in 0..7 {
        let request = AgentViewRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&id(1)).unwrap(),
            session_id: SessionId::parse(&id(2)).unwrap(),
            action: AgentViewAction::Clear { source: None },
        };
        let expected = request.clone();
        let mut reply = serde_json::json!({"protocolVersion":1,"requestId":id(1),"sessionId":id(2),"outcome":{"kind":"available","snapshot":{"revision":"1","spec":null}}});
        match case {
            1 => reply["requestId"] = serde_json::json!(id(9)),
            2 => reply["sessionId"] = serde_json::json!(id(9)),
            3 => {
                reply["outcome"]["snapshot"]
                    .as_object_mut()
                    .unwrap()
                    .remove("spec");
            }
            _ => {}
        }
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 60)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::AgentView(expected)
            );
            match case {
                4 => stream.write_all(&[0; 12])?,
                5 => return Ok(()),
                6 => std::thread::sleep(Duration::from_millis(200)),
                3 => {
                    // A raw malformed body is intentionally rejected by the reader.
                    let bytes = serde_json::to_vec(&{
                        let mut value = reply.clone();
                        value["kind"] = serde_json::json!("agent.view.result");
                        value
                    })
                    .unwrap();
                    let mut frame = b"OBWP".to_vec();
                    frame.extend_from_slice(&1u16.to_be_bytes());
                    frame.extend_from_slice(&48u16.to_be_bytes());
                    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                    frame.extend_from_slice(&bytes);
                    stream.write_all(&frame)?;
                }
                _ => {
                    let result: AgentViewResult = serde_json::from_value(reply).unwrap();
                    write_workbench_response(&mut stream, &WorkbenchResponse::AgentView(result))
                        .map_err(io::Error::other)?;
                }
            }
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte)?, 0, "no mutation replay");
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let early = client
            .agent_view(
                request.clone(),
                client.operation_clock().now() - Duration::from_millis(1),
                None,
            )
            .unwrap_err();
        assert!(!early.submitted);
        let deadline = client.operation_clock().now()
            + if case == 6 {
                Duration::from_millis(50)
            } else {
                SERVER_GUARD_TIMEOUT
            };
        let result = client.agent_view(request, deadline, None);
        if case == 0 {
            assert!(result.is_ok());
            assert!(client.generation().is_some());
        } else {
            let error = result.unwrap_err();
            assert!(error.submitted);
            assert_eq!(
                error.reason,
                match case {
                    1 | 2 => Reason::Correlation,
                    3 | 4 => Reason::Protocol,
                    5 => Reason::Read,
                    _ => Reason::ResponseDeadline,
                }
            );
            assert!(client.generation().is_none());
        }
        drop(client);
        server.finish()?;
    }
    Ok(())
}

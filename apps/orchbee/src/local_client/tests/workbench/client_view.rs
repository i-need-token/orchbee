use super::*;
use crate::local_client::workbench::client_view::ClientViewExchangeError;
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewOutcome, ClientViewRecord,
    ClientViewSequence, WorkbenchClientId, WorkbenchClientViewResult,
};

#[test]
fn view_exchange_correlates_owner_sample_and_session_and_never_replays() -> TestResult {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    for fault in 0..9 {
        let WorkbenchRequest::ClientView(request) =
            orchbee_workbench_protocol::parse_workbench_request(
                &serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap(),
            )
            .unwrap()
        else {
            panic!("view request");
        };
        let ClientViewAction::Synchronize {
            heartbeat,
            view_revision,
            view,
            ..
        } = &request.action
        else {
            panic!("sample");
        };
        let mut record = ClientViewRecord {
            client_id: WorkbenchClientId::parse(&id(60)).unwrap(),
            heartbeat: *heartbeat,
            view_revision: *view_revision,
            view: *view.clone(),
            availability: ClientViewAvailability::Current,
        };
        match fault {
            3 => record.client_id = WorkbenchClientId::parse(&id(61)).unwrap(),
            4 => record.heartbeat = ClientViewSequence::new(99).unwrap(),
            5 => record.view.workbench_revision = WorkbenchRevision::new(99),
            _ => {}
        }
        let mut reply = WorkbenchClientViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            outcome: ClientViewOutcome::Synchronized {
                client: Box::new(record),
                control: None,
            },
        };
        if fault == 1 {
            reply.request_id = WorkbenchRequestId::parse(&id(90)).unwrap();
        }
        if fault == 2 {
            reply.session_id = SessionId::parse(&id(90)).unwrap();
        }
        let expected = request.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 60)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::ClientView(expected)
            );
            if fault == 7 {
                return Ok(());
            }
            if fault == 8 {
                stream.write_all(b"OBWP")?;
            } else if fault == 6 {
                stream.write_all(&[0; 12])?;
            } else {
                write_workbench_response(&mut stream, &WorkbenchResponse::ClientView(reply))
                    .map_err(io::Error::other)?;
            }
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte)?,
                0,
                "one exchange, no implicit replay"
            );
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let deadline = client.operation_clock().now()
            + if fault == 8 {
                Duration::from_millis(200)
            } else {
                SERVER_GUARD_TIMEOUT
            };
        let result = client.exchange_client_view(request, deadline, None);
        if fault == 0 {
            assert!(result.is_ok());
            assert!(client.generation().is_some());
        } else {
            assert_eq!(
                result,
                Err(if fault == 7 {
                    ClientViewExchangeError::Transport
                } else if fault == 8 {
                    ClientViewExchangeError::Timeout
                } else {
                    ClientViewExchangeError::Protocol
                })
            );
            assert!(client.generation().is_none());
        }
        drop(client);
        server.finish()?;
    }
    Ok(())
}

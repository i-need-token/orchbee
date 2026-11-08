use super::*;
use crate::local_client::workbench::worktree_query::WorktreeQueryError;
use orchbee_workbench_protocol::{
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome as O, WorktreeSource,
};

#[test]
fn query_correlates_session_source_and_request_without_replay() -> TestResult {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../../../schemas/workbench/v1/workbench-worktree-query-conformance-v1.json"
    ))
    .unwrap();
    let document = &corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["document"]["outcome"]["kind"] == "found")
        .unwrap()["document"];
    for case in 0..6 {
        let WorkbenchResponse::WorktreeQuery(mut reply) =
            orchbee_workbench_protocol::parse_workbench_response(
                &serde_json::to_vec(document).unwrap(),
            )
            .unwrap()
        else {
            panic!("query")
        };
        let O::Found {
            session_id, source, ..
        } = &mut reply.outcome
        else {
            unreachable!()
        };
        source.workspace_id = None;
        let request = WorkbenchWorktreeQuery {
            protocol_version: ProtocolVersion,
            request_id: reply.request_id.clone(),
            session_id: session_id.clone(),
            source: WorktreeSource::Directory {
                path: source.requested_path.clone(),
            },
            trust_repository: false,
        };
        match case {
            1 => reply.request_id = WorkbenchRequestId::parse(&id(90)).unwrap(),
            2 => *session_id = SessionId::parse(&id(90)).unwrap(),
            3 => {
                source.requested_path =
                    orchbee_workbench_protocol::WorkbenchDirectoryPath::parse("C:/other").unwrap()
            }
            _ => {}
        }
        let expected = request.clone();
        let server = FakeServer::start(move |mut stream| {
            handshake::expect_hello(&mut stream)?;
            admit(&mut stream, 60)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::WorktreeQuery(expected)
            );
            if case == 5 {
                return Ok(());
            }
            if case == 4 {
                stream.write_all(&[0; 12])?;
            } else {
                write_workbench_response(&mut stream, &WorkbenchResponse::WorktreeQuery(reply))
                    .map_err(io::Error::other)?;
            }
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte)?, 0, "one query, never replayed");
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint())?;
        let result = client.query_worktrees(
            request,
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
            None,
        );
        if case == 0 {
            assert!(result.is_ok());
            assert!(client.generation().is_some());
        } else {
            assert_eq!(
                result,
                Err(if case == 5 {
                    WorktreeQueryError::Transport
                } else {
                    WorktreeQueryError::Protocol
                })
            );
            assert!(client.generation().is_none());
        }
        drop(client);
        server.finish()?;
    }
    Ok(())
}

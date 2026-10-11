use super::*;
use crate::local_client::test_server::FakeServer;
use crate::terminal_session::agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome};
use orchbee_agent_observation_protocol::AgentReadRequest;
use orchbee_agent_observation_protocol::{
    AgentRequest, AgentResult, read_agent_request, write_agent_result,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    num::NonZeroU64,
    sync::mpsc,
    time::Duration,
};

fn token() -> String {
    format!("v1.{}.{}", "A".repeat(59), "A".repeat(43))
}

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
fn attention_lane_aligns_a_moving_list_and_recovers_foreign_boot_cursor() {
    for reset in [false, true] {
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            accept(&mut stream)?;
            if index == 0 { return Ok(()); }
            let checkpoint = json!({"kind":"current", "cursor":token(), "revision":0});
            let listed = json!({"kind":"listed","items":[],"nextCursor":null,"revision":1});
            let page = json!({"kind":"changes","cursor":token(),"observedThroughRevision":1,"runtimeRevision":1,"hasMore":false,"changes":[{
                "revision":1,"transition":"session-observed","session":{"lifecycle":"live","observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":1,"source":"native"},"parentSessionId":null,"pendingRequestCount":0,"providerId":"test","sessionId":"agent-1","status":{"kind":"idle"}}
            }]});
            let mut outcomes: Vec<(&str, Value)> = vec![];
            if reset { outcomes.push(("changes",json!({"kind":"rejected","reason":"cursor-invalid"}))); }
            outcomes.extend([("changes",checkpoint), ("list",listed.clone()), ("changes",page), ("list",listed)]);
            for (kind, outcome) in outcomes {
                let request = read_agent_request(&mut stream).map_err(io::Error::other)?;
                let (request_id, actual, response_kind) = match request {
                    AgentRequest::Changes(request) => (request.request_id().clone(), "changes", "agent.session.changes.result"),
                    AgentRequest::Read(AgentReadRequest::List { request_id, .. }) => (request_id,"list","agent.session.list.result"),
                    _ => panic!("unexpected request"),
                };
                assert_eq!(kind, actual);
                let document = json!({"kind":response_kind,"requestId":request_id,"result":{"outcome":outcome}});
                let result = if kind == "changes" { AgentResult::Changes(serde_json::from_value(document).unwrap()) }
                    else { AgentResult::Read(serde_json::from_value(document).unwrap()) };
                write_agent_result(&mut stream, &result).map_err(io::Error::other)?;
            }
            assert_eq!(stream.read(&mut [0])?, 0);
            Ok(())
        }).unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let (origin, clock) = (client.origin(), client.operation_clock());
        drop(client);
        let lane = AgentReadLane::spawn(origin, clock.clone()).unwrap();
        assert!(lane.send(LaneCommand::Attention {
            epoch: 7,
            cursor: reset.then(|| AgentChangesCursor::parse(&token()).unwrap()),
            revision: reset.then_some(40),
            deadline: clock.now() + Duration::from_secs(5)
        }));
        let LaneOutcome::Attention { epoch, result } =
            lane.outcomes.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("attention outcome")
        };
        let round = result.unwrap();
        assert_eq!(epoch, 7);
        assert_eq!(round.revision, 1);
        assert_eq!(round.changes.len(), 1);
        assert_eq!(round.reset, reset);
        assert_eq!(round.prior_revision, None);
        lane.close();
        server.finish().unwrap();
    }
}

#[test]
fn attention_lane_drop_interrupts_and_joins_blocked_exchange() {
    let (reached_tx, reached_rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        accept(&mut stream)?;
        if index == 1 {
            assert!(matches!(
                read_agent_request(&mut stream).map_err(io::Error::other)?,
                AgentRequest::Changes(_)
            ));
            reached_tx.send(()).unwrap();
            assert_eq!(stream.read(&mut [0])?, 0);
        }
        Ok(())
    })
    .unwrap();
    let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
    let (origin, clock) = (client.origin(), client.operation_clock());
    drop(client);
    let lane = AgentReadLane::spawn(origin, clock.clone()).unwrap();
    assert!(lane.send(LaneCommand::Attention {
        epoch: 1,
        cursor: None,
        revision: None,
        deadline: clock.now() + Duration::from_secs(5)
    }));
    reached_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    drop(lane);
    server.finish().unwrap();
}

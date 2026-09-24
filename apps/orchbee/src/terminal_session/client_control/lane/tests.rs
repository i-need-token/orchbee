use super::*;
use crate::local_client::{LocalClient, test_server::FakeServer};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    WorkbenchRequest, parse_workbench_request, read_workbench_request,
};
use std::{io::Read, num::NonZeroU64, sync::mpsc};

#[test]
fn closing_the_lane_wakes_and_joins_blocked_admission_and_exchange() {
    for during_admission in [false, true] {
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            assert!(matches!(
                read_message(&mut stream).map_err(io::Error::other)?,
                LocalClientMessage::ClientHello(_)
            ));
            if index == 0 || !during_admission {
                write_message(
                    &mut stream,
                    &LocalClientMessage::ClientAccepted {
                        generation: NonZeroU64::new(u64::try_from(index + 1).unwrap()).unwrap(),
                    },
                )
                .map_err(io::Error::other)?;
            }
            if index == 1 {
                if !during_admission {
                    assert!(matches!(
                        read_workbench_request(&mut stream).map_err(io::Error::other)?,
                        WorkbenchRequest::ClientView(_)
                    ));
                }
                reached_tx.send(()).unwrap();
                let mut byte = [0];
                assert_eq!(
                    stream.read(&mut byte)?,
                    0,
                    "lane shutdown closes its connection and wakes blocked IO"
                );
            }
            Ok(())
        })
        .unwrap();
        let client = LocalClient::connect(server.endpoint(), "test-cohort").unwrap();
        let origin = client.origin();
        let clock = client.operation_clock();
        drop(client);
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
        ))
        .unwrap();
        let WorkbenchRequest::ClientView(request) =
            parse_workbench_request(&serde_json::to_vec(&corpus["valid"][0]["document"]).unwrap())
                .unwrap()
        else {
            panic!("view request");
        };
        let ClientViewAction::Synchronize {
            heartbeat,
            view_revision,
            view,
            ..
        } = request.action
        else {
            panic!("sample");
        };
        let mut lane = ClientControlLane::spawn(origin, clock, request.site_id).unwrap();
        assert!(lane.publish(Sample {
            heartbeat,
            revision: view_revision,
            view: *view
        }));
        reached_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        lane.close();
        assert!(lane.task.is_none());
        lane.close();
        server.finish().unwrap();
    }
}

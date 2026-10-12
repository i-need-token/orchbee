use super::*;
use orchbee_local_client_protocol::{
    ClientHello, LocalClientMessage, RejectionReason, SiteStopRequest, SiteStopStatus, StopAction,
    StopId, read_message, write_message,
};
use std::io::Read;

fn exchange(endpoint: &Path, request: LocalClientMessage) -> LocalClientMessage {
    let mut client = UnixStream::connect(endpoint).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    client
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write_message(&mut client, &request).unwrap();
    read_message(&mut client).unwrap()
}

#[test]
fn control_fences_cohort_freezes_new_hellos_and_joins_idle_and_partial_clients() {
    let endpoint = crate::tests::endpoint();
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        crate::worker_registry::WorkerRegistry::new(),
    ))));
    let agents = Arc::new(
        AgentService::open(crate::agent_service::AgentPolicy {
            read: true,
            manage: true,
        })
        .unwrap(),
    );
    let shutdown = Arc::new(SiteShutdown::default());
    let mut service = start(&endpoint, "test", terminals, agents, Arc::clone(&shutdown)).unwrap();
    let mut admitted = UnixStream::connect(&endpoint).unwrap();
    admitted
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write_message(
        &mut admitted,
        &LocalClientMessage::ClientHello(ClientHello::new("test").unwrap()),
    )
    .unwrap();
    assert!(matches!(
        read_message(&mut admitted).unwrap(),
        LocalClientMessage::ClientAccepted { .. }
    ));
    let request = SiteStopRequest {
        hello: ClientHello::new("foreign").unwrap(),
        id: StopId::new([1; 16]).unwrap(),
        action: StopAction::Start,
    };
    assert_eq!(
        exchange(
            &endpoint,
            LocalClientMessage::SiteStopRequest(request.clone())
        ),
        LocalClientMessage::Rejected {
            reason: RejectionReason::IncompatibleCohort
        }
    );
    assert!(shutdown.enter().is_some());
    let request = SiteStopRequest {
        hello: ClientHello::new("test").unwrap(),
        ..request
    };
    assert!(
        matches!(exchange(&endpoint, LocalClientMessage::SiteStopRequest(request)), LocalClientMessage::SiteStopResult(result) if matches!(result.status, SiteStopStatus::Pending { .. }))
    );
    assert_eq!(
        exchange(
            &endpoint,
            LocalClientMessage::ClientHello(ClientHello::new("test").unwrap())
        ),
        LocalClientMessage::Rejected {
            reason: RejectionReason::Stopping
        }
    );
    let mut partial = UnixStream::connect(&endpoint).unwrap();
    partial
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    service
        .stop_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
    let mut byte = [0];
    assert_eq!(admitted.read(&mut byte).unwrap(), 0);
    assert_eq!(partial.read(&mut byte).unwrap(), 0);
    assert!(service.control.streams.lock().unwrap().is_empty());
    assert!(service.thread.is_none());
}

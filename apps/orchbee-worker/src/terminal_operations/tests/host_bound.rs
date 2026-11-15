//! A Host that stays silent past the exchange bound ends the connection, and the exit names the
//! bound as its reason. The bound is injected, so the witness waits a fraction of a second.
use super::super::*;
use super::shell::{WORKER, connected_host, record, timeout};
use crate::diagnostic_events::{ExitCause, Peer, Reason, Site};
use crate::host_connection::Pacing;
use orchbee_capability_node_protocol::{self as node, *};
use orchbee_local_ipc::LocalStream;
use std::{
    thread,
    time::{Duration, Instant},
};

#[test]
fn a_host_silent_past_the_bound_ends_the_connection_with_reason_deadline() {
    const BOUND: Duration = Duration::from_millis(300);
    let (mut host, drain, mut host_peer) = connected_host();
    host.pacing = Pacing::bounded(BOUND, Duration::from_secs(1));
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        node::write_message(
            &mut peer,
            &CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
                request_id: OperationId::from_bytes([1; 16]).unwrap(),
                worker_id: node::WorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
                worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
                attachment_id: OperationId::from_bytes([90; 16]).unwrap(),
                action: TerminalViewerAction::Attach {
                    host_epoch: node::HostEpoch::from_bytes([4; 16]).unwrap(),
                    terminal_id: HostTerminalId::from_bytes([91; 16]).unwrap(),
                },
            }),
        )
        .unwrap();
        // The Host reads the request and never answers; its connection stays open.
        let _attach = record(&mut host_peer);
        (peer, host_peer)
    });
    let started = Instant::now();
    let result = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        &SystemShell::resolve(),
    );
    let elapsed = started.elapsed();
    let _peers = driver.join().unwrap();
    let error = result.expect_err("a silent Host ends the connection");
    let cause = error
        .downcast_ref::<ExitCause>()
        .expect("a classified exit");
    assert_eq!(
        (cause.site, cause.reason, cause.peer),
        (Site::Viewer, Reason::Deadline, Peer::Host)
    );
    let (waited, budget) = cause.waited.expect("the waited time");
    assert!(budget <= BOUND && waited >= budget, "{waited:?} {budget:?}");
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
}

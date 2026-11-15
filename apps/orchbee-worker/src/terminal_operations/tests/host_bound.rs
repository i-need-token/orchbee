//! A Host that stays silent past the exchange bound ends the connection, and the exit names the
//! bound as its reason. A Host that answers every exchange inside the bound is never lost, however
//! its answers add up: the request bound only limits which exchanges start, and the cleanup it
//! leaves waits for the next request. The bounds are injected, so each witness waits a fraction of
//! a second.
use super::super::*;
use super::shell::{WORKER, connected_host, record, reply, timeout};
use crate::diagnostic_events::{ExitCause, Peer, Reason, Site};
use crate::host_connection::Pacing;
use orchbee_capability_node_protocol::{self as node, *};
use orchbee_local_ipc::LocalStream;
use orchbee_terminal_protocol::{
    AttachmentControl, decode_attachment_control, decode_transport_envelope,
    encode_attachment_control,
};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
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

#[test]
fn a_slow_host_that_answers_every_exchange_is_served_across_requests_past_the_request_bound() {
    // Every answer comes inside the exchange bound, and the four the cleanup needs add up past
    // the request bound: two fit in one request, and the next request takes the rest.
    const EXCHANGE: Duration = Duration::from_millis(400);
    const REQUEST: Duration = Duration::from_millis(900);
    const ANSWER: Duration = Duration::from_millis(300);
    const CLOSED: [u8; 4] = [1, 2, 3, 4];
    let (mut host, drain, mut host_peer) = connected_host();
    host.pacing = Pacing::bounded(EXCHANGE, REQUEST);
    // Viewers closed on saturation wait for cleanup to detach them.
    for id in CLOSED {
        let viewer = OperationId::from_bytes([id; 16]).unwrap();
        host.viewers
            .borrow_mut()
            .admit(&TerminalViewerRequest {
                request_id: OperationId::from_bytes([100 + id; 16]).unwrap(),
                worker_id: node::WorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
                worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
                attachment_id: viewer,
                action: TerminalViewerAction::Attach {
                    host_epoch: node::HostEpoch::from_bytes([4; 16]).unwrap(),
                    terminal_id: HostTerminalId::from_bytes([91; 16]).unwrap(),
                },
            })
            .unwrap();
        host.viewers.borrow_mut().close(viewer);
    }
    let answered = Arc::new(AtomicUsize::new(0));
    let host_script = {
        let answered = Arc::clone(&answered);
        thread::spawn(move || {
            let mut detached = Vec::new();
            while detached.len() < CLOSED.len() {
                let request = record(&mut host_peer);
                let envelope = decode_transport_envelope(&request).unwrap();
                let AttachmentControl::DetachTerminal(target) =
                    decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                        .unwrap()
                else {
                    panic!("cleanup sends only Detaches");
                };
                detached.push(target.attachment_id.as_bytes()[0]);
                thread::sleep(ANSWER);
                answered.fetch_add(1, Ordering::AcqRel);
                let accepted =
                    encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(target))
                        .unwrap();
                reply(&mut host_peer, &request, accepted.kind, &accepted.payload);
            }
            (host_peer, detached)
        })
    };
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        // Requests with no Host work of their own, each followed by cleanup, until cleanup is
        // done; dropping the Server end afterwards ends the loop.
        let mut served = 0_u8;
        while answered.load(Ordering::Acquire) < CLOSED.len() && served < 8 {
            node::write_message(
                &mut peer,
                &CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
                    request_id: OperationId::from_bytes([10 + served; 16]).unwrap(),
                    worker_id: node::WorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
                    worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
                    attachment_id: OperationId::from_bytes([80; 16]).unwrap(),
                    action: TerminalViewerAction::Poll,
                }),
            )
            .unwrap();
            // The closed viewers' ready hints may come first.
            loop {
                match node::read_message(&mut peer).unwrap() {
                    CapabilityNodeMessage::TerminalViewerResult(_) => break,
                    CapabilityNodeMessage::TerminalViewerRecordReady(_) => {}
                    other => panic!("{other:?}"),
                }
            }
            served += 1;
        }
        served
    });
    let result = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        &SystemShell::resolve(),
    );
    let served = driver.join().unwrap();
    let (mut host_peer, mut detached) = host_script.join().unwrap();
    let error = result.expect_err("closing the Server end ends the connection");
    let cause = error
        .downcast_ref::<ExitCause>()
        .expect("a classified exit");
    assert_eq!(cause.peer, Peer::Server, "{cause:?}");
    assert_eq!(host.pacing.failure(), None);
    assert!(
        served >= 2,
        "cleanup past the request bound waits: {served}"
    );
    detached.sort_unstable();
    assert_eq!(detached, CLOSED, "each closed viewer is detached once");
    host_peer.set_nonblocking(true).unwrap();
    assert_eq!(
        host_peer.read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

//! A Host message this Worker cannot use fails only the scope it names. Driven through the real
//! serving loop, a flow break closes only its viewer and an invalid answer refuses only its own
//! request; the Worker keeps serving another terminal until the Server leaves, and the record
//! names the site and the anomaly.
use super::super::*;
use super::shell::{WORKER, connected_host, record, reply, timeout};
use crate::diagnostic_events::{ExitCause, Peer, take_noted};
use orchbee_capability_node_protocol::{self as node, *};
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::{LogField, LogValue};
use orchbee_terminal_protocol::{
    self as terminal, AttachmentControl, decode_attachment_control, decode_transport_envelope,
    encode_attachment_control,
};
use std::{
    io::{Read, Write},
    thread,
};

const A: u8 = 1;
const B: u8 = 2;
const EPOCH: [u8; 16] = [4; 16];

#[test]
fn a_flow_break_closes_only_its_viewer_while_the_neighbour_is_served() {
    let (mut host, drain, mut host_peer) = connected_host();
    admit(&host, &[(A, 91), (B, 92)]);
    let host_script = thread::spawn(move || {
        // A replayed bell breaks A's sequence; B's first bell is in order.
        for event in [bell(A, 2), bell(A, 2), bell(B, 1)] {
            host_peer.write_all(&event).unwrap();
        }
        // The cleanup after the next request detaches the closed viewer.
        let detached = answer_detach(&mut host_peer, |target| target);
        (host_peer, detached)
    });
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        // B's hint follows A's events, so all three have been routed.
        loop {
            match node::read_message(&mut peer).unwrap() {
                CapabilityNodeMessage::TerminalViewerRecordReady(ready)
                    if ready.attachment_id.as_bytes() == [B; 16] =>
                {
                    break;
                }
                CapabilityNodeMessage::TerminalViewerRecordReady(_) => {}
                other => panic!("{other:?}"),
            }
        }
        let closed = serve_viewer(&mut peer, viewer(10, A, TerminalViewerAction::Poll));
        let neighbour = serve_viewer(&mut peer, viewer(11, B, TerminalViewerAction::Poll));
        (closed, neighbour)
    });
    let result = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        &SystemShell::resolve(),
    );
    let noted = take_noted();
    let (closed, neighbour) = driver.join().unwrap();
    let (host_peer, detached) = host_script.join().unwrap();
    assert_served_until_the_server_left(result, &host_peer);
    assert_eq!(
        closed,
        TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
    );
    assert!(
        matches!(neighbour, TerminalViewerOutcome::Record(_)),
        "{neighbour:?}"
    );
    assert_eq!(detached, A);
    assert!(contained(&noted, "viewer", "flow"), "{noted:?}");
}

#[test]
fn an_invalid_answer_refuses_only_its_request_while_another_terminal_is_served() {
    let (mut host, drain, mut host_peer) = connected_host();
    admit(&host, &[(A, 91), (B, 92)]);
    let host_script = thread::spawn(move || {
        // An intact acceptance that names B answers A's Detach.
        let refused = answer_detach(&mut host_peer, |_| target(B));
        // The cleanup after that request detaches A, which the refusal closed; B's own Detach
        // follows.
        let cleaned = answer_detach(&mut host_peer, |target| target);
        let served = answer_detach(&mut host_peer, |target| target);
        (host_peer, [refused, cleaned, served])
    });
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        let refused = serve_viewer(&mut peer, viewer(10, A, TerminalViewerAction::Detach));
        let served = serve_viewer(&mut peer, viewer(11, B, TerminalViewerAction::Detach));
        (refused, served)
    });
    let result = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        &SystemShell::resolve(),
    );
    let noted = take_noted();
    let (refused, served) = driver.join().unwrap();
    let (host_peer, detached) = host_script.join().unwrap();
    assert_served_until_the_server_left(result, &host_peer);
    assert_eq!(
        refused,
        TerminalViewerOutcome::Rejected(ViewerRejection::HostRejected)
    );
    assert_eq!(served, TerminalViewerOutcome::Detached);
    assert_eq!(detached, [A, A, B]);
    assert!(contained(&noted, "viewer", "answer"), "{noted:?}");
}

/// The loop ended only when the Server closed its end, and sent the Host nothing unanswered.
fn assert_served_until_the_server_left(
    result: Result<(), Box<dyn std::error::Error>>,
    host_peer: &LocalStream,
) {
    let error = result.expect_err("closing the Server end ends the connection");
    let cause = error
        .downcast_ref::<ExitCause>()
        .expect("a classified exit");
    assert_eq!(cause.peer, Peer::Server, "{cause:?}");
    host_peer.set_nonblocking(true).unwrap();
    assert_eq!(
        (&*host_peer).read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

/// Whether a `host-unexpected` record names `site` and `anomaly`.
fn contained(
    noted: &[(&'static str, Vec<LogField>)],
    site: &'static str,
    anomaly: &'static str,
) -> bool {
    noted.iter().any(|(event, fields)| {
        *event == "host-unexpected"
            && fields.contains(&("site", LogValue::Token(site)))
            && fields.contains(&("anomaly", LogValue::Token(anomaly)))
    })
}

/// Admits viewers as their Attach would have, each on its own terminal.
fn admit(host: &HostBrokerConnection, viewers: &[(u8, u8)]) {
    for &(attachment, terminal) in viewers {
        let attach = TerminalViewerAction::Attach {
            host_epoch: node::HostEpoch::from_bytes(EPOCH).unwrap(),
            terminal_id: HostTerminalId::from_bytes([terminal; 16]).unwrap(),
        };
        host.viewers
            .borrow_mut()
            .admit(&viewer(100 + attachment, attachment, attach))
            .unwrap();
    }
}

fn viewer(request: u8, attachment: u8, action: TerminalViewerAction) -> TerminalViewerRequest {
    TerminalViewerRequest {
        request_id: OperationId::from_bytes([request; 16]).unwrap(),
        worker_id: node::WorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        attachment_id: OperationId::from_bytes([attachment; 16]).unwrap(),
        action,
    }
}

/// The Server's side of one viewer request: its result, past any readiness hints.
fn serve_viewer(peer: &mut LocalStream, request: TerminalViewerRequest) -> TerminalViewerOutcome {
    node::write_message(peer, &CapabilityNodeMessage::TerminalViewerRequest(request)).unwrap();
    loop {
        match node::read_message(peer).unwrap() {
            CapabilityNodeMessage::TerminalViewerResult(result) => return result.outcome,
            CapabilityNodeMessage::TerminalViewerRecordReady(_) => {}
            other => panic!("{other:?}"),
        }
    }
}

fn target(attachment: u8) -> terminal::AttachmentTarget {
    terminal::AttachmentTarget {
        attachment_id: terminal::AttachmentId::from_bytes([attachment; 16]).unwrap(),
    }
}

/// The Host's side of one Detach: it answers with the acceptance of `accepted(target)` and returns
/// the attachment the Detach named.
fn answer_detach(
    host_peer: &mut LocalStream,
    accepted: impl FnOnce(terminal::AttachmentTarget) -> terminal::AttachmentTarget,
) -> u8 {
    let request = record(host_peer);
    let envelope = decode_transport_envelope(&request).unwrap();
    let AttachmentControl::DetachTerminal(detach) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload).unwrap()
    else {
        panic!("the Worker sends the Host only Detaches here");
    };
    let named = detach.attachment_id.as_bytes()[0];
    let answer =
        encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(accepted(detach)))
            .unwrap();
    reply(host_peer, &request, answer.kind, &answer.payload);
    named
}

fn bell(attachment: u8, sequence: u64) -> Vec<u8> {
    let event = terminal::encode_terminal_effect_event(&terminal::TerminalEffectEvent::Bell(
        terminal::TerminalBell {
            attachment_id: terminal::AttachmentId::from_bytes([attachment; 16]).unwrap(),
            sequence: NonZeroU64::new(sequence).unwrap(),
        },
    ))
    .unwrap();
    terminal::encode_transport_envelope(&terminal::Envelope {
        correlation_id: None,
        host_epoch: Some(terminal::HostEpoch::from_bytes(EPOCH).unwrap()),
        kind: event.kind,
        optional_message: false,
        payload: &event.payload,
        payload_encoding: terminal::PayloadEncoding::ControlJson,
        protocol: terminal::CURRENT_PROTOCOL_VERSION,
        role: terminal::MessageRole::Event,
    })
    .unwrap()
}

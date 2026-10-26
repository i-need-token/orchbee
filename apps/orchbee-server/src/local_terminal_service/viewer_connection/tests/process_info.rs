use super::*;
use orchbee_terminal_protocol::{
    ConnectionControl, ProcessInfoControl, TerminalId, TerminalProcessInfo, TypedFailure,
    decode_connection_control, encode_connection_control, encode_process_info_control,
};

#[derive(Clone, Copy, Debug)]
enum Reply {
    Observed,
    Unavailable,
    WrongEpoch,
    WrongCorrelation,
    WrongAttachment,
    WrongTerminal,
    Malformed,
    Empty,
}

fn answer(request: WorkerRequest, reply: Reply) -> WorkerOutcome {
    if matches!(reply, Reply::Empty) {
        return WorkerOutcome::Empty;
    }
    let mut epoch = HostEpoch::from_bytes([3; 16]).unwrap();
    let mut correlation = CorrelationId::from_bytes(request.request_id.as_bytes()).unwrap();
    let mut info = TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes(request.attachment_id.as_bytes()).unwrap(),
        terminal_id: TerminalId::from_bytes([4; 16]).unwrap(),
        root_pid: 41,
        processes: vec![],
    };
    match reply {
        Reply::WrongEpoch => epoch = HostEpoch::from_bytes([90; 16]).unwrap(),
        Reply::WrongCorrelation => correlation = CorrelationId::from_bytes([90; 16]).unwrap(),
        Reply::WrongAttachment => info.attachment_id = AttachmentId::from_bytes([90; 16]).unwrap(),
        Reply::WrongTerminal => info.terminal_id = TerminalId::from_bytes([90; 16]).unwrap(),
        _ => {}
    }
    let (kind, payload) = if matches!(reply, Reply::Unavailable) {
        let value = encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
            code: "process-sample-unavailable".into(),
            message: "OS observation failed".into(),
            retryable: false,
        }))
        .unwrap();
        (value.kind, value.payload)
    } else {
        let value = encode_process_info_control(&ProcessInfoControl::ProcessInfo(info)).unwrap();
        (
            value.kind,
            if matches!(reply, Reply::Malformed) {
                b"{}".to_vec()
            } else {
                value.payload
            },
        )
    };
    WorkerOutcome::Record(
        encode_transport_envelope(&Envelope {
            correlation_id: Some(correlation),
            host_epoch: Some(epoch),
            kind,
            optional_message: false,
            payload: &payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Response,
        })
        .unwrap(),
    )
}

fn process_connection(reply: Reply) -> ScriptedConnection {
    let service = service();
    let carrier = create_worker_carrier(WORKER_ID, NonZeroU64::MIN);
    let handle = carrier.request_handle();
    assert!(matches!(
        service.workers.lock().unwrap().admit(WORKER_ID),
        crate::worker_registry::WorkerAdmission::Accepted { .. }
    ));
    let owner =
        WorkerRegistry::register_carrier(&service.workers, NonZeroU64::MIN, carrier).unwrap();
    let (server, mut peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let stop = server.try_clone().unwrap();
    let owner = thread::spawn(move || owner.run(server));
    let peer = thread::spawn(move || {
        while let Ok(CapabilityNodeMessage::TerminalViewerRequest(request)) =
            read_message(&mut peer)
        {
            let outcome = match request.action {
                WorkerAction::ReadProcessInfo => answer(request, reply),
                WorkerAction::Detach => WorkerOutcome::Detached,
                _ => panic!("process query must not send projection feedback or another action"),
            };
            if write_message(
                &mut peer,
                &CapabilityNodeMessage::TerminalViewerResult(WorkerResult {
                    request_id: request.request_id,
                    attachment_id: request.attachment_id,
                    outcome,
                }),
            )
            .is_err()
            {
                break;
            }
        }
    });
    (service, handle, owner, peer, stop)
}

#[test]
fn process_reads_keep_typed_failures_and_reject_wrong_routes_without_projection_feedback() {
    for reply in [
        Reply::Observed,
        Reply::Unavailable,
        Reply::WrongEpoch,
        Reply::WrongCorrelation,
        Reply::WrongAttachment,
        Reply::WrongTerminal,
        Reply::Malformed,
        Reply::Empty,
    ] {
        let (service, handle, owner, peer, stop) = process_connection(reply);
        let mut connection = service.client_connection();
        assert!(matches!(
            connection.publish_attach(viewer(&service, handle), attached_result()),
            Outcome::Attached { .. }
        ));
        let result = execute_viewer(
            &mut connection,
            "88888888888888888888888888888884",
            Action::ReadProcessInfo {
                attachment_id: poll().attachment_id().unwrap().clone(),
            },
        );
        let accepted = matches!(reply, Reply::Observed | Reply::Unavailable);
        assert_eq!(
            matches!(result.outcome(), Outcome::Record(_)),
            accepted,
            "{reply:?}"
        );
        assert_eq!(
            connection.viewers.iter().any(Option::is_some),
            accepted,
            "{reply:?}"
        );
        if matches!(reply, Reply::Unavailable) {
            let Outcome::Record(record) = result.outcome() else {
                panic!("typed failure lost")
            };
            let envelope = orchbee_terminal_protocol::decode_transport_envelope(record).unwrap();
            assert!(
                matches!(decode_connection_control(envelope.kind, envelope.role, envelope.payload),
                Ok(ConnectionControl::TypedFailure(failure)) if failure.code == "process-sample-unavailable")
            );
        }
        drop(connection);
        let _ = stop.shutdown(std::net::Shutdown::Both);
        let _ = owner.join();
        peer.join().unwrap();
    }
}

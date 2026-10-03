//! Pane effects traverse the actual Server/Worker carrier without spending frame feedback.
use super::*;
use orchbee_terminal_protocol::{
    TerminalBell, TerminalClipboardWriteRequest, TerminalEffectEvent, encode_terminal_effect_event,
};

fn effect_record(clipboard: bool, attachment: [u8; 16], epoch: [u8; 16]) -> Vec<u8> {
    let attachment_id = AttachmentId::from_bytes(attachment).unwrap();
    let event = if clipboard {
        TerminalEffectEvent::ClipboardWriteRequest(TerminalClipboardWriteRequest {
            attachment_id,
            sequence: NonZeroU64::MIN,
            text: "中文e\u{301}\tline\r\nnext".into(),
        })
    } else {
        TerminalEffectEvent::Bell(TerminalBell {
            attachment_id,
            sequence: NonZeroU64::MIN,
        })
    };
    let encoded = encode_terminal_effect_event(&event).unwrap();
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::from_bytes(epoch).unwrap()),
        kind: encoded.kind,
        optional_message: false,
        payload: &encoded.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap()
}

#[test]
fn program_poll_effects_preserve_bytes_order_and_the_pending_frame_flight() {
    let bell = effect_record(false, [2; 16], [3; 16]);
    let clipboard = effect_record(true, [2; 16], [3; 16]);
    let (service, handle, owner, peer, stop) = scripted_session(VecDeque::from([
        WorkerOutcome::Record(frame_record(1)),
        WorkerOutcome::Record(bell.clone()),
        WorkerOutcome::Record(clipboard.clone()),
        WorkerOutcome::FeedbackAccepted,
    ]));
    let mut session = service.session();
    assert!(matches!(
        session.publish_attach(viewer(&service, handle), attached_result()),
        Outcome::Attached { .. }
    ));
    assert!(matches!(
        execute_viewer(&mut session, "88888888888888888888888888888881", poll()).outcome(),
        Outcome::Record(_)
    ));
    for (id, expected) in [
        ("88888888888888888888888888888882", bell),
        ("88888888888888888888888888888883", clipboard),
    ] {
        let result = execute_viewer(&mut session, id, poll());
        let Outcome::Record(record) = result.outcome() else {
            panic!("a legal program effect must reach Client without retiring its viewer")
        };
        assert_eq!(*record, expected);
    }
    let feedback = execute_viewer(
        &mut session,
        "88888888888888888888888888888884",
        Action::FrameApplied {
            attachment_id: poll().attachment_id().unwrap().clone(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
        },
    );
    assert_eq!(feedback.outcome(), &Outcome::FeedbackAccepted {});
    drop(session);
    let _ = stop.shutdown(std::net::Shutdown::Both);
    let _ = owner.join();
    peer.join().unwrap();
}

fn repack(
    record: &[u8],
    role: MessageRole,
    correlated: bool,
    optional: bool,
    encoding: PayloadEncoding,
    zero_sequence: bool,
) -> Vec<u8> {
    let envelope = orchbee_terminal_protocol::decode_transport_envelope(record).unwrap();
    let payload = if zero_sequence {
        String::from_utf8(envelope.payload.to_vec())
            .unwrap()
            .replace("\"sequence\":\"1\"", "\"sequence\":\"0\"")
            .into_bytes()
    } else {
        envelope.payload.to_vec()
    };
    // The transport encoder rightly refuses malformed kind/role/encoding pairs.
    // Mutate the public OBTP header only after creating a valid event envelope.
    let mut bytes = encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: envelope.host_epoch,
        kind: envelope.kind,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: envelope.protocol,
        role: MessageRole::Event,
    })
    .unwrap();
    let flags: u16 = (if role == MessageRole::Response { 2 } else { 4 }) | u16::from(optional);
    bytes[6..8].copy_from_slice(&flags.to_le_bytes());
    if encoding == PayloadEncoding::RawBytes {
        bytes[22] = 2;
    }
    if correlated {
        bytes[24..40].fill(8);
    }
    bytes
}

#[test]
fn wrong_program_effect_scope_envelope_or_action_still_retires_the_viewer() {
    for clipboard in [false, true] {
        let valid = effect_record(clipboard, [2; 16], [3; 16]);
        let invalid = [
            effect_record(clipboard, [9; 16], [3; 16]),
            effect_record(clipboard, [2; 16], [9; 16]),
            repack(
                &valid,
                MessageRole::Response,
                true,
                false,
                PayloadEncoding::ControlJson,
                false,
            ),
            repack(
                &valid,
                MessageRole::Event,
                false,
                true,
                PayloadEncoding::ControlJson,
                false,
            ),
            repack(
                &valid,
                MessageRole::Event,
                false,
                false,
                PayloadEncoding::RawBytes,
                false,
            ),
            repack(
                &valid,
                MessageRole::Event,
                false,
                false,
                PayloadEncoding::ControlJson,
                true,
            ),
        ];
        for (record, action) in invalid
            .into_iter()
            .map(|record| (record, poll()))
            .chain([(valid, read_rows())])
        {
            let (service, handle, owner, peer, stop) = scripted_session(VecDeque::from([
                WorkerOutcome::Record(frame_record(1)),
                WorkerOutcome::Record(record),
            ]));
            let mut session = service.session();
            assert!(matches!(
                session.publish_attach(viewer(&service, handle), attached_result()),
                Outcome::Attached { .. }
            ));
            assert!(matches!(
                execute_viewer(&mut session, "88888888888888888888888888888881", poll()).outcome(),
                Outcome::Record(_)
            ));
            assert_eq!(
                execute_viewer(&mut session, "88888888888888888888888888888882", action).outcome(),
                &unavailable(Unavailable::TerminalRuntimeUnavailable)
            );
            assert!(session.viewers.iter().all(Option::is_none));
            drop(session);
            let _ = stop.shutdown(std::net::Shutdown::Both);
            let _ = owner.join();
            peer.join().unwrap();
        }
    }
}

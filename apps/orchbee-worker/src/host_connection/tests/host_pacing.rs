//! Host pacing witnesses: a drain past its deadline still reads whole queued events, a busy or
//! briefly paused Host is waited for, and a late event or a refused answer for one viewer leaves
//! the Worker serving the next request. Each bound is injected, so no witness waits ten seconds.
use super::super::*;
use orchbee_capability_node_protocol::{
    HostTerminalId, OperationId, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, ViewerRejection, WorkerGeneration, WorkerId,
};
use orchbee_terminal_protocol::{
    AttachmentControl, DecodedEnvelope, HostEpoch, ProcessInfoControl, ProjectionFeedbackControl,
    TerminalApplicationSignal, TerminalApplicationSignalChannel, TerminalId, TerminalIdentity,
    TerminalProcess, TerminalProcessConsole, TerminalProcessInfo, TypedFailure,
    decode_attachment_control, decode_process_info_control, decode_projection_feedback_control,
    encode_attachment_control, encode_process_info_control, encode_projection_feedback_control,
    encode_terminal_effect_event,
};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const EPOCH: [u8; 16] = [4; 16];
const TERMINAL: [u8; 16] = [5; 16];
/// Longer than the one-second budget that used to end the Worker, well below the injected bound.
const PAUSE: Duration = Duration::from_millis(1200);

fn bounded() -> Pacing {
    Pacing::bounded(Duration::from_secs(3), Duration::from_secs(5))
}

fn host(pacing: Pacing) -> TestResult<(HostBrokerConnection, UnixStream)> {
    let (stream, peer) = UnixStream::pair()?;
    peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    peer.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok((
        HostBrokerConnection {
            endpoint: std::path::PathBuf::new(),
            host_epoch: HostEpoch::from_bytes(EPOCH)?,
            prompt_gated_input: false,
            negotiated_limits: NegotiatedHostLimits {
                frame_bytes: 1_048_576,
                input_bytes: 4096,
                row_window_bytes: 65_536,
            },
            stream,
            viewers: RefCell::new(viewers::HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: RefCell::new(titles::Titles::default()),
            host_colors: RefCell::new(crate::host_connection::HostColors::default()),
            drain: Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())),
            pacing,
            inbound: Default::default(),
        },
        peer,
    ))
}

/// One Host event any connection accepts with no prior state: an Agent signal.
fn signal(sequence: u64) -> Vec<u8> {
    let epoch = HostEpoch::from_bytes(EPOCH).expect("epoch");
    let event = encode_terminal_effect_event(&TerminalEffectEvent::ApplicationSignal(
        TerminalApplicationSignal {
            channel: TerminalApplicationSignalChannel::OrchbeeAgentV1,
            identity: TerminalIdentity {
                creation_id: orchbee_terminal_protocol::CreationId::from_bytes([6; 16])
                    .expect("creation"),
                host_epoch: epoch,
                terminal_id: TerminalId::from_bytes([7; 16]).expect("terminal"),
            },
            payload: "{}".to_string(),
            sequence: NonZeroU64::new(sequence).expect("sequence"),
        },
    ))
    .expect("event");
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(epoch),
        kind: event.kind,
        optional_message: false,
        payload: &event.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: event.role,
    })
    .expect("record")
}

/// A frame event for viewer `id`, at revision 5 of generation 1.
fn frame(id: u8, sequence: u64) -> TestResult<Vec<u8>> {
    let mut payload =
        include_bytes!("../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin").to_vec();
    payload[6] = 1;
    payload[56..64].fill(255);
    payload[16..32].copy_from_slice(&EPOCH);
    payload[32..48].fill(id);
    payload[48..56].copy_from_slice(&sequence.to_le_bytes());
    payload[64..72].copy_from_slice(&5_u64.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(HostEpoch::from_bytes(EPOCH)?),
        kind: 41,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::TerminalFrame,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })?)
}

fn viewer(id: u8, action: TerminalViewerAction) -> TestResult<TerminalViewerRequest> {
    Ok(TerminalViewerRequest {
        request_id: OperationId::from_bytes([100 + id; 16])?,
        attachment_id: OperationId::from_bytes([id; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action,
    })
}

const fn frame_applied() -> TerminalViewerAction {
    TerminalViewerAction::FrameApplied {
        sequence: NonZeroU64::MIN,
        generation: NonZeroU64::MIN,
        revision: 5,
    }
}

/// Admits viewer `id` and delivers it one frame, so its next feedback goes to the Host.
fn delivered_viewer(host: &HostBrokerConnection, id: u8) -> TestResult {
    let attach = TerminalViewerAction::Attach {
        host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes(EPOCH)?,
        terminal_id: HostTerminalId::from_bytes(TERMINAL)?,
    };
    host.viewers
        .borrow_mut()
        .admit(&viewer(id, attach)?)
        .map_err(|reason| format!("{reason:?}"))?;
    let record = frame(id, 1)?;
    host.receive_event(record.clone())?;
    assert_eq!(
        host.viewers
            .borrow_mut()
            .poll(OperationId::from_bytes([id; 16])?),
        TerminalViewerOutcome::Record(record)
    );
    Ok(())
}

fn execute(
    host: &HostBrokerConnection,
    server: &UnixStream,
    request: TerminalViewerRequest,
) -> TestResult<TerminalViewerOutcome> {
    Ok(crate::terminal_viewer_operation::execute(
        request,
        orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]),
        NonZeroU64::MIN,
        host,
        server,
    )?
    .outcome)
}

#[derive(Clone, Copy)]
enum Answer {
    Accept,
    Refuse,
}

/// The scripted Host's whole response record to one request.
fn response(request: &[u8], answer: Answer) -> Vec<u8> {
    let envelope = decode_transport_envelope(request).expect("request");
    let (kind, payload) = match answer {
        Answer::Accept => accepted(&envelope),
        Answer::Refuse => {
            let control =
                encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                    code: "attachment-unavailable".into(),
                    message: "refused".into(),
                    retryable: false,
                    reason: None,
                }))
                .expect("refusal");
            (control.kind, control.payload)
        }
    };
    encode_transport_envelope(&Envelope {
        correlation_id: envelope.correlation_id,
        host_epoch: envelope.host_epoch,
        kind,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    })
    .expect("response")
}

fn accepted(request: &DecodedEnvelope<'_>) -> (u16, Vec<u8>) {
    let (kind, role, payload) = (request.kind, request.role, request.payload);
    if let Ok(ProjectionFeedbackControl::FrameAppliedRequest(feedback)) =
        decode_projection_feedback_control(kind, role, payload)
    {
        let control = encode_projection_feedback_control(
            &ProjectionFeedbackControl::FrameAppliedResponse(feedback),
        )
        .expect("feedback");
        return (control.kind, control.payload);
    }
    if let Ok(ProcessInfoControl::ReadProcessInfo(read)) =
        decode_process_info_control(kind, role, payload)
    {
        let control =
            encode_process_info_control(&ProcessInfoControl::ProcessInfo(TerminalProcessInfo {
                attachment_id: read.target.attachment_id,
                terminal_id: TerminalId::from_bytes(TERMINAL).expect("terminal"),
                launch_kind: None,
                root_pid: 41,
                processes: vec![TerminalProcess {
                    pid: 41,
                    parent_pid: Some(17),
                    creation_time: 133_999_999_999_999_999,
                    console: TerminalProcessConsole::Attached,
                    image: None,
                    command_line: None,
                    current_directory: None,
                }],
            }))
            .expect("process info");
        return (control.kind, control.payload);
    }
    if let Ok(AttachmentControl::DetachTerminal(target)) =
        decode_attachment_control(kind, role, payload)
    {
        let control = encode_attachment_control(&AttachmentControl::DetachTerminalAccepted(target))
            .expect("detach");
        return (control.kind, control.payload);
    }
    panic!("unexpected Host request kind {kind}");
}

/// A Host that keeps writing events while it answers. Every record is written whole, and an
/// answer goes ahead of the next event.
struct BusyHost {
    stop: Arc<AtomicBool>,
    writer: JoinHandle<u64>,
    answers: JoinHandle<()>,
}

fn busy_host(peer: UnixStream, answers: Vec<(Answer, Duration)>) -> TestResult<BusyHost> {
    let mut reader = peer.try_clone()?;
    let output = Arc::new(Mutex::new(peer));
    let answering = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (output, answering, stop) = (
            Arc::clone(&output),
            Arc::clone(&answering),
            Arc::clone(&stop),
        );
        thread::spawn(move || {
            let mut written = 0;
            while !stop.load(Ordering::Acquire) {
                if answering.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                let record = signal(written + 1);
                if output.lock().expect("output").write_all(&record).is_err() {
                    break;
                }
                written += 1;
            }
            written
        })
    };
    let answers = thread::spawn(move || {
        for (answer, delay) in answers {
            let request = read_transport_record(&mut reader).expect("request");
            thread::sleep(delay);
            answering.store(true, Ordering::Release);
            output
                .lock()
                .expect("output")
                .write_all(&response(&request, answer))
                .expect("answer");
            answering.store(false, Ordering::Release);
        }
    });
    Ok(BusyHost {
        stop,
        writer,
        answers,
    })
}

#[test]
fn a_drain_reads_whole_queued_events_and_keeps_a_partial_one_for_the_next_read() -> TestResult {
    let (host, mut peer) = host(Pacing::default())?;
    let (server, _server_peer) = UnixStream::pair()?;
    peer.write_all(&signal(1))?;
    peer.write_all(&signal(2))?;
    thread::sleep(Duration::from_millis(50));
    let late = HostExchange::new(&host, &server, Duration::from_millis(20))?;
    // The Worker loses the CPU past the deadline before it drains.
    thread::sleep(Duration::from_millis(80));
    assert_eq!(late.prepare(), Ok(()));
    assert_eq!(
        host.drain.borrow().snapshot().1,
        2,
        "both whole events are read"
    );
    assert_eq!(host.pacing.failure(), None);

    // A record the Host has only begun never holds the reader: the whole event before it is read,
    // the begun one stays partial, and a later read finishes it.
    let cut = signal(4);
    peer.write_all(&signal(3))?;
    peer.write_all(&cut[..12])?;
    thread::sleep(Duration::from_millis(50));
    assert_eq!(late.prepare(), Ok(()));
    assert_eq!(host.drain.borrow().snapshot().1, 3);
    peer.write_all(&cut[12..])?;
    thread::sleep(Duration::from_millis(50));
    assert_eq!(late.prepare(), Ok(()));
    assert_eq!(host.drain.borrow().snapshot().1, 4);
    assert_eq!(host.pacing.failure(), None);
    Ok(())
}

#[test]
fn feedback_and_a_process_read_are_answered_by_a_continuously_busy_host() -> TestResult {
    let (host, peer) = host(bounded())?;
    let (server, _server_peer) = UnixStream::pair()?;
    delivered_viewer(&host, 1)?;
    let busy = busy_host(
        peer,
        vec![(Answer::Accept, PAUSE), (Answer::Accept, Duration::ZERO)],
    )?;
    // Events are pending before the first request and throughout both.
    thread::sleep(Duration::from_millis(100));
    let applied = execute(&host, &server, viewer(1, frame_applied())?)?;
    let read = execute(
        &host,
        &server,
        viewer(1, TerminalViewerAction::ReadProcessInfo)?,
    )?;
    busy.stop.store(true, Ordering::Release);
    let failure = host.pacing.failure();
    // Closing the Worker's end releases a writer that waits on a full stream.
    drop(host);
    let written = busy.writer.join().map_err(|_| "writer panicked")?;
    busy.answers.join().map_err(|_| "answers panicked")?;
    assert_eq!(applied, TerminalViewerOutcome::FeedbackAccepted);
    assert!(matches!(read, TerminalViewerOutcome::Record(_)));
    assert_eq!(failure, None);
    assert!(written > 64, "more events than one drain reads: {written}");
    Ok(())
}

#[test]
fn feedback_and_a_process_read_are_answered_when_the_host_pauses_mid_record() -> TestResult {
    let (host, mut peer) = host(bounded())?;
    let (server, _server_peer) = UnixStream::pair()?;
    delivered_viewer(&host, 1)?;
    let script = thread::spawn(move || {
        let event = signal(1);
        peer.write_all(&event[..12]).expect("event head");
        thread::sleep(PAUSE);
        peer.write_all(&event[12..]).expect("event rest");
        let request = read_transport_record(&mut peer).expect("feedback");
        peer.write_all(&response(&request, Answer::Accept))
            .expect("feedback answer");
        let request = read_transport_record(&mut peer).expect("process read");
        let answer = response(&request, Answer::Accept);
        peer.write_all(&answer[..12]).expect("answer head");
        thread::sleep(PAUSE);
        peer.write_all(&answer[12..]).expect("answer rest");
        peer
    });
    thread::sleep(Duration::from_millis(50));
    let applied = execute(&host, &server, viewer(1, frame_applied())?)?;
    let read = execute(
        &host,
        &server,
        viewer(1, TerminalViewerAction::ReadProcessInfo)?,
    )?;
    let _peer = script.join().map_err(|_| "Host script panicked")?;
    assert_eq!(applied, TerminalViewerOutcome::FeedbackAccepted);
    assert!(matches!(read, TerminalViewerOutcome::Record(_)));
    assert_eq!(host.drain.borrow().snapshot().1, 1);
    assert_eq!(host.pacing.failure(), None);
    Ok(())
}

#[test]
fn a_late_event_a_refused_detach_and_refused_feedback_leave_the_worker_serving() -> TestResult {
    let (host, mut peer) = host(bounded())?;
    let (server, _server_peer) = UnixStream::pair()?;
    for id in 1..=4 {
        delivered_viewer(&host, id)?;
    }
    let script = thread::spawn(move || {
        // Each answer, then the frame the Host had already sent for a viewer it just let go.
        for (answer, late) in [
            (Answer::Accept, Some(1)),
            (Answer::Refuse, Some(2)),
            (Answer::Refuse, None),
            (Answer::Accept, None),
        ] {
            let request = read_transport_record(&mut peer).expect("request");
            peer.write_all(&response(&request, answer)).expect("answer");
            if let Some(id) = late {
                peer.write_all(&frame(id, 2).expect("frame"))
                    .expect("late frame");
            }
        }
        peer
    });
    let detached = execute(&host, &server, viewer(1, TerminalViewerAction::Detach)?)?;
    let refused_detach = execute(&host, &server, viewer(2, TerminalViewerAction::Detach)?)?;
    let refused_feedback = execute(&host, &server, viewer(3, frame_applied())?)?;
    let read = execute(
        &host,
        &server,
        viewer(4, TerminalViewerAction::ReadProcessInfo)?,
    )?;
    let _peer = script.join().map_err(|_| "Host script panicked")?;
    assert_eq!(detached, TerminalViewerOutcome::Detached);
    // A refused Detach counts as detached.
    assert_eq!(refused_detach, TerminalViewerOutcome::Detached);
    // Refused feedback refuses that request and closes only its viewer.
    assert_eq!(
        refused_feedback,
        TerminalViewerOutcome::Rejected(ViewerRejection::HostRejected)
    );
    assert!(
        !host
            .viewers
            .borrow()
            .open(OperationId::from_bytes([3; 16])?)
    );
    assert!(matches!(read, TerminalViewerOutcome::Record(_)));
    // Each late frame was read and dropped by the next exchange; another is dropped the same way.
    host.receive_event(frame(2, 3)?)?;
    assert_eq!(host.pacing.failure(), None);
    Ok(())
}

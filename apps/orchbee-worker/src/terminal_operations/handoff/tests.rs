use super::*;
use orchbee_capability_node_protocol::{
    OperationId, WorkerGeneration, WorkerId as CapabilityWorkerId, read_message, write_message,
};
use orchbee_local_ipc::LocalListener;
use std::{
    io::{self, Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
};

const WORKER: WorkerId = WorkerId::from_bytes([2; 16]);
fn generation() -> NonZeroU64 {
    NonZeroU64::new(3).unwrap()
}
fn request(action: Action, ticket: u8) -> WorkerHandoffRequest {
    WorkerHandoffRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: CapabilityWorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
        worker_generation: WorkerGeneration::new(generation()),
        target: WorkerHandoffTarget {
            handoff_id: WorkerHandoffId::from_bytes([ticket; 16]).unwrap(),
            cohort_sha256: [5; 32],
        },
        action,
    }
}
fn runtime() -> PathBuf {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    // Address derivation only: no directory is created or adopted by this fixture.
    std::env::temp_dir().join(format!(
        "orchbee-handoff-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ))
}
fn pair() -> (LocalStream, LocalStream) {
    let (worker, peer) = LocalStream::pair().unwrap();
    worker.set_nonblocking(true).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    (worker, peer)
}
fn result(peer: &mut LocalStream, request: WorkerHandoffRequest, outcome: Outcome) {
    assert_eq!(
        read_message(peer).unwrap(),
        Message::WorkerHandoffResult(WorkerHandoffResult::new(request, outcome).unwrap())
    );
}

enum Ready {
    Exact,
    Changed(u8),
    Extra,
    Partial,
}
struct Standby {
    stopped: Arc<AtomicBool>,
    connection: Arc<Mutex<Option<LocalStream>>>,
    task: Option<JoinHandle<Option<Message>>>,
}
impl Standby {
    fn new(runtime: &Path, request: WorkerHandoffRequest, ready: Ready) -> Self {
        let endpoint =
            orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::handoff_endpoint_path(
                runtime,
                request.target.handoff_id.as_bytes(),
            );
        let listener = LocalListener::bind(endpoint).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let connection = Arc::new(Mutex::new(None));
        let cancel = Arc::clone(&stopped);
        let registration = Arc::clone(&connection);
        let task = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(4);
            let mut peer = loop {
                if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
                    return None;
                }
                match listener.accept() {
                    Ok((peer, ())) => break peer,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::yield_now(),
                    Err(error) => panic!("standby admission: {error}"),
                }
            };
            *registration.lock().unwrap() = Some(peer.try_clone().unwrap());
            peer.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
            peer.set_write_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            assert_eq!(
                read_message(&mut peer).unwrap(),
                Message::WorkerHandoffHello(request.peer())
            );
            let mut actual = request.peer();
            if let Ready::Changed(field) = ready {
                match field {
                    0 => actual.worker_id = CapabilityWorkerId::from_bytes([9; 16]).unwrap(),
                    1 => {
                        actual.worker_generation =
                            WorkerGeneration::new(NonZeroU64::new(9).unwrap())
                    }
                    2 => actual.target.handoff_id = WorkerHandoffId::from_bytes([9; 16]).unwrap(),
                    3 => actual.target.cohort_sha256 = [9; 32],
                    _ => unreachable!(),
                }
            }
            let mut bytes = orchbee_capability_node_protocol::encode_message(
                &Message::WorkerHandoffReady(actual),
            )
            .unwrap();
            match ready {
                Ready::Extra => bytes.push(42),
                Ready::Partial => bytes.truncate(9),
                _ => {}
            }
            peer.write_all(&bytes).unwrap();
            read_message(&mut peer).ok()
        });
        Self {
            stopped,
            connection,
            task: Some(task),
        }
    }
    fn finish(mut self) -> Option<Message> {
        self.task.take().unwrap().join().unwrap()
    }
}
impl Drop for Standby {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(connection) = self.connection.lock().unwrap().take() {
            let _closed = connection.shutdown(Shutdown::Both);
        }
        if let Some(task) = self.task.take() {
            task.join().unwrap();
        }
    }
}

#[test]
fn handoff_identity_management_attachment_ticket_and_capacity_guards_precede_dial() {
    let mut slot = Handoff::new(Some(&runtime()));
    let prepare = request(Action::Prepare, 4);
    assert_eq!(slot.refusal(prepare, WORKER, generation(), true), None);
    assert_eq!(
        slot.refusal(prepare, WorkerId::from_bytes([9; 16]), generation(), true),
        Some(Refusal::WrongWorker)
    );
    assert_eq!(
        slot.refusal(prepare, WORKER, NonZeroU64::MIN, true),
        Some(Refusal::WrongWorker)
    );
    assert_eq!(
        Handoff::new(None).refusal(prepare, WORKER, generation(), true),
        Some(Refusal::Unmanaged)
    );
    assert_eq!(
        slot.refusal(prepare, WORKER, generation(), false),
        Some(Refusal::AttachmentsBusy)
    );
    assert_eq!(
        slot.refusal(request(Action::Commit, 4), WORKER, generation(), true),
        Some(Refusal::NotPrepared)
    );
    for id in 1..=MAXIMUM_ATTEMPTS {
        slot.spent
            .insert(WorkerHandoffId::from_bytes([u8::try_from(id).unwrap(); 16]).unwrap());
    }
    assert_eq!(
        slot.refusal(prepare, WORKER, generation(), true),
        Some(Refusal::ReusedTicket)
    );
    assert_eq!(
        slot.refusal(request(Action::Prepare, 65), WORKER, generation(), true),
        Some(Refusal::Capacity)
    );
}

#[test]
fn handoff_abort_closes_only_standby_and_never_reuses_a_spent_ticket() {
    let runtime = runtime();
    let prepare = request(Action::Prepare, 4);
    let standby = Standby::new(&runtime, prepare, Ready::Exact);
    let (mut worker, mut old) = pair();
    let mut slot = Handoff::new(Some(&runtime));
    slot.execute(&mut worker, None, prepare, WORKER, generation())
        .unwrap();
    result(&mut old, prepare, Outcome::Prepared);
    assert!(slot.is_prepared());
    let mismatch = request(Action::Abort, 8);
    slot.execute(&mut worker, None, mismatch, WORKER, generation())
        .unwrap();
    result(
        &mut old,
        mismatch,
        Outcome::Refused(Refusal::TicketMismatch),
    );
    slot.execute(&mut worker, None, prepare, WORKER, generation())
        .unwrap();
    result(&mut old, prepare, Outcome::Refused(Refusal::Busy));
    let abort = request(Action::Abort, 4);
    slot.execute(&mut worker, None, abort, WORKER, generation())
        .unwrap();
    result(&mut old, abort, Outcome::Aborted);
    assert!(!slot.is_prepared());
    assert_eq!(standby.finish(), None);
    slot.execute(&mut worker, None, prepare, WORKER, generation())
        .unwrap();
    result(&mut old, prepare, Outcome::Refused(Refusal::ReusedTicket));
    write_message(
        &mut old,
        &Message::WorkerHandoffRequest(request(Action::Prepare, 7)),
    )
    .unwrap();
    assert!(
        matches!(
            server_transport::receive(&worker, None).unwrap(),
            Message::WorkerHandoffRequest(_)
        ),
        "original wire remains readable"
    );
}

#[test]
fn handoff_exact_expiry_closes_standby_and_late_commit_cannot_move_the_old_wire() {
    let runtime = runtime();
    let prepare = request(Action::Prepare, 4);
    let standby = Standby::new(&runtime, prepare, Ready::Exact);
    let (mut worker, mut old) = pair();
    let mut slot = Handoff::new(Some(&runtime));
    slot.execute(&mut worker, None, prepare, WORKER, generation())
        .unwrap();
    result(&mut old, prepare, Outcome::Prepared);
    let cutoff = Instant::now();
    slot.prepared.as_mut().unwrap().expires = cutoff;
    slot.expire(cutoff);
    assert_eq!(standby.finish(), None);
    let commit = request(Action::Commit, 4);
    slot.execute(&mut worker, None, commit, WORKER, generation())
        .unwrap();
    result(&mut old, commit, Outcome::Refused(Refusal::NotPrepared));
    assert!(slot.spent.contains(&prepare.target.handoff_id));
}

#[test]
fn handoff_rejects_every_ready_echo_mismatch_and_unsolicited_tail_without_switching() {
    for ready in [
        Ready::Changed(0),
        Ready::Changed(1),
        Ready::Changed(2),
        Ready::Changed(3),
        Ready::Extra,
    ] {
        let runtime = runtime();
        let prepare = request(Action::Prepare, 4);
        let standby = Standby::new(&runtime, prepare, ready);
        let (mut worker, mut old) = pair();
        let mut slot = Handoff::new(Some(&runtime));
        slot.execute(&mut worker, None, prepare, WORKER, generation())
            .unwrap();
        result(&mut old, prepare, Outcome::Refused(Refusal::Authentication));
        assert!(!slot.is_prepared());
        assert_eq!(standby.finish(), None);
        assert!(slot.spent.contains(&prepare.target.handoff_id));
    }
}

#[test]
fn handoff_partial_ready_and_failed_old_commit_write_preserve_original_connection_selection() {
    let runtime = runtime();
    let prepare = request(Action::Prepare, 4);
    let standby = Standby::new(&runtime, prepare, Ready::Partial);
    let (mut worker, mut old) = pair();
    let mut slot = Handoff::new(Some(&runtime));
    slot.execute(&mut worker, None, prepare, WORKER, generation())
        .unwrap();
    result(
        &mut old,
        prepare,
        Outcome::Refused(Refusal::TargetUnavailable),
    );
    assert_eq!(standby.finish(), None);
    let next = request(Action::Prepare, 6);
    let standby = Standby::new(&runtime, next, Ready::Exact);
    slot.execute(&mut worker, None, next, WORKER, generation())
        .unwrap();
    result(&mut old, next, Outcome::Prepared);
    old.shutdown(Shutdown::Both).unwrap();
    assert!(
        slot.execute(
            &mut worker,
            None,
            request(Action::Commit, 6),
            WORKER,
            generation()
        )
        .is_err()
    );
    assert!(
        slot.is_prepared(),
        "failed old write cannot consume the prepared connection"
    );
    drop(slot);
    assert_eq!(
        standby.finish(),
        None,
        "no Owned is sent before the old write completes"
    );
}

#[test]
fn handoff_commit_emits_exact_owned_on_new_wire_and_closes_old_wire() {
    let runtime = runtime();
    let prepare = request(Action::Prepare, 4);
    let standby = Standby::new(&runtime, prepare, Ready::Exact);
    let (mut worker, mut old) = pair();
    let mut slot = Handoff::new(Some(&runtime));
    slot.execute(&mut worker, None, prepare, WORKER, generation())
        .unwrap();
    result(&mut old, prepare, Outcome::Prepared);
    let commit = request(Action::Commit, 4);
    slot.execute(&mut worker, None, commit, WORKER, generation())
        .unwrap();
    result(&mut old, commit, Outcome::Committed);
    assert_eq!(old.read(&mut [0]).unwrap(), 0);
    assert_eq!(
        standby.finish(),
        Some(Message::WorkerHandoffOwned(commit.peer()))
    );
    assert!(!slot.is_prepared());
    assert!(slot.spent.contains(&prepare.target.handoff_id));
}

#[test]
fn handoff_idle_cutoff_and_partial_record_have_distinct_retirement_results() {
    let (worker, mut peer) = pair();
    assert_eq!(
        server_transport::receive_until(
            &worker,
            None,
            Some(Instant::now() + Duration::from_millis(20))
        )
        .unwrap(),
        None
    );
    peer.write_all(b"O").unwrap();
    assert!(
        server_transport::receive_until(
            &worker,
            None,
            Some(Instant::now() + Duration::from_millis(20))
        )
        .is_err(),
        "a partial frame cannot be discarded as idle expiry"
    );
}

#[test]
fn handoff_expired_commit_write_deadline_emits_no_record() {
    let (worker, mut peer) = pair();
    let committed =
        WorkerHandoffResult::new(request(Action::Commit, 4), Outcome::Committed).unwrap();
    assert!(
        server_transport::send_until(
            &worker,
            None,
            &Message::WorkerHandoffResult(committed),
            Instant::now()
        )
        .is_err()
    );
    peer.set_nonblocking(true).unwrap();
    assert_eq!(
        peer.read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn handoff_actual_admitted_loop_preserves_host_and_cached_terminal_binding_on_new_server() {
    use crate::terminal_authorization::{
        AuthorizedHostOperationKernel, HostOperationAdmission, LaunchHostCompletion,
    };
    use orchbee_capability_node_protocol::{
        AuthorizedTerminalLaunch, HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity,
        LeaseExpiresAtUnixMilliseconds, LeaseId, TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence,
        TerminalId, TerminalLaunchMode, TerminalOperationAuthorization, TerminalOperationLease,
        TerminalPixelGeometry,
    };
    use std::{cell::RefCell, rc::Rc};
    let runtime = runtime();
    let prepare = request(Action::Prepare, 4);
    let endpoint = orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::handoff_endpoint_path(
        &runtime,
        prepare.target.handoff_id.as_bytes(),
    );
    let listener = LocalListener::bind(endpoint).unwrap();
    let (worker, mut old) = pair();
    let (mut host, mut host_peer) = connected_host();
    host.stream().set_nonblocking(true).unwrap();
    let epoch = HostEpoch::from_bytes(*host.host_epoch().as_bytes()).unwrap();
    let authorization = TerminalOperationAuthorization::new(
        TerminalId::from_bytes([6; 16]).unwrap(),
        OperationId::from_bytes([7; 16]).unwrap(),
        prepare.worker_id,
        prepare.worker_generation,
        TerminalFence::new(NonZeroU64::MIN),
        TerminalOperationLease::new(
            LeaseId::from_bytes([8; 16]).unwrap(),
            LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(u64::MAX).unwrap()),
        ),
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    let launch = AuthorizedTerminalLaunch::new(
        authorization,
        None,
        TerminalLaunchMode::Immediate(TerminalPixelGeometry::new(80, 24, 0, 0).unwrap()),
        None,
    );
    let mut kernel = AuthorizedHostOperationKernel::new(WORKER, generation()).unwrap();
    let HostOperationAdmission::Dispatch(dispatch) =
        kernel.admit_launch(launch.clone(), 0).unwrap()
    else {
        panic!("first launch");
    };
    kernel.begin_host_handoff(&dispatch, 0, epoch).unwrap();
    kernel
        .record_launch_completion(
            &dispatch,
            LaunchHostCompletion::Accepted {
                identity: HostTerminalIdentity::new(
                    HostCreationId::from_bytes([9; 16]).unwrap(),
                    epoch,
                    HostTerminalId::from_bytes([10; 16]).unwrap(),
                ),
            },
        )
        .unwrap();
    let expected = super::super::take_result(&mut kernel).unwrap();
    let new_server = thread::spawn(move || {
        let (mut new, ()) = listener.accept().unwrap();
        new.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
        new.set_write_timeout(Some(Duration::from_secs(4))).unwrap();
        assert_eq!(
            read_message(&mut new).unwrap(),
            Message::WorkerHandoffHello(prepare.peer())
        );
        write_message(&mut new, &Message::WorkerHandoffReady(prepare.peer())).unwrap();
        assert_eq!(
            read_message(&mut new).unwrap(),
            Message::WorkerHandoffOwned(prepare.peer())
        );
        write_message(&mut new, &Message::AuthorizedTerminalLaunch(launch)).unwrap();
        assert_eq!(
            read_message(&mut new).unwrap(),
            expected,
            "same cached exact binding, without another Host launch"
        );
        new.shutdown(Shutdown::Both).unwrap();
    });
    let old_server = thread::spawn(move || {
        write_message(&mut old, &Message::WorkerHandoffRequest(prepare)).unwrap();
        result(&mut old, prepare, Outcome::Prepared);
        let commit = request(Action::Commit, 4);
        write_message(&mut old, &Message::WorkerHandoffRequest(commit)).unwrap();
        result(&mut old, commit, Outcome::Committed);
        assert_eq!(old.read(&mut [0]).unwrap(), 0);
    });
    let drain = Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new()));
    let jobs = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let mutations = crate::worktree_mutation_job::WorktreeMutationSlot::new(
        crate::worktree_mutation_windows::run,
    );
    let plugins = crate::plugin_jobs::PluginJobs::new();
    assert!(
        super::super::run_admitted(
            worker,
            &mut host,
            WORKER,
            generation(),
            kernel,
            super::super::AdmittedAux {
                preferences: None,
                shell: &Default::default(),
                drain: &drain,
                worktree_slot: &jobs,
                mutation_slot: &mutations,
                plugin_jobs: &plugins,
                session_runtime: Some(&runtime),
                managed_host: None,
            }
        )
        .is_err(),
        "new Server EOF retains ordinary retirement"
    );
    old_server.join().unwrap();
    new_server.join().unwrap();
    assert_eq!(*host.host_epoch().as_bytes(), epoch.as_bytes());
    host_peer.set_nonblocking(true).unwrap();
    assert_eq!(
        host_peer.read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock,
        "handoff/replay emitted no Host command and kept connection alive"
    );
}

fn connected_host() -> (HostBrokerConnection, LocalStream) {
    use orchbee_terminal_protocol::{
        ConnectionControl, Envelope, HelloAccepted, MessageRole, PayloadEncoding,
        decode_connection_control, decode_transport_envelope, encode_connection_control,
        encode_transport_envelope,
    };
    use std::{cell::RefCell, rc::Rc};
    let endpoint = crate::tests::endpoint();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let owner = thread::spawn(move || {
        let (mut peer, ()) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let mut header = [0; 64];
        peer.read_exact(&mut header).unwrap();
        let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        assert!((64..=4096).contains(&length));
        let mut record = vec![0; length];
        record[..64].copy_from_slice(&header);
        peer.read_exact(&mut record[64..]).unwrap();
        let envelope = decode_transport_envelope(&record).unwrap();
        let ConnectionControl::Hello(hello) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload).unwrap()
        else {
            panic!("Host hello");
        };
        let control = encode_connection_control(&ConnectionControl::HelloAccepted(HelloAccepted {
            frame_bytes: hello.frame_bytes,
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([4; 16]).unwrap(),
            host_version: "0.1.0".into(),
            input_bytes: hello.input_bytes,
            protocol: hello.protocol,
            row_window_bytes: hello.row_window_bytes,
            supported_optional_features: vec![],
        }))
        .unwrap();
        peer.write_all(
            &encode_transport_envelope(&Envelope {
                correlation_id: envelope.correlation_id,
                host_epoch: Some(
                    orchbee_terminal_protocol::HostEpoch::from_bytes([4; 16]).unwrap(),
                ),
                kind: control.kind,
                optional_message: false,
                payload: &control.payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: hello.protocol,
                role: MessageRole::Response,
            })
            .unwrap(),
        )
        .unwrap();
        peer
    });
    let host = HostBrokerConnection::connect(
        &endpoint,
        Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())),
    )
    .unwrap();
    (host, owner.join().unwrap())
}

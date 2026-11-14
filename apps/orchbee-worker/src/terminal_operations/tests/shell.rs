use super::super::*;
use orchbee_capability_node_protocol::{self as node, *};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::Cohort;
use orchbee_terminal_protocol::{
    self as terminal, ConnectionControl, Envelope, HelloAccepted, PayloadEncoding,
    TerminalCreationControl, decode_connection_control, decode_terminal_creation_control,
    decode_transport_envelope, encode_connection_control, encode_terminal_creation_control,
    encode_transport_envelope,
};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Instant,
};

pub(super) const WORKER: orchbee_local_worker_protocol::WorkerId =
    orchbee_local_worker_protocol::WorkerId::from_bytes([3; 16]);
fn launch(id: u8, executable: &str) -> AuthorizedTerminalLaunch {
    let n = NonZeroU64::MIN;
    AuthorizedTerminalLaunch::new(
        TerminalOperationAuthorization::new(
            TerminalId::from_bytes([id; 16]).unwrap(),
            OperationId::from_bytes([id; 16]).unwrap(),
            node::WorkerId::from_bytes([3; 16]).unwrap(),
            WorkerGeneration::new(n),
            TerminalFence::new(n),
            TerminalOperationLease::new(
                LeaseId::from_bytes([id; 16]).unwrap(),
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        ),
        Some(AbsoluteCwd::new(std::env::temp_dir().to_str().unwrap()).unwrap()),
        TerminalLaunchMode::Deferred,
        Some(LaunchSelection::new(executable.into(), Vec::new()).unwrap()),
    )
}

fn send(peer: &mut LocalStream, launch: &AuthorizedTerminalLaunch) {
    node::write_message(
        peer,
        &CapabilityNodeMessage::AuthorizedTerminalLaunch(launch.clone()),
    )
    .unwrap();
}
fn outcome(peer: &mut LocalStream) -> TerminalLaunchOutcome {
    let CapabilityNodeMessage::TerminalLaunchResult(result) = node::read_message(peer).unwrap()
    else {
        panic!("launch result");
    };
    result.outcome()
}
pub(super) fn timeout(stream: &LocalStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
}
pub(super) fn record(peer: &mut LocalStream) -> Vec<u8> {
    let mut header = [0; 64];
    peer.read_exact(&mut header).unwrap();
    let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    assert!((64..=2_000_000).contains(&length));
    let mut bytes = vec![0; length];
    bytes[..64].copy_from_slice(&header);
    peer.read_exact(&mut bytes[64..]).unwrap();
    bytes
}
pub(super) fn reply(peer: &mut LocalStream, request: &[u8], kind: u16, payload: &[u8]) {
    let request = decode_transport_envelope(request).unwrap();
    peer.write_all(
        &encode_transport_envelope(&Envelope {
            correlation_id: request.correlation_id,
            host_epoch: Some(terminal::HostEpoch::from_bytes([4; 16]).unwrap()),
            kind,
            optional_message: false,
            payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: terminal::CURRENT_PROTOCOL_VERSION,
            role: terminal::MessageRole::Response,
        })
        .unwrap(),
    )
    .unwrap();
}
pub(super) fn connected_host() -> (HostBrokerConnection, Rc<RefCell<AgentDrain>>, LocalStream) {
    let endpoint = crate::tests::endpoint();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let task = thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        timeout(&peer);
        crate::host_connection::answer_test_offer(&mut peer).unwrap();
        let bytes = record(&mut peer);
        let envelope = decode_transport_envelope(&bytes).unwrap();
        let ConnectionControl::Hello(hello) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload).unwrap()
        else {
            panic!("hello");
        };
        let accepted =
            encode_connection_control(&ConnectionControl::HelloAccepted(HelloAccepted {
                frame_bytes: hello.frame_bytes,
                host_epoch: terminal::HostEpoch::from_bytes([4; 16]).unwrap(),
                host_version: "0.1.0".into(),
                input_bytes: hello.input_bytes,
                protocol: hello.protocol,
                row_window_bytes: hello.row_window_bytes,
                supported_optional_features: vec![],
            }))
            .unwrap();
        reply(&mut peer, &bytes, accepted.kind, &accepted.payload);
        peer
    });
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let connection = HostBrokerConnection::connect(&endpoint, Rc::clone(&drain)).unwrap();
    (connection, drain, task.join().unwrap())
}

#[test]
fn admitted_worker_survives_failed_preparation_then_launches_and_fences_replay() {
    let (mut host, drain, mut host_peer) = connected_host();
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        let bad = launch(1, "C:/unused.exe")
            .with_preferences(LaunchPreferences::Unavailable)
            .unwrap();
        send(&mut peer, &bad);
        let failed = outcome(&mut peer);
        assert!(matches!(failed, TerminalLaunchOutcome::Failed(_)));
        send(&mut peer, &bad);
        assert_eq!(outcome(&mut peer), failed);
        let good = launch(2, "C:/Windows/System32/cmd.exe");
        send(&mut peer, &good);
        let bytes = record(&mut host_peer);
        let envelope = decode_transport_envelope(&bytes).unwrap();
        let TerminalCreationControl::CreateTerminal(request) =
            decode_terminal_creation_control(envelope.kind, envelope.role, envelope.payload)
                .unwrap()
        else {
            panic!("create");
        };
        assert!(
            request
                .launch
                .executable
                .to_ascii_lowercase()
                .ends_with("cmd.exe")
        );
        assert!(request.launch.arguments.is_empty());
        let response = encode_terminal_creation_control(&TerminalCreationControl::TerminalCreated(
            terminal::TerminalIdentity {
                creation_id: request.creation_id,
                host_epoch: terminal::HostEpoch::from_bytes([4; 16]).unwrap(),
                terminal_id: terminal::TerminalId::from_bytes([5; 16]).unwrap(),
            },
        ))
        .unwrap();
        reply(&mut host_peer, &bytes, response.kind, &response.payload);
        let accepted = outcome(&mut peer);
        assert!(matches!(accepted, TerminalLaunchOutcome::HostAccepted(_)));
        send(&mut peer, &good);
        assert_eq!(outcome(&mut peer), accepted);
        let conflicting = launch(2, "C:/changed.exe");
        send(&mut peer, &conflicting);
        assert_eq!(
            outcome(&mut peer),
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::ConflictingReplay)
        );
        // Host may receive no command for failures or either replay.
        host_peer.set_nonblocking(true).unwrap();
        assert_eq!(
            host_peer.read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        peer.shutdown(std::net::Shutdown::Both).unwrap();
    });
    server.set_nonblocking(true).unwrap();
    host.stream().set_nonblocking(true).unwrap();
    let worktrees = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let mutations =
        crate::worktree_mutation_job::WorktreeMutationSlot::new(crate::worktree_mutation_git::run);
    assert!(
        run_admitted(
            server,
            &mut host,
            WORKER,
            NonZeroU64::MIN,
            AuthorizedHostOperationKernel::new(WORKER, NonZeroU64::MIN).unwrap(),
            AdmittedAux {
                drain: &drain,
                system_shell: &SystemShell::resolve(),
                worktree_slot: &worktrees,
                agent_integrations: &crate::agent_integration_jobs::AgentIntegrationJobs::new(),
                agent_probes: &crate::agent_probe_jobs::AgentProbeJobs::new(),
                mutation_slot: &mutations,
                plugin_jobs: &crate::plugin_jobs::PluginJobs::new(),
                session_runtime: None,
                managed_host: None,
            }
        )
        .is_err()
    );
    driver.join().unwrap();
}

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct CohortFixture(PathBuf);
impl CohortFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchbee-shell-reload-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let owner = Self(root);
        let hash = "396cfec2d1c795ce71dce2d9c92f29cddec638632050c17861d771936a99282d";
        let mut entries = Vec::new();
        for (name, artifact) in [
            ("orchbee", orchbee_local_runtime::CohortArtifact::Client),
            (
                "orchbee-server",
                orchbee_local_runtime::CohortArtifact::Server,
            ),
            (
                "orchbee-worker",
                orchbee_local_runtime::CohortArtifact::Worker,
            ),
            (
                "orchbee-terminal-host",
                orchbee_local_runtime::CohortArtifact::TerminalHost,
            ),
        ] {
            std::fs::write(owner.0.join(artifact.file_name()), b"native fixture\n").unwrap();
            entries.push(format!("\"{name}\":\"{hash}\""));
        }
        entries.push(format!("\"agent-observation-adapters\":\"{hash}\""));
        std::fs::write(
            owner.0.join("cohort.json"),
            format!(
                "{{\"schemaVersion\":1,\"artifacts\":{{{}}}}}",
                entries.join(",")
            ),
        )
        .unwrap();
        std::fs::create_dir(owner.0.join("runtime")).unwrap();
        owner
    }
}
impl Drop for CohortFixture {
    fn drop(&mut self) {
        let target = self.0.canonicalize().unwrap();
        assert!(target.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            target
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-shell-reload-")
        );
        std::fs::remove_dir_all(target).unwrap();
    }
}

#[test]
fn lazy_worker_retains_invalid_launch_receipt_and_answers_next_request_without_starting_host() {
    let fixture = CohortFixture::new();
    let cohort = Cohort::load(&fixture.0, Instant::now() + Duration::from_secs(3)).unwrap();
    let runtime = fixture.0.join("runtime");
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        let bad = launch(1, "C:/unused.exe")
            .with_preferences(LaunchPreferences::Unavailable)
            .unwrap();
        send(&mut peer, &bad);
        let failed = outcome(&mut peer);
        assert!(matches!(failed, TerminalLaunchOutcome::Failed(_)));
        send(&mut peer, &bad);
        assert_eq!(outcome(&mut peer), failed);
        node::write_message(
            &mut peer,
            &CapabilityNodeMessage::AgentDrainRequest(AgentDrainRequest {
                worker_id: node::WorkerId::from_bytes([3; 16]).unwrap(),
                worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
                token: NonZeroU64::MIN,
                acknowledged: 0,
            }),
        )
        .unwrap();
        assert!(
            matches!(node::read_message(&mut peer).unwrap(),CapabilityNodeMessage::AgentDrainReply(reply) if reply.entries().is_empty())
        );
        peer.shutdown(std::net::Shutdown::Both).unwrap();
    });
    assert!(
        lazy::run(
            server,
            WORKER,
            NonZeroU64::MIN,
            &cohort,
            &runtime,
            &SystemShell::resolve()
        )
        .is_err()
    );
    driver.join().unwrap();
    assert!(
        !orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::host_runtime_directory(&runtime)
            .exists()
    );
}

#[test]
fn lazy_handoff_retains_ticket_history_and_worker_identity_without_starting_host() {
    use node::{
        WorkerHandoffAction as Action, WorkerHandoffId, WorkerHandoffOutcome as Outcome,
        WorkerHandoffRefusal, WorkerHandoffRequest, WorkerHandoffResult, WorkerHandoffTarget,
    };
    use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
    let fixture = CohortFixture::new();
    let cohort = Cohort::load(&fixture.0, Instant::now() + Duration::from_secs(3)).unwrap();
    let runtime = fixture.0.join("runtime");
    let prepare = WorkerHandoffRequest {
        operation_id: OperationId::from_bytes([41; 16]).unwrap(),
        worker_id: node::WorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        target: WorkerHandoffTarget {
            handoff_id: WorkerHandoffId::from_bytes([42; 16]).unwrap(),
            cohort_sha256: [43; 32],
        },
        action: Action::Prepare,
    };
    let listener = LocalListener::bind(ModeCheckedLocalSessionRendezvous::handoff_endpoint_path(
        &runtime,
        prepare.target.handoff_id.as_bytes(),
    ))
    .unwrap();
    listener.set_nonblocking(true).unwrap();
    let next = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut peer = loop {
            match listener.accept() {
                Ok((peer, ())) => break peer,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    thread::yield_now();
                }
                Err(error) => panic!("handoff standby: {error}"),
            }
        };
        timeout(&peer);
        assert_eq!(
            node::read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerHandoffHello(prepare.peer())
        );
        node::write_message(
            &mut peer,
            &CapabilityNodeMessage::WorkerHandoffReady(prepare.peer()),
        )
        .unwrap();
        assert_eq!(
            node::read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerHandoffOwned(prepare.peer())
        );
        node::write_message(
            &mut peer,
            &CapabilityNodeMessage::WorkerHandoffRequest(prepare),
        )
        .unwrap();
        assert_eq!(
            node::read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerHandoffResult(
                WorkerHandoffResult::new(
                    prepare,
                    Outcome::Refused(WorkerHandoffRefusal::ReusedTicket)
                )
                .unwrap()
            )
        );
        let drain = AgentDrainRequest {
            worker_id: prepare.worker_id,
            worker_generation: prepare.worker_generation,
            token: NonZeroU64::new(44).unwrap(),
            acknowledged: 0,
        };
        node::write_message(&mut peer, &CapabilityNodeMessage::AgentDrainRequest(drain)).unwrap();
        assert!(
            matches!(node::read_message(&mut peer).unwrap(), CapabilityNodeMessage::AgentDrainReply(reply) if reply.token_echo() == drain.token && reply.entries().is_empty() && reply.queue_tail() == 0)
        );
        peer.shutdown(std::net::Shutdown::Both).unwrap();
    });
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let original = thread::spawn(move || {
        node::write_message(
            &mut peer,
            &CapabilityNodeMessage::WorkerHandoffRequest(prepare),
        )
        .unwrap();
        assert_eq!(
            node::read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerHandoffResult(
                WorkerHandoffResult::new(prepare, Outcome::Prepared).unwrap()
            )
        );
        let commit = WorkerHandoffRequest {
            action: Action::Commit,
            ..prepare
        };
        node::write_message(
            &mut peer,
            &CapabilityNodeMessage::WorkerHandoffRequest(commit),
        )
        .unwrap();
        assert_eq!(
            node::read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerHandoffResult(
                WorkerHandoffResult::new(commit, Outcome::Committed).unwrap()
            )
        );
        assert_eq!(peer.read(&mut [0]).unwrap(), 0);
    });
    let result = lazy::run(
        server,
        WORKER,
        NonZeroU64::MIN,
        &cohort,
        &runtime,
        &SystemShell::resolve(),
    );
    original.join().unwrap();
    next.join().unwrap();
    assert!(
        result.is_err(),
        "ordinary new Server EOF is not another handoff"
    );
    assert!(!ModeCheckedLocalSessionRendezvous::host_runtime_directory(&runtime).exists());
}

/// Starts one real pre-Host plugin process that marks its start, then outlives the test unless
/// the Worker's exit settlement terminates its Job.
fn start_plugin_process(peer: &mut LocalStream, directory: &std::path::Path) {
    let script = "Set-Content -LiteralPath started -Value 1; Start-Sleep -Seconds 30; \
                  Set-Content -LiteralPath finished -Value 1";
    node::write_message(
        peer,
        &CapabilityNodeMessage::PluginJobRequest(PluginJobRequest {
            operation_id: OperationId::from_bytes([90; 16]).unwrap(),
            worker_id: node::WorkerId::from_bytes([3; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            action: PluginJobAction::Start(PluginCommandIntent {
                plugin_id: "test".into(),
                command_id: "exit-settlement".into(),
                directory: directory.to_str().unwrap().into(),
                argv: [
                    "powershell.exe",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    script,
                ]
                .map(str::to_owned)
                .to_vec(),
                environment: vec![],
            }),
        }),
    )
    .unwrap();
    assert!(matches!(
        node::read_message(peer).unwrap(),
        CapabilityNodeMessage::PluginJobResult(PluginJobResult {
            outcome: PluginJobOutcome::Started { .. },
            ..
        })
    ));
    let deadline = Instant::now() + Duration::from_secs(15);
    while !directory.join("started").exists() {
        assert!(Instant::now() < deadline, "plugin process started");
        thread::sleep(Duration::from_millis(10));
    }
}

/// Fault injection for the fallback exit path: the Server disconnects, or the first Launch
/// cannot start its Host, while a real plugin process tree is running.
#[test]
fn lazy_exit_after_disconnect_or_host_start_failure_settles_running_plugin_process() {
    for host_start_failure in [false, true] {
        let fixture = CohortFixture::new();
        let cohort = Cohort::load(&fixture.0, Instant::now() + Duration::from_secs(3)).unwrap();
        let runtime = fixture.0.join("runtime");
        let work = fixture.0.join("plugin");
        std::fs::create_dir(&work).unwrap();
        let (server, mut peer) = LocalStream::pair().unwrap();
        timeout(&peer);
        peer.set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let driver_work = work.clone();
        let driver = thread::spawn(move || {
            start_plugin_process(&mut peer, &driver_work);
            let fault = Instant::now();
            if host_start_failure {
                // A valid Launch passes preparation; the fixture cohort's Host executable is
                // not runnable, so startup fails and the loop ends through the fallback exit.
                send(&mut peer, &launch(91, "C:/Windows/System32/cmd.exe"));
                assert_eq!(
                    outcome(&mut peer),
                    TerminalLaunchOutcome::Failed(
                        TerminalLaunchFailure::HostUnavailableBeforeHandoff
                    )
                );
            }
            drop(peer);
            fault
        });
        let result = lazy::run(
            server,
            WORKER,
            NonZeroU64::MIN,
            &cohort,
            &runtime,
            &SystemShell::resolve(),
        );
        let exited = Instant::now();
        let fault = driver.join().unwrap();
        let error = result.unwrap_err();
        let cleanup = error.downcast_ref::<io::Error>().is_some_and(|error| {
            error.kind() == io::ErrorKind::TimedOut || error.to_string().contains("cleanup unknown")
        });
        assert!(!cleanup, "exit settlement proved cleanup: {error}");
        // Only the Host-start branch prepares the Host directory, so a preparation refusal
        // followed by the disconnect cannot satisfy the start-failure case.
        assert_eq!(
            orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::host_runtime_directory(
                &runtime
            )
            .exists(),
            host_start_failure
        );
        // Fault-to-exit stays within the six-second shared settlement window plus slack.
        assert!(exited.saturating_duration_since(fault) < Duration::from_secs(8));
        // Settlement terminated the Job: the process can no longer reach its final write.
        assert!(work.join("started").exists());
        thread::sleep(Duration::from_millis(500));
        assert!(!work.join("finished").exists());
        std::fs::remove_file(work.join("started")).unwrap();
        std::fs::remove_dir(&work).unwrap();
    }
}

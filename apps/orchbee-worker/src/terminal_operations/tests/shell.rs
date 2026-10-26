use super::super::*;
use orchbee_capability_node_protocol::{self as node, *};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_site_runtime::{NativeCohort, WindowsShellSettings};
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

const WORKER: orchbee_local_worker_protocol::WorkerId =
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
        None,
    )
    .with_default_shell(Some(
        DefaultShellSelection::new(executable.into(), DefaultShellMode::NonLogin).unwrap(),
    ))
    .unwrap()
}

fn editor_source() -> TerminalEditorSource {
    TerminalEditorSource {
        attachment_id: OperationId::from_bytes([70; 16]).unwrap(),
        host_epoch: node::HostEpoch::from_bytes([4; 16]).unwrap(),
        terminal_id: HostTerminalId::from_bytes([71; 16]).unwrap(),
        generation: NonZeroU64::MIN,
        revision: 7,
    }
}
fn editor_launch(id: u8, source: TerminalEditorSource) -> AuthorizedTerminalLaunch {
    let ordinary = launch(id, "unused.exe");
    AuthorizedTerminalLaunch::new(
        ordinary.authorization(),
        ordinary.cwd().cloned(),
        TerminalLaunchMode::Deferred,
        Some(
            LaunchSelection::new(
                "C:/powershell.exe".into(),
                vec!["--wait".into(), "".into(), "literal & % 字".into()],
            )
            .unwrap(),
        ),
    )
    .with_editor_source(Some(source))
    .unwrap()
}

fn reply_editor_source_inventory(peer: &mut LocalStream, source: TerminalEditorSource) {
    let bytes = record(peer);
    let envelope = decode_transport_envelope(&bytes).unwrap();
    let terminal::InventoryControl::ListTerminals(request) =
        terminal::decode_inventory_control(envelope.kind, envelope.role, envelope.payload).unwrap()
    else {
        panic!("source lifecycle inventory before Attached")
    };
    assert_eq!(request.cursor, None);
    assert_eq!(request.maximum_terminals, 128);
    let inventory = terminal::encode_inventory_control(&terminal::InventoryControl::TerminalList(
        terminal::TerminalList {
            inventory_generation: NonZeroU64::MIN,
            next_cursor: None,
            terminals: vec![terminal::TerminalSummary::Running {
                core: terminal::TerminalSummaryCore {
                    close_activity: None,
                    configuration: terminal::TerminalConfiguration {
                        scrollback_bytes: 0,
                    },
                    configuration_revision: 0,
                    identity: terminal::TerminalIdentity {
                        creation_id: terminal::CreationId::from_bytes([68; 16]).unwrap(),
                        host_epoch: terminal::HostEpoch::from_bytes(source.host_epoch.as_bytes())
                            .unwrap(),
                        terminal_id: terminal::TerminalId::from_bytes(
                            source.terminal_id.as_bytes(),
                        )
                        .unwrap(),
                    },
                    revision: source.revision,
                    theme: terminal::TerminalThemeState {
                        operation_id: None,
                        revision: 0,
                        terminal: terminal::TerminalTheme {
                            color_scheme: terminal::ColorScheme::Dark,
                            default_background_rgb: 0,
                            default_cursor_rgb: None,
                            default_foreground_rgb: 0xffffff,
                            palette: vec![0; 256],
                        },
                    },
                },
                geometry: terminal::TerminalGeometry {
                    columns: 80,
                    rows: 24,
                },
                terminate_requested: false,
            }],
        },
    ))
    .unwrap();
    reply(peer, &bytes, inventory.kind, &inventory.payload);
}

#[test]
fn editor_source_launch_reaches_host_capture_with_literal_argv_and_retains_known_or_unknown_receipts()
 {
    for mode in ["confirmed", "lost", "wrong-target"] {
        let (mut host, drain, mut host_peer) = connected_host();
        let (server, mut peer) = LocalStream::pair().unwrap();
        timeout(&peer);
        let driver = thread::spawn(move || {
            let source = editor_source();
            let attach = TerminalViewerRequest {
                request_id: OperationId::from_bytes([69; 16]).unwrap(),
                worker_id: node::WorkerId::from_bytes([3; 16]).unwrap(),
                worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
                attachment_id: source.attachment_id,
                action: TerminalViewerAction::Attach {
                    host_epoch: source.host_epoch,
                    terminal_id: source.terminal_id,
                },
            };
            node::write_message(
                &mut peer,
                &CapabilityNodeMessage::TerminalViewerRequest(attach),
            )
            .unwrap();
            let bytes = record(&mut host_peer);
            let envelope = decode_transport_envelope(&bytes).unwrap();
            let terminal::AttachmentControl::AttachTerminal(request) =
                terminal::decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                    .unwrap()
            else {
                panic!("source attach")
            };
            assert!(
                request.capabilities.read
                    && !request.capabilities.input
                    && !request.capabilities.resize
                    && !request.request_resize_lease
            );
            let granted = terminal::encode_attachment_control(
                &terminal::AttachmentControl::AttachmentCapabilities(
                    terminal::AttachmentCapabilities {
                        attachment_id: request.attachment_id,
                        terminal_id: request.terminal_id,
                        capabilities: request.capabilities,
                        capability_revision: NonZeroU64::MIN,
                        resize_lease: false,
                    },
                ),
            )
            .unwrap();
            reply(&mut host_peer, &bytes, granted.kind, &granted.payload);
            let bytes = record(&mut host_peer);
            let envelope = decode_transport_envelope(&bytes).unwrap();
            let terminal::ProjectionDemandControl::SetProjectionDemand(demand) =
                terminal::decode_projection_demand_control(
                    envelope.kind,
                    envelope.role,
                    envelope.payload,
                )
                .unwrap()
            else {
                panic!("source demand")
            };
            let accepted = terminal::encode_projection_demand_control(
                &terminal::ProjectionDemandControl::ProjectionDemandAccepted(
                    terminal::ProjectionDemandAccepted {
                        attachment_id: demand.attachment_id,
                        demand: demand.demand,
                    },
                ),
            )
            .unwrap();
            reply(&mut host_peer, &bytes, accepted.kind, &accepted.payload);
            reply_editor_source_inventory(&mut host_peer, source);
            assert!(
                matches!(node::read_message(&mut peer).unwrap(),CapabilityNodeMessage::TerminalViewerResult(result) if result.outcome==TerminalViewerOutcome::Attached)
            );
            let launch = editor_launch(72, source);
            send(&mut peer, &launch);
            let bytes = record(&mut host_peer);
            let envelope = decode_transport_envelope(&bytes).unwrap();
            let TerminalCreationControl::CreateScrollbackEditor(editor) =
                decode_terminal_creation_control(envelope.kind, envelope.role, envelope.payload)
                    .unwrap()
            else {
                panic!("editor capture create")
            };
            assert_eq!(
                editor.creation.launch.start_mode,
                terminal::LaunchStartMode::Deferred
            );
            assert_eq!(
                editor.creation.launch.arguments,
                ["--wait", "", "literal & % 字"]
            );
            assert_eq!(
                *editor.source.attachment_id.as_bytes(),
                source.attachment_id.as_bytes()
            );
            assert_eq!(
                *editor.source.terminal_id.as_bytes(),
                source.terminal_id.as_bytes()
            );
            assert_eq!(
                (editor.source.generation, editor.source.revision),
                (source.generation, source.revision)
            );
            if mode == "lost" {
                host_peer.shutdown(std::net::Shutdown::Both).unwrap();
                assert!(matches!(
                    outcome(&mut peer),
                    TerminalLaunchOutcome::OutcomeUnknown { .. }
                ));
                return;
            }
            let accepted = terminal::encode_terminal_creation_control(
                &TerminalCreationControl::TerminalCreated(terminal::TerminalIdentity {
                    creation_id: if mode == "wrong-target" {
                        terminal::CreationId::from_bytes([73; 16]).unwrap()
                    } else {
                        editor.creation.creation_id
                    },
                    host_epoch: terminal::HostEpoch::from_bytes([4; 16]).unwrap(),
                    terminal_id: terminal::TerminalId::from_bytes([74; 16]).unwrap(),
                }),
            )
            .unwrap();
            reply(&mut host_peer, &bytes, accepted.kind, &accepted.payload);
            let result = outcome(&mut peer);
            if mode == "wrong-target" {
                assert!(matches!(
                    result,
                    TerminalLaunchOutcome::OutcomeUnknown { .. }
                ));
                return;
            }
            assert!(
                matches!(result,TerminalLaunchOutcome::HostAccepted(value) if value.state()==HostLaunchState::AwaitingStart)
            );
            for field in 0..3 {
                let mut invalid = source;
                match field {
                    0 => invalid.host_epoch = node::HostEpoch::from_bytes([77; 16]).unwrap(),
                    1 => invalid.terminal_id = HostTerminalId::from_bytes([78; 16]).unwrap(),
                    _ => invalid.attachment_id = OperationId::from_bytes([79; 16]).unwrap(),
                }
                send(&mut peer, &editor_launch(80 + field, invalid));
                assert_eq!(
                    outcome(&mut peer),
                    TerminalLaunchOutcome::Failed(
                        TerminalLaunchFailure::HostUnavailableBeforeHandoff
                    )
                );
            }
            node::write_message(
                &mut peer,
                &CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
                    request_id: OperationId::from_bytes([83; 16]).unwrap(),
                    action: TerminalViewerAction::Detach,
                    ..attach
                }),
            )
            .unwrap();
            let bytes = record(&mut host_peer);
            let envelope = decode_transport_envelope(&bytes).unwrap();
            let terminal::AttachmentControl::DetachTerminal(target) =
                terminal::decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                    .unwrap()
            else {
                panic!("source detach")
            };
            let released = terminal::encode_attachment_control(
                &terminal::AttachmentControl::DetachTerminalAccepted(target),
            )
            .unwrap();
            reply(&mut host_peer, &bytes, released.kind, &released.payload);
            assert!(
                matches!(node::read_message(&mut peer).unwrap(),CapabilityNodeMessage::TerminalViewerResult(result) if result.outcome==TerminalViewerOutcome::Detached)
            );
            send(&mut peer, &editor_launch(84, source));
            assert_eq!(
                outcome(&mut peer),
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
            );
            send(&mut peer, &launch);
            assert_eq!(outcome(&mut peer), result);
            for field in 0..5 {
                let mut changed = source;
                match field {
                    0 => changed.attachment_id = OperationId::from_bytes([85; 16]).unwrap(),
                    1 => changed.host_epoch = node::HostEpoch::from_bytes([86; 16]).unwrap(),
                    2 => changed.terminal_id = HostTerminalId::from_bytes([87; 16]).unwrap(),
                    3 => changed.generation = NonZeroU64::MAX,
                    _ => changed.revision = source.revision + 1,
                }
                send(&mut peer, &editor_launch(72, changed));
                assert_eq!(
                    outcome(&mut peer),
                    TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::ConflictingReplay)
                );
            }
            host_peer.set_nonblocking(true).unwrap();
            assert_eq!(
                host_peer.read(&mut [0]).unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
            peer.shutdown(std::net::Shutdown::Both).unwrap();
        });
        let result = run(
            &server,
            &mut host,
            WORKER,
            NonZeroU64::MIN,
            &drain,
            None,
            &WindowsShellSettings::default(),
        );
        driver.join().unwrap();
        assert!(result.is_err());
    }
}

#[test]
fn editor_source_missing_viewer_fails_before_host_create_or_lazy_startup() {
    let (mut host, drain, mut host_peer) = connected_host();
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        send(&mut peer, &editor_launch(75, editor_source()));
        assert_eq!(
            outcome(&mut peer),
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
        );
        host_peer.set_nonblocking(true).unwrap();
        assert_eq!(
            host_peer.read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        peer.shutdown(std::net::Shutdown::Both).unwrap();
    });
    let result = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        None,
        &WindowsShellSettings::default(),
    );
    driver.join().unwrap();
    assert!(result.is_err());
    let fixture = CohortFixture::new();
    let cohort = NativeCohort::load(&fixture.0, Instant::now() + Duration::from_secs(3)).unwrap();
    let runtime = fixture.0.join("runtime");
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        send(&mut peer, &editor_launch(76, editor_source()));
        assert_eq!(
            outcome(&mut peer),
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
        );
        peer.shutdown(std::net::Shutdown::Both).unwrap();
    });
    let result = lazy::run(
        server,
        WORKER,
        NonZeroU64::MIN,
        &cohort,
        &runtime,
        None,
        &WindowsShellSettings::default(),
    );
    driver.join().unwrap();
    assert!(result.is_err());
    assert!(
        !orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous::host_runtime_directory(
            &runtime
        )
        .exists()
    );
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
fn timeout(stream: &LocalStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
}
fn record(peer: &mut LocalStream) -> Vec<u8> {
    let mut header = [0; 64];
    peer.read_exact(&mut header).unwrap();
    let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    assert!((64..=2_000_000).contains(&length));
    let mut bytes = vec![0; length];
    bytes[..64].copy_from_slice(&header);
    peer.read_exact(&mut bytes[64..]).unwrap();
    bytes
}
fn reply(peer: &mut LocalStream, request: &[u8], kind: u16, payload: &[u8]) {
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
fn connected_host() -> (HostBrokerConnection, Rc<RefCell<AgentDrain>>, LocalStream) {
    let endpoint = crate::tests::endpoint();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let task = thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        timeout(&peer);
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
fn admitted_worker_survives_invalid_defaults_then_launches_fresh_policy_and_fences_replay() {
    let (mut host, drain, mut host_peer) = connected_host();
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        let bad = launch(1, "orchbee-nonexistent-default-shell.exe");
        send(&mut peer, &bad);
        let failed = outcome(&mut peer);
        assert!(matches!(failed, TerminalLaunchOutcome::Failed(_)));
        send(&mut peer, &bad);
        assert_eq!(outcome(&mut peer), failed);
        let good = launch(2, "cmd.exe");
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
        let conflicting = launch(2, "changed.exe");
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
    let mutations = crate::worktree_mutation_job::WorktreeMutationSlot::new(
        crate::worktree_mutation_windows::run,
    );
    let fallback = WindowsShellSettings::new("unavailable-startup.exe", "auto").unwrap();
    assert!(
        run_admitted(
            server,
            &mut host,
            WORKER,
            NonZeroU64::MIN,
            AuthorizedHostOperationKernel::new(WORKER, NonZeroU64::MIN).unwrap(),
            AdmittedAux {
                preferences: None,
                shell: &fallback,
                drain: &drain,
                worktree_slot: &worktrees,
                mutation_slot: &mutations,
                plugin_jobs: &crate::plugin_jobs::PluginJobs::new(),
                site_runtime: None,
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
        for name in [
            "orchbee",
            "orchbee-server",
            "orchbee-worker",
            "orchbee-terminal-host",
        ] {
            std::fs::write(owner.0.join(format!("{name}.exe")), b"native fixture\n").unwrap();
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
    let cohort = NativeCohort::load(&fixture.0, Instant::now() + Duration::from_secs(3)).unwrap();
    let runtime = fixture.0.join("runtime");
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        let bad = launch(1, "relative/shell.exe");
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
            None,
            &WindowsShellSettings::default()
        )
        .is_err()
    );
    driver.join().unwrap();
    assert!(
        !orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous::host_runtime_directory(
            &runtime
        )
        .exists()
    );
}

#[test]
fn lazy_handoff_retains_ticket_history_and_worker_identity_without_starting_host() {
    use node::{
        WorkerHandoffAction as Action, WorkerHandoffId, WorkerHandoffOutcome as Outcome,
        WorkerHandoffRefusal, WorkerHandoffRequest, WorkerHandoffResult, WorkerHandoffTarget,
    };
    use orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous;
    let fixture = CohortFixture::new();
    let cohort = NativeCohort::load(&fixture.0, Instant::now() + Duration::from_secs(3)).unwrap();
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
    let listener = LocalListener::bind(ModeCheckedLocalSiteRendezvous::handoff_endpoint_path(
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
        None,
        &WindowsShellSettings::default(),
    );
    original.join().unwrap();
    next.join().unwrap();
    assert!(
        result.is_err(),
        "ordinary new Server EOF is not another handoff"
    );
    assert!(!ModeCheckedLocalSiteRendezvous::host_runtime_directory(&runtime).exists());
}

#[test]
fn lazy_editor_command_query_echoes_refusals_and_current_resolution_without_starting_host() {
    let fixture = CohortFixture::new();
    let cohort = NativeCohort::load(&fixture.0, Instant::now() + Duration::from_secs(3)).unwrap();
    let runtime = fixture.0.join("runtime");
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        let request = TerminalEditorCommandRequest {
            request_id: OperationId::from_bytes([67; 16]).unwrap(),
            worker_id: node::WorkerId::from_bytes([3; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        };
        for field in 0..3 {
            let mut current = request;
            if field == 0 {
                current.worker_id = node::WorkerId::from_bytes([8; 16]).unwrap();
            }
            if field == 1 {
                current.worker_generation = WorkerGeneration::new(NonZeroU64::MAX);
            }
            node::write_message(
                &mut peer,
                &CapabilityNodeMessage::TerminalEditorCommandRequest(current),
            )
            .unwrap();
            let CapabilityNodeMessage::TerminalEditorCommandResult(result) =
                node::read_message(&mut peer).unwrap()
            else {
                panic!("missing editor response")
            };
            assert_eq!(result.request, current);
            match field {
                0 => assert_eq!(
                    result.outcome,
                    TerminalEditorCommandOutcome::Refused(
                        TerminalEditorCommandRefusal::WrongWorker
                    )
                ),
                1 => assert_eq!(
                    result.outcome,
                    TerminalEditorCommandOutcome::Refused(
                        TerminalEditorCommandRefusal::StaleWorkerGeneration
                    )
                ),
                _ => assert!(matches!(
                    result.outcome,
                    TerminalEditorCommandOutcome::Resolved { .. }
                        | TerminalEditorCommandOutcome::Refused(
                            TerminalEditorCommandRefusal::InvalidCommand
                                | TerminalEditorCommandRefusal::ProgramUnavailable
                        )
                )),
            }
        }
        peer.shutdown(std::net::Shutdown::Both).unwrap();
    });
    let result = lazy::run(
        server,
        WORKER,
        NonZeroU64::MIN,
        &cohort,
        &runtime,
        None,
        &WindowsShellSettings::default(),
    );
    driver.join().unwrap();
    assert!(result.is_err());
    assert!(
        !orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous::host_runtime_directory(
            &runtime
        )
        .exists()
    );
}

#[test]
fn admitted_editor_command_query_uses_no_host_request_or_terminal_effect() {
    let (mut host, drain, mut host_peer) = connected_host();
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        let request = TerminalEditorCommandRequest {
            request_id: OperationId::from_bytes([68; 16]).unwrap(),
            worker_id: node::WorkerId::from_bytes([3; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        };
        node::write_message(
            &mut peer,
            &CapabilityNodeMessage::TerminalEditorCommandRequest(request),
        )
        .unwrap();
        let CapabilityNodeMessage::TerminalEditorCommandResult(result) =
            node::read_message(&mut peer).unwrap()
        else {
            panic!("missing editor response")
        };
        assert_eq!(result.request, request);
        peer.shutdown(std::net::Shutdown::Both).unwrap();
    });
    let outcome = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        None,
        &WindowsShellSettings::default(),
    );
    driver.join().unwrap();
    assert!(outcome.is_err());
    host_peer
        .set_read_timeout(Some(Duration::from_millis(40)))
        .unwrap();
    let mut byte = [0];
    let error = host_peer.read(&mut byte).unwrap_err();
    assert!(matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ));
}

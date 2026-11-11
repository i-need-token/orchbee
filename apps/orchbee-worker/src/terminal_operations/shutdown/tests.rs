use super::*;
use crate::worktree_discovery_job::WorktreeDiscoverySlot;
use orchbee_capability_node_protocol::{
    OperationId, ShutdownBudget, WorkerGeneration, read_message, write_message,
};
use orchbee_local_ipc::LocalListener;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, Envelope, HelloAccepted, InventoryControl,
    LifecycleControl, MessageRole, PayloadEncoding, TerminalSummary, TypedFailure,
    decode_connection_control, decode_inventory_control, decode_lifecycle_control,
    decode_transport_envelope, encode_connection_control, encode_inventory_control,
    encode_lifecycle_control, encode_transport_envelope,
};
use std::{
    cell::RefCell,
    io::{Read, Write},
    rc::Rc,
};

fn request() -> WorkerShutdownRequest {
    WorkerShutdownRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(3).unwrap()),
        budget: ShutdownBudget::new(1_000).unwrap(),
        terminals: ShutdownTerminals::Dispose,
    }
}

#[test]
fn unmanaged_and_identity_fences_precede_stop_admission() {
    let request = request();
    let worker = WorkerId::from_bytes([2; 16]);
    let generation = NonZeroU64::new(3).unwrap();
    assert_eq!(refusal(request, worker, generation, true), None);
    assert_eq!(
        refusal(request, worker, generation, false),
        Some(ShutdownRefusal::Unmanaged)
    );
    assert_eq!(
        refusal(request, WorkerId::from_bytes([4; 16]), generation, true),
        Some(ShutdownRefusal::WrongWorker)
    );
    assert_eq!(
        refusal(request, worker, NonZeroU64::new(4).unwrap(), true),
        Some(ShutdownRefusal::WrongWorker)
    );
}

#[test]
fn pre_host_stop_delivers_exact_no_host_result_then_ends() {
    let (server, mut peer) = LocalStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let request = request();
    let consumer = thread::spawn(move || {
        assert_eq!(
            read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerShutdownResult(WorkerShutdownResult {
                request,
                outcome: WorkerShutdownOutcome::NoHost
            })
        );
    });
    let slot = WorktreeDiscoverySlot::new();
    run(
        &server,
        request,
        WorkerId::from_bytes([2; 16]),
        NonZeroU64::new(3).unwrap(),
        BackgroundJobs {
            discovery: &slot,
            integrations: &crate::agent_integration_jobs::AgentIntegrationJobs::new(),
            probes: &crate::agent_probe_jobs::AgentProbeJobs::new(),
            mutations: &crate::worktree_mutation_job::WorktreeMutationSlot::new(
                crate::worktree_mutation_windows::run,
            ),
            plugins: &crate::plugin_jobs::PluginJobs::new(),
        },
        None,
    )
    .unwrap();
    consumer.join().unwrap();
}

#[test]
fn wrong_fence_in_stop_mode_preserves_owner_for_next_explicit_request() {
    let (server, mut peer) = LocalStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let request = request();
    let wrong = WorkerShutdownRequest {
        worker_generation: WorkerGeneration::new(NonZeroU64::new(4).unwrap()),
        ..request
    };
    let consumer = thread::spawn(move || {
        assert_eq!(
            read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerShutdownResult(WorkerShutdownResult {
                request: wrong,
                outcome: WorkerShutdownOutcome::Refused(ShutdownRefusal::WrongWorker)
            })
        );
        write_message(
            &mut peer,
            &CapabilityNodeMessage::WorkerShutdownRequest(request),
        )
        .unwrap();
        assert_eq!(
            read_message(&mut peer).unwrap(),
            CapabilityNodeMessage::WorkerShutdownResult(WorkerShutdownResult {
                request,
                outcome: WorkerShutdownOutcome::NoHost
            })
        );
    });
    run(
        &server,
        wrong,
        WorkerId::from_bytes([2; 16]),
        NonZeroU64::new(3).unwrap(),
        BackgroundJobs {
            discovery: &WorktreeDiscoverySlot::new(),
            integrations: &crate::agent_integration_jobs::AgentIntegrationJobs::new(),
            probes: &crate::agent_probe_jobs::AgentProbeJobs::new(),
            mutations: &crate::worktree_mutation_job::WorktreeMutationSlot::new(
                crate::worktree_mutation_windows::run,
            ),
            plugins: &crate::plugin_jobs::PluginJobs::new(),
        },
        None,
    )
    .unwrap();
    consumer.join().unwrap();
}

fn read_record(peer: &mut LocalStream) -> Vec<u8> {
    let mut header = [0; 64];
    peer.read_exact(&mut header).unwrap();
    let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    assert!((64..=2_000_000).contains(&length));
    let mut record = vec![0; length];
    record[..64].copy_from_slice(&header);
    peer.read_exact(&mut record[64..]).unwrap();
    record
}

fn reply(peer: &mut LocalStream, request: &[u8], kind: u16, payload: &[u8]) {
    let envelope = decode_transport_envelope(request).unwrap();
    peer.write_all(
        &encode_transport_envelope(&Envelope {
            correlation_id: envelope.correlation_id,
            host_epoch: Some(orchbee_terminal_protocol::HostEpoch::from_bytes([4; 16]).unwrap()),
            kind,
            optional_message: false,
            payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Response,
        })
        .unwrap(),
    )
    .unwrap();
}

fn connected_host() -> (HostBrokerConnection, LocalStream) {
    let endpoint = crate::tests::endpoint();
    let listener = LocalListener::bind(&endpoint).unwrap();
    let peer = thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        crate::host_connection::answer_test_offer(&mut peer).unwrap();
        let record = read_record(&mut peer);
        let envelope = decode_transport_envelope(&record).unwrap();
        let ConnectionControl::Hello(hello) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload).unwrap()
        else {
            panic!("hello");
        };
        let accepted =
            encode_connection_control(&ConnectionControl::HelloAccepted(HelloAccepted {
                frame_bytes: hello.frame_bytes,
                host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([4; 16]).unwrap(),
                host_version: "0.1.0".into(),
                input_bytes: hello.input_bytes,
                protocol: hello.protocol,
                row_window_bytes: hello.row_window_bytes,
                supported_optional_features: vec![],
            }))
            .unwrap();
        reply(&mut peer, &record, accepted.kind, &accepted.payload);
        peer
    });
    let host = HostBrokerConnection::connect(
        &endpoint,
        Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())),
    )
    .unwrap();
    (host, peer.join().unwrap())
}

fn core(terminal: u8) -> orchbee_terminal_protocol::TerminalSummaryCore {
    use orchbee_terminal_protocol::{
        ColorScheme, TerminalConfiguration, TerminalIdentity, TerminalSummaryCore, TerminalTheme,
        TerminalThemeState,
    };
    TerminalSummaryCore {
        configuration: TerminalConfiguration {
            scrollback_bytes: 0,
        },
        configuration_revision: 0,
        identity: TerminalIdentity {
            creation_id: orchbee_terminal_protocol::CreationId::from_bytes([9; 16]).unwrap(),
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([4; 16]).unwrap(),
            terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([terminal; 16]).unwrap(),
        },
        revision: 42,
        theme: TerminalThemeState {
            operation_id: None,
            revision: 0,
            terminal: TerminalTheme {
                color_scheme: ColorScheme::Dark,
                default_background_rgb: 0,
                default_cursor_rgb: None,
                default_foreground_rgb: 0xffffff,
                palette: vec![0; 256],
            },
        },
    }
}

fn inventory(peer: &mut LocalStream, summary: Option<TerminalSummary>) {
    let record = read_record(peer);
    let envelope = decode_transport_envelope(&record).unwrap();
    let InventoryControl::ListTerminals(request) =
        decode_inventory_control(envelope.kind, envelope.role, envelope.payload).unwrap()
    else {
        panic!("inventory");
    };
    assert_eq!(request.cursor, None);
    assert_eq!(request.maximum_terminals, 1);
    let reply_control = encode_inventory_control(&InventoryControl::TerminalList(
        orchbee_terminal_protocol::TerminalList {
            inventory_generation: NonZeroU64::new(7).unwrap(),
            next_cursor: None,
            terminals: summary.into_iter().collect(),
        },
    ))
    .unwrap();
    reply(peer, &record, reply_control.kind, &reply_control.payload);
}

fn lifecycle(peer: &mut LocalStream, terminal: u8, dispose: bool) {
    let record = read_record(peer);
    let envelope = decode_transport_envelope(&record).unwrap();
    let target = orchbee_terminal_protocol::TerminalTarget {
        terminal_id: core(terminal).identity.terminal_id,
    };
    assert_eq!(
        decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload).unwrap(),
        if dispose {
            LifecycleControl::DisposeTerminal(target)
        } else {
            LifecycleControl::ForceTerminateTerminal(target)
        }
    );
    let accepted = encode_lifecycle_control(&if dispose {
        LifecycleControl::DisposeTerminalAccepted(target)
    } else {
        LifecycleControl::ForceTerminateTerminalAccepted(target)
    })
    .unwrap();
    reply(peer, &record, accepted.kind, &accepted.payload);
}

#[test]
fn inventory_force_settle_dispose_and_local_retirement_are_ordered() {
    use orchbee_terminal_protocol::{ExitResult, TerminalGeometry};
    let (host, mut peer) = connected_host();
    let (server, _server_peer) = LocalStream::pair().unwrap();
    let driver = thread::spawn(move || {
        let geometry = TerminalGeometry {
            columns: 80,
            rows: 24,
        };
        inventory(
            &mut peer,
            Some(TerminalSummary::Running {
                core: core(5),
                geometry,
                terminate_requested: false,
            }),
        );
        lifecycle(&mut peer, 5, false);
        inventory(
            &mut peer,
            Some(TerminalSummary::Finishing {
                core: core(5),
                geometry,
                terminate_requested: true,
                exit: None,
            }),
        );
        inventory(
            &mut peer,
            Some(TerminalSummary::Exited {
                core: core(5),
                geometry,
                terminate_requested: true,
                exit: ExitResult::ExitCode(1),
            }),
        );
        lifecycle(&mut peer, 5, true);
        inventory(
            &mut peer,
            Some(TerminalSummary::AwaitingStart { core: core(6) }),
        );
        lifecycle(&mut peer, 6, true);
        inventory(&mut peer, None);
    });
    let mut disposed = 0;
    assert!(
        host::drain(
            &host,
            &server,
            request().operation_id,
            Instant::now() + Duration::from_secs(4),
            &mut disposed
        )
        .is_ok()
    );
    assert_eq!(disposed, 2);
    for terminal in [5, 6] {
        assert!(host.disposed_terminal(
            orchbee_capability_node_protocol::HostTerminalId::from_bytes([terminal; 16]).unwrap()
        ));
    }
    driver.join().unwrap();
}

fn exited(terminal: u8) -> TerminalSummary {
    TerminalSummary::Exited {
        core: core(terminal),
        geometry: orchbee_terminal_protocol::TerminalGeometry {
            columns: 80,
            rows: 24,
        },
        terminate_requested: true,
        exit: orchbee_terminal_protocol::ExitResult::ExitCode(0),
    }
}

fn refuse_dispose(peer: &mut LocalStream, terminal: u8, code: &str) {
    let record = read_record(peer);
    let envelope = decode_transport_envelope(&record).unwrap();
    assert_eq!(
        decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload).unwrap(),
        LifecycleControl::DisposeTerminal(orchbee_terminal_protocol::TerminalTarget {
            terminal_id: core(terminal).identity.terminal_id,
        })
    );
    let refusal = encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
        code: code.to_owned(),
        message: "refused".to_owned(),
        retryable: true,
        reason: None,
    }))
    .unwrap();
    reply(peer, &record, refusal.kind, &refusal.payload);
}

/// The Host refuses a settled terminal's disposal until its next event poll publishes the
/// outcome; the same disposal is asked again and the stop completes in one attempt.
#[test]
fn unpublished_outcome_refusal_retries_the_same_disposal() {
    let (host, mut peer) = connected_host();
    let (server, _server_peer) = LocalStream::pair().unwrap();
    let driver = thread::spawn(move || {
        inventory(&mut peer, Some(exited(5)));
        refuse_dispose(&mut peer, 5, "outcome-publication-pending");
        lifecycle(&mut peer, 5, true);
        inventory(&mut peer, None);
    });
    let mut disposed = 0;
    assert!(
        host::drain(
            &host,
            &server,
            request().operation_id,
            Instant::now() + Duration::from_secs(4),
            &mut disposed
        )
        .is_ok()
    );
    assert_eq!(disposed, 1);
    assert!(host.disposed_terminal(
        orchbee_capability_node_protocol::HostTerminalId::from_bytes([5; 16]).unwrap()
    ));
    driver.join().unwrap();
}

/// Any other refusal, retryable or not, still ends the attempt as a Host refusal.
#[test]
fn other_disposal_refusal_ends_the_attempt() {
    let (host, mut peer) = connected_host();
    let (server, _server_peer) = LocalStream::pair().unwrap();
    let driver = thread::spawn(move || {
        inventory(&mut peer, Some(exited(5)));
        refuse_dispose(&mut peer, 5, "cleanup-contain-process");
    });
    let mut disposed = 0;
    let failure = host::drain(
        &host,
        &server,
        request().operation_id,
        Instant::now() + Duration::from_secs(4),
        &mut disposed,
    )
    .err()
    .unwrap();
    assert_eq!(failure.stage, ShutdownStage::Dispose);
    assert_eq!(failure.failure, ShutdownFailure::HostRefused);
    assert!(!failure.retire);
    assert_eq!(disposed, 0);
    assert!(!host.disposed_terminal(
        orchbee_capability_node_protocol::HostTerminalId::from_bytes([5; 16]).unwrap()
    ));
    driver.join().unwrap();
}

#[test]
fn lost_dispose_reply_preserves_known_progress_without_replay() {
    let (host, mut peer) = connected_host();
    let (server, _server_peer) = LocalStream::pair().unwrap();
    let driver = thread::spawn(move || {
        inventory(
            &mut peer,
            Some(TerminalSummary::AwaitingStart { core: core(5) }),
        );
        lifecycle(&mut peer, 5, true);
        inventory(
            &mut peer,
            Some(TerminalSummary::AwaitingStart { core: core(6) }),
        );
        let record = read_record(&mut peer);
        let envelope = decode_transport_envelope(&record).unwrap();
        assert!(matches!(
            decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload).unwrap(),
            LifecycleControl::DisposeTerminal(_)
        ));
        peer.shutdown(Shutdown::Both).unwrap();
    });
    let mut disposed = 0;
    let failure = host::drain(
        &host,
        &server,
        request().operation_id,
        Instant::now() + Duration::from_secs(4),
        &mut disposed,
    )
    .err()
    .unwrap();
    assert_eq!(failure.stage, ShutdownStage::Dispose);
    assert_eq!(failure.failure, ShutdownFailure::Transport);
    assert!(failure.retire);
    assert_eq!(disposed, 1);
    assert!(!host.disposed_terminal(
        orchbee_capability_node_protocol::HostTerminalId::from_bytes([6; 16]).unwrap()
    ));
    driver.join().unwrap();
}

#[test]
fn empty_inventory_is_insufficient_until_retained_child_exits() {
    let (host, mut peer) = connected_host();
    let (server, _server_peer) = LocalStream::pair().unwrap();
    let driver = thread::spawn(move || {
        inventory(&mut peer, None);
        let mut byte = [0];
        assert_eq!(peer.read(&mut byte).unwrap(), 0);
    });
    let executable = std::env::current_exe().unwrap();
    let fixture_name = concat!(module_path!(), "::retained_host_fixture")
        .split_once("::")
        .unwrap()
        .1;
    let arguments: Vec<_> = ["--ignored", "--exact", fixture_name, "--nocapture"]
        .into_iter()
        .map(std::ffi::OsStr::new)
        .collect();
    let child = orchbee_local_runtime::spawn_retained(&executable, &arguments).unwrap();
    let mut owner = ManagedHost::new(child);
    let first = owner.stop(
        &host,
        &server,
        request(),
        Instant::now() + Duration::from_millis(100),
    );
    let second = owner.stop(
        &host,
        &server,
        request(),
        Instant::now() + Duration::from_secs(5),
    );
    driver.join().unwrap();
    assert!(matches!(
        first,
        WorkerShutdownOutcome::Incomplete {
            stage: ShutdownStage::HostExit,
            failure: ShutdownFailure::Deadline,
            ..
        }
    ));
    assert!(matches!(
        second,
        WorkerShutdownOutcome::Stopped {
            disposed_terminals: 0,
            ..
        }
    ));
}

/// A Server `exit` reaches the Worker as a Keep shutdown: the Host receives no inventory,
/// containment or disposal request, only the closed broker connection, and its process is neither
/// awaited nor ended.
#[test]
fn keep_shutdown_releases_the_host_without_a_terminal_request() {
    let (host, mut peer) = connected_host();
    let (server, mut server_peer) = LocalStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    server_peer
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let host_side = thread::spawn(move || {
        let mut byte = [0];
        assert_eq!(
            peer.read(&mut byte).unwrap(),
            0,
            "the Host received a request"
        );
    });
    let executable = std::env::current_exe().unwrap();
    let fixture_name = concat!(module_path!(), "::retained_host_fixture")
        .split_once("::")
        .unwrap()
        .1;
    let arguments: Vec<_> = ["--ignored", "--exact", fixture_name, "--nocapture"]
        .into_iter()
        .map(std::ffi::OsStr::new)
        .collect();
    let mut owner =
        ManagedHost::new(orchbee_local_runtime::spawn_retained(&executable, &arguments).unwrap());
    let request = WorkerShutdownRequest {
        terminals: ShutdownTerminals::Keep,
        ..request()
    };
    run(
        &server,
        request,
        WorkerId::from_bytes([2; 16]),
        NonZeroU64::new(3).unwrap(),
        BackgroundJobs {
            discovery: &WorktreeDiscoverySlot::new(),
            integrations: &crate::agent_integration_jobs::AgentIntegrationJobs::new(),
            probes: &crate::agent_probe_jobs::AgentProbeJobs::new(),
            mutations: &crate::worktree_mutation_job::WorktreeMutationSlot::new(
                crate::worktree_mutation_windows::run,
            ),
            plugins: &crate::plugin_jobs::PluginJobs::new(),
        },
        Some((&host, &mut owner)),
    )
    .unwrap();
    assert_eq!(
        read_message(&mut server_peer).unwrap(),
        CapabilityNodeMessage::WorkerShutdownResult(WorkerShutdownResult {
            request,
            outcome: WorkerShutdownOutcome::Released {
                host_epoch: HostEpoch::from_bytes([4; 16]).unwrap(),
            },
        })
    );
    host_side.join().unwrap();
    // The Host child outlives the shutdown it was not asked to end.
    assert!(owner.child.try_wait().unwrap().is_none());
    while owner.child.try_wait().unwrap().is_none() {
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "child lifetime fixture launched by its owning parent test"]
fn retained_host_fixture() {
    thread::sleep(Duration::from_millis(700));
}

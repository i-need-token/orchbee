use super::*;
use crate::terminal_authorization::{HostOperationAdmission, LifecycleAdmission};
use orchbee_capability_node_protocol::{
    AuthorizedTerminalStart, HostCreationId, HostTerminalId, HostTerminalIdentity,
    LeaseExpiresAtUnixMilliseconds, LeaseId, LifecycleStepOrdinal, OperationId,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalFence,
    TerminalId, TerminalLifecycleAuthorization, TerminalOperationAuthorization,
    TerminalOperationLease, TerminalPixelGeometry, WorkerGeneration, WorkerId,
};
use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
use orchbee_terminal_protocol::{
    HelloAccepted, HostEpoch, TRANSPORT_ENVELOPE_HEADER_BYTES, encode_connection_control,
};
use std::{
    cell::RefCell,
    fs,
    io::{Read, Write},
    net::Shutdown,
    num::NonZeroU64,
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const BUDGET: Duration = Duration::from_secs(1);
static NEXT: AtomicU64 = AtomicU64::new(0);

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("fixture nonzero")
}

fn fresh_drain() -> Rc<RefCell<crate::agent_drain::AgentDrain>> {
    Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new()))
}
fn request(action: TerminalLifecycleAction) -> TestResult<TerminalLifecycleRequest> {
    Ok(TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: TerminalId::from_bytes([1; 16])?,
            worker_id: WorkerId::from_bytes([3; 16])?,
            worker_generation: WorkerGeneration::new(nz(7)),
            binding_fence: TerminalFence::new(nz(1)),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes([8; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(nz(u64::MAX)),
            ),
        },
        close_operation_id: OperationId::from_bytes([20; 16])?,
        request_id: OperationId::from_bytes([30; 16])?,
        attempt_generation: nz(1),
        step_ordinal: LifecycleStepOrdinal::new(1)?,
        target: HostTerminalIdentity::new(
            HostCreationId::from_bytes([2; 16])?,
            CapabilityHostEpoch::from_bytes([4; 16])?,
            HostTerminalId::from_bytes([5; 16])?,
        ),
        action,
    })
}
fn kernel() -> TestResult<AuthorizedHostOperationKernel> {
    let request = request(TerminalLifecycleAction::Dispose)?;
    let mut kernel = AuthorizedHostOperationKernel::new(
        orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
        nz(7),
    )?;
    let authorization = request.authorization;
    let start = AuthorizedTerminalStart::new(
        TerminalOperationAuthorization::new(
            authorization.terminal_id,
            OperationId::from_bytes([9; 16])?,
            authorization.worker_id,
            authorization.worker_generation,
            authorization.binding_fence,
            authorization.lease,
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        ),
        request.target,
        TerminalPixelGeometry::new(80, 24, 8, 16)?,
    );
    let HostOperationAdmission::Dispatch(dispatch) =
        kernel.admit_start(start, 100, request.target.host_epoch())?
    else {
        panic!("start");
    };
    kernel.record_pre_handoff_failure(&dispatch, 100)?;
    assert!(kernel.take_result().is_some());
    Ok(kernel)
}
fn dispatch(
    kernel: &mut AuthorizedHostOperationKernel,
    request: TerminalLifecycleRequest,
) -> TestResult<LifecycleDispatch> {
    let LifecycleAdmission::Dispatch(dispatch) =
        kernel.admit_lifecycle(request, 100, request.target.host_epoch())?
    else {
        panic!("dispatch");
    };
    Ok(dispatch)
}
fn wire(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
    epoch: HostEpoch,
    correlation: CorrelationId,
) -> TestResult<Vec<u8>> {
    Ok(encode_transport_envelope(&Envelope {
        kind,
        role,
        payload,
        host_epoch: Some(epoch),
        correlation_id: Some(correlation),
        optional_message: false,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
    })?)
}
fn ack(request: TerminalLifecycleRequest) -> TestResult<Vec<u8>> {
    let target = TerminalTarget {
        terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
            request.target.terminal_id().as_bytes(),
        )?,
    };
    let control = encode_lifecycle_control(&match request.action {
        TerminalLifecycleAction::Terminate => LifecycleControl::TerminateTerminalAccepted(target),
        TerminalLifecycleAction::Force => LifecycleControl::ForceTerminateTerminalAccepted(target),
        TerminalLifecycleAction::Dispose => LifecycleControl::DisposeTerminalAccepted(target),
        TerminalLifecycleAction::InventoryPage { .. } => panic!("mutation fixture"),
    })?;
    wire(
        control.kind,
        control.role,
        &control.payload,
        HostEpoch::from_bytes([4; 16])?,
        CorrelationId::from_bytes(request.request_id.as_bytes())?,
    )
}
fn read_record(peer: &mut UnixStream) -> TestResult<Vec<u8>> {
    let mut header = [0; TRANSPORT_ENVELOPE_HEADER_BYTES];
    peer.read_exact(&mut header)?;
    let length = u32::from_le_bytes(header[8..12].try_into()?) as usize;
    if !(64..=2_000_000).contains(&length) {
        return Err("fixture frame length".into());
    }
    let mut record = vec![0; length];
    record[..64].copy_from_slice(&header);
    peer.read_exact(&mut record[64..])?;
    Ok(record)
}
fn no_bytes(peer: &mut UnixStream) -> TestResult {
    peer.set_nonblocking(true)?;
    let mut byte = [0];
    assert_eq!(
        peer.read(&mut byte).expect_err("no extra request").kind(),
        io::ErrorKind::WouldBlock
    );
    peer.set_nonblocking(false)?;
    Ok(())
}
struct Fixture {
    host: HostBrokerConnection,
    drain: Rc<RefCell<crate::agent_drain::AgentDrain>>,
    peer: UnixStream,
    server: UnixStream,
    server_peer: UnixStream,
    directory: PathBuf,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = PathBuf::from(format!(
            "/tmp/oblifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory)?;
        #[cfg(unix)]
        let endpoint = directory.join("host.s");
        #[cfg(windows)]
        let endpoint = crate::tests::endpoint();
        let listener = UnixListener::bind(&endpoint)?;
        let thread = thread::spawn(move || -> Result<UnixStream, String> {
            let run = || -> TestResult<UnixStream> {
                let (mut peer, _) = listener.accept()?;
                peer.set_read_timeout(Some(Duration::from_secs(3)))?;
                peer.set_write_timeout(Some(Duration::from_secs(3)))?;
                let record = read_record(&mut peer)?;
                let envelope = decode_transport_envelope(&record)?;
                let ConnectionControl::Hello(hello) =
                    decode_connection_control(envelope.kind, envelope.role, envelope.payload)?
                else {
                    panic!("Hello");
                };
                let epoch = HostEpoch::from_bytes([4; 16])?;
                let control =
                    encode_connection_control(&ConnectionControl::HelloAccepted(HelloAccepted {
                        frame_bytes: hello.frame_bytes,
                        host_epoch: epoch,
                        host_version: "0.1.0".into(),
                        input_bytes: hello.input_bytes,
                        protocol: hello.protocol,
                        row_window_bytes: hello.row_window_bytes,
                        supported_optional_features: Vec::new(),
                    }))?;
                peer.write_all(&wire(
                    control.kind,
                    control.role,
                    &control.payload,
                    epoch,
                    envelope.correlation_id.ok_or("correlation")?,
                )?)?;
                Ok(peer)
            };
            run().map_err(|e| e.to_string())
        });
        let drain = fresh_drain();
        let host = HostBrokerConnection::connect(&endpoint, Rc::clone(&drain))?;
        let peer = thread
            .join()
            .map_err(|_| "handshake panic")?
            .map_err(io::Error::other)?;
        let (server, server_peer) = UnixStream::pair()?;
        Ok(Self {
            host,
            drain,
            peer,
            server,
            server_peer,
            directory,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn exact_mutation_ack_is_one_request_and_cached_replay_has_no_host_write() -> TestResult {
    for (action, outcome) in [
        (
            TerminalLifecycleAction::Terminate,
            TerminalLifecycleOutcome::TerminateAccepted,
        ),
        (
            TerminalLifecycleAction::Force,
            TerminalLifecycleOutcome::ForceAccepted,
        ),
        (
            TerminalLifecycleAction::Dispose,
            TerminalLifecycleOutcome::Disposed,
        ),
    ] {
        let mut fixture = Fixture::new()?;
        let mut kernel = kernel()?;
        let request = request(action)?;
        let operation =
            PreparedLifecycleOperation::new(dispatch(&mut kernel, request)?, &fixture.host)?;
        let mut peer = fixture.peer.try_clone()?;
        let reply = ack(request)?;
        let responder = thread::spawn(move || -> Result<(), String> {
            let record = read_record(&mut peer).map_err(|e| e.to_string())?;
            let envelope = decode_transport_envelope(&record).map_err(|e| e.to_string())?;
            assert_eq!(
                envelope.correlation_id.map(|id| *id.as_bytes()),
                Some(request.request_id.as_bytes())
            );
            let command = decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)
                .map_err(|e| e.to_string())?;
            assert!(matches!(
                command,
                LifecycleControl::TerminateTerminal(_)
                    | LifecycleControl::ForceTerminateTerminal(_)
                    | LifecycleControl::DisposeTerminal(_)
            ));
            peer.write_all(&reply).map_err(|e| e.to_string())
        });
        assert_eq!(
            operation.execute(&mut kernel, &fixture.host, &fixture.server, || 100, BUDGET)?,
            HostOperationDisposition::ResultReady
        );
        responder
            .join()
            .map_err(|_| "responder panic")?
            .map_err(io::Error::other)?;
        let result = kernel.take_lifecycle_result().ok_or("result")?;
        assert_eq!(result.outcome(), outcome);
        assert_eq!(
            kernel.admit_lifecycle(request, 200, request.target.host_epoch())?,
            LifecycleAdmission::ResultReady
        );
        assert_eq!(kernel.take_lifecycle_result(), Some(result));
        no_bytes(&mut fixture.peer)?;
    }
    Ok(())
}

#[test]
fn host_ack_checks_envelope_action_and_terminal_without_invented_creation_echo() -> TestResult {
    let request = request(TerminalLifecycleAction::Dispose)?;
    let valid = ack(request)?;
    assert_eq!(
        response_outcome(request, &valid)?,
        TerminalLifecycleOutcome::Disposed
    );
    for (offset, value) in [(12, 0_u8), (20, 12), (24, 99), (40, 99), (4, 99)] {
        let mut invalid = valid.clone();
        invalid[offset] = value;
        assert!(
            response_outcome(request, &invalid).is_err(),
            "offset {offset}"
        );
    }
    let wrong = TerminalLifecycleRequest {
        target: HostTerminalIdentity::new(
            request.target.creation_id(),
            request.target.host_epoch(),
            HostTerminalId::from_bytes([99; 16])?,
        ),
        ..request
    };
    assert!(response_outcome(wrong, &valid).is_err());
    // Creation identity belongs to the retained authorization, not the Host ACK payload.
    assert!(std::str::from_utf8(&valid[64..])?.contains("terminalId"));
    assert!(!std::str::from_utf8(&valid[64..])?.contains("creationId"));
    Ok(())
}

#[test]
fn prehandoff_loss_and_deadline_after_handoff_have_distinct_truthful_results() -> TestResult {
    let fixture = Fixture::new()?;
    let mut kernel = kernel()?;
    let request = request(TerminalLifecycleAction::Dispose)?;
    let operation =
        PreparedLifecycleOperation::new(dispatch(&mut kernel, request)?, &fixture.host)?;
    fixture.peer.shutdown(Shutdown::Both)?;
    assert_eq!(
        operation.execute(&mut kernel, &fixture.host, &fixture.server, || 100, BUDGET)?,
        HostOperationDisposition::HostClosed
    );
    assert_eq!(
        kernel
            .take_lifecycle_result()
            .ok_or("preflight result")?
            .outcome(),
        TerminalLifecycleOutcome::HostUnavailableBeforeHandoff
    );
    let fixture = Fixture::new()?;
    let mut kernel = self::kernel()?;
    let operation =
        PreparedLifecycleOperation::new(dispatch(&mut kernel, request)?, &fixture.host)?;
    assert_eq!(
        operation.execute(
            &mut kernel,
            &fixture.host,
            &fixture.server,
            || 100,
            Duration::from_millis(10)
        )?,
        HostOperationDisposition::HostClosed
    );
    assert_eq!(
        kernel
            .take_lifecycle_result()
            .ok_or("timeout result")?
            .outcome(),
        TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::Deadline)
    );
    assert!(!fixture.host.disposed_terminal(request.target.terminal_id()));
    Ok(())
}

#[test]
fn typed_failures_remain_known_and_never_imply_disposal() -> TestResult {
    for (action, code, expected) in [
        (
            TerminalLifecycleAction::Dispose,
            "cleanup-incomplete",
            TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::CleanupIncomplete),
        ),
        (
            TerminalLifecycleAction::Force,
            "invalid-phase",
            TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::HostRefused),
        ),
        (
            TerminalLifecycleAction::InventoryPage { cursor: None },
            "stale-inventory-cursor",
            TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::StaleCursor),
        ),
    ] {
        let request = request(action)?;
        let control = encode_connection_control(&ConnectionControl::TypedFailure(
            orchbee_terminal_protocol::TypedFailure {
                code: code.into(),
                message: "known refusal".into(),
                retryable: true,
            },
        ))?;
        let record = wire(
            control.kind,
            control.role,
            &control.payload,
            HostEpoch::from_bytes([4; 16])?,
            CorrelationId::from_bytes(request.request_id.as_bytes())?,
        )?;
        assert_eq!(response_outcome(request, &record)?, expected);
    }
    Ok(())
}

#[test]
fn server_loss_after_known_dispose_cannot_erase_retained_receipt() -> TestResult {
    let fixture = Fixture::new()?;
    let mut kernel = kernel()?;
    let request = request(TerminalLifecycleAction::Dispose)?;
    let operation =
        PreparedLifecycleOperation::new(dispatch(&mut kernel, request)?, &fixture.host)?;
    let mut peer = fixture.peer.try_clone()?;
    let reply = ack(request)?;
    let responder = thread::spawn(move || -> Result<(), String> {
        read_record(&mut peer).map_err(|e| e.to_string())?;
        peer.write_all(&reply).map_err(|e| e.to_string())
    });
    assert_eq!(
        operation.execute(&mut kernel, &fixture.host, &fixture.server, || 100, BUDGET)?,
        HostOperationDisposition::ResultReady
    );
    responder
        .join()
        .map_err(|_| "responder")?
        .map_err(io::Error::other)?;
    fixture.server_peer.shutdown(Shutdown::Both)?;
    let result = kernel.take_lifecycle_result().ok_or("known result")?;
    assert_eq!(result.outcome(), TerminalLifecycleOutcome::Disposed);
    assert!(fixture.host.disposed_terminal(request.target.terminal_id()));
    assert_eq!(
        kernel.admit_lifecycle(request, 200, request.target.host_epoch())?,
        LifecycleAdmission::ResultReady
    );
    assert_eq!(kernel.take_lifecycle_result(), Some(result));
    Ok(())
}

#[test]
fn real_dispatch_loop_bypasses_unrelated_closed_viewer_cleanup_for_one_lifecycle_step() -> TestResult
{
    use orchbee_capability_node_protocol::{
        CapabilityNodeMessage, TerminalViewerAction, TerminalViewerRequest, read_message,
        write_message,
    };
    let mut fixture = Fixture::new()?;
    let unrelated = TerminalViewerRequest {
        request_id: OperationId::from_bytes([61; 16])?,
        attachment_id: OperationId::from_bytes([60; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(nz(7)),
        action: TerminalViewerAction::Attach {
            host_epoch: CapabilityHostEpoch::from_bytes([4; 16])?,
            terminal_id: HostTerminalId::from_bytes([99; 16])?,
        },
    };
    fixture
        .host
        .viewers
        .borrow_mut()
        .admit(&unrelated)
        .map_err(|e| format!("{e:?}"))?;
    let mut payload = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))
    .to_vec();
    payload[6] = 1;
    payload[56..64].fill(255);
    payload[16..32].fill(4);
    payload[32..48].fill(60);
    payload[48..56].copy_from_slice(&1_u64.to_le_bytes());
    payload[64..72].copy_from_slice(&5_u64.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    let event = encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(fixture.host.host_epoch()),
        kind: 41,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::TerminalFrame,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })?;
    fixture.host.receive_event(event.clone())?;
    fixture.host.receive_event(event)?;
    assert_eq!(fixture.host.viewers.borrow().closed_ids(), vec![[60; 16]]);
    let request = request(TerminalLifecycleAction::Dispose)?;
    let kernel = kernel()?;
    let mut server_peer = fixture.server_peer.try_clone()?;
    server_peer.set_read_timeout(Some(Duration::from_secs(3)))?;
    server_peer.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut host_peer = fixture.peer.try_clone()?;
    // The admitted loop must run on this thread: the connection's drain owner
    // is Rc-shared and cannot cross threads. The spawned thread drives both
    // socket peers instead.
    let runner = thread::spawn(move || -> Result<(), String> {
        let mut run = || -> TestResult {
            write_message(
                &mut server_peer,
                &CapabilityNodeMessage::TerminalLifecycleRequest(request),
            )?;
            let record = read_record(&mut host_peer)?;
            let envelope = decode_transport_envelope(&record)?;
            assert!(matches!(
                decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)?,
                LifecycleControl::DisposeTerminal(_)
            ));
            host_peer.write_all(&ack(request)?)?;
            let CapabilityNodeMessage::TerminalLifecycleResult(result) =
                read_message(&mut server_peer)?
            else {
                panic!("lifecycle result");
            };
            assert_eq!(result.request(), request);
            assert_eq!(result.outcome(), TerminalLifecycleOutcome::Disposed);
            no_bytes(&mut host_peer)?;
            server_peer.shutdown(Shutdown::Both)?;
            Ok(())
        };
        run().map_err(|e| e.to_string())
    });
    fixture.server.set_nonblocking(true)?;
    fixture.host.stream().set_nonblocking(true)?;
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let outcome = crate::terminal_operations::run_admitted(
        fixture.server.try_clone()?,
        &mut fixture.host,
        orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
        nz(7),
        kernel,
        crate::terminal_operations::AdmittedAux {
            preferences: None,
            #[cfg(windows)]
            shell: &Default::default(),
            drain: &fixture.drain,
            worktree_slot: &worktree_slot,
            #[cfg(windows)]
            mutation_slot: &crate::worktree_mutation_job::WorktreeMutationSlot::new(
                crate::worktree_mutation_windows::run,
            ),
            #[cfg(windows)]
            plugin_jobs: &crate::plugin_jobs::PluginJobs::new(),
            site_runtime: None,
            #[cfg(windows)]
            managed_host: None,
        },
    );
    assert!(outcome.is_err());
    runner.join().map_err(|_| "peer driver panic")??;
    Ok(())
}

#[test]
fn inventory_is_one_fixed_128_page_and_invalid_progress_is_known_failure() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut kernel = kernel()?;
    let request = request(TerminalLifecycleAction::InventoryPage { cursor: None })?;
    let operation =
        PreparedLifecycleOperation::new(dispatch(&mut kernel, request)?, &fixture.host)?;
    let mut peer = fixture.peer.try_clone()?;
    let responder = thread::spawn(move || -> Result<(), String> {
        let mut run = || -> TestResult {
            let record = read_record(&mut peer)?;
            let envelope = decode_transport_envelope(&record)?;
            let InventoryControl::ListTerminals(query) =
                decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?
            else {
                panic!("one page query");
            };
            assert_eq!(query.maximum_terminals, 128);
            assert_eq!(query.cursor, None);
            let response = encode_inventory_control(&InventoryControl::TerminalList(
                orchbee_terminal_protocol::TerminalList {
                    inventory_generation: nz(7),
                    next_cursor: None,
                    terminals: Vec::new(),
                },
            ))?;
            peer.write_all(&wire(
                response.kind,
                response.role,
                &response.payload,
                HostEpoch::from_bytes([4; 16])?,
                envelope.correlation_id.ok_or("query correlation")?,
            )?)?;
            Ok(())
        };
        run().map_err(|e| e.to_string())
    });
    assert_eq!(
        operation.execute(&mut kernel, &fixture.host, &fixture.server, || 100, BUDGET)?,
        HostOperationDisposition::ResultReady
    );
    responder
        .join()
        .map_err(|_| "query peer")?
        .map_err(io::Error::other)?;
    assert_eq!(
        kernel
            .take_lifecycle_result()
            .ok_or("query result")?
            .outcome(),
        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
            inventory_generation: nz(7),
            next_cursor: None,
            returned_count: 0,
            target: None,
        })
    );
    no_bytes(&mut fixture.peer)?;
    for payload in [
        br#"{"inventoryGeneration":"7","nextCursor":{"inventoryGeneration":"7","offset":"1"},"terminals":[]}"#.as_slice(),
        br#"{"inventoryGeneration":"7","nextCursor":null,"terminals":[],"unknown":true}"#.as_slice(),
    ] {
        assert_eq!(inventory_outcome(request,10,MessageRole::Response,payload),TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage));
    }
    Ok(())
}

fn summary_core(
    terminal: u8,
    creation: u8,
) -> TestResult<orchbee_terminal_protocol::TerminalSummaryCore> {
    use orchbee_terminal_protocol::{
        ColorScheme, TerminalConfiguration, TerminalIdentity, TerminalSummaryCore, TerminalTheme,
        TerminalThemeState,
    };
    Ok(TerminalSummaryCore {
        configuration: TerminalConfiguration {
            scrollback_bytes: 0,
        },
        configuration_revision: 0,
        identity: TerminalIdentity {
            creation_id: orchbee_terminal_protocol::CreationId::from_bytes([creation; 16])?,
            host_epoch: HostEpoch::from_bytes([4; 16])?,
            terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([terminal; 16])?,
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
    })
}
fn page_outcome(
    request: TerminalLifecycleRequest,
    terminals: Vec<TerminalSummary>,
    next_cursor: Option<TerminalInventoryCursor>,
) -> TestResult<TerminalLifecycleOutcome> {
    let control = encode_inventory_control(&InventoryControl::TerminalList(
        orchbee_terminal_protocol::TerminalList {
            inventory_generation: nz(7),
            next_cursor,
            terminals,
        },
    ))?;
    Ok(inventory_outcome(
        request,
        control.kind,
        control.role,
        &control.payload,
    ))
}

#[test]
fn inventory_projects_every_exact_target_phase_and_rejects_conflicting_identities() -> TestResult {
    use orchbee_terminal_protocol::{
        ExitResult, FailureContainment, ProcessObservation, RuntimeFailure, TerminalGeometry,
    };
    let request = request(TerminalLifecycleAction::InventoryPage { cursor: None })?;
    let core = summary_core(5, 2)?;
    let geometry = TerminalGeometry {
        columns: 80,
        rows: 24,
    };
    for (summary, phase) in [
        (
            TerminalSummary::AwaitingStart { core: core.clone() },
            LifecycleTerminalPhase::AwaitingStart,
        ),
        (
            TerminalSummary::Running {
                core: core.clone(),
                geometry,
                terminate_requested: true,
            },
            LifecycleTerminalPhase::Running {
                terminate_requested: true,
            },
        ),
        (
            TerminalSummary::Finishing {
                core: core.clone(),
                geometry,
                exit: None,
                terminate_requested: true,
            },
            LifecycleTerminalPhase::Finishing {
                terminate_requested: true,
            },
        ),
        (
            TerminalSummary::ContainingFailure {
                core: core.clone(),
                geometry,
                failure: RuntimeFailure::ReadOutput,
                failure_containment: FailureContainment::Stopping { process: None },
            },
            LifecycleTerminalPhase::ContainingFailure,
        ),
        (
            TerminalSummary::Exited {
                core: core.clone(),
                geometry,
                exit: ExitResult::ExitCode(0),
                terminate_requested: false,
            },
            LifecycleTerminalPhase::Exited {
                terminate_requested: false,
            },
        ),
        (
            TerminalSummary::Failed {
                core: core.clone(),
                geometry,
                failure: RuntimeFailure::ReadOutput,
                process: ProcessObservation::Unavailable,
            },
            LifecycleTerminalPhase::Failed,
        ),
    ] {
        let TerminalLifecycleOutcome::InventoryPage(page) =
            page_outcome(request, vec![summary], None)?
        else {
            panic!("exact page");
        };
        assert_eq!(
            page.target,
            Some(LifecycleTargetObservation {
                revision: 42,
                phase
            })
        );
    }
    for (terminal, creation) in [(5, 99), (99, 2)] {
        assert_eq!(
            page_outcome(
                request,
                vec![TerminalSummary::AwaitingStart {
                    core: summary_core(terminal, creation)?
                }],
                None
            )?,
            TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::IdentityConflict)
        );
    }
    let exact = TerminalSummary::AwaitingStart { core };
    assert_eq!(
        page_outcome(request, vec![exact.clone(), exact], None)?,
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::IdentityConflict)
    );
    let unrelated = TerminalSummary::AwaitingStart {
        core: summary_core(99, 99)?,
    };
    assert_eq!(
        page_outcome(request, vec![unrelated.clone(), unrelated], None)?,
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage)
    );
    Ok(())
}

#[test]
fn viewer_lifecycle_reads_only_exact_settlement_without_acquiring_a_close_owner() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    };
    use orchbee_terminal_protocol::{
        ExitResult, FailureContainment, ProcessObservation, RuntimeFailure, TerminalGeometry,
    };
    let core = summary_core(5, 2)?;
    let geometry = TerminalGeometry {
        columns: 80,
        rows: 24,
    };
    for (summary, settled) in [
        (TerminalSummary::AwaitingStart { core: core.clone() }, false),
        (
            TerminalSummary::Running {
                core: core.clone(),
                geometry,
                terminate_requested: false,
            },
            false,
        ),
        (
            TerminalSummary::Finishing {
                core: core.clone(),
                geometry,
                exit: None,
                terminate_requested: true,
            },
            false,
        ),
        (
            TerminalSummary::ContainingFailure {
                core: core.clone(),
                geometry,
                failure: RuntimeFailure::ReadOutput,
                failure_containment: FailureContainment::Stopping { process: None },
            },
            false,
        ),
        (
            TerminalSummary::Exited {
                core: core.clone(),
                geometry,
                exit: ExitResult::ExitCode(0),
                terminate_requested: false,
            },
            true,
        ),
        (
            TerminalSummary::Failed {
                core: core.clone(),
                geometry,
                failure: RuntimeFailure::ReadOutput,
                process: ProcessObservation::Unavailable,
            },
            true,
        ),
    ] {
        let fixture = Fixture::new()?;
        let lifecycle = request(TerminalLifecycleAction::Dispose)?;
        let attach = TerminalViewerRequest {
            request_id: OperationId::from_bytes([61; 16])?,
            attachment_id: OperationId::from_bytes([60; 16])?,
            worker_id: lifecycle.authorization.worker_id,
            worker_generation: lifecycle.authorization.worker_generation,
            action: TerminalViewerAction::Attach {
                host_epoch: lifecycle.target.host_epoch(),
                terminal_id: lifecycle.target.terminal_id(),
            },
        };
        fixture.host.viewers.borrow_mut().admit(&attach).unwrap();
        let mut peer = fixture.peer.try_clone()?;
        let task = thread::spawn(move || -> Result<(), String> {
            let run = || -> TestResult {
                let bytes = read_record(&mut peer)?;
                let envelope = decode_transport_envelope(&bytes)?;
                assert_eq!(
                    decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?,
                    InventoryControl::ListTerminals(ListTerminals {
                        cursor: None,
                        maximum_terminals: 128
                    })
                );
                let control = encode_inventory_control(&InventoryControl::TerminalList(
                    orchbee_terminal_protocol::TerminalList {
                        inventory_generation: nz(7),
                        next_cursor: None,
                        terminals: vec![summary],
                    },
                ))?;
                peer.write_all(&wire(
                    control.kind,
                    control.role,
                    &control.payload,
                    HostEpoch::from_bytes([4; 16])?,
                    envelope.correlation_id.ok_or("correlation")?,
                )?)?;
                Ok(())
            };
            run().map_err(|e| e.to_string())
        });
        let read = TerminalViewerRequest {
            action: TerminalViewerAction::ReadLifecycle {
                target: lifecycle.target,
            },
            ..attach
        };
        let result = crate::terminal_viewer_operation::execute(
            read,
            orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
            nz(7),
            &fixture.host,
            &fixture.server,
        );
        task.join()
            .map_err(|_| "observation peer panic")?
            .map_err(io::Error::other)?;
        assert_eq!(
            result?.outcome,
            TerminalViewerOutcome::LifecycleObserved {
                target: lifecycle.target,
                revision: 42,
                settled
            }
        );
        fixture
            .host
            .viewers
            .borrow_mut()
            .remove(attach.attachment_id);
        // The read path has no lifecycle-kernel authority. Its first real close can still
        // acquire an independent owner and must reach Host exactly once.
        let mut kernel = kernel()?;
        assert!(matches!(
            kernel.admit_lifecycle(lifecycle, 100, lifecycle.target.host_epoch())?,
            LifecycleAdmission::Dispatch(_)
        ));
    }
    Ok(())
}

#[test]
fn viewer_lifecycle_absence_and_recycled_host_identity_never_mean_exit() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerRejection,
    };
    for terminals in [
        vec![],
        vec![TerminalSummary::AwaitingStart {
            core: summary_core(5, 99)?,
        }],
        vec![TerminalSummary::AwaitingStart {
            core: summary_core(99, 2)?,
        }],
    ] {
        let fixture = Fixture::new()?;
        let target = request(TerminalLifecycleAction::Dispose)?.target;
        let attach = TerminalViewerRequest {
            request_id: OperationId::from_bytes([61; 16])?,
            attachment_id: OperationId::from_bytes([60; 16])?,
            worker_id: WorkerId::from_bytes([3; 16])?,
            worker_generation: WorkerGeneration::new(nz(7)),
            action: TerminalViewerAction::Attach {
                host_epoch: target.host_epoch(),
                terminal_id: target.terminal_id(),
            },
        };
        fixture.host.viewers.borrow_mut().admit(&attach).unwrap();
        let empty = terminals.is_empty();
        let mut peer = fixture.peer.try_clone()?;
        let task = thread::spawn(move || -> Result<(), String> {
            let run = || -> TestResult {
                let bytes = read_record(&mut peer)?;
                let envelope = decode_transport_envelope(&bytes)?;
                let control = encode_inventory_control(&InventoryControl::TerminalList(
                    orchbee_terminal_protocol::TerminalList {
                        inventory_generation: nz(7),
                        next_cursor: None,
                        terminals,
                    },
                ))?;
                peer.write_all(&wire(
                    control.kind,
                    control.role,
                    &control.payload,
                    HostEpoch::from_bytes([4; 16])?,
                    envelope.correlation_id.ok_or("correlation")?,
                )?)?;
                Ok(())
            };
            run().map_err(|e| e.to_string())
        });
        let result = crate::terminal_viewer_operation::execute(
            TerminalViewerRequest {
                action: TerminalViewerAction::ReadLifecycle { target },
                ..attach
            },
            orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
            nz(7),
            &fixture.host,
            &fixture.server,
        );
        task.join()
            .map_err(|_| "observation peer panic")?
            .map_err(io::Error::other)?;
        if empty {
            assert_eq!(
                result?.outcome,
                TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable)
            );
        } else {
            assert!(result.is_err());
        }
        fixture
            .host
            .viewers
            .borrow_mut()
            .remove(attach.attachment_id);
    }
    Ok(())
}

#[test]
fn inventory_enforces_record_bytes_and_cursor_generation_offset_without_an_extra_query()
-> TestResult {
    let request = request(TerminalLifecycleAction::InventoryPage {
        cursor: Some(LifecycleInventoryCursor {
            inventory_generation: nz(7),
            offset: 128,
        }),
    })?;
    let mut summaries = Vec::new();
    for id in 1..=32 {
        summaries.push(TerminalSummary::AwaitingStart {
            core: summary_core(id, id)?,
        });
    }
    // This page deliberately omits both target identities.
    for summary in &mut summaries {
        if let TerminalSummary::AwaitingStart { core } = summary {
            if *core.identity.creation_id.as_bytes() == [2; 16] {
                core.identity.creation_id =
                    orchbee_terminal_protocol::CreationId::from_bytes([200; 16])?;
            }
            if *core.identity.terminal_id.as_bytes() == [5; 16] {
                core.identity.terminal_id =
                    orchbee_terminal_protocol::TerminalId::from_bytes([201; 16])?;
            }
        }
    }
    let TerminalLifecycleOutcome::InventoryPage(page) = page_outcome(
        request,
        summaries.clone(),
        Some(TerminalInventoryCursor {
            inventory_generation: nz(7),
            offset: 160,
        }),
    )?
    else {
        panic!("bounded page");
    };
    assert_eq!(page.returned_count, 32);
    assert_eq!(page.target, None);
    assert_eq!(
        page_outcome(
            request,
            summaries.clone(),
            Some(TerminalInventoryCursor {
                inventory_generation: nz(7),
                offset: 159
            })
        )?,
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage)
    );
    let changed = TerminalLifecycleRequest {
        action: TerminalLifecycleAction::InventoryPage {
            cursor: Some(LifecycleInventoryCursor {
                inventory_generation: nz(8),
                offset: 128,
            }),
        },
        ..request
    };
    assert_eq!(
        page_outcome(changed, summaries.clone(), None)?,
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage)
    );
    // The request maximum is 128, but full summaries also obey the control byte
    // bound. This fixture reaches that byte limit before 128 serialized rows.
    let oversized_rows = (0..128).map(|_| summaries[0].clone()).collect();
    assert!(
        encode_inventory_control(&InventoryControl::TerminalList(
            orchbee_terminal_protocol::TerminalList {
                inventory_generation: nz(7),
                next_cursor: None,
                terminals: oversized_rows,
            }
        ))
        .is_err()
    );
    // Construct a 129-row external payload without using the validating encoder.
    let one = encode_inventory_control(&InventoryControl::TerminalList(
        orchbee_terminal_protocol::TerminalList {
            inventory_generation: nz(7),
            next_cursor: None,
            terminals: vec![summaries.remove(0)],
        },
    ))?;
    let json = std::str::from_utf8(&one.payload)?;
    let begin = json.find("\"terminals\":[").ok_or("terminals")? + "\"terminals\":[".len();
    let row = &json[begin..json.len() - 2];
    let oversized = format!(
        "{{\"inventoryGeneration\":\"7\",\"nextCursor\":null,\"terminals\":[{}]}}",
        vec![row; 129].join(",")
    );
    assert_eq!(
        inventory_outcome(request, 10, MessageRole::Response, oversized.as_bytes()),
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage)
    );
    Ok(())
}

use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
use std::{
    cell::RefCell,
    error::Error,
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    num::NonZeroU64,
    path::PathBuf,
    process::{Child, Command, Stdio},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostCreationId,
    HostLaunchState, HostTerminalId, HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds, LeaseId,
    OperationId, TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId,
    TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchRejection,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    TerminalStartOutcome, TerminalStartRejection, WorkerGeneration, WorkerId,
};
use orchbee_local_worker_protocol::WorkerId as LocalWorkerId;
use orchbee_terminal_protocol::{
    ColorScheme, CursorShape, DeviceAttributes, HelloAccepted, InventoryControl, ListTerminals,
    MAXIMUM_CONTROL_JSON_BYTES, TERMINAL_THEME_PALETTE_ENTRIES, TRANSPORT_ENVELOPE_HEADER_BYTES,
    TerminalIdentity, TerminalProfile, TerminalSummary, TypedFailure, decode_inventory_control,
    encode_connection_control, encode_inventory_control,
};

use super::*;
use crate::terminal_authorization::{AuthorizedHostOperationResult, HostOperationAdmission};

const BUDGET: Duration = Duration::from_secs(3);
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn exact_launch_success_remains_in_the_kernel_slot_and_replays_without_an_effect() -> TestResult {
    for mode in [
        TerminalLaunchMode::Deferred,
        TerminalLaunchMode::Immediate(geometry()?),
    ] {
        let mut fixture = Fixture::fake()?;
        let mut kernel = kernel()?;
        let launch = AuthorizedTerminalLaunch::new(authorization(1)?, None, mode, None);
        let dispatch = dispatch(kernel.admit_launch(launch.clone(), 100)?)?;
        let operation =
            PreparedHostOperation::launch(&dispatch, facts()?, correlation()?, None, false)?;
        let peer = respond(fixture.peer.try_clone()?, Ok);
        assert_eq!(
            operation.execute(
                &mut kernel,
                &mut fixture.host,
                &fixture.server,
                || 100,
                BUDGET
            )?,
            HostOperationDisposition::ResultReady
        );
        join(peer)?;
        assert_eq!(
            kernel.admit_launch(launch.clone(), 100)?,
            HostOperationAdmission::SlotOccupied
        );
        let result = kernel.take_result().ok_or("missing result")?;
        let AuthorizedHostOperationResult::Launch(value) = &result else {
            return Err("wrong result kind".into());
        };
        let TerminalLaunchOutcome::HostAccepted(accepted) = value.outcome() else {
            return Err("Host did not accept".into());
        };
        assert_eq!(
            accepted.identity(),
            capability_identity(fixture.host.host_epoch())?
        );
        assert_eq!(
            accepted.state(),
            match mode {
                TerminalLaunchMode::Deferred => HostLaunchState::AwaitingStart,
                TerminalLaunchMode::Immediate(size) => HostLaunchState::Started(size),
            }
        );
        assert_eq!(
            kernel.admit_launch(launch, 300)?,
            HostOperationAdmission::ResultReady
        );
        assert_eq!(kernel.take_result(), Some(result));
        require_no_bytes(&fixture.peer)?;
    }
    Ok(())
}

#[test]
fn handoff_rejections_and_invalid_preparation_never_send_a_command() -> TestResult {
    let mut fixture = Fixture::fake()?;
    let mut kernel = kernel()?;
    let dispatch = launch_dispatch(&mut kernel)?;
    assert!(PreparedHostOperation::start(&dispatch, correlation()?).is_err());
    let invalid = PreparedHostOperation::launch(&dispatch, facts()?, correlation()?, None, false)?;
    assert!(matches!(
        invalid.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            Duration::ZERO
        ),
        Err(HostOperationError::InvalidBudget)
    ));
    assert!(kernel.take_result().is_none());
    let expired = PreparedHostOperation::launch(&dispatch, facts()?, correlation()?, None, false)?;
    assert_eq!(
        expired.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 200,
            BUDGET
        )?,
        HostOperationDisposition::ResultReady
    );
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Launch(result)) if result.outcome() == TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LeaseExpired))
    );
    require_no_bytes(&fixture.peer)?;

    let mut kernel = self::kernel()?;
    let old_epoch = HostEpoch::from_bytes([9; 16])?;
    let start = AuthorizedTerminalStart::new(
        authorization(1)?,
        capability_identity(old_epoch)?,
        geometry()?,
    );
    let dispatch = self::dispatch(kernel.admit_start(
        start,
        100,
        CapabilityHostEpoch::from_bytes(*old_epoch.as_bytes())?,
    )?)?;
    let operation = PreparedHostOperation::start(&dispatch, correlation()?)?;
    assert_eq!(
        operation.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            BUDGET
        )?,
        HostOperationDisposition::ResultReady
    );
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Start(result)) if result.outcome() == TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged { observed_host_epoch: CapabilityHostEpoch::from_bytes(*fixture.host.host_epoch().as_bytes())? }))
    );
    require_no_bytes(&fixture.peer)?;
    Ok(())
}

#[test]
fn exact_generic_failure_is_failed_even_when_its_diagnostics_say_rejected_and_retryable()
-> TestResult {
    let mut fixture = Fixture::fake()?;
    let mut kernel = kernel()?;
    let operation = PreparedHostOperation::launch(
        &launch_dispatch(&mut kernel)?,
        facts()?,
        correlation()?,
        None,
        false,
    )?;
    let peer = respond(fixture.peer.try_clone()?, |_record| {
        let control = encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
            code: "rejected".to_owned(),
            message: "retry this operation".to_owned(),
            retryable: true,
        }))
        .map_err(io_error)?;
        wire(
            control.kind,
            control.role,
            &control.payload,
            HostEpoch::from_bytes([4; 16]).map_err(io_error)?,
            correlation().map_err(io_error)?,
        )
    });
    assert_eq!(
        operation.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            BUDGET
        )?,
        HostOperationDisposition::ResultReady
    );
    join(peer)?;
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Launch(result)) if result.outcome() == TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostReportedFailure { host_epoch: CapabilityHostEpoch::from_bytes([4;16])? }))
    );
    Ok(())
}

#[test]
fn malformed_response_partitions_close_host_cache_unknown_and_never_resend() -> TestResult {
    // Each mutation changes one semantic field of an otherwise valid Host response.
    for partition in 0..13 {
        let mut fixture = Fixture::fake()?;
        let mut kernel = kernel()?;
        let launch = AuthorizedTerminalLaunch::new(
            authorization(1)?,
            None,
            TerminalLaunchMode::Deferred,
            None,
        );
        let dispatch = dispatch(kernel.admit_launch(launch.clone(), 100)?)?;
        let operation =
            PreparedHostOperation::launch(&dispatch, facts()?, correlation()?, None, false)?;
        let peer = respond(fixture.peer.try_clone()?, move |mut record| {
            match partition {
                0 => record[6] = 0,                                          // Request role.
                1 => record[6] = 4,                                          // Event role.
                2 => record[6] |= 1,                                         // Optional message.
                3 => record[24] ^= 1,                                        // Correlation.
                4 => record[40] ^= 1,                                        // Host epoch.
                5 => record[16] ^= 1,                                        // Protocol major.
                6 => record[20] = 3, // Different response kind.
                7 => record[22] = 2, // Encoding.
                8 => record[TRANSPORT_ENVELOPE_HEADER_BYTES] = b'[', // JSON shape.
                9 => record[8..12].copy_from_slice(&u32::MAX.to_le_bytes()), // Oversized header.
                10 => {
                    // Correct envelope, different creation identity in payload.
                    let identity = TerminalIdentity {
                        creation_id: CreationId::from_bytes([8; 16]).map_err(io_error)?,
                        ..host_identity().map_err(io_error)?
                    };
                    let control = encode_terminal_creation_control(
                        &TerminalCreationControl::TerminalCreated(identity),
                    )
                    .map_err(io_error)?;
                    record = wire(
                        control.kind,
                        control.role,
                        &control.payload,
                        identity.host_epoch,
                        correlation().map_err(io_error)?,
                    )?;
                }
                11 => {
                    record.truncate(TRANSPORT_ENVELOPE_HEADER_BYTES + 1);
                }
                12 => {
                    let failure =
                        encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                            code: "rejected".to_owned(),
                            message: "wrong operation".to_owned(),
                            retryable: true,
                        }))
                        .map_err(io_error)?;
                    record = wire(
                        failure.kind,
                        failure.role,
                        &failure.payload,
                        host_identity().map_err(io_error)?.host_epoch,
                        CorrelationId::from_bytes([9; 16]).map_err(io_error)?,
                    )?;
                }
                _ => unreachable!(),
            }
            Ok(record)
        });
        assert_eq!(
            operation.execute(
                &mut kernel,
                &mut fixture.host,
                &fixture.server,
                || 100,
                if partition == 11 {
                    Duration::from_millis(30)
                } else {
                    BUDGET
                }
            )?,
            HostOperationDisposition::HostClosed,
            "partition {partition}"
        );
        join(peer)?;
        let result = kernel.take_result().ok_or("missing unknown result")?;
        assert!(
            matches!(&result, AuthorizedHostOperationResult::Launch(value) if matches!(value.outcome(), TerminalLaunchOutcome::OutcomeUnknown { .. })),
            "partition {partition}"
        );
        assert_eq!(
            kernel.admit_launch(launch, 100)?,
            HostOperationAdmission::ResultReady
        );
        assert_eq!(kernel.take_result(), Some(result));
        require_closed(&fixture.peer)?;
    }
    Ok(())
}

#[test]
fn observed_host_loss_before_handoff_is_a_definite_failure() -> TestResult {
    let mut fixture = Fixture::fake()?;
    fixture.peer.shutdown(Shutdown::Both)?;
    let mut kernel = kernel()?;
    let operation = PreparedHostOperation::launch(
        &launch_dispatch(&mut kernel)?,
        facts()?,
        correlation()?,
        None,
        false,
    )?;
    assert_eq!(
        operation.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            BUDGET
        )?,
        HostOperationDisposition::HostClosed
    );
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Launch(value)) if value.outcome() == TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff))
    );
    Ok(())
}

#[test]
fn server_loss_before_handoff_abandons_without_sending_or_inventing_a_result() -> TestResult {
    let mut fixture = Fixture::fake()?;
    fixture.server_peer.shutdown(Shutdown::Both)?;
    let mut kernel = kernel()?;
    let operation = PreparedHostOperation::launch(
        &launch_dispatch(&mut kernel)?,
        facts()?,
        correlation()?,
        None,
        false,
    )?;
    assert_eq!(
        operation.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            BUDGET
        )?,
        HostOperationDisposition::ServerClosed
    );
    assert!(kernel.take_result().is_none());
    require_closed(&fixture.peer)?;
    Ok(())
}

#[test]
fn start_responses_preserve_transitions_and_reject_contradictory_payloads() -> TestResult {
    use orchbee_terminal_protocol::{StartTransition, TerminalStartAccepted};
    for partition in 0..8 {
        let mut fixture = Fixture::fake()?;
        let mut kernel = kernel()?;
        let start = AuthorizedTerminalStart::new(
            authorization(1)?,
            capability_identity(fixture.host.host_epoch())?,
            geometry()?,
        );
        let dispatch =
            dispatch(kernel.admit_start(start, 100, CapabilityHostEpoch::from_bytes([4; 16])?)?)?;
        let operation = PreparedHostOperation::start(&dispatch, correlation()?)?;
        let duplicate = PreparedHostOperation::start(&dispatch, correlation()?)?;
        let mut peer = fixture.peer.try_clone()?;
        let response = thread::spawn(move || -> io::Result<()> {
            let request = read_record(&mut peer)?;
            let envelope = decode_transport_envelope(&request).map_err(io_error)?;
            let LifecycleControl::StartTerminal(request) =
                decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io_error)?
            else {
                return Err(io::Error::other("wrong Start command"));
            };
            let mut accepted = TerminalStartAccepted {
                geometry: request.geometry,
                identity: host_identity().map_err(io_error)?,
                transition: if partition == 1 {
                    StartTransition::AlreadyStarted
                } else {
                    StartTransition::Started
                },
            };
            match partition {
                2 => {
                    accepted.identity.terminal_id =
                        orchbee_terminal_protocol::TerminalId::from_bytes([9; 16])
                            .map_err(io_error)?
                }
                3 => {
                    accepted.identity.creation_id =
                        CreationId::from_bytes([9; 16]).map_err(io_error)?
                }
                4 => {
                    accepted.identity.host_epoch =
                        HostEpoch::from_bytes([9; 16]).map_err(io_error)?
                }
                5 => accepted.geometry.cell_pixel_width += 1,
                7 => {
                    peer.shutdown(Shutdown::Both)?;
                    return Ok(());
                }
                _ => {}
            }
            let (kind, role, payload) = if partition == 6 {
                let failure =
                    encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
                        code: "denied".to_owned(),
                        message: "unclassified failure".to_owned(),
                        retryable: true,
                    }))
                    .map_err(io_error)?;
                (failure.kind, failure.role, failure.payload)
            } else {
                let control =
                    encode_lifecycle_control(&LifecycleControl::TerminalStartAccepted(accepted))
                        .map_err(io_error)?;
                (control.kind, control.role, control.payload)
            };
            peer.write_all(&wire(
                kind,
                role,
                &payload,
                HostEpoch::from_bytes([4; 16]).map_err(io_error)?,
                correlation().map_err(io_error)?,
            )?)
        });
        let disposition = operation.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            BUDGET,
        )?;
        join(response)?;
        assert_eq!(
            disposition,
            if matches!(partition, 0 | 1 | 6) {
                HostOperationDisposition::ResultReady
            } else {
                HostOperationDisposition::HostClosed
            }
        );
        if matches!(partition, 0 | 1 | 6) {
            assert!(matches!(
                duplicate.execute(
                    &mut kernel,
                    &mut fixture.host,
                    &fixture.server,
                    || 100,
                    BUDGET
                ),
                Err(HostOperationError::Kernel(
                    AuthorizationKernelError::InvalidDispatchPhase
                ))
            ));
            require_no_bytes(&fixture.peer)?;
        }
        let Some(AuthorizedHostOperationResult::Start(result)) = kernel.take_result() else {
            return Err("missing Start result".into());
        };
        match partition {
            0 | 1 => {
                let TerminalStartOutcome::HostAccepted(accepted) = result.outcome() else {
                    return Err("missing Host acceptance".into());
                };
                assert_eq!(
                    accepted.transition(),
                    if partition == 0 {
                        orchbee_capability_node_protocol::StartTransition::Started
                    } else {
                        orchbee_capability_node_protocol::StartTransition::AlreadyStarted
                    }
                );
            }
            6 => assert!(matches!(
                result.outcome(),
                TerminalStartOutcome::Failed(
                    orchbee_capability_node_protocol::TerminalStartFailure::HostReportedFailure { .. }
                )
            )),
            _ => assert_eq!(result.outcome(), TerminalStartOutcome::OutcomeUnknown),
        }
    }
    Ok(())
}

#[test]
fn server_loss_interrupts_a_waiting_host_and_closes_both_links() -> TestResult {
    let mut fixture = Fixture::fake()?;
    let mut kernel = kernel()?;
    let operation = PreparedHostOperation::launch(
        &launch_dispatch(&mut kernel)?,
        facts()?,
        correlation()?,
        None,
        false,
    )?;
    let mut peer = fixture.peer.try_clone()?;
    let server = fixture.server_peer.try_clone()?;
    let barrier = thread::spawn(move || -> io::Result<()> {
        read_record(&mut peer)?;
        server.shutdown(Shutdown::Both)?;
        require_closed(&peer)
    });
    assert_eq!(
        operation.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            BUDGET
        )?,
        HostOperationDisposition::ServerClosed
    );
    join(barrier)?;
    assert!(
        matches!(kernel.take_result(), Some(AuthorizedHostOperationResult::Launch(value)) if matches!(value.outcome(), TerminalLaunchOutcome::OutcomeUnknown { .. }))
    );
    Ok(())
}

#[test]
fn a_silent_host_is_bounded_by_the_operation_deadline() -> TestResult {
    let mut fixture = Fixture::fake()?;
    let mut kernel = kernel()?;
    let operation = PreparedHostOperation::launch(
        &launch_dispatch(&mut kernel)?,
        facts()?,
        correlation()?,
        None,
        false,
    )?;
    let started = Instant::now();
    assert_eq!(
        operation.execute(
            &mut kernel,
            &mut fixture.host,
            &fixture.server,
            || 100,
            Duration::from_millis(30)
        )?,
        HostOperationDisposition::HostClosed
    );
    assert!(started.elapsed() < BUDGET);
    let mut peer = &fixture.peer;
    read_record(&mut peer)?;
    require_closed(&fixture.peer)?;
    Ok(())
}

#[test]
fn real_zig_host_creates_starts_and_retains_the_same_terminal_after_broker_loss() -> TestResult {
    let directory = Directory::new()?;
    let endpoint_id = format!("{:032x}", u128::from(std::process::id()) + (1u128 << 96));
    #[cfg(unix)]
    let endpoint = directory.0.join(format!("{endpoint_id}.s"));
    #[cfg(windows)]
    let endpoint = PathBuf::from(format!(r"\\.\pipe\orchbee-terminal-host-{endpoint_id}"));
    let binary = std::env::var_os("ORCHBEE_TERMINAL_HOST_TEST_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../terminal-host/zig-out/bin/orchbee-terminal-host{}",
                std::env::consts::EXE_SUFFIX
            ))
        });
    let mut child = HostChild(
        Command::new(binary)
            .args(["serve", "--runtime-directory"])
            .arg(&directory.0)
            .args(["--endpoint-id", &endpoint_id])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let deadline = Instant::now() + BUDGET;
    loop {
        #[cfg(unix)]
        let appeared = endpoint.try_exists()?;
        #[cfg(windows)]
        let appeared = orchbee_local_ipc::endpoint_exists(&endpoint)?;
        if appeared {
            break;
        }
        if let Some(status) = child.0.try_wait()? {
            return Err(format!("Host exited: {status}").into());
        }
        if Instant::now() >= deadline {
            return Err("Host endpoint did not appear".into());
        }
        thread::yield_now();
    }
    let mut host = loop {
        match HostBrokerConnection::connect(&endpoint, fresh_drain()?) {
            Ok(host) => break host,
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::ConnectionRefused)
                    && Instant::now() < deadline =>
            {
                thread::yield_now()
            }
            Err(error) => return Err(format!("initial Host connection: {error}").into()),
        }
    };
    let (server, _server_peer) = UnixStream::pair()?;
    let mut kernel = kernel()?;
    let operation = PreparedHostOperation::launch(
        &launch_dispatch(&mut kernel)?,
        facts()?,
        correlation()?,
        None,
        false,
    )?;
    assert_eq!(
        operation.execute(&mut kernel, &mut host, &server, || 100, BUDGET)?,
        HostOperationDisposition::ResultReady
    );
    let Some(AuthorizedHostOperationResult::Launch(result)) = kernel.take_result() else {
        return Err("missing launch result".into());
    };
    let TerminalLaunchOutcome::HostAccepted(accepted) = result.outcome() else {
        return Err(format!("launch outcome: {:?}", result.outcome()).into());
    };
    assert_eq!(accepted.state(), HostLaunchState::AwaitingStart);
    let identity = accepted.identity();
    let start = AuthorizedTerminalStart::new(authorization(2)?, identity, geometry()?);
    let dispatch = self::dispatch(kernel.admit_start(start, 100, identity.host_epoch())?)?;
    let operation = PreparedHostOperation::start(&dispatch, CorrelationId::from_bytes([7; 16])?)?;
    assert_eq!(
        operation.execute(&mut kernel, &mut host, &server, || 100, BUDGET)?,
        HostOperationDisposition::ResultReady
    );
    let Some(AuthorizedHostOperationResult::Start(result)) = kernel.take_result() else {
        return Err("missing start result".into());
    };
    assert!(
        matches!(result.outcome(), TerminalStartOutcome::HostAccepted(value) if value.identity() == identity && value.geometry() == geometry()?)
    );
    drop(host);
    let observer = HostBrokerConnection::connect(&endpoint, fresh_drain()?)
        .map_err(|error| format!("retained Host connection: {error}"))?;
    assert_eq!(
        CapabilityHostEpoch::from_bytes(*observer.host_epoch().as_bytes())?,
        identity.host_epoch()
    );
    let control = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
        cursor: None,
        maximum_terminals: 1,
    }))?;
    let mut stream = observer.stream();
    stream.set_read_timeout(Some(BUDGET))?;
    stream.write_all(&wire(
        control.kind,
        control.role,
        &control.payload,
        observer.host_epoch(),
        CorrelationId::from_bytes([8; 16])?,
    )?)?;
    let record = read_host_reply(&observer)?;
    let envelope = decode_transport_envelope(&record)?;
    let InventoryControl::TerminalList(list) =
        decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err("missing retained inventory".into());
    };
    let expected_identity = TerminalIdentity {
        creation_id: CreationId::from_bytes(identity.creation_id().as_bytes())?,
        host_epoch: HostEpoch::from_bytes(identity.host_epoch().as_bytes())?,
        terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
            identity.terminal_id().as_bytes(),
        )?,
    };
    assert!(
        matches!(list.terminals.as_slice(), [TerminalSummary::Running { core, .. }] if core.identity == expected_identity)
    );
    // The witness owns this isolated process tree; explicitly terminate it
    // after the retained-inventory checkpoint before tearing down the Host.
    let control = encode_lifecycle_control(&LifecycleControl::ForceTerminateTerminal(
        orchbee_terminal_protocol::TerminalTarget {
            terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
                identity.terminal_id().as_bytes(),
            )?,
        },
    ))?;
    stream.write_all(&wire(
        control.kind,
        control.role,
        &control.payload,
        observer.host_epoch(),
        CorrelationId::from_bytes([9; 16])?,
    )?)?;
    let record = read_host_reply(&observer)?;
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(envelope.role, MessageRole::Response);
    assert_eq!(envelope.kind, control.kind);
    assert_eq!(envelope.host_epoch, Some(observer.host_epoch()));
    assert_eq!(
        envelope.correlation_id,
        Some(CorrelationId::from_bytes([9; 16])?)
    );
    assert_eq!(envelope.payload, control.payload);
    Ok(())
}

// Real shells may publish metadata before the reply. Use the production
// event validator and keep a bounded read budget instead of assuming silence.
fn read_host_reply(host: &HostBrokerConnection) -> TestResult<Vec<u8>> {
    let deadline = Instant::now() + BUDGET;
    let mut stream = host.stream();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("Host reply deadline".into());
        }
        stream.set_read_timeout(Some(remaining))?;
        let record = read_record(&mut stream)?;
        let envelope = decode_transport_envelope(&record)?;
        if envelope.role == MessageRole::Event {
            host.receive_event(record)?;
        } else {
            return Ok(record);
        }
    }
}

fn respond(
    mut peer: UnixStream,
    transform: impl FnOnce(Vec<u8>) -> io::Result<Vec<u8>> + Send + 'static,
) -> thread::JoinHandle<io::Result<()>> {
    thread::spawn(move || {
        let request = read_record(&mut peer)?;
        let envelope = decode_transport_envelope(&request).map_err(io_error)?;
        let TerminalCreationControl::CreateTerminal(request) =
            decode_terminal_creation_control(envelope.kind, envelope.role, envelope.payload)
                .map_err(io_error)?
        else {
            return Err(io::Error::other("wrong Host command"));
        };
        assert_eq!(
            request.creation_id,
            host_identity().map_err(io_error)?.creation_id
        );
        #[cfg(not(windows))]
        assert!(request.environment_overlay.is_empty());
        #[cfg(windows)]
        assert_eq!(
            request
                .environment_overlay
                .iter()
                .map(|entry| (entry.name.as_str(), entry.value.as_str()))
                .collect::<Vec<_>>(),
            [
                ("ORCHBEE_TERMINAL_ID", "01010101010101010101010101010101"),
                ("ORCHBEE_WORKER_ID", "03030303030303030303030303030303"),
            ]
        );
        let control = encode_terminal_creation_control(&TerminalCreationControl::TerminalCreated(
            host_identity().map_err(io_error)?,
        ))
        .map_err(io_error)?;
        let record = wire(
            control.kind,
            control.role,
            &control.payload,
            host_identity().map_err(io_error)?.host_epoch,
            envelope
                .correlation_id
                .ok_or_else(|| io::Error::other("missing correlation"))?,
        )?;
        peer.write_all(&transform(record)?)
    })
}

fn launch_dispatch(
    kernel: &mut AuthorizedHostOperationKernel,
) -> TestResult<AuthorizedHostDispatch> {
    dispatch(kernel.admit_launch(
        AuthorizedTerminalLaunch::new(authorization(1)?, None, TerminalLaunchMode::Deferred, None),
        100,
    )?)
}
fn dispatch(admission: HostOperationAdmission) -> TestResult<AuthorizedHostDispatch> {
    match admission {
        HostOperationAdmission::Dispatch(value) => Ok(value),
        other => Err(format!("unexpected admission: {other:?}").into()),
    }
}
fn kernel() -> TestResult<AuthorizedHostOperationKernel> {
    Ok(AuthorizedHostOperationKernel::new(
        LocalWorkerId::from_bytes([3; 16]),
        nonzero(7)?,
    )?)
}
fn nonzero(value: u64) -> TestResult<NonZeroU64> {
    NonZeroU64::new(value).ok_or_else(|| "zero fixture identity".into())
}
fn authorization(operation: u8) -> TestResult<TerminalOperationAuthorization> {
    Ok(TerminalOperationAuthorization::new(
        TerminalId::from_bytes([1; 16])?,
        OperationId::from_bytes([operation; 16])?,
        WorkerId::from_bytes([3; 16])?,
        WorkerGeneration::new(nonzero(7)?),
        TerminalFence::new(nonzero(u64::from(operation))?),
        TerminalOperationLease::new(
            LeaseId::from_bytes([operation; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(nonzero(200)?),
        ),
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    ))
}
fn geometry() -> TestResult<TerminalPixelGeometry> {
    Ok(TerminalPixelGeometry::new(80, 24, 8, 16)?)
}
fn correlation() -> TestResult<CorrelationId> {
    Ok(CorrelationId::from_bytes([6; 16])?)
}
fn fresh_drain() -> TestResult<Rc<RefCell<crate::agent_drain::AgentDrain>>> {
    Ok(Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())))
}

fn host_identity() -> TestResult<TerminalIdentity> {
    Ok(TerminalIdentity {
        creation_id: CreationId::from_bytes([2; 16])?,
        host_epoch: HostEpoch::from_bytes([4; 16])?,
        terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([5; 16])?,
    })
}
fn capability_identity(epoch: HostEpoch) -> TestResult<HostTerminalIdentity> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([2; 16])?,
        CapabilityHostEpoch::from_bytes(*epoch.as_bytes())?,
        HostTerminalId::from_bytes([5; 16])?,
    ))
}
fn facts() -> TestResult<ResolvedTerminalLaunchFacts> {
    #[cfg(unix)]
    let (executable, cwd) = ("/bin/sh".to_owned(), "/tmp".to_owned());
    #[cfg(windows)]
    let (executable, cwd) = (
        std::env::var("COMSPEC")?,
        std::env::temp_dir()
            .to_str()
            .ok_or("non-Unicode fixture path")?
            .to_owned(),
    );
    Ok(ResolvedTerminalLaunchFacts::new(
        CreationId::from_bytes([2; 16])?,
        executable,
        Vec::new(),
        Vec::new(),
        AbsoluteCwd::new(&cwd)?,
        TerminalProfile {
            color_scheme: ColorScheme::Dark,
            default_background_rgb: 0,
            default_cursor_blinking: false,
            default_cursor_rgb: None,
            default_cursor_shape: CursorShape::Block,
            default_foreground_rgb: 0xffffff,
            device_attributes: DeviceAttributes {
                primary_conformance_level: 65,
                primary_features: vec![1, 4, 22],
                secondary_device_type: 1,
                secondary_firmware_version: 0,
                secondary_rom_cartridge: 0,
                tertiary_unit_id: 0,
            },
            enquiry_response: String::new(),
            palette: vec![0; TERMINAL_THEME_PALETTE_ENTRIES],
            version_response: String::new(),
        },
        None,
    ))
}
fn wire(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
    epoch: HostEpoch,
    correlation: CorrelationId,
) -> io::Result<Vec<u8>> {
    encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(epoch),
        kind,
        optional_message: false,
        payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role,
    })
    .map_err(io_error)
}
fn read_record(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut header = [0; TRANSPORT_ENVELOPE_HEADER_BYTES];
    stream.read_exact(&mut header)?;
    let length = u32::from_le_bytes(header[8..12].try_into().map_err(io_error)?) as usize;
    if !(TRANSPORT_ENVELOPE_HEADER_BYTES
        ..=TRANSPORT_ENVELOPE_HEADER_BYTES + MAXIMUM_CONTROL_JSON_BYTES)
        .contains(&length)
    {
        return Err(io::Error::other("invalid test record size"));
    }
    let mut record = vec![0; length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    stream.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}
fn require_no_bytes(mut stream: &UnixStream) -> io::Result<()> {
    stream.set_nonblocking(true)?;
    let result = stream.read(&mut [0]);
    stream.set_nonblocking(false)?;
    assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::WouldBlock));
    Ok(())
}
fn require_closed(mut stream: &UnixStream) -> io::Result<()> {
    stream.set_read_timeout(Some(BUDGET))?;
    let outcome = stream.read(&mut [0]);
    assert!(
        matches!(&outcome, Ok(0))
            || matches!(&outcome, Err(error) if matches!(error.kind(), io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionAborted)),
        "expected closed peer, got {outcome:?}"
    );
    Ok(())
}
fn io_error(error: impl fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}
fn join(peer: thread::JoinHandle<io::Result<()>>) -> TestResult {
    peer.join().map_err(|_| "peer panicked")??;
    Ok(())
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "obex-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut fs::DirBuilder::new(), 0o700).create(&path)?;
        #[cfg(windows)]
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}
struct HostChild(Child);
impl Drop for HostChild {
    fn drop(&mut self) {
        let _killed = self.0.kill();
        let _waited = self.0.wait();
    }
}
struct Fixture {
    host: HostBrokerConnection,
    peer: UnixStream,
    server: UnixStream,
    server_peer: UnixStream,
    _directory: Directory,
}
impl Fixture {
    fn fake() -> TestResult<Self> {
        let directory = Directory::new()?;
        #[cfg(unix)]
        let endpoint = directory.0.join("host.s");
        #[cfg(windows)]
        let endpoint = crate::tests::endpoint();
        let listener = UnixListener::bind(&endpoint)?;
        let peer = thread::spawn(move || -> io::Result<UnixStream> {
            let (mut peer, _) = listener.accept()?;
            peer.set_read_timeout(Some(BUDGET))?;
            peer.set_write_timeout(Some(BUDGET))?;
            let record = read_record(&mut peer)?;
            let envelope = decode_transport_envelope(&record).map_err(io_error)?;
            let ConnectionControl::Hello(hello) =
                decode_connection_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io_error)?
            else {
                return Err(io::Error::other("missing hello"));
            };
            let epoch = HostEpoch::from_bytes([4; 16]).map_err(io_error)?;
            let control =
                encode_connection_control(&ConnectionControl::HelloAccepted(HelloAccepted {
                    frame_bytes: hello.frame_bytes,
                    host_epoch: epoch,
                    host_version: "0.1.0".to_owned(),
                    input_bytes: hello.input_bytes,
                    protocol: hello.protocol,
                    row_window_bytes: hello.row_window_bytes,
                    supported_optional_features: Vec::new(),
                }))
                .map_err(io_error)?;
            peer.write_all(&wire(
                control.kind,
                control.role,
                &control.payload,
                epoch,
                envelope
                    .correlation_id
                    .ok_or_else(|| io::Error::other("missing correlation"))?,
            )?)?;
            Ok(peer)
        });
        let host = HostBrokerConnection::connect(&endpoint, fresh_drain()?)?;
        let peer = peer.join().map_err(|_| "handshake peer panicked")??;
        let (server, server_peer) = UnixStream::pair()?;
        Ok(Self {
            host,
            peer,
            server,
            server_peer,
            _directory: directory,
        })
    }
}

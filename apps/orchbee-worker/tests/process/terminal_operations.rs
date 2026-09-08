use std::time::{SystemTime, UNIX_EPOCH};

use nix::unistd::{Uid, User};
use orchbee_capability_node_protocol::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, CapabilityNodeMessage,
    HostLaunchState, LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLaunchResult,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    TerminalStartOutcome, WorkerGeneration, WorkerId as CapabilityWorkerId, encode_message,
    read_message as read_obcn, write_message as write_obcn,
};
use orchbee_terminal_protocol::{
    InventoryControl, LifecycleControl, ListTerminals, MAXIMUM_INPUT_BYTES,
    MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_TERMINAL_FRAME_BYTES, TerminalCreationControl,
    TerminalIdentity, TerminalSummary, TerminalTarget, decode_inventory_control,
    decode_terminal_creation_control, encode_inventory_control, encode_lifecycle_control,
    encode_terminal_creation_control,
};

use super::*;

#[path = "terminal_operations/lifecycle.rs"]
mod lifecycle;

#[test]
fn real_worker_creates_starts_replays_and_leaves_terminals_in_the_host() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let _host = RunningHost::start(&fixture)?;
    wait_for_socket(&fixture.host_endpoint())?;
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));

    let deferred = launch(1, None, TerminalLaunchMode::Deferred)?;
    let first = request_launch(&mut server, &deferred)
        .map_err(|error| format!("deferred Create: {error}"))?;
    let TerminalLaunchOutcome::HostAccepted(created) = first.outcome() else {
        return Err(io::Error::other(format!("Host did not create terminal: {first:?}")).into());
    };
    assert_eq!(created.state(), HostLaunchState::AwaitingStart);
    assert_eq!(
        request_launch(&mut server, &deferred)
            .map_err(|error| format!("Create replay: {error}"))?,
        first
    );

    let start = CapabilityNodeMessage::AuthorizedTerminalStart(AuthorizedTerminalStart::new(
        authorization(1, 2)?,
        created.identity(),
        geometry()?,
    ));
    if let Err(error) = write_obcn(&mut server, &start) {
        let output = worker.finish()?;
        return Err(format!(
            "Start write: {error}, Worker status: {:?}, stderr: {:?}",
            output.status, output.stderr
        )
        .into());
    }
    let started = read_obcn(&mut server).map_err(|error| format!("Start result: {error}"))?;
    let CapabilityNodeMessage::TerminalStartResult(result) = &started else {
        return Err(io::Error::other("missing Start result").into());
    };
    assert_eq!(result.authorization(), authorization(1, 2)?);
    let TerminalStartOutcome::HostAccepted(accepted) = result.outcome() else {
        return Err(io::Error::other(format!("Host did not start terminal: {result:?}")).into());
    };
    assert_eq!(accepted.identity(), created.identity());
    assert_eq!(accepted.geometry(), geometry()?);
    if let Err(error) = write_obcn(&mut server, &start) {
        let output = worker.finish()?;
        return Err(format!(
            "Start write: {error}, Worker status: {:?}, stderr: {:?}",
            output.status, output.stderr
        )
        .into());
    }
    assert_eq!(read_obcn(&mut server)?, started);

    let immediate = launch(
        3,
        Some(AbsoluteCwd::new("/tmp")?),
        TerminalLaunchMode::Immediate(geometry()?),
    )?;
    let second = request_launch(&mut server, &immediate)
        .map_err(|error| format!("immediate Create: {error}"))?;
    let TerminalLaunchOutcome::HostAccepted(second_created) = second.outcome() else {
        return Err(io::Error::other("Host did not create immediate terminal").into());
    };
    assert_eq!(
        second_created.state(),
        HostLaunchState::Started(geometry()?)
    );
    assert_ne!(
        second_created.identity().creation_id(),
        created.identity().creation_id()
    );
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);

    let (mut observer, epoch) = connect_observer(&fixture.host_endpoint())?;
    let list = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
        cursor: None,
        maximum_terminals: 2,
    }))?;
    observer.write_all(&host_record(
        list.kind,
        list.role,
        &list.payload,
        epoch,
        CorrelationId::from_bytes([8; 16])?,
    )?)?;
    let record = read_terminal_record(&mut observer)?;
    let envelope = decode_transport_envelope(&record)?;
    let InventoryControl::TerminalList(list) =
        decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err(io::Error::other("missing retained terminal inventory").into());
    };
    assert_eq!(
        list.terminals.len(),
        2,
        "replays must not create another terminal"
    );
    for expected in [created.identity(), second_created.identity()] {
        let identity = TerminalIdentity {
            creation_id: orchbee_terminal_protocol::CreationId::from_bytes(
                expected.creation_id().as_bytes(),
            )?,
            host_epoch: epoch,
            terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
                expected.terminal_id().as_bytes(),
            )?,
        };
        assert!(
            list.terminals
                .iter()
                .any(|terminal| matches!(terminal, TerminalSummary::Running { core, .. } if core.identity == identity))
        );
        let terminate =
            encode_lifecycle_control(&LifecycleControl::ForceTerminateTerminal(TerminalTarget {
                terminal_id: identity.terminal_id,
            }))?;
        observer.write_all(&host_record(
            terminate.kind,
            terminate.role,
            &terminate.payload,
            epoch,
            CorrelationId::from_bytes([9; 16])?,
        )?)?;
        let record = read_terminal_record(&mut observer)?;
        let response = decode_transport_envelope(&record)?;
        assert_eq!(response.kind, terminate.kind);
        assert_eq!(response.role, MessageRole::Response);
        assert_eq!(response.host_epoch, Some(epoch));
        assert_eq!(
            response.correlation_id,
            Some(CorrelationId::from_bytes([9; 16])?)
        );
        assert_eq!(response.payload, terminate.payload);
    }
    Ok(())
}

#[test]
fn worker_resolves_only_admitted_launches_and_replays_without_another_command() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let home = User::from_uid(Uid::effective())?
            .ok_or_else(|| io::Error::other("missing account"))?
            .dir;
        let mut identities = Vec::new();
        let mut correlations = Vec::new();
        for expected_cwd in [
            home.to_str()
                .ok_or_else(|| io::Error::other("non-UTF8 home"))?,
            "/tmp/literal/../ordinary",
        ] {
            let record = read_terminal_record(&mut host)?;
            let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
            let TerminalCreationControl::CreateTerminal(request) =
                decode_terminal_creation_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(protocol_io)?
            else {
                return Err(io::Error::other("missing CreateTerminal"));
            };
            assert_eq!(
                request.launch.current_directory.as_deref(),
                Some(expected_cwd)
            );
            assert_eq!(request.launch.executable, "/bin/sh");
            assert!(request.launch.arguments.is_empty());
            assert!(request.environment_overlay.is_empty());
            assert!(request.launch.configuration.is_none());
            let environment: Vec<_> = request
                .launch
                .environment
                .iter()
                .map(|entry| (entry.name.as_str(), entry.value.as_deref()))
                .collect();
            assert_eq!(
                environment,
                [
                    ("TERM", Some("xterm-256color")),
                    ("COLORTERM", Some("truecolor")),
                    ("TERM_PROGRAM", Some("ghostty"))
                ]
            );
            assert_eq!(request.launch.terminal_profile.palette.len(), 256);
            assert_eq!(request.launch.terminal_profile.palette.get(16), Some(&0));
            assert_eq!(
                request.launch.terminal_profile.palette.get(231),
                Some(&0xffffff)
            );
            assert_eq!(request.launch.terminal_profile.version_response, "Orchbee");
            assert!(!identities.contains(&request.creation_id));
            assert!(!correlations.contains(&envelope.correlation_id));
            identities.push(request.creation_id);
            correlations.push(envelope.correlation_id);
            let epoch = HostEpoch::from_bytes([0x44; 16]).map_err(protocol_io)?;
            let response = encode_terminal_creation_control(
                &TerminalCreationControl::TerminalCreated(TerminalIdentity {
                    creation_id: request.creation_id,
                    host_epoch: epoch,
                    terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
                        *request.creation_id.as_bytes(),
                    )
                    .map_err(protocol_io)?,
                }),
            )
            .map_err(protocol_io)?;
            let event =
                encode_lifecycle_control(&LifecycleControl::TerminalStarted(TerminalIdentity {
                    creation_id: orchbee_terminal_protocol::CreationId::from_bytes([77; 16])
                        .map_err(protocol_io)?,
                    host_epoch: epoch,
                    terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([78; 16])
                        .map_err(protocol_io)?,
                }))
                .map_err(protocol_io)?;
            let event_record = encode_transport_envelope(&Envelope {
                correlation_id: None,
                host_epoch: Some(epoch),
                kind: event.kind,
                optional_message: false,
                payload: &event.payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Event,
            })
            .map_err(protocol_io)?;
            // A retained terminal's notification may precede this command's reply.
            host.write_all(&event_record)?;
            host.write_all(
                &host_record(
                    response.kind,
                    response.role,
                    &response.payload,
                    epoch,
                    envelope
                        .correlation_id
                        .ok_or_else(|| io::Error::other("missing correlation"))?,
                )
                .map_err(protocol_io)?,
            )?;
            host.write_all(&event_record)?;
        }
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));

    let original = authorization(1, 1)?;
    let wrong_worker = TerminalOperationAuthorization::new(
        original.terminal_id(),
        original.operation_id(),
        CapabilityWorkerId::from_bytes([99; 16])?,
        original.worker_generation(),
        original.fence(),
        original.lease(),
        original.capability_version(),
    );
    let rejected = request_launch(
        &mut server,
        &AuthorizedTerminalLaunch::new(wrong_worker, None, TerminalLaunchMode::Deferred),
    )?;
    assert_eq!(
        rejected.outcome(),
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::WrongWorker)
    );
    for (id, cwd) in [
        (1, None),
        (2, Some(AbsoluteCwd::new("/tmp/literal/../ordinary")?)),
    ] {
        let request = launch(id, cwd, TerminalLaunchMode::Deferred)?;
        let result = request_launch(&mut server, &request)?;
        assert!(matches!(
            result.outcome(),
            TerminalLaunchOutcome::HostAccepted(_)
        ));
        assert_eq!(request_launch(&mut server, &request)?, result);
    }
    drop(server);
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host launch observer")?;
    Ok(())
}

#[test]
fn malformed_partial_and_pipelined_server_records_never_reach_host() -> TestResult {
    let request = launch(1, None, TerminalLaunchMode::Deferred)?;
    let valid = encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
        request.clone(),
    ))?;
    let mut pipelined = valid.clone();
    pipelined.extend_from_slice(&valid);
    let wrong_direction = encode_message(&CapabilityNodeMessage::TerminalLaunchResult(
        TerminalLaunchResult::new(
            request.authorization(),
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LeaseExpired),
        )?,
    ))?;
    for bytes in [
        b"OBCN\x00\x01\xff\xff".to_vec(),
        b"OB".to_vec(),
        wrong_direction,
        pipelined,
    ] {
        let fixture = RuntimeFixture::new()?;
        let listener = fixture.bind_host_listener()?;
        let host = thread::spawn(move || {
            let mut host = admit_host(&listener)?;
            require_connection_closed(&mut host)
        });
        let worker = RunningWorker::start(&fixture)?;
        let (mut server, _) = admit_worker(&fixture.server_listener)?;
        assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
        server.write_all(&bytes)?;
        require_connection_closed(&mut server)?;
        assert_failed_after_exact_readiness(worker.finish()?);
        join_peer(host, "Host no-command observer")?;
    }
    Ok(())
}

#[test]
fn silent_host_returns_unknown_once_then_worker_disconnects() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || {
        let mut host = admit_host(&listener)?;
        let record = read_terminal_record(&mut host)?;
        let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
        assert_eq!(envelope.role, MessageRole::Request);
        assert_eq!(envelope.kind, 11);
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    let result = request_launch(&mut server, &launch(1, None, TerminalLaunchMode::Deferred)?)?;
    assert_eq!(
        result.outcome(),
        TerminalLaunchOutcome::OutcomeUnknown {
            host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes([0x44; 16])?
        }
    );
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "silent Host")?;
    Ok(())
}

#[test]
fn exact_result_is_delivered_even_when_host_closes_after_its_response() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || -> io::Result<()> {
        let mut host = admit_host(&listener)?;
        let record = read_terminal_record(&mut host)?;
        let envelope = decode_transport_envelope(&record).map_err(protocol_io)?;
        let TerminalCreationControl::CreateTerminal(request) =
            decode_terminal_creation_control(envelope.kind, envelope.role, envelope.payload)
                .map_err(protocol_io)?
        else {
            return Err(io::Error::other("missing CreateTerminal"));
        };
        let epoch = HostEpoch::from_bytes([0x44; 16]).map_err(protocol_io)?;
        let response = encode_terminal_creation_control(&TerminalCreationControl::TerminalCreated(
            TerminalIdentity {
                creation_id: request.creation_id,
                host_epoch: epoch,
                terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([55; 16])
                    .map_err(protocol_io)?,
            },
        ))
        .map_err(protocol_io)?;
        host.write_all(
            &host_record(
                response.kind,
                response.role,
                &response.payload,
                epoch,
                envelope
                    .correlation_id
                    .ok_or_else(|| io::Error::other("missing correlation"))?,
            )
            .map_err(protocol_io)?,
        )
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    let result = request_launch(&mut server, &launch(1, None, TerminalLaunchMode::Deferred)?)?;
    assert!(matches!(
        result.outcome(),
        TerminalLaunchOutcome::HostAccepted(_)
    ));
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "Host final-response observer")?;
    Ok(())
}

#[test]
fn a_partial_idle_host_event_has_a_deadline_and_closes_both_peers() -> TestResult {
    let fixture = RuntimeFixture::new()?;
    let listener = fixture.bind_host_listener()?;
    let host = thread::spawn(move || {
        let mut host = admit_host(&listener)?;
        host.write_all(b"OB")?;
        require_connection_closed(&mut host)
    });
    let worker = RunningWorker::start(&fixture)?;
    let (mut server, _) = admit_worker(&fixture.server_listener)?;
    assert_eq!(worker.await_readiness()?, Some(*b"ready\n"));
    require_connection_closed(&mut server)?;
    assert_failed_after_exact_readiness(worker.finish()?);
    join_peer(host, "partial Host event observer")?;
    Ok(())
}

fn admit_host(listener: &UnixListener) -> io::Result<UnixStream> {
    let mut stream = accept_with_deadline(listener)?;
    configure_peer_stream(&stream)?;
    let (hello, correlation) = read_host_hello(&mut stream)?;
    write_hello_accepted(&mut stream, &hello, correlation)?;
    Ok(stream)
}

fn request_launch(
    server: &mut UnixStream,
    request: &AuthorizedTerminalLaunch,
) -> TestResult<TerminalLaunchResult> {
    write_obcn(
        server,
        &CapabilityNodeMessage::AuthorizedTerminalLaunch(request.clone()),
    )?;
    let CapabilityNodeMessage::TerminalLaunchResult(result) = read_obcn(server)? else {
        return Err(io::Error::other("missing Launch result").into());
    };
    assert_eq!(result.authorization(), request.authorization());
    Ok(result)
}

fn launch(
    id: u8,
    cwd: Option<AbsoluteCwd>,
    mode: TerminalLaunchMode,
) -> TestResult<AuthorizedTerminalLaunch> {
    Ok(AuthorizedTerminalLaunch::new(
        authorization(id, id)?,
        cwd,
        mode,
    ))
}

fn authorization(terminal: u8, operation: u8) -> TestResult<TerminalOperationAuthorization> {
    Ok(TerminalOperationAuthorization::new(
        TerminalId::from_bytes([terminal; 16])?,
        OperationId::from_bytes([operation; 16])?,
        CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        WorkerGeneration::new(nonzero(7)?),
        TerminalFence::new(nonzero(u64::from(operation))?),
        TerminalOperationLease::new(
            LeaseId::from_bytes([operation; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(nonzero(lease_expiry()?)?),
        ),
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    ))
}

// Stable within one test process so repeated constructions preserve the exact lease echo.
fn lease_expiry() -> TestResult<u64> {
    static EXPIRY: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    if let Some(expiry) = EXPIRY.get() {
        return Ok(*expiry);
    }
    let expiry = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())? + 60_000;
    Ok(*EXPIRY.get_or_init(|| expiry))
}

fn nonzero(value: u64) -> TestResult<NonZeroU64> {
    NonZeroU64::new(value).ok_or_else(|| io::Error::other("zero test value").into())
}

fn geometry() -> TestResult<TerminalPixelGeometry> {
    Ok(TerminalPixelGeometry::new(100, 30, 8, 16)?)
}

fn host_record(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
    epoch: HostEpoch,
    correlation: CorrelationId,
) -> TestResult<Vec<u8>> {
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(epoch),
        kind,
        optional_message: false,
        payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role,
    })?)
}

fn connect_observer(endpoint: &Path) -> TestResult<(UnixStream, HostEpoch)> {
    let mut stream = UnixStream::connect(endpoint)?;
    configure_peer_stream(&stream)?;
    let control = encode_connection_control(&ConnectionControl::Hello(Hello {
        client_id: ClientId::from_bytes([88; 16])?,
        protocol: CURRENT_PROTOCOL_VERSION,
        frame_bytes: u32::try_from(MAXIMUM_TERMINAL_FRAME_BYTES)?,
        input_bytes: u32::try_from(MAXIMUM_INPUT_BYTES)?,
        row_window_bytes: u32::try_from(MAXIMUM_ROW_WINDOW_BYTES)?,
        supported_optional_features: Vec::new(),
    }))?;
    stream.write_all(&encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([7; 16])?),
        host_epoch: None,
        kind: control.kind,
        optional_message: false,
        payload: &control.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: control.role,
    })?)?;
    let record = read_terminal_record(&mut stream)?;
    let envelope = decode_transport_envelope(&record)?;
    let ConnectionControl::HelloAccepted(accepted) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err(io::Error::other("Host did not admit observer").into());
    };
    Ok((stream, accepted.host_epoch))
}

#[path = "terminal_operations/viewers.rs"]
mod viewers;

#[path = "terminal_operations/controllers.rs"]
mod controllers;

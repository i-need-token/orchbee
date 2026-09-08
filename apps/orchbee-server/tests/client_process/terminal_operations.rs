use super::{
    FIRST_WORKER, PROCESS_TIMEOUT, RunningServer, TestResult, connect_bounded_client,
    connect_worker,
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostStartAccepted, HostTerminalId, HostTerminalIdentity,
    StartTransition as WorkerStartTransition, TerminalLaunchOutcome, TerminalLaunchResult,
    TerminalStartOutcome, TerminalStartResult, read_message as read_obcn,
    write_message as write_obcn,
};
use orchbee_client_server_protocol::{
    ClientMessage, CreateAcceptedState, CreateOutcome, Geometry, RequestId, ServerMessage,
    StartMode, StartOutcome, StartRejectedReason, StartTransition, StartUnavailableReason,
    TerminalCreateRequest, TerminalStartRequest, read_server_message, write_client_message,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ClientId, ConnectionControl, CorrelationId, Envelope, Hello,
    InventoryControl, ListTerminals, MAXIMUM_INPUT_BYTES, MAXIMUM_ROW_WINDOW_BYTES,
    MAXIMUM_TERMINAL_FRAME_BYTES, MessageRole, PayloadEncoding, TerminalSummary,
    decode_connection_control, decode_inventory_control, decode_transport_envelope,
    encode_connection_control, encode_inventory_control, encode_transport_envelope,
};
use orchbee_terminal_protocol::{MAXIMUM_TRANSPORT_RECORD_BYTES, TRANSPORT_ENVELOPE_HEADER_BYTES};
use std::{
    io::{self, Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn id(value: u128) -> TestResult<RequestId> {
    Ok(RequestId::parse(&format!("{value:032x}"))?)
}
fn geometry() -> TestResult<Geometry> {
    Ok(Geometry::new(80, 24, 8, 16)?)
}
fn create(value: u128) -> TestResult<ClientMessage> {
    Ok(ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        id(value)?,
        None,
        None,
        StartMode::Deferred,
    )))
}
fn start(value: u128, reference: u128) -> TestResult<ClientMessage> {
    Ok(ClientMessage::TerminalStart(TerminalStartRequest::new(
        id(value)?,
        id(reference)?,
        geometry()?,
    )?))
}
fn exchange(client: &mut UnixStream, request: &ClientMessage) -> TestResult<ServerMessage> {
    write_client_message(client, request)?;
    Ok(read_server_message(client)?)
}
fn identity() -> TestResult<HostTerminalIdentity> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([3; 16])?,
        HostEpoch::from_bytes([4; 16])?,
        HostTerminalId::from_bytes([5; 16])?,
    ))
}

#[test]
fn client_loss_during_create_preserves_binding_for_another_session_and_serializes_start()
-> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let mut peers = Vec::new();
    for _slot in 1..8 {
        peers.push(connect_bounded_client(server.client_endpoint())?.0);
    }
    write_client_message(&mut first, &create(1)?)?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_obcn(&mut worker)? else {
        return Err("missing launch".into());
    };
    assert_eq!(
        launch.authorization().worker_id().as_bytes(),
        FIRST_WORKER.as_bytes()
    );
    drop(first);
    write_obcn(
        &mut worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity()?,
                HostLaunchState::AwaitingStart,
            )),
        )?),
    )?;
    // All slots were occupied. Re-admission proves the disconnected handler finished
    // its result publication/cleanup, which follows coordinator commitment.
    let _settled = super::wait_for_client_admission(server.client_endpoint())?;
    let mut second = peers.remove(0);
    let mut third = peers.remove(0);
    write_client_message(&mut second, &start(2, 1)?)?;
    let CapabilityNodeMessage::AuthorizedTerminalStart(started) = read_obcn(&mut worker)? else {
        return Err("missing Start command".into());
    };
    assert_ne!(
        started.authorization().operation_id(),
        launch.authorization().operation_id()
    );
    assert_eq!(
        started.authorization().terminal_id(),
        launch.authorization().terminal_id()
    );
    assert_eq!(started.authorization().fence().get(), 2);
    assert_eq!(started.identity(), identity()?);
    assert!(
        matches!(exchange(&mut third,&start(100,1)?)?,ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::Unavailable {reason:StartUnavailableReason::ServerBusy})
    );
    write_obcn(
        &mut worker,
        &CapabilityNodeMessage::TerminalStartResult(TerminalStartResult::new(
            started.authorization(),
            started.identity(),
            TerminalStartOutcome::HostAccepted(HostStartAccepted::new(
                started.identity(),
                started.geometry(),
                WorkerStartTransition::Started,
            )),
        )?),
    )?;
    assert!(
        matches!(read_server_message(&mut second)?,ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::Accepted {geometry:geometry()?,transition:StartTransition::Started})
    );
    assert!(
        matches!(exchange(&mut third,&start(101,1)?)?,ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::Accepted {geometry:geometry()?,transition:StartTransition::AlreadyStarted})
    );
    Ok(())
}

#[test]
fn lost_worker_after_delivery_returns_unknown_and_never_retargets_create_reference() -> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut client, _) = connect_bounded_client(server.client_endpoint())?;
    write_client_message(&mut client, &create(1)?)?;
    assert!(matches!(
        read_obcn(&mut worker)?,
        CapabilityNodeMessage::AuthorizedTerminalLaunch(_)
    ));
    drop(worker);
    assert!(
        matches!(read_server_message(&mut client)?,ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
    );
    assert!(
        matches!(exchange(&mut client,&start(2,1)?)?,ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::TerminalCreateUnavailable})
    );
    Ok(())
}

#[test]
fn real_clients_server_worker_host_start_view_and_disconnect_retain_one_running_terminal()
-> TestResult {
    run_real_terminal_vertical(false)
}

#[test]
fn real_standalone_opens_share_and_reopen_one_retained_shell() -> TestResult {
    run_real_terminal_vertical(true)
}

fn run_real_terminal_vertical(standalone: bool) -> TestResult {
    let server = RunningServer::start()?;
    let host_directory = HostDirectory::new()?;
    let runtime = host_directory.0.as_path();
    std::fs::set_permissions(runtime, std::fs::Permissions::from_mode(0o700))?;
    let endpoint_id = "1234567890abcdef1234567890abcdef";
    let endpoint = runtime.join(format!("{endpoint_id}.s"));
    let host_binary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../terminal-host/zig-out/bin/orchbee-terminal-host");
    let worker_binary =
        Path::new(env!("CARGO_BIN_EXE_orchbee-server")).with_file_name("orchbee-worker");
    for binary in [&host_binary, &worker_binary] {
        assert!(
            binary.is_file(),
            "build the Worker and Terminal Host prerequisites before this process witness: {}",
            binary.display()
        );
    }
    let mut host = ChildGuard::spawn(
        Command::new(host_binary)
            .args(["serve", "--runtime-directory"])
            .arg(runtime)
            .args(["--endpoint-id", endpoint_id]),
        false,
    )?;
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    while !endpoint.exists() {
        if let Some(status) = host.0.try_wait()? {
            let mut diagnostic = String::new();
            if let Some(mut stderr) = host.0.stderr.take() {
                stderr.read_to_string(&mut diagnostic)?;
            }
            return Err(format!("Host exited before readiness: {status}; {diagnostic}").into());
        }
        if Instant::now() >= deadline {
            return Err("Host endpoint did not appear".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    let worker = ChildGuard::spawn(
        Command::new(worker_binary)
            .env("SHELL", "/bin/sh")
            .args(["--local-endpoint"])
            .arg(server.worker_endpoint())
            .arg("--terminal-host-runtime-directory")
            .arg(runtime)
            .args(["--terminal-host-endpoint-id", endpoint_id]),
        true,
    )?;
    let (mut second, reference) = if standalone {
        let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
        let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
        write_client_message(&mut first, &standalone::open(1, geometry()?)?)?;
        write_client_message(&mut second, &standalone::open(2, geometry()?)?)?;
        let reference = standalone::opened(read_server_message(&mut first)?, 1)?;
        assert_eq!(
            standalone::opened(read_server_message(&mut second)?, 2)?,
            reference
        );
        assert_ne!(reference, id(1)?);
        assert_ne!(reference, id(2)?);
        drop(first);
        drop(second);
        let (mut reopened, _) = connect_bounded_client(server.client_endpoint())?;
        assert_eq!(
            standalone::opened(
                exchange(
                    &mut reopened,
                    &standalone::open(3, Geometry::new(100, 30, 8, 16)?)?
                )?,
                3
            )?,
            reference
        );
        (reopened, reference)
    } else {
        let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
        assert!(
            matches!(exchange(&mut first,&create(1)?)?,ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Accepted {state:CreateAcceptedState::AwaitingStart})
        );
        drop(first);
        let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
        assert!(
            matches!(exchange(&mut second,&start(2,1)?)?,ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::Accepted {geometry:geometry()?,transition:StartTransition::Started})
        );
        assert!(
            matches!(exchange(&mut second,&start(3,1)?)?,ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::Accepted {geometry:geometry()?,transition:StartTransition::AlreadyStarted})
        );

        (second, id(1)?)
    };
    viewers::exercise_real_viewers(&server, &mut second, runtime, &reference, !standalone)?;
    drop(second);
    drop(worker);
    assert_eq!(
        running_inventory(&endpoint)?,
        1,
        "Client and Worker loss must preserve one running terminal"
    );
    Ok(())
}

struct ChildGuard(Child);
impl ChildGuard {
    fn spawn(command: &mut Command, ready: bool) -> TestResult<Self> {
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut guard = Self(child);
        if ready {
            let mut output = guard.0.stdout.take().ok_or("missing child output")?;
            let (sender, receiver) = mpsc::sync_channel(1);
            thread::spawn(move || {
                let mut marker = [0; 6];
                let _sent = sender.send(output.read_exact(&mut marker).map(|()| marker));
            });
            assert_eq!(receiver.recv_timeout(PROCESS_TIMEOUT)??, *b"ready\n");
        }
        Ok(guard)
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _killed = self.0.kill();
        let _waited = self.0.wait();
    }
}

fn running_inventory(endpoint: &Path) -> TestResult<usize> {
    let mut observer = UnixStream::connect(endpoint)?;
    observer.set_read_timeout(Some(PROCESS_TIMEOUT))?;
    observer.set_write_timeout(Some(PROCESS_TIMEOUT))?;
    let hello = encode_connection_control(&ConnectionControl::Hello(Hello {
        client_id: ClientId::from_bytes([88; 16])?,
        protocol: CURRENT_PROTOCOL_VERSION,
        frame_bytes: u32::try_from(MAXIMUM_TERMINAL_FRAME_BYTES)?,
        input_bytes: u32::try_from(MAXIMUM_INPUT_BYTES)?,
        row_window_bytes: u32::try_from(MAXIMUM_ROW_WINDOW_BYTES)?,
        supported_optional_features: Vec::new(),
    }))?;
    observer.write_all(&encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([7; 16])?),
        host_epoch: None,
        kind: hello.kind,
        optional_message: false,
        payload: &hello.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?)?;
    let record = read_terminal_record(&mut observer)?;
    let response = decode_transport_envelope(&record)?;
    let ConnectionControl::HelloAccepted(accepted) =
        decode_connection_control(response.kind, response.role, response.payload)?
    else {
        return Err("missing Host acceptance".into());
    };
    let list = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
        cursor: None,
        maximum_terminals: 2,
    }))?;
    observer.write_all(&encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([8; 16])?),
        host_epoch: Some(accepted.host_epoch),
        kind: list.kind,
        optional_message: false,
        payload: &list.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?)?;
    let record = read_terminal_record(&mut observer)?;
    let response = decode_transport_envelope(&record)?;
    let InventoryControl::TerminalList(list) =
        decode_inventory_control(response.kind, response.role, response.payload)?
    else {
        return Err("missing Host inventory".into());
    };
    assert!(list.terminals.iter().all(|terminal|matches!(terminal,TerminalSummary::Running {core,..} if core.identity.host_epoch==accepted.host_epoch)));
    for terminal in &list.terminals {
        let TerminalSummary::Running { core, .. } = terminal else {
            continue;
        };
        let stop = orchbee_terminal_protocol::encode_lifecycle_control(
            &orchbee_terminal_protocol::LifecycleControl::ForceTerminateTerminal(
                orchbee_terminal_protocol::TerminalTarget {
                    terminal_id: core.identity.terminal_id,
                },
            ),
        )?;
        observer.write_all(&encode_transport_envelope(&Envelope {
            correlation_id: Some(CorrelationId::from_bytes([9; 16])?),
            host_epoch: Some(accepted.host_epoch),
            kind: stop.kind,
            optional_message: false,
            payload: &stop.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        })?)?;
        let record = read_terminal_record(&mut observer)?;
        let response = decode_transport_envelope(&record)?;
        assert_eq!(response.kind, stop.kind);
        assert_eq!(response.role, MessageRole::Response);
        assert_eq!(response.payload, stop.payload);
    }
    Ok(list.terminals.len())
}

fn read_terminal_record(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
    stream.read_exact(&mut header)?;
    let total_length = usize::try_from(u32::from_le_bytes([
        header[8], header[9], header[10], header[11],
    ]))
    .map_err(io::Error::other)?;
    if !(TRANSPORT_ENVELOPE_HEADER_BYTES..=MAXIMUM_TRANSPORT_RECORD_BYTES).contains(&total_length) {
        return Err(io::Error::other(
            "Worker sent an invalid Terminal Protocol record length",
        ));
    }
    let mut record = vec![0_u8; total_length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    stream.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}

struct HostDirectory(std::path::PathBuf);
impl HostDirectory {
    fn new() -> io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        let number = super::NEXT_FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::path::PathBuf::from("/tmp")
            .join(format!("ob-server-{}-{number}", std::process::id()));
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for HostDirectory {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}

#[path = "terminal_operations/viewers.rs"]
mod viewers;

#[path = "terminal_operations/standalone.rs"]
mod standalone;

#[path = "terminal_operations/tab_close.rs"]
mod tab_close;

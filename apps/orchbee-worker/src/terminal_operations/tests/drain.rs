use orchbee_local_ipc::{LocalListener as UnixListener, LocalStream as UnixStream};
use std::{
    cell::RefCell,
    error::Error,
    fs,
    io::{self, Read, Write},
    num::NonZeroU64,
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use orchbee_capability_node_protocol::{
    AgentDrainEntry, AgentDrainRequest, CapabilityNodeMessage, WorkerGeneration, read_message,
    write_message,
};
use orchbee_terminal_protocol::{
    ConnectionControl, Envelope, HelloAccepted, HostEpoch, PayloadEncoding,
    TRANSPORT_ENVELOPE_HEADER_BYTES, TerminalApplicationSignal, TerminalApplicationSignalChannel,
    TerminalEffectEvent, TerminalIdentity, decode_connection_control, decode_transport_envelope,
    encode_connection_control, encode_terminal_effect_event, encode_transport_envelope,
};

use super::super::dispatch_drain_request;
use crate::{
    agent_drain::AgentDrain, host_connection::HostBrokerConnection,
    terminal_authorization::AuthorizedHostOperationKernel,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
static NEXT: AtomicU64 = AtomicU64::new(0);

const LOCAL_WORKER_ID: orchbee_local_worker_protocol::WorkerId =
    orchbee_local_worker_protocol::WorkerId::from_bytes(*b"orchbee-local-v1");
const GENERATION: u64 = 7;

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("fixture nonzero")
}

fn request(token: u64, acknowledged: u64) -> AgentDrainRequest {
    AgentDrainRequest {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes(*b"orchbee-local-v1")
            .expect("worker id"),
        worker_generation: WorkerGeneration::new(nz(GENERATION)),
        token: nz(token),
        acknowledged,
    }
}

fn read_record(peer: &mut UnixStream) -> TestResult<Vec<u8>> {
    let mut header = [0; TRANSPORT_ENVELOPE_HEADER_BYTES];
    peer.read_exact(&mut header)?;
    let length = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let mut record = vec![0; length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    peer.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}

fn signal_record(epoch: HostEpoch, payload: &str) -> TestResult<Vec<u8>> {
    let event = encode_terminal_effect_event(&TerminalEffectEvent::ApplicationSignal(
        TerminalApplicationSignal {
            channel: TerminalApplicationSignalChannel::OrchbeeAgentV1,
            identity: TerminalIdentity {
                creation_id: orchbee_terminal_protocol::CreationId::from_bytes([5; 16])?,
                host_epoch: epoch,
                terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([6; 16])?,
            },
            payload: payload.to_string(),
            sequence: nz(1),
        },
    ))?;
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(epoch),
        kind: event.kind,
        optional_message: false,
        payload: &event.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
        role: event.role,
    })?)
}

/// A scripted socket peer performing the real Host handshake: the listener
/// thread validates the exact Hello and answers HelloAccepted, mirroring the
/// lifecycle fixture. This is socket-level integration, not a real Host
/// process — the peer is a test thread, not zig-out code.
fn scripted_host_peer() -> TestResult<(
    HostBrokerConnection,
    Rc<RefCell<AgentDrain>>,
    UnixStream,
    PathBuf,
)> {
    let directory = PathBuf::from(format!(
        "/tmp/obdrain-{}-{}",
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
                return Err("expected Hello".into());
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
            let reply = encode_transport_envelope(&Envelope {
                correlation_id: envelope.correlation_id,
                host_epoch: Some(epoch),
                kind: control.kind,
                optional_message: false,
                payload: &control.payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
                role: control.role,
            })?;
            peer.write_all(&reply)?;
            Ok(peer)
        };
        run().map_err(|error| error.to_string())
    });
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let connection = HostBrokerConnection::connect(&endpoint, Rc::clone(&drain))?;
    let peer = thread
        .join()
        .map_err(|_| "handshake peer panicked")?
        .map_err(io::Error::other)?;
    Ok((connection, drain, peer, directory))
}

/// A kind-17 request answered before any Host exists must carry a truthful
/// empty page; `send` reports host_closed=true for the absent Host and that
/// flag must not be treated as a failure or universal close.
#[test]
fn pre_host_request_gets_truthful_empty_reply() -> TestResult {
    let (server, mut peer) = UnixStream::pair()?;
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let closed = dispatch_drain_request(
        &server,
        None,
        &drain,
        &request(11, 0),
        LOCAL_WORKER_ID,
        nz(GENERATION),
    )?;
    assert!(closed, "no Host attached must report host_closed");
    let reply = match read_message(&mut peer)? {
        CapabilityNodeMessage::AgentDrainReply(reply) => reply,
        _ => return Err("expected AgentDrainReply".into()),
    };
    assert_eq!(reply.token_echo(), nz(11));
    assert_eq!(reply.acknowledged_echo(), 0);
    assert_eq!(reply.queue_tail(), 0);
    assert!(reply.entries().is_empty());
    Ok(())
}

/// The shared dispatch path refuses a drain request fenced to another Worker
/// or generation — a fatal error rather than a typed rejection.
#[test]
fn wrong_worker_fence_is_fatal() -> TestResult {
    let (server, _peer) = UnixStream::pair()?;
    let drain = Rc::new(RefCell::new(AgentDrain::new()));
    let mut fenced = request(11, 0);
    fenced.worker_id =
        orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).expect("worker");
    assert!(
        dispatch_drain_request(
            &server,
            None,
            &drain,
            &fenced,
            LOCAL_WORKER_ID,
            nz(GENERATION)
        )
        .is_err()
    );
    let mut stale_generation = request(11, 0);
    stale_generation.worker_generation = WorkerGeneration::new(nz(GENERATION + 1));
    assert!(
        dispatch_drain_request(
            &server,
            None,
            &drain,
            &stale_generation,
            LOCAL_WORKER_ID,
            nz(GENERATION),
        )
        .is_err()
    );
    Ok(())
}

/// An application signal for a terminal with no attached viewer still enters
/// the drain queue through the real receive_event routing path.
#[test]
fn application_signal_without_viewer_reaches_the_queue() -> TestResult {
    let (connection, drain, _peer, directory) = scripted_host_peer()?;
    let epoch = connection.host_epoch();
    connection.receive_event(signal_record(epoch, "{\"event\":\"ready\"}")?)?;
    let (entries, minted, _, _) = drain.borrow().snapshot();
    let _ = fs::remove_dir_all(&directory);
    assert_eq!(minted, 1);
    assert_eq!(entries.len(), 1);
    let Some(AgentDrainEntry::Event {
        sequence,
        payload,
        identity,
    }) = entries.first()
    else {
        return Err("expected a queued event".into());
    };
    assert_eq!(sequence.get(), 1);
    assert_eq!(payload, "{\"event\":\"ready\"}");
    assert_eq!(identity.host_epoch().as_bytes(), *epoch.as_bytes());
    Ok(())
}

/// A signal carrying a different Host epoch remains a fatal protocol error.
#[test]
fn foreign_epoch_signal_stays_fatal() -> TestResult {
    let (connection, _drain, _peer, directory) = scripted_host_peer()?;
    let foreign = HostEpoch::from_bytes([8; 16])?;
    let result = connection.receive_event(signal_record(foreign, "{}")?);
    let _ = fs::remove_dir_all(&directory);
    assert!(result.is_err());
    Ok(())
}

/// End-to-end admitted dispatch: the Host peer writes an application signal
/// that the idle loop consumes through consume_event, then a real kind-17
/// request returns it on the wire through one shared queue state.
#[test]
fn admitted_request_replies_with_host_observed_event() -> TestResult {
    let (mut connection, drain, mut host_peer, directory) = scripted_host_peer()?;
    let (server, mut server_peer) = UnixStream::pair()?;
    server_peer.set_read_timeout(Some(Duration::from_secs(3)))?;
    server_peer.set_write_timeout(Some(Duration::from_secs(3)))?;
    let epoch = connection.host_epoch();
    // The admitted loop runs on this thread: the connection's drain owner is
    // Rc-shared and cannot cross threads. The spawned thread drives both
    // socket peers instead.
    let runner = thread::spawn(move || -> Result<(), String> {
        let mut run = || -> TestResult {
            // The Host application signal arrives while the loop is idle;
            // consume_event delivers it through receive_event into the queue.
            host_peer.write_all(&signal_record(epoch, "ready")?)?;
            // Let the poll loop consume the pending Host record before the
            // request lands so the reply is deterministic.
            thread::sleep(Duration::from_millis(50));
            write_message(
                &mut server_peer,
                &CapabilityNodeMessage::AgentDrainRequest(request(21, 0)),
            )?;
            let reply = match read_message(&mut server_peer)? {
                CapabilityNodeMessage::AgentDrainReply(reply) => reply,
                other => {
                    return Err(format!("expected AgentDrainReply, got {other:?}").into());
                }
            };
            assert_eq!(reply.token_echo(), nz(21));
            assert_eq!(reply.queue_tail(), 1);
            assert_eq!(reply.entries().len(), 1);
            assert!(matches!(
                reply.entries().first(),
                Some(AgentDrainEntry::Event { sequence, .. }) if sequence.get() == 1
            ));
            server_peer.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        };
        run().map_err(|error| error.to_string())
    });
    server.set_nonblocking(true)?;
    connection.stream().set_nonblocking(true)?;
    let kernel = AuthorizedHostOperationKernel::new(LOCAL_WORKER_ID, nz(GENERATION))?;
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let outcome = super::super::run_admitted(
        server,
        &mut connection,
        LOCAL_WORKER_ID,
        nz(GENERATION),
        kernel,
        super::super::AdmittedAux {
            preferences: None,
            #[cfg(windows)]
            shell: &Default::default(),
            drain: &drain,
            worktree_slot: &worktree_slot,
            #[cfg(windows)]
            mutation_slot: &crate::worktree_mutation_job::WorktreeMutationSlot::new(
                crate::worktree_mutation_windows::run,
            ),
            #[cfg(windows)]
            plugin_jobs: &crate::plugin_jobs::PluginJobs::new(),
            session_runtime: None,
            #[cfg(windows)]
            managed_host: None,
        },
    );
    assert!(outcome.is_err());
    runner.join().map_err(|_| "peer driver panic")??;
    let _ = fs::remove_dir_all(&directory);
    Ok(())
}

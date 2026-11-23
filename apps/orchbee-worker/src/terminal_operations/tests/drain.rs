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
    AgentDrainEntry, AgentDrainRequest, CapabilityNodeMessage, OperationId,
    TerminalViewerRecordReady, WorkerGeneration, WorkerRequestDeclined, encode_message,
    read_message, write_message,
};
use orchbee_local_runtime::LogValue;
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

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;
static NEXT: AtomicU64 = AtomicU64::new(0);

const LOCAL_WORKER_ID: orchbee_local_worker_protocol::WorkerId =
    orchbee_local_worker_protocol::WorkerId::from_bytes(*b"orchbee-local-v1");
const GENERATION: u64 = 7;

pub(super) fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("fixture nonzero")
}

pub(super) fn request(token: u64, acknowledged: u64) -> AgentDrainRequest {
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
pub(super) fn scripted_host_peer() -> TestResult<(
    HostBrokerConnection,
    Rc<RefCell<AgentDrain>>,
    UnixStream,
    PathBuf,
)> {
    // A Unix socket path must stay short, so the socket's directory sits in /tmp there. Windows
    // names its endpoint a pipe, and `/tmp` there is a folder on the current drive that a machine
    // need not have.
    #[cfg(unix)]
    let parent = PathBuf::from("/tmp");
    #[cfg(windows)]
    let parent = std::env::temp_dir();
    let directory = parent.join(format!(
        "obdrain-{}-{}",
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
            crate::host_connection::answer_test_offer(&mut peer)?;
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

/// Before any Host exists, a whole frame the Worker cannot decode is answered as unreadable and the
/// wait goes on to the next request: the absent Host's closed flag ends nothing.
#[test]
fn pre_host_unreadable_frame_is_answered_and_the_next_request_arrives() -> TestResult {
    let (server, mut peer) = UnixStream::pair()?;
    peer.set_read_timeout(Some(Duration::from_secs(3)))?;
    let next = CapabilityNodeMessage::AgentDrainRequest(request(13, 0));
    let sent = next.clone();
    // Like the Server, the peer writes its next request only once the first is answered.
    let server_peer = thread::spawn(move || -> Result<CapabilityNodeMessage, String> {
        let mut run = || -> TestResult<CapabilityNodeMessage> {
            let mut unknown =
                encode_message(&CapabilityNodeMessage::AgentDrainRequest(request(12, 0)))?;
            unknown[8] = 0xfe;
            peer.write_all(&unknown)?;
            let answer = read_message(&mut peer)?;
            write_message(&mut peer, &sent)?;
            Ok(answer)
        };
        run().map_err(|error| error.to_string())
    });
    let (received, _readable) = super::super::server_transport::receive_timed(&server, None)?;
    assert_eq!(received, next);
    assert_eq!(
        server_peer.join().map_err(|_panic| "peer panicked")??,
        CapabilityNodeMessage::WorkerRequestUnreadable
    );
    Ok(())
}

/// A Server request that breaks its contract fails only itself: through the real admitted loop, a
/// drain fenced to another Worker or generation and one acknowledging more than was offered are
/// each declined, a message only a Worker sends is dropped unanswered, a whole frame the Worker
/// cannot decode is answered as unreadable, and the next drain is served from the unchanged queue.
/// Each is recorded with its cause.
#[test]
fn server_contract_breaks_are_declined_while_the_loop_serves_on() -> TestResult {
    let (connection, drain, mut host_peer, directory) = scripted_host_peer()?;
    let (server, mut server_peer) = UnixStream::pair()?;
    server_peer.set_read_timeout(Some(Duration::from_secs(3)))?;
    server_peer.set_write_timeout(Some(Duration::from_secs(3)))?;
    let epoch = connection.host_epoch();
    let mut other_worker = request(31, 0);
    other_worker.worker_id = orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16])?;
    let mut stale_generation = request(32, 0);
    stale_generation.worker_generation = WorkerGeneration::new(nz(GENERATION + 1));
    let unoffered = request(33, 1);
    // The Host's end stays open until the loop has ended, so only the Server's close can end it.
    let runner = thread::spawn(move || -> Result<UnixStream, String> {
        let mut run = || -> TestResult {
            host_peer.write_all(&signal_record(epoch, "ready")?)?;
            for broken in [other_worker, stale_generation, unoffered] {
                let message = CapabilityNodeMessage::AgentDrainRequest(broken);
                write_message(&mut server_peer, &message)?;
                let answer = read_message(&mut server_peer)?;
                assert_eq!(
                    answer,
                    CapabilityNodeMessage::WorkerRequestDeclined(
                        WorkerRequestDeclined::of(&message).ok_or("a request")?
                    )
                );
            }
            write_message(
                &mut server_peer,
                &CapabilityNodeMessage::TerminalViewerRecordReady(TerminalViewerRecordReady {
                    attachment_id: OperationId::from_bytes([7; 16])?,
                }),
            )?;
            // A whole frame whose kind no message has, as from a newer Server.
            let mut unknown =
                encode_message(&CapabilityNodeMessage::AgentDrainRequest(request(35, 0)))?;
            unknown[8] = 0xfe;
            server_peer.write_all(&unknown)?;
            assert_eq!(
                read_message(&mut server_peer)?,
                CapabilityNodeMessage::WorkerRequestUnreadable
            );
            write_message(
                &mut server_peer,
                &CapabilityNodeMessage::AgentDrainRequest(request(34, 0)),
            )?;
            let CapabilityNodeMessage::AgentDrainReply(reply) = read_message(&mut server_peer)?
            else {
                return Err("expected AgentDrainReply".into());
            };
            assert_eq!(reply.token_echo(), nz(34));
            assert_eq!(reply.entries().len(), 1);
            server_peer.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        };
        run().map_err(|error| error.to_string())?;
        Ok(host_peer)
    });
    server.set_nonblocking(true)?;
    connection.stream().set_nonblocking(true)?;
    let kernel = AuthorizedHostOperationKernel::new(LOCAL_WORKER_ID, nz(GENERATION))?;
    let worktree_slot = crate::worktree_discovery_job::WorktreeDiscoverySlot::new();
    let outcome = super::super::run_admitted(
        server,
        connection,
        LOCAL_WORKER_ID,
        nz(GENERATION),
        kernel,
        super::super::AdmittedAux {
            drain: &drain,
            system_shell: &crate::default_interactive_launch::SystemShell::resolve(),
            worktree_slot: &worktree_slot,
            agent_integrations: &crate::agent_integration_jobs::AgentIntegrationJobs::new(),
            agent_probes: &crate::agent_probe_jobs::AgentProbeJobs::new(),
            mutation_slot: &crate::worktree_mutation_job::WorktreeMutationSlot::new(
                crate::worktree_mutation_git::run,
            ),
            plugin_jobs: &crate::plugin_jobs::PluginJobs::new(),
            session_runtime: None,
        },
    );
    let noted = crate::diagnostic_events::take_noted();
    let _host_peer = runner.join().map_err(|_| "peer driver panic")??;
    let _ = fs::remove_dir_all(&directory);
    let cause = outcome
        .expect_err("closing the Server end ends the connection")
        .downcast::<crate::diagnostic_events::ExitCause>()
        .map_err(|_| "a classified exit")?;
    assert_eq!(cause.peer, crate::diagnostic_events::Peer::Server);
    let declined = |cause: &'static str| {
        noted
            .iter()
            .filter(|(event, fields)| {
                *event == "request-declined"
                    && fields.contains(&("kind", LogValue::Token("agent-drain")))
                    && fields.contains(&("cause", LogValue::Token(cause)))
            })
            .count()
    };
    assert_eq!(
        (declined("fence"), declined("watermark")),
        (2, 1),
        "{noted:?}"
    );
    let unexpected = |message: &'static str| {
        noted.iter().any(|(event, fields)| {
            *event == "server-unexpected" && fields.contains(&("message", LogValue::Token(message)))
        })
    };
    assert!(
        unexpected("worker-only") && unexpected("unreadable"),
        "{noted:?}"
    );
    Ok(())
}

/// An application signal for a terminal with no attached viewer still enters
/// the drain queue through the real receive_event routing path.
#[test]
fn application_signal_without_viewer_reaches_the_queue() -> TestResult {
    let (connection, drain, _peer, directory) = scripted_host_peer()?;
    let epoch = connection.host_epoch();
    connection.receive_event(signal_record(epoch, "{\"event\":\"ready\"}")?);
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

/// A signal carrying a different Host epoch is dropped and mints nothing.
#[test]
fn foreign_epoch_signal_mints_nothing() -> TestResult {
    let (connection, drain, _peer, directory) = scripted_host_peer()?;
    let foreign = HostEpoch::from_bytes([8; 16])?;
    connection.receive_event(signal_record(foreign, "{}")?);
    let (entries, minted, _, _) = drain.borrow().snapshot();
    let _ = fs::remove_dir_all(&directory);
    assert_eq!(minted, 0);
    assert!(entries.is_empty());
    Ok(())
}

/// End-to-end admitted dispatch: the Host peer writes an application signal
/// that the idle loop consumes through consume_event, then a real kind-17
/// request returns it on the wire through one shared queue state.
#[test]
fn admitted_request_replies_with_host_observed_event() -> TestResult {
    let (connection, drain, mut host_peer, directory) = scripted_host_peer()?;
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
        connection,
        LOCAL_WORKER_ID,
        nz(GENERATION),
        kernel,
        super::super::AdmittedAux {
            drain: &drain,
            system_shell: &crate::default_interactive_launch::SystemShell::resolve(),
            worktree_slot: &worktree_slot,
            agent_integrations: &crate::agent_integration_jobs::AgentIntegrationJobs::new(),
            agent_probes: &crate::agent_probe_jobs::AgentProbeJobs::new(),
            mutation_slot: &crate::worktree_mutation_job::WorktreeMutationSlot::new(
                crate::worktree_mutation_git::run,
            ),
            plugin_jobs: &crate::plugin_jobs::PluginJobs::new(),
            session_runtime: None,
        },
    );
    assert!(outcome.is_err());
    runner.join().map_err(|_| "peer driver panic")??;
    let _ = fs::remove_dir_all(&directory);
    Ok(())
}

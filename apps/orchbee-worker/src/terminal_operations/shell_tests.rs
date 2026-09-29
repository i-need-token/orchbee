use super::*;
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

const WORKER: orchbee_local_site_protocol::WorkerId =
    orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]);
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
    let endpoint = crate::test_support::endpoint();
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
            &server,
            &mut host,
            WORKER,
            NonZeroU64::MIN,
            AuthorizedHostOperationKernel::new(WORKER, NonZeroU64::MIN).unwrap(),
            AdmittedAux {
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
                "{{\"schemaVersion\":2,\"artifacts\":{{{}}}}}",
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
            &server,
            WORKER,
            NonZeroU64::MIN,
            &cohort,
            &runtime,
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

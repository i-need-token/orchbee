use std::{
    fs,
    io::{self, Read},
    num::NonZeroU64,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, AgentDrainReply, AgentDrainRequest, CapabilityNodeMessage, OperationId,
    TerminalCwdValidationOutcome, TerminalCwdValidationRequest, WorkerGeneration,
    WorkerId as CapabilityWorkerId, WorktreeJobAction, WorktreeJobOutcome, WorktreeJobPollOutcome,
    WorktreeJobRequest, WorktreeJobStartOutcome, read_message, write_message,
};
use orchbee_local_runtime::{Cohort, ModeCheckedLocalSessionRendezvous};
use orchbee_local_worker_protocol::WorkerId;

use super::run;

const ARTIFACTS: [&str; 4] = [
    "orchbee",
    "orchbee-server",
    "orchbee-worker",
    "orchbee-terminal-host",
];
const ARTIFACT_CONTENT: &[u8] = b"native fixture\n";

/// The file name a cohort role is staged under; the manifest keeps naming the role.
fn staged_name(role: &str) -> String {
    use orchbee_local_runtime::CohortArtifact;
    match role {
        "orchbee" => CohortArtifact::Client,
        "orchbee-server" => CohortArtifact::Server,
        "orchbee-worker" => CohortArtifact::Worker,
        "orchbee-terminal-host" => CohortArtifact::TerminalHost,
        _ => panic!("unknown cohort role {role:?}"),
    }
    .file_name()
}
const ARTIFACT_SHA256: &str = "396cfec2d1c795ce71dce2d9c92f29cddec638632050c17861d771936a99282d";
// Controlled dummy digest declared for the adapters payload — the fixture
// stages no sidecar; cohort admission stays lazy.
const ADAPTERS_SHA256: &str = "c6ee8257e95c140bc8e1ddbd7ab30ffa42b52e4550176be88905f0ec8e4a6f5e";
const WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);
const GENERATION: NonZeroU64 = NonZeroU64::MIN;
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn validation_before_first_launch_uses_real_socket_and_never_creates_host_runtime() -> TestResult {
    let fixture = TestCohort::new()?;
    let cohort = Cohort::load(fixture.path(), Instant::now() + Duration::from_secs(2))?;
    let available = fixture.path().join("available");
    fs::create_dir(&available)?;
    let missing = fixture.path().join("missing");
    let session_runtime = fixture.path().join("runtime");
    fs::create_dir(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSessionRendezvous::host_runtime_directory(&session_runtime);
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
    let (mut server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(Duration::from_secs(2)))?;
    server.set_write_timeout(Some(Duration::from_secs(2)))?;
    let worker_runtime = session_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(
            worker,
            WORKER_ID,
            GENERATION,
            &cohort,
            &worker_runtime,
            &crate::default_interactive_launch::SystemShell::resolve(),
        )
        .is_err()
    });

    for (operation, path, expected) in [
        (
            1,
            available.as_path(),
            TerminalCwdValidationOutcome::DirectoryAvailable,
        ),
        (
            2,
            missing.as_path(),
            TerminalCwdValidationOutcome::DirectoryMissing,
        ),
    ] {
        let request = request(operation, path)?;
        write_message(
            &mut server,
            &CapabilityNodeMessage::TerminalCwdValidationRequest(request.clone()),
        )?;
        let CapabilityNodeMessage::TerminalCwdValidationResult(result) = read_message(&mut server)?
        else {
            return Err(io::Error::other("Worker returned the wrong validation message").into());
        };
        assert_eq!(result.request, request);
        assert_eq!(result.outcome, expected);
        assert!(!host_runtime.exists());
        assert!(!host_endpoint.exists());
    }

    drop(server);
    assert!(
        worker_thread
            .join()
            .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?
    );
    assert!(!host_runtime.exists());
    assert!(!host_endpoint.exists());
    Ok(())
}

/// A Worktree Job request before the first Launch must be answered inside the
/// lazy loop — never retire the connection and never spawn a Host. A plain
/// directory deterministically fails the Git probe, which still proves the
/// full request → fenced dispatch → typed result exchange stayed alive.
#[test]
fn worktree_job_before_first_launch_answers_without_a_host() -> TestResult {
    let fixture = TestCohort::new()?;
    let cohort = Cohort::load(fixture.path(), Instant::now() + Duration::from_secs(2))?;
    let directory = fixture.path().join("plain-directory");
    fs::create_dir(&directory)?;
    let session_runtime = fixture.path().join("runtime");
    fs::create_dir(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSessionRendezvous::host_runtime_directory(&session_runtime);
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
    let (mut server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(Duration::from_secs(5)))?;
    server.set_write_timeout(Some(Duration::from_secs(2)))?;
    let worker_runtime = session_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(
            worker,
            WORKER_ID,
            GENERATION,
            &cohort,
            &worker_runtime,
            &crate::default_interactive_launch::SystemShell::resolve(),
        )
        .is_err()
    });

    let start = WorktreeJobRequest {
        operation_id: OperationId::from_bytes([9; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        action: WorktreeJobAction::Start {
            directory: directory.as_os_str().as_encoded_bytes().to_vec(),
            trust_repository: None,
        },
    };
    write_message(
        &mut server,
        &CapabilityNodeMessage::WorktreeJobRequest(start),
    )?;
    let job_id = {
        let CapabilityNodeMessage::WorktreeJobResult(result) = read_message(&mut server)? else {
            return Err(io::Error::other("Worker returned the wrong worktree message").into());
        };
        assert_eq!(result.operation_id.as_bytes(), [9; 16]);
        let WorktreeJobOutcome::Start(outcome) = result.outcome else {
            return Err(io::Error::other("expected a Start outcome").into());
        };
        assert_eq!(outcome, WorktreeJobStartOutcome::Accepted);
        result.job_id
    };
    assert!(!host_runtime.exists());
    assert!(!host_endpoint.exists());

    // The connection stays alive for follow-up traffic on the admitted job.
    let deadline = Instant::now() + Duration::from_secs(5);
    let terminal = loop {
        let poll = WorktreeJobRequest {
            operation_id: OperationId::from_bytes([10; 16])?,
            worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(GENERATION),
            action: WorktreeJobAction::Poll { job_id },
        };
        write_message(
            &mut server,
            &CapabilityNodeMessage::WorktreeJobRequest(poll),
        )?;
        let CapabilityNodeMessage::WorktreeJobResult(result) = read_message(&mut server)? else {
            return Err(io::Error::other("Worker returned the wrong worktree message").into());
        };
        assert_eq!(result.operation_id.as_bytes(), [10; 16]);
        assert_eq!(result.job_id, job_id);
        let WorktreeJobOutcome::Poll(outcome) = result.outcome else {
            return Err(io::Error::other("expected a Poll outcome").into());
        };
        match outcome {
            WorktreeJobPollOutcome::Pending => {
                assert!(Instant::now() < deadline, "job never settled");
                thread::sleep(Duration::from_millis(25));
            }
            terminal => break terminal,
        }
    };
    // A plain directory fails the probe stage; the typed outcome is what
    // matters, not Git's exact exit status.
    assert!(
        matches!(
            terminal,
            WorktreeJobPollOutcome::GitFailed { .. } | WorktreeJobPollOutcome::Completed { .. }
        ),
        "unexpected terminal outcome: {terminal:?}"
    );
    assert!(!host_runtime.exists());
    assert!(!host_endpoint.exists());

    drop(server);
    assert!(
        worker_thread
            .join()
            .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?
    );
    Ok(())
}

fn request(operation: u8, path: &Path) -> TestResult<TerminalCwdValidationRequest> {
    Ok(TerminalCwdValidationRequest {
        operation_id: OperationId::from_bytes([operation; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        cwd: AbsoluteCwd::new(
            path.to_str()
                .ok_or_else(|| io::Error::other("non-UTF8 test path"))?,
        )?,
    })
}

struct TestCohort(PathBuf);

impl TestCohort {
    fn new() -> io::Result<Self> {
        let identity = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        // The product reaches a Session runtime through its canonical data root, and the runtime
        // rendezvous refuses symlinked components; macOS's temporary folder sits behind /var.
        let path = std::env::temp_dir().canonicalize()?.join(format!(
            "orchbee-worker-lazy-validation-{}-{identity}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        for artifact in ARTIFACTS {
            let artifact_path = path.join(staged_name(artifact));
            fs::write(&artifact_path, ARTIFACT_CONTENT)?;
            fs::set_permissions(artifact_path, fs::Permissions::from_mode(0o700))?;
        }
        let manifest = path.join("cohort.json");
        fs::write(
            &manifest,
            format!(
                r#"{{"schemaVersion":1,"artifacts":{{"orchbee":"{}","orchbee-server":"{}","orchbee-worker":"{}","orchbee-terminal-host":"{}","agent-observation-adapters":"{}"}}}}"#,
                ARTIFACT_SHA256, ARTIFACT_SHA256, ARTIFACT_SHA256, ARTIFACT_SHA256, ADAPTERS_SHA256
            ),
        )?;
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600))?;
        assert_eq!(
            fs::metadata(&manifest)?.permissions().mode() & 0o777,
            0o600,
            "test cohort manifest must be owner-only"
        );
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestCohort {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Agent Drain process fixtures (provider43 drain slice)
// ---------------------------------------------------------------------------

const FIXTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// Single owned monotone token minter — mirrors the Server's token
/// responsibility; the Worker only echoes. All drain calls in one test share
/// this minter, so polling can never rewind a token.
struct TokenMint(u64);
impl TokenMint {
    fn next(&mut self) -> NonZeroU64 {
        self.0 += 1;
        NonZeroU64::new(self.0).expect("token overflow in fixture")
    }
}

/// Reads the Worker's next answer to the Server. The Worker also sends unsolicited notes when a
/// viewer has a record to Poll or terminal titles changed, as a real Host's shell sets its title;
/// these witnesses Poll explicitly and own no titles, so they read past the notes.
fn read_answer(server: &mut UnixStream) -> TestResult<CapabilityNodeMessage> {
    loop {
        match read_message(server)? {
            CapabilityNodeMessage::TerminalViewerRecordReady(_)
            | CapabilityNodeMessage::TerminalTitlesChanged(_) => {}
            message => return Ok(message),
        }
    }
}

fn drain(
    server: &mut UnixStream,
    mint: &mut TokenMint,
    acknowledged: u64,
) -> TestResult<AgentDrainReply> {
    let token = mint.next();
    write_message(
        server,
        &CapabilityNodeMessage::AgentDrainRequest(AgentDrainRequest {
            worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(GENERATION),
            token,
            acknowledged,
        }),
    )?;
    let CapabilityNodeMessage::AgentDrainReply(reply) = read_answer(server)? else {
        return Err(io::Error::other("missing AgentDrainReply").into());
    };
    assert_eq!(reply.token_echo(), token);
    Ok(reply)
}

/// A kind-17 request answered before any Host exists must carry a truthful
/// empty page through the real lazy loop — and must not start the Host.
#[test]
fn lazy_prehost_drain_replies_empty_and_never_starts_host() -> TestResult {
    let fixture = TestCohort::new()?;
    let cohort = Cohort::load(fixture.path(), Instant::now() + FIXTURE_TIMEOUT)?;
    let session_runtime = fixture.path().join("runtime");
    fs::create_dir(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSessionRendezvous::host_runtime_directory(&session_runtime);
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
    let (mut server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(FIXTURE_TIMEOUT))?;
    server.set_write_timeout(Some(FIXTURE_TIMEOUT))?;
    let worker_runtime = session_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(
            worker,
            WORKER_ID,
            GENERATION,
            &cohort,
            &worker_runtime,
            &crate::default_interactive_launch::SystemShell::resolve(),
        )
        .is_err()
    });
    let mut mint = TokenMint(0);
    let reply = drain(&mut server, &mut mint, 0)?;
    assert_eq!(reply.acknowledged_echo(), 0);
    assert_eq!(reply.queue_tail(), 0);
    assert!(reply.entries().is_empty());
    // The witness: the drain read did not start the Host.
    assert!(!host_runtime.exists());
    assert!(!host_endpoint.exists());

    drop(server);
    assert!(
        worker_thread
            .join()
            .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?
    );
    assert!(!host_runtime.exists());
    Ok(())
}

/// A whole request this Worker does not serve, here the Server-wire handoff only Windows serves,
/// gets a decline that echoes it, and the Worker answers the next request instead of ending.
#[test]
fn a_request_this_platform_does_not_serve_is_declined_and_the_worker_keeps_serving() -> TestResult {
    use orchbee_capability_node_protocol::{
        DeclinedRequestKind, WorkerHandoffAction, WorkerHandoffId, WorkerHandoffRequest,
        WorkerHandoffTarget, WorkerRequestDeclined,
    };
    let fixture = TestCohort::new()?;
    let cohort = Cohort::load(fixture.path(), Instant::now() + FIXTURE_TIMEOUT)?;
    let session_runtime = fixture.path().join("runtime");
    fs::create_dir(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let (mut server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(FIXTURE_TIMEOUT))?;
    server.set_write_timeout(Some(FIXTURE_TIMEOUT))?;
    let worker_runtime = session_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(
            worker,
            WORKER_ID,
            GENERATION,
            &cohort,
            &worker_runtime,
            &crate::default_interactive_launch::SystemShell::resolve(),
        )
        .is_err()
    });
    let handoff = WorkerHandoffRequest {
        operation_id: OperationId::from_bytes([31; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        target: WorkerHandoffTarget {
            handoff_id: WorkerHandoffId::from_bytes([32; 16])?,
            cohort_sha256: [33; 32],
        },
        action: WorkerHandoffAction::Prepare,
    };
    write_message(
        &mut server,
        &CapabilityNodeMessage::WorkerHandoffRequest(handoff),
    )?;
    assert_eq!(
        read_message(&mut server)?,
        CapabilityNodeMessage::WorkerRequestDeclined(WorkerRequestDeclined {
            kind: DeclinedRequestKind::Handoff,
            identity: [31; 16],
        })
    );
    let mut mint = TokenMint(0);
    assert!(drain(&mut server, &mut mint, 0)?.entries().is_empty());

    drop(server);
    assert!(
        worker_thread
            .join()
            .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?
    );
    Ok(())
}

/// A record the Worker noted: its event name and fields.
type Noted = (&'static str, Vec<orchbee_local_runtime::LogField>);

/// The lazy loop's thread: whether the loop failed, and the records it noted.
type LazyWorker = thread::JoinHandle<(bool, Vec<Noted>)>;

/// Sends one immediate Launch and requires its definite pre-handoff failure.
fn launch_fails_before_handoff(server: &mut UnixStream, id: u8) -> TestResult {
    use orchbee_capability_node_protocol::{
        AuthorizedTerminalLaunch, LeaseExpiresAtUnixMilliseconds, LeaseId,
        TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId, TerminalLaunchFailure,
        TerminalLaunchMode, TerminalLaunchOutcome, TerminalOperationAuthorization,
        TerminalOperationLease, TerminalPixelGeometry,
    };
    let authorization = TerminalOperationAuthorization::new(
        TerminalId::from_bytes([id; 16])?,
        OperationId::from_bytes([10 + id; 16])?,
        CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
        WorkerGeneration::new(GENERATION),
        TerminalFence::new(NonZeroU64::MIN),
        TerminalOperationLease::new(
            LeaseId::from_bytes([20 + id; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
        ),
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    write_message(
        server,
        &CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
            authorization,
            None,
            TerminalLaunchMode::Immediate(TerminalPixelGeometry::new(100, 30, 8, 16)?),
            None,
        )),
    )?;
    let CapabilityNodeMessage::TerminalLaunchResult(result) = read_answer(server)? else {
        return Err(io::Error::other("missing Launch result").into());
    };
    assert_eq!(
        result.outcome(),
        TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
    );
    Ok(())
}

/// Runs the lazy loop over a fixture cohort, whose Host artifact is not a program, and returns
/// the Server end, the Session runtime folder, and the thread that yields whether the loop
/// failed together with its noted records.
fn lazy_worker_over_fixture() -> TestResult<(UnixStream, TestCohort, PathBuf, LazyWorker)> {
    let fixture = TestCohort::new()?;
    let cohort = Cohort::load(fixture.path(), Instant::now() + FIXTURE_TIMEOUT)?;
    let session_runtime = fixture.path().join("runtime");
    fs::create_dir(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let (server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(FIXTURE_TIMEOUT))?;
    server.set_write_timeout(Some(FIXTURE_TIMEOUT))?;
    let worker_runtime = session_runtime.clone();
    let worker_thread = thread::spawn(move || {
        let ended = run(
            worker,
            WORKER_ID,
            GENERATION,
            &cohort,
            &worker_runtime,
            &crate::default_interactive_launch::SystemShell::resolve(),
        )
        .is_err();
        (ended, crate::diagnostic_events::take_noted())
    });
    Ok((server, fixture, session_runtime, worker_thread))
}

/// The `stage` of every `host-start-failed` record.
fn failed_start_stages(noted: &[Noted]) -> Vec<orchbee_local_runtime::LogValue> {
    noted
        .iter()
        .filter(|(event, _)| *event == "host-start-failed")
        .filter_map(|(_, fields)| {
            fields
                .iter()
                .find_map(|(name, value)| (*name == "stage").then_some(*value))
        })
        .collect()
}

/// Requires the Worker to have closed its end of the Server connection on its own.
fn require_worker_ended(server: &mut UnixStream) -> TestResult {
    let mut byte = [0_u8; 1];
    assert_eq!(server.read(&mut byte)?, 0, "the Worker kept its connection");
    Ok(())
}

/// A start registers its Host first, and neither a failed registration nor a registered Host that
/// exited leaves a further start: the fixture keeps no Session record, so its first start fails at
/// the registration, which is recorded with its stage, its Launch is answered, and the Worker ends,
/// so the Server stops the Session.
#[test]
fn a_host_start_that_fails_its_registration_ends_the_worker_after_its_answer() -> TestResult {
    use orchbee_local_runtime::LogValue;
    let (mut server, _fixture, _runtime, worker_thread) = lazy_worker_over_fixture()?;
    launch_fails_before_handoff(&mut server, 1)?;
    require_worker_ended(&mut server)?;
    let (ended, noted) = worker_thread
        .join()
        .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?;
    assert!(ended);
    assert_eq!(
        failed_start_stages(&noted),
        [LogValue::Token("registration")],
        "{noted:?}"
    );
    Ok(())
}

/// An endpoint already at the Host's address, such as one a Host left when it ended, would fail
/// every later start too: the Launch is answered, then the Worker ends at once.
#[test]
fn an_endpoint_present_before_the_start_ends_the_worker_after_its_answer() -> TestResult {
    use orchbee_local_runtime::LogValue;
    let (mut server, _fixture, session_runtime, worker_thread) = lazy_worker_over_fixture()?;
    let endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
    let socket_root = ModeCheckedLocalSessionRendezvous::socket_root_path();
    if !socket_root.exists() {
        fs::create_dir_all(&socket_root)?;
        fs::set_permissions(&socket_root, fs::Permissions::from_mode(0o700))?;
    }
    // A listener dropped without removing its path leaves the socket a Host that died leaves.
    drop(std::os::unix::net::UnixListener::bind(&endpoint)?);
    let outcome = (|| -> TestResult {
        launch_fails_before_handoff(&mut server, 1)?;
        require_worker_ended(&mut server)?;
        Ok(())
    })();
    let _ = fs::remove_file(&endpoint);
    outcome?;
    let (ended, noted) = worker_thread
        .join()
        .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?;
    assert!(ended);
    assert_eq!(
        failed_start_stages(&noted),
        [LogValue::Token("endpoint")],
        "{noted:?}"
    );
    Ok(())
}

/// A Server close while a drain request is in flight ends the connection
/// epoch: `run` returns an error. This is honest scope — drop/shutdown races
/// can legitimately observe the full reply first, so it is not a
/// partial-write proof; write-error semantics stay with the queue/send unit
/// tests.
#[test]
fn server_close_mid_drain_ends_the_epoch() -> TestResult {
    let fixture = TestCohort::new()?;
    let cohort = Cohort::load(fixture.path(), Instant::now() + FIXTURE_TIMEOUT)?;
    let session_runtime = fixture.path().join("runtime");
    fs::create_dir(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let (mut server, worker) = UnixStream::pair()?;
    let worker_runtime = session_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(
            worker,
            WORKER_ID,
            GENERATION,
            &cohort,
            &worker_runtime,
            &crate::default_interactive_launch::SystemShell::resolve(),
        )
        .is_err()
    });

    let mut mint = TokenMint(0);
    write_message(
        &mut server,
        &CapabilityNodeMessage::AgentDrainRequest(AgentDrainRequest {
            worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(GENERATION),
            token: mint.next(),
            acknowledged: 0,
        }),
    )?;
    drop(server); // close before or while the reply is written
    assert!(
        worker_thread
            .join()
            .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?
    );
    Ok(())
}

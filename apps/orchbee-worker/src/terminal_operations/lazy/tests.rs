#[cfg(target_os = "linux")]
use std::process::Command;
use std::{
    fs, io,
    num::NonZeroU64,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, AgentDrainReply, AgentDrainRequest, AuthorizedTerminalLaunch,
    CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalCwdValidationOutcome, TerminalCwdValidationRequest,
    TerminalFence, TerminalId, TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalOperationAuthorization, TerminalOperationLease, WorkerGeneration,
    WorkerId as CapabilityWorkerId, WorktreeJobAction, WorktreeJobOutcome, WorktreeJobPollOutcome,
    WorktreeJobRequest, WorktreeJobStartOutcome, read_message, write_message,
};
#[cfg(target_os = "linux")]
use orchbee_capability_node_protocol::{
    AgentDrainEntry, SendRecord, SendRecords, SendText, SendTextInput, TerminalPixelGeometry,
    TerminalSendOutcome, TerminalSendRequest,
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

#[test]
fn overlong_host_endpoint_fails_before_host_directory_or_spawn() -> TestResult {
    let fixture = TestCohort::new()?;
    let cohort = Cohort::load(fixture.path(), Instant::now() + Duration::from_secs(2))?;
    // Pad the Session runtime until the derived `host/<id>.s` endpoint cannot fit
    // one socket address, while the Session directory itself is still creatable.
    let mut session_runtime = fixture.path().join("runtime");
    while nix::sys::socket::UnixAddr::new(&ModeCheckedLocalSessionRendezvous::host_endpoint_path(
        &session_runtime,
    ))
    .is_ok()
    {
        session_runtime = session_runtime.join("x");
    }
    fs::create_dir_all(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSessionRendezvous::host_runtime_directory(&session_runtime);
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

    // A fully valid authorized Launch must still be refused by the endpoint
    // budget — before the Host directory exists and before any Host spawn.
    let authorization = TerminalOperationAuthorization::new(
        TerminalId::from_bytes([1; 16])?,
        OperationId::from_bytes([2; 16])?,
        CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
        WorkerGeneration::new(GENERATION),
        TerminalFence::new(NonZeroU64::MIN),
        TerminalOperationLease::new(
            LeaseId::from_bytes([3; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
        ),
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    write_message(
        &mut server,
        &CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
            authorization,
            None,
            TerminalLaunchMode::Deferred,
            None,
        )),
    )?;
    let CapabilityNodeMessage::TerminalLaunchResult(result) = read_message(&mut server)? else {
        return Err(io::Error::other("Worker returned the wrong launch message").into());
    };
    assert_eq!(
        result.outcome(),
        TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
    );
    drop(server);
    assert!(
        worker_thread
            .join()
            .map_err(|_panic| io::Error::other("lazy Worker test thread panicked"))?
    );
    assert!(!host_runtime.exists());
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

/// Canonical, Server-parseable v3 correlation hint — the payload is genuinely
/// consumable by the future Server hint path; the Worker transports it as
/// opaque UTF-8 with no schema parse.
#[cfg(target_os = "linux")]
const HINT_JSON: &str = concat!(
    r#"{"kind":"correlation","version":1,"#,
    r#""correlationId":"fixture-correlation-0001","#,
    r#""eventId":"fixture-event-00000001"}"#
);

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
    let CapabilityNodeMessage::AgentDrainReply(reply) = read_message(server)? else {
        return Err(io::Error::other("missing AgentDrainReply").into());
    };
    assert_eq!(reply.token_echo(), token);
    Ok(reply)
}

/// Poll the drain until `ready` or the absolute deadline. Each poll consumes
/// a fresh monotone token from the shared mint — never rewound.
#[cfg(target_os = "linux")]
fn drain_until(
    server: &mut UnixStream,
    mint: &mut TokenMint,
    acknowledged: u64,
    deadline: Instant,
    ready: impl Fn(&AgentDrainReply) -> bool,
) -> TestResult<AgentDrainReply> {
    loop {
        let reply = drain(server, mint, acknowledged)?;
        if ready(&reply) {
            return Ok(reply);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other("drained signal did not arrive").into());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// Combined fixture-private lifecycle owner (Root27 / Recovery38 shape):
/// owns the Server-side socket, the worker `JoinHandle`, the cohort dir,
/// and the pidfd process guard so teardown order is deterministic on the
/// Ok, `?`, and panic exits alike. `settle` performs server close ->
/// bounded actual worker completion (a detached `JoinHandle` is not
/// settled) -> pidfd-pinned scan + reclaim of ONLY this fixture's Host
/// (exe link AND exact argv equality, revalidated after the pin) ->
/// directory removal ONLY on a fully clean verdict. Any unresolved state
/// is an explicit `Incomplete`: the fixture dir is retained with its path
/// printed, never removed under an ongoing worker, never reported clean.
/// `finish()` is the load-bearing success-path assertion; `Drop` is the
/// honest best-effort fallback that reports every Incomplete.
#[cfg(target_os = "linux")]
mod fixture_lifecycle {
    use std::{
        fs, io,
        io::Write as _,
        mem,
        os::{
            fd::{AsFd, OwnedFd},
            unix::ffi::OsStrExt,
            unix::net::UnixStream,
        },
        path::{Path, PathBuf},
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
    use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
    use rustix::process::{Pid as RustixPid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

    use super::TestCohort;

    /// Bounded worker-settle wait after server close; signal grace;
    /// overall scan/reclaim budget. Finite by construction — every wait
    /// has a deadline.
    const SETTLE_BUDGET: Duration = Duration::from_secs(3);
    const TERM_GRACE: Duration = Duration::from_secs(2);
    const RECLAIM_DEADLINE: Duration = Duration::from_secs(5);

    /// The three-state settle verdict. `Incomplete` is explicit and
    /// always carries which bound was hit; it never collapses into Clean.
    #[derive(Debug)]
    enum SettleOutcome {
        Clean,
        Incomplete(&'static str),
    }

    /// One pidfd-pinned process. The fd makes the pin immune to PID
    /// reuse: signalling through it can only reach the verified instance.
    struct PinnedProcess {
        pid: i32,
        pidfd: OwnedFd,
    }

    impl PinnedProcess {
        fn signal(&self, signal: Signal) -> io::Result<()> {
            match pidfd_send_signal(&self.pidfd, signal) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
                Err(error) => Err(error.into()),
            }
        }

        fn exited(&self) -> io::Result<bool> {
            let mut descriptor = [PollFd::new(self.pidfd.as_fd(), PollFlags::POLLIN)];
            poll(&mut descriptor, PollTimeout::ZERO).map_err(io::Error::from)?;
            let events = descriptor[0].revents().unwrap_or(PollFlags::POLLNVAL);
            if events.contains(PollFlags::POLLNVAL) {
                return Err(io::Error::other("pidfd became invalid"));
            }
            Ok(events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR))
        }

        fn wait_exit_before(&self, deadline: Instant) -> io::Result<bool> {
            while !self.exited()? {
                if Instant::now() >= deadline {
                    return Ok(false);
                }
                thread::sleep(Duration::from_millis(10));
            }
            Ok(true)
        }
    }

    enum Verification {
        Verified(PinnedProcess),
        Gone,
        /// Alive but identity/proc fields unsettled — never signalled
        /// and never counted as clean; retried inside the reclaim budget.
        Unsettled,
    }

    /// Scan result that PRESERVES uncertainty: a nonzero `unsettled`
    /// means the verdict is unknown, not clean.
    struct Scan {
        verified: Vec<PinnedProcess>,
        unsettled: usize,
    }

    /// Fixture-private reclaim target: exactly this cohort's Host argv
    /// shape — `<exe> serve --runtime-directory <host runtime>
    /// --endpoint-id <id> --endpoint-directory <socket folder>` as
    /// `start_host` spawns it.
    struct HostReclaim {
        executable: PathBuf,
        host_runtime: PathBuf,
    }

    impl HostReclaim {
        fn expected_argv(&self) -> Vec<Vec<u8>> {
            let session_runtime = self.host_runtime.parent().unwrap_or(&self.host_runtime);
            let endpoint_id = ModeCheckedLocalSessionRendezvous::host_endpoint_id(session_runtime);
            let socket_root = ModeCheckedLocalSessionRendezvous::socket_root_path();
            [
                self.executable.as_os_str(),
                std::ffi::OsStr::new("serve"),
                std::ffi::OsStr::new("--runtime-directory"),
                self.host_runtime.as_os_str(),
                std::ffi::OsStr::new("--endpoint-id"),
                std::ffi::OsStr::new(&endpoint_id),
                std::ffi::OsStr::new("--endpoint-directory"),
                socket_root.as_os_str(),
            ]
            .iter()
            .map(|value| value.as_bytes().to_vec())
            .collect()
        }

        /// /proc scan preserving Unsettled.
        fn scan(&self) -> io::Result<Scan> {
            let expected = self.expected_argv();
            let mut scan = Scan {
                verified: Vec::new(),
                unsettled: 0,
            };
            for entry in fs::read_dir("/proc")? {
                let entry = entry?;
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse::<i32>().ok())
                else {
                    continue;
                };
                let argv0 = fs::read(entry.path().join("cmdline"))
                    .ok()
                    .and_then(|raw| raw.split(|b| *b == 0).next().map(|a| a.to_vec()));
                let exe_matches = fs::read_link(entry.path().join("exe"))
                    .ok()
                    .is_some_and(|path| exe_link_matches(&path, &self.executable));
                if argv0.as_deref() != Some(self.executable.as_os_str().as_bytes()) && !exe_matches
                {
                    continue;
                }
                match open_verified_process(pid, &self.executable, &expected)? {
                    Verification::Verified(process) => scan.verified.push(process),
                    Verification::Gone => {}
                    Verification::Unsettled => scan.unsettled += 1,
                }
            }
            Ok(scan)
        }

        /// Pin children of a verified Host with POST-PIN revalidation:
        /// capture ppid+starttime before `pidfd_open`; after the pin,
        /// re-read both and require the pinned parent still live — a
        /// recycled pid or reparented child is excluded, never signalled.
        fn pin_children_of(&self, host: &PinnedProcess) -> io::Result<Vec<PinnedProcess>> {
            let mut children = Vec::new();
            for entry in fs::read_dir("/proc")? {
                let entry = entry?;
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|n| n.parse::<i32>().ok())
                else {
                    continue;
                };
                if pid == host.pid {
                    continue;
                }
                let Ok(pre_stat) = fs::read(entry.path().join("stat")) else {
                    continue;
                };
                let (Some(pre_ppid), Some(pre_start)) =
                    (stat_ppid(&pre_stat), stat_starttime(&pre_stat))
                else {
                    continue;
                };
                if pre_ppid != host.pid {
                    continue;
                }
                let Verification::Verified(child) = pin_process(pid)? else {
                    continue;
                };
                let revalidated = fs::read(format!("/proc/{pid}/stat"))
                    .ok()
                    .is_some_and(|post| {
                        stat_ppid(&post) == Some(host.pid)
                            && stat_starttime(&post) == Some(pre_start)
                    })
                    && matches!(host.exited(), Ok(false));
                if revalidated {
                    children.push(child);
                }
            }
            Ok(children)
        }

        /// Bounded reclaim. `Clean` only when a final scan finds nothing
        /// verified AND nothing unsettled.
        fn reclaim(&self) -> io::Result<SettleOutcome> {
            let deadline = Instant::now() + RECLAIM_DEADLINE;
            let mut clean = true;
            for host in self.scan()?.verified {
                let children = self.pin_children_of(&host)?;
                host.signal(Signal::TERM)?;
                let graceful = Instant::now()
                    + TERM_GRACE.min(deadline.saturating_duration_since(Instant::now()));
                if !host.wait_exit_before(graceful)? {
                    host.signal(Signal::KILL)?;
                }
                if !host.wait_exit_before(deadline)? {
                    clean = false;
                }
                for child in children {
                    let child_grace = Instant::now()
                        + TERM_GRACE.min(deadline.saturating_duration_since(Instant::now()));
                    // Host death closes the pty master -> SIGHUP; only a
                    // lingerer is signalled — through its revalidated pin.
                    if !child.wait_exit_before(child_grace)? {
                        child.signal(Signal::TERM)?;
                        if !child.wait_exit_before(deadline)? {
                            child.signal(Signal::KILL)?;
                            if !child.wait_exit_before(deadline)? {
                                clean = false;
                            }
                        }
                    }
                }
            }
            // Retry-until-deadline on residual candidates; an unresolved
            // candidate at the deadline is Incomplete, not clean.
            loop {
                let scan = self.scan()?;
                if scan.verified.is_empty() && scan.unsettled == 0 {
                    break;
                }
                if Instant::now() >= deadline {
                    return Ok(SettleOutcome::Incomplete(
                        "host scan still verified/unsettled at reclaim deadline",
                    ));
                }
                thread::sleep(Duration::from_millis(25));
            }
            Ok(if clean {
                SettleOutcome::Clean
            } else {
                SettleOutcome::Incomplete("signalled host/child did not exit in budget")
            })
        }
    }

    pub struct FixtureLifecycle {
        server: Option<UnixStream>,
        worker: Option<JoinHandle<bool>>,
        cohort: Option<TestCohort>,
        reclaim: HostReclaim,
        settled: bool,
    }

    impl FixtureLifecycle {
        pub fn new(
            cohort: TestCohort,
            server: UnixStream,
            worker: JoinHandle<bool>,
            executable: PathBuf,
            host_runtime: PathBuf,
        ) -> Self {
            Self {
                server: Some(server),
                worker: Some(worker),
                cohort: Some(cohort),
                reclaim: HostReclaim {
                    executable,
                    host_runtime,
                },
                settled: false,
            }
        }

        /// Test-body wire access; the socket stays owned so close order
        /// is guaranteed regardless of which path the test takes.
        pub fn server(&mut self) -> &mut UnixStream {
            self.server.as_mut().expect("fixture server already closed")
        }

        /// Deterministic, idempotent teardown:
        ///   1. close the server socket (worker run() sees EOF)
        ///   2. bounded wait for actual worker completion (`is_finished`
        ///      + `join` — `JoinHandle::drop` alone only detaches)
        ///   3. only when settled: pidfd scan + reclaim the fixture Host
        ///   4. remove the cohort dirs ONLY on Clean; any Incomplete
        ///      retains the dir (mem::forget) with its path printed —
        ///      never rm under an ongoing worker.
        fn settle(&mut self) -> SettleOutcome {
            if self.settled {
                return SettleOutcome::Clean;
            }
            drop(self.server.take());
            let worker_settled = match self.worker.take() {
                Some(handle) => {
                    let deadline = Instant::now() + SETTLE_BUDGET;
                    while !handle.is_finished() && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(10));
                    }
                    if handle.is_finished() {
                        // join() returns the worker run() verdict —
                        // settling needs completion, not the result.
                        let _run_result = handle.join();
                        true
                    } else {
                        false
                    }
                }
                None => true,
            };
            if !worker_settled {
                self.retain();
                self.settled = true;
                return SettleOutcome::Incomplete("worker thread did not settle in budget");
            }
            let outcome = match self.reclaim.reclaim() {
                Ok(outcome) => outcome,
                Err(error) => {
                    let _ignored =
                        writeln!(io::stderr(), "FixtureLifecycle: reclaim error: {error}");
                    SettleOutcome::Incomplete("reclaim io error")
                }
            };
            if matches!(outcome, SettleOutcome::Incomplete(_)) {
                self.retain();
            }
            self.settled = true;
            outcome
        }

        /// Keep the cohort dirs on disk for later attributed cleanup
        /// instead of removing them under an unresolved process state.
        fn retain(&mut self) {
            if let Some(cohort) = self.cohort.take() {
                let _ignored = writeln!(
                    io::stderr(),
                    "FixtureLifecycle: incomplete teardown — retaining fixture dir {}",
                    cohort.path().display()
                );
                mem::forget(cohort);
            }
        }

        /// Explicit success-path finish: incomplete teardown FAILS the
        /// test — a leaked or unresolved fixture never passes as clean.
        pub fn finish(mut self) -> io::Result<()> {
            match self.settle() {
                SettleOutcome::Clean => Ok(()),
                SettleOutcome::Incomplete(reason) => Err(io::Error::other(format!(
                    "fixture teardown incomplete: {reason}"
                ))),
            }
        }

        /// Test-owned pidfd pin of this fixture's currently-running Host
        /// — the same exe+argv verification the reclaim uses, but the
        /// returned fd NEVER signals; after settle it must report exited,
        /// pinning THE initial instance so pid reuse cannot fake it.
        pub fn pin_fixture_host(&self) -> io::Result<Option<OwnedFd>> {
            let mut scan = self.reclaim.scan()?;
            Ok(scan.verified.pop().map(|process| process.pidfd))
        }
    }

    impl Drop for FixtureLifecycle {
        fn drop(&mut self) {
            // Best-effort fallback for error/panic exits — reports both
            // the io-error and Incomplete verdicts, never panics (a
            // panicking Drop during unwind would abort the test binary).
            match self.settle() {
                SettleOutcome::Clean => {}
                SettleOutcome::Incomplete(reason) => {
                    let _ignored = writeln!(
                        io::stderr(),
                        "FixtureLifecycle: best-effort teardown incomplete: {reason}"
                    );
                }
            }
        }
    }

    /// pidfd-pin a pid, then confirm it is still alive — the second check
    /// strictly after the pin.
    fn pin_process(raw_pid: i32) -> io::Result<Verification> {
        let Some(rustix_pid) = RustixPid::from_raw(raw_pid) else {
            return Ok(Verification::Gone);
        };
        let pidfd = match pidfd_open(rustix_pid, PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            Err(rustix::io::Errno::SRCH) => return Ok(Verification::Gone),
            Err(error) => return Err(error.into()),
        };
        let process = PinnedProcess {
            pid: raw_pid,
            pidfd,
        };
        if process.exited()? {
            return Ok(Verification::Gone);
        }
        Ok(Verification::Verified(process))
    }

    /// Pin + full identity verification: exe link AND complete argv
    /// equality, all strictly after the pin.
    fn open_verified_process(
        raw_pid: i32,
        executable: &Path,
        expected_argv: &[Vec<u8>],
    ) -> io::Result<Verification> {
        let process = match pin_process(raw_pid)? {
            Verification::Verified(process) => process,
            outcome => return Ok(outcome),
        };
        let proc_dir = PathBuf::from(format!("/proc/{raw_pid}"));
        let observed_exe = match fs::read_link(proc_dir.join("exe")) {
            Ok(path) => Some(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let observed_cmdline = match fs::read(proc_dir.join("cmdline")) {
            Ok(raw) => Some(raw),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let (Some(exe), Some(cmdline)) = (observed_exe, observed_cmdline) else {
            return if process.exited()? {
                Ok(Verification::Gone)
            } else {
                Ok(Verification::Unsettled)
            };
        };
        let exe_ok = exe_link_matches(&exe, executable);
        let argv_ok = cmdline_matches(&cmdline, expected_argv);
        if exe_ok && argv_ok {
            Ok(Verification::Verified(process))
        } else if process.exited()? {
            Ok(Verification::Gone)
        } else {
            // Alive but NOT ours — never signal it, never count settled.
            Ok(Verification::Unsettled)
        }
    }

    /// Full argv equality: one trailing NUL stripped, then exact segment
    /// match — no prefix/substring acceptance.
    fn cmdline_matches(cmdline: &[u8], expected_argv: &[Vec<u8>]) -> bool {
        cmdline.strip_suffix(&[0]).is_some_and(|body| {
            body.split(|b| *b == 0)
                .eq(expected_argv.iter().map(Vec::as_slice))
        })
    }

    /// Pure argv check for the negative control: would this cmdline be
    /// claimed as THIS fixture's Host? Never signals, never scans.
    pub fn argv_is_fixture_host(cmdline: &[u8], executable: &Path, host_runtime: &Path) -> bool {
        let reclaim = HostReclaim {
            executable: executable.to_path_buf(),
            host_runtime: host_runtime.to_path_buf(),
        };
        cmdline_matches(cmdline, &reclaim.expected_argv())
    }

    /// Bounded pidfd exit proof for a test-owned pin: the pinned
    /// instance itself must die — pid reuse cannot fake an exited pin.
    pub fn pin_exited(pin: &OwnedFd, deadline: Instant) -> bool {
        loop {
            let mut descriptor = [PollFd::new(pin.as_fd(), PollFlags::POLLIN)];
            if poll(&mut descriptor, PollTimeout::ZERO).is_ok()
                && descriptor[0]
                    .revents()
                    .unwrap_or(PollFlags::POLLNVAL)
                    .intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR)
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// exe symlink equality tolerating the ` (deleted)` suffix /proc
    /// adds after the cohort tree is removed.
    fn exe_link_matches(observed: &Path, expected: &Path) -> bool {
        if observed == expected {
            return true;
        }
        observed
            .as_os_str()
            .as_bytes()
            .strip_suffix(b" (deleted)")
            .is_some_and(|stripped| stripped == expected.as_os_str().as_bytes())
    }

    /// ppid = /proc/<pid>/stat field 4 (index 1 after comm-in-parens).
    fn stat_ppid(stat: &[u8]) -> Option<i32> {
        let close = stat.iter().rposition(|b| *b == b')')?;
        std::str::from_utf8(stat.get(close + 2..)?)
            .ok()?
            .split(' ')
            .nth(1)?
            .parse()
            .ok()
    }

    /// starttime = /proc/<pid>/stat field 22 (index 19 after comm).
    fn stat_starttime(stat: &[u8]) -> Option<u64> {
        let close = stat.iter().rposition(|b| *b == b')')?;
        std::str::from_utf8(stat.get(close + 2..)?)
            .ok()?
            .split(' ')
            .nth(19)?
            .parse()
            .ok()
    }
}

#[cfg(target_os = "linux")]
use fixture_lifecycle::FixtureLifecycle;
#[cfg(target_os = "linux")]
use std::os::fd::OwnedFd;

/// Guarded real-Host setup: cohort, session runtime, socket pair, and the
/// worker thread all move into one `FixtureLifecycle` owner. Returns the
/// owner plus the paths the caller needs for readiness assertions — the
/// cohort root is captured BEFORE the dir owner moves, so post-drop
/// cleanup can be observed. A missing real zig Host artifact is a hard
/// failure here (this leg is a requested real-process witness).
#[cfg(target_os = "linux")]
fn guarded_real_host() -> TestResult<(FixtureLifecycle, PathBuf, PathBuf)> {
    let Some((cohort_dir, cohort)) = TestCohort::with_real_host()? else {
        return Err(io::Error::other(
            "required witness artifact absent: zig Host binary \
             (build: pnpm --filter @orchbee/terminal-host build)",
        )
        .into());
    };
    let cohort_root = cohort_dir.path().to_path_buf();
    let session_runtime = cohort_root.join("runtime");
    fs::create_dir(&session_runtime)?;
    fs::set_permissions(&session_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSessionRendezvous::host_runtime_directory(&session_runtime);
    let executable = cohort_root.join(staged_name("orchbee-terminal-host"));
    let (server, worker) = UnixStream::pair()?;
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
    let lifecycle =
        FixtureLifecycle::new(cohort_dir, server, worker_thread, executable, host_runtime);
    Ok((lifecycle, session_runtime, cohort_root))
}

/// A real `AuthorizedTerminalLaunch` in Immediate mode; on HostAccepted the
/// spawned Host's CNP identity is returned for follow-up sends.
#[cfg(target_os = "linux")]
fn launch_real_terminal(
    server: &mut UnixStream,
) -> TestResult<orchbee_capability_node_protocol::HostTerminalIdentity> {
    let authorization = TerminalOperationAuthorization::new(
        TerminalId::from_bytes([1; 16])?,
        OperationId::from_bytes([2; 16])?,
        CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
        WorkerGeneration::new(GENERATION),
        TerminalFence::new(NonZeroU64::MIN),
        TerminalOperationLease::new(
            LeaseId::from_bytes([3; 16])?,
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
    let CapabilityNodeMessage::TerminalLaunchResult(result) = read_message(server)? else {
        return Err(io::Error::other("missing Launch result").into());
    };
    let TerminalLaunchOutcome::HostAccepted(created) = result.outcome() else {
        return Err(io::Error::other(format!("Host did not accept launch: {result:?}")).into());
    };
    Ok(created.identity())
}

/// Owned-child READINESS: the real Host's endpoint socket exists — the
/// process is up and bound, not merely spawned. Bounded wait.
#[cfg(target_os = "linux")]
fn wait_endpoint(endpoint: &Path) -> TestResult {
    let deadline = Instant::now() + FIXTURE_TIMEOUT;
    while !endpoint.exists() {
        if Instant::now() >= deadline {
            return Err(io::Error::other("real Host endpoint never appeared").into());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}

/// The full F2 wire sequence through program-under-test readiness:
/// pre-Host drain -> Immediate launch -> endpoint bound -> bounded APC
/// resend -> drained EVENT asserted -> stale-ack + ack-trim. This is the
/// strongest owned-child readiness: the spawned shell actually executed
/// printf and the real VT emitted the signal. Returns the Host's CNP
/// identity once readiness is proven.
#[cfg(target_os = "linux")]
fn prove_readiness(
    server: &mut UnixStream,
    mint: &mut TokenMint,
    host_endpoint: &Path,
    deadline: Instant,
) -> TestResult<orchbee_capability_node_protocol::HostTerminalIdentity> {
    // The same owner answers empty pre-Host.
    let reply = drain(server, mint, 0)?;
    assert!(reply.queue_tail() == 0 && reply.entries().is_empty());
    assert!(!host_endpoint.exists());

    // A real launch in Immediate mode spawns the real Host.
    let identity = launch_real_terminal(server)?;
    // Proof the real Host actually spawned, distinguishing PASS from NOTRUN.
    assert!(host_endpoint.exists(), "real Host endpoint must appear");

    // The interactive child starts asynchronously; an input record admitted
    // before its first read can be lost to pty/line-discipline setup. A
    // bounded re-send of the same APC is honest here: the witness contract is
    // that a real APC through the real VT pipeline reaches the queue, not
    // which admission produced it. Every send must still be Accepted.
    let mut reply = None;
    let mut attempt = 0u8;
    while Instant::now() < deadline {
        attempt += 1;
        let result = send_apc(server, identity, attempt)?;
        assert!(
            matches!(result.outcome(), TerminalSendOutcome::Accepted { .. }),
            "the APC input must be accepted by the real Host: {:?}",
            result.outcome()
        );
        let window = (Instant::now() + Duration::from_secs(4)).min(deadline);
        match drain_until(server, mint, 0, window, |r| !r.entries().is_empty()) {
            Ok(found) => {
                reply = Some(found);
                break;
            }
            Err(_) if Instant::now() < deadline => continue,
            Err(error) => return Err(error),
        }
    }
    let Some(reply) = reply else {
        return Err(io::Error::other("drained signal did not arrive").into());
    };
    assert_eq!(reply.queue_tail(), 1);
    assert_eq!(reply.entries().len(), 1);
    let AgentDrainEntry::Event {
        sequence,
        identity: seen,
        payload,
    } = &reply.entries()[0]
    else {
        return Err(io::Error::other("expected an EVENT entry").into());
    };
    assert_eq!(sequence.get(), 1); // queue-minted, not signal.sequence
    assert_eq!(seen, &identity); // full CNP identity, host_epoch included
    assert_eq!(payload.as_str(), HINT_JSON); // canonical hint, unchanged bytes

    // A stale ACK is a trim no-op: the offered-but-unconfirmed sequence 1 is
    // re-served, never suppressed or dropped.
    let stale = drain(server, mint, 0)?;
    assert_eq!(stale.queue_tail(), 1);
    assert_eq!(
        stale.entries().len(),
        1,
        "unconfirmed offered entry must redeliver"
    );
    assert!(
        matches!(
            stale.entries().first(),
            Some(AgentDrainEntry::Event { sequence, .. }) if sequence.get() == 1
        ) || matches!(
            stale.entries().first(),
            Some(AgentDrainEntry::Gap { low, high }) if low.get() == 1 && *high == 1
        )
    );

    // Real confirmation: ack=1 trims coverage; the same reply is empty.
    let confirmed = drain(server, mint, 1)?;
    assert_eq!(confirmed.acknowledged_echo(), 1);
    assert_eq!(confirmed.queue_tail(), 1); // tail = minted_hi, stays 1
    assert!(confirmed.entries().is_empty());
    Ok(identity)
}

/// Load-bearing teardown proof, run strictly AFTER the lifecycle owner
/// dropped/finished: the pinned INITIAL Host instance exited (pid reuse
/// cannot fake an exited pidfd), and a Clean settle removed the cohort
/// dirs. Called only after `finish()`/`drop(lifecycle)` — never inside.
#[cfg(target_os = "linux")]
fn assert_clean_teardown(host_pin: Option<OwnedFd>, cohort_root: &Path) {
    if let Some(pin) = host_pin {
        assert!(
            fixture_lifecycle::pin_exited(&pin, Instant::now() + Duration::from_secs(5)),
            "the pinned fixture Host did not exit"
        );
    }
    assert!(
        !cohort_root.exists(),
        "cohort dir must be removed on clean teardown: {}",
        cohort_root.display()
    );
}

/// One APC correlation signal delivered through a real Attach → input →
/// Detach exchange. `attempt` varies the request and attachment identities so
/// a re-send can never collide with an in-flight exchange.
#[cfg(target_os = "linux")]
fn send_apc(
    server: &mut UnixStream,
    target: orchbee_capability_node_protocol::HostTerminalIdentity,
    attempt: u8,
) -> TestResult<orchbee_capability_node_protocol::TerminalSendResult> {
    let apc = format!(
        "printf '\\033_orchbee.agent.v1;{}\\033\\\\'\n",
        base64_url_nopad(HINT_JSON.as_bytes()),
    );
    // The wire payload must be pure ASCII shell text: `\033` is four ASCII
    // bytes for the spawned shell's printf to interpret, never a Rust NUL
    // (`\0`) or a raw ESC byte pushed into the interactive line editor.
    assert!(apc.is_ascii() && !apc.bytes().any(|byte| byte < 0x20 && byte != b'\n'));
    assert!(
        apc.starts_with("printf '\\033_orchbee.agent.v1;") && apc.ends_with("\\033\\\\'\n"),
        "APC shell command must carry ASCII printf escapes, got {apc:?}"
    );
    write_message(
        server,
        &CapabilityNodeMessage::TerminalSendRequest(TerminalSendRequest {
            request_id: OperationId::from_bytes([attempt; 16])?,
            worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(GENERATION),
            target,
            attachment_id: OperationId::from_bytes([attempt.wrapping_add(100); 16])?,
            precondition: orchbee_capability_node_protocol::SendPrecondition::None,
            records: SendRecords::new(vec![SendRecord::Text(SendTextInput::new(
                SendText::new(apc)?,
                true,
            )?)])?,
        }),
    )?;
    let CapabilityNodeMessage::TerminalSendResult(result) = read_message(server)? else {
        return Err(io::Error::other("missing Send result").into());
    };
    Ok(result)
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

/// Pre-Host drain, a real first launch, a real Host APC signal, and the same
/// drain owner — every assertion runs only with the real zig Host binary.
/// This is a requested real-process witness: a missing artifact is a failure,
/// never a silent green early return. The `FixtureLifecycle` owner settles
/// server-close -> worker-join -> pidfd reclaim on every exit; `finish()`
/// is load-bearing so an incomplete teardown fails the test.
#[cfg(target_os = "linux")]
#[test]
fn lazy_drain_survives_first_launch_and_captures_real_agent_signal() -> TestResult {
    let (mut lifecycle, session_runtime, cohort_root) = guarded_real_host()?;
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
    let deadline = Instant::now() + FIXTURE_TIMEOUT;

    let mut mint = TokenMint(0);
    let _identity = prove_readiness(lifecycle.server(), &mut mint, &host_endpoint, deadline)?;

    // Pin THE initial Host instance before teardown — its pidfd exit is the
    // load-bearing proof; a reused numeric pid cannot fake an exited pin.
    let pin = lifecycle
        .pin_fixture_host()?
        .expect("the spawned fixture Host must pin");
    // Success path: finish() performs server close -> worker join -> pidfd
    // reclaim -> dir removal; an Incomplete verdict fails the test here.
    lifecycle.finish()?;
    assert_clean_teardown(Some(pin), &cohort_root);
    Ok(())
}

/// REGRESSION I0 — error BEFORE any spawn: one pre-Host drain exchange ran,
/// no Host exists (`pin_fixture_host` is None), then the fixture is
/// abandoned. Clean teardown still means worker finished + dirs removed.
#[cfg(target_os = "linux")]
#[test]
fn fixture_lifecycle_removes_dirs_on_pre_spawn_error() -> TestResult {
    let (mut lifecycle, session_runtime, cohort_root) = guarded_real_host()?;
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
    let abandoned = (|| -> TestResult {
        let mut mint = TokenMint(0);
        let reply = drain(lifecycle.server(), &mut mint, 0)?;
        assert!(reply.queue_tail() == 0 && reply.entries().is_empty());
        assert!(!host_endpoint.exists());
        assert!(
            lifecycle.pin_fixture_host()?.is_none(),
            "no Host may be pinned before the first launch"
        );
        Err(io::Error::other("injected pre-spawn failure").into())
    })();
    assert!(abandoned.is_err(), "the injected error must surface");
    drop(lifecycle); // the error-path Drop settle, observable afterwards
    assert_clean_teardown(None, &cohort_root);
    Ok(())
}

/// REGRESSION I1 — error after Host bind but BEFORE program readiness: the
/// spawned Host exists and is pinned; the shell is unproven. The pinned
/// initial instance must exit and dirs must be removed.
#[cfg(target_os = "linux")]
#[test]
fn fixture_lifecycle_reclaims_pinned_host_on_post_spawn_error() -> TestResult {
    let (mut lifecycle, session_runtime, cohort_root) = guarded_real_host()?;
    let mut host_pin = None;
    let abandoned = (|| -> TestResult {
        let _identity = launch_real_terminal(lifecycle.server())?;
        wait_endpoint(&ModeCheckedLocalSessionRendezvous::host_endpoint_path(
            &session_runtime,
        ))?;
        host_pin = Some(
            lifecycle
                .pin_fixture_host()?
                .expect("the spawned fixture Host must pin"),
        );
        Err(io::Error::other("injected post-spawn failure").into())
    })();
    assert!(abandoned.is_err(), "the injected error must surface");
    drop(lifecycle);
    assert_clean_teardown(host_pin, &cohort_root);
    Ok(())
}

/// REGRESSION I2 — error after program-under-test readiness: the full wire
/// sequence ran, the drained EVENT proved the spawned shell executed, then
/// the fixture is abandoned without `finish()`. The pinned initial instance
/// must still exit and dirs must be removed.
#[cfg(target_os = "linux")]
#[test]
fn fixture_lifecycle_reclaims_pinned_host_on_post_readiness_error() -> TestResult {
    let (mut lifecycle, session_runtime, cohort_root) = guarded_real_host()?;
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
    let deadline = Instant::now() + FIXTURE_TIMEOUT;
    let mut host_pin = None;
    let abandoned = (|| -> TestResult {
        let mut mint = TokenMint(0);
        let _identity = prove_readiness(lifecycle.server(), &mut mint, &host_endpoint, deadline)?;
        host_pin = Some(
            lifecycle
                .pin_fixture_host()?
                .expect("the spawned fixture Host must pin"),
        );
        Err(io::Error::other("injected post-readiness failure").into())
    })();
    assert!(abandoned.is_err(), "the injected error must surface");
    drop(lifecycle);
    assert_clean_teardown(host_pin, &cohort_root);
    Ok(())
}

/// REGRESSION — panic after program-under-test readiness: `panic!()` unwinds
/// through `FixtureLifecycle::drop` -> the same deterministic settle.
/// `catch_unwind` lets the test assert the aftermath — and proves the Drop
/// path itself never panicked (a panicking Drop would abort the binary).
#[cfg(target_os = "linux")]
#[test]
fn fixture_lifecycle_reclaims_pinned_host_on_panic_unwind() {
    let mut cohort_root = None;
    let mut host_pin = None;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (mut lifecycle, session_runtime, root) =
            guarded_real_host().expect("guarded real-Host fixture");
        cohort_root = Some(root);
        let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&session_runtime);
        let deadline = Instant::now() + FIXTURE_TIMEOUT;
        let mut mint = TokenMint(0);
        prove_readiness(lifecycle.server(), &mut mint, &host_endpoint, deadline)
            .expect("program-under-test readiness");
        host_pin = Some(
            lifecycle
                .pin_fixture_host()
                .expect("pin scan")
                .expect("the spawned fixture Host must pin"),
        );
        panic!("induced panic after program-under-test readiness");
    }));
    // The unwind must carry THIS test's injected payload — an earlier
    // readiness .expect() would also produce Err but with a different
    // payload and no populated pin (the report-45 false-positive shape).
    let payload = outcome.expect_err("the induced panic must unwind");
    let message = payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
    assert_eq!(
        message,
        Some("induced panic after program-under-test readiness"),
        "the unwind must be the injected panic, not an earlier expect failure"
    );
    let pin = host_pin.expect("host pin must be populated before the panic");
    assert_clean_teardown(Some(pin), cohort_root.as_deref().expect("cohort root"));
}

/// NEGATIVE CONTROL — the exact-match must never claim another cohort's
/// Host. Pure-function level: synthesize a second cohort's would-be argv
/// and prove `argv_is_fixture_host` rejects it while accepting this
/// fixture's own argv — no second spawn needed.
#[cfg(target_os = "linux")]
#[test]
fn reclaim_never_matches_an_unrelated_cohort() -> TestResult {
    let own = TestCohort::new()?;
    let other = TestCohort::new()?;
    let own_exe = own.path().join(staged_name("orchbee-terminal-host"));
    let own_host_runtime =
        ModeCheckedLocalSessionRendezvous::host_runtime_directory(&own.path().join("runtime"));
    let other_host_runtime =
        ModeCheckedLocalSessionRendezvous::host_runtime_directory(&other.path().join("runtime"));
    // Synthetic argv exactly as lazy::start_host builds it, once under this
    // fixture's own paths and once under the OTHER cohort's unique paths.
    let push = |argv: &mut Vec<u8>, arg: &std::ffi::OsStr| {
        argv.extend_from_slice(arg.as_encoded_bytes());
        argv.push(0);
    };
    let argv_of = |exe: &Path, host_runtime: &Path| {
        let mut argv = Vec::new();
        push(&mut argv, exe.as_os_str());
        push(&mut argv, std::ffi::OsStr::new("serve"));
        push(&mut argv, std::ffi::OsStr::new("--runtime-directory"));
        push(&mut argv, host_runtime.as_os_str());
        let session_runtime = host_runtime.parent().unwrap_or(host_runtime);
        let endpoint_id = ModeCheckedLocalSessionRendezvous::host_endpoint_id(session_runtime);
        push(&mut argv, std::ffi::OsStr::new("--endpoint-id"));
        push(&mut argv, std::ffi::OsStr::new(&endpoint_id));
        push(&mut argv, std::ffi::OsStr::new("--endpoint-directory"));
        push(
            &mut argv,
            ModeCheckedLocalSessionRendezvous::socket_root_path().as_os_str(),
        );
        argv
    };
    let own_argv = argv_of(&own_exe, &own_host_runtime);
    let foreign_argv = argv_of(
        &other.path().join(staged_name("orchbee-terminal-host")),
        &other_host_runtime,
    );
    assert!(fixture_lifecycle::argv_is_fixture_host(
        &own_argv,
        &own_exe,
        &own_host_runtime
    ));
    assert!(!fixture_lifecycle::argv_is_fixture_host(
        &foreign_argv,
        &own_exe,
        &own_host_runtime
    ));
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

/// RFC 4648 url-safe, no padding — the APC payload alphabet. Hand-rolled so
/// no new dependency is needed.
#[cfg(target_os = "linux")]
fn base64_url_nopad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

/// `TestCohort::with_real_host` copies the built
/// `apps/terminal-host/zig-out/bin/orchbee-terminal-host` into the cohort as
/// the TerminalHost artifact. `Cohort::load` hashes every artifact
/// against cohort.json, so the manifest must carry each file's real sha256;
/// orchbee-worker has no sha2/serde_json dependency, so hashing goes through
/// the system `sha256sum` subprocess (Linux-only, matching lazy::run).
/// Returns None — the SKIPPED verdict — when the zig binary or sha256sum is
/// absent. The returned pair keeps the cohort directory alive for the whole
/// test because start_host spawns from `cohort.path()`.
impl TestCohort {
    #[cfg(target_os = "linux")]
    fn with_real_host() -> io::Result<Option<(Self, Cohort)>> {
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../terminal-host/zig-out/bin/orchbee-terminal-host");
        if !binary.is_file() || Command::new("sha256sum").arg("--version").output().is_err() {
            return Ok(None);
        }
        let cohort = Self::new()?;
        let host_path = cohort.path().join(staged_name("orchbee-terminal-host"));
        fs::copy(&binary, &host_path)?;
        fs::set_permissions(&host_path, fs::Permissions::from_mode(0o700))?;
        eprintln!(
            "real Host artifact: {} sha256={}",
            binary.display(),
            sha256_file(&binary)?
        );
        let mut hashes = Vec::new();
        for name in ARTIFACTS {
            hashes.push(sha256_file(&cohort.path().join(staged_name(name)))?);
        }
        let manifest = cohort.path().join("cohort.json");
        fs::write(
            &manifest,
            format!(
                r#"{{"schemaVersion":1,"artifacts":{{"orchbee":"{}","orchbee-server":"{}","orchbee-worker":"{}","orchbee-terminal-host":"{}","agent-observation-adapters":"{}"}}}}"#,
                hashes[0], hashes[1], hashes[2], hashes[3], ADAPTERS_SHA256
            ),
        )?;
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600))?;
        let loaded = Cohort::load(cohort.path(), Instant::now() + FIXTURE_TIMEOUT)?;
        Ok(Some((cohort, loaded)))
    }
}

#[cfg(target_os = "linux")]
fn sha256_file(path: &Path) -> io::Result<String> {
    let output = Command::new("sha256sum").arg(path).output()?;
    if !output.status.success() {
        return Err(io::Error::other("sha256sum failed"));
    }
    let text = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    text.split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("sha256sum produced no hash"))
}

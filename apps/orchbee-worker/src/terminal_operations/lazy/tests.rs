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
    AbsoluteCwd, AuthorizedTerminalLaunch, CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds,
    LeaseId, OperationId, TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalCwdValidationOutcome,
    TerminalCwdValidationRequest, TerminalFence, TerminalId, TerminalLaunchFailure,
    TerminalLaunchMode, TerminalLaunchOutcome, TerminalOperationAuthorization,
    TerminalOperationLease, WorkerGeneration, WorkerId as CapabilityWorkerId, WorktreeJobAction,
    WorktreeJobOutcome, WorktreeJobPollOutcome, WorktreeJobRequest, WorktreeJobStartOutcome,
    read_message, write_message,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_local_site_runtime::{ModeCheckedLocalSiteRendezvous, NativeCohort};

use super::run;

const ARTIFACTS: [&str; 4] = [
    "orchbee",
    "orchbee-server",
    "orchbee-worker",
    "orchbee-terminal-host",
];
const ARTIFACT_CONTENT: &[u8] = b"native fixture\n";
const ARTIFACT_SHA256: &str = "396cfec2d1c795ce71dce2d9c92f29cddec638632050c17861d771936a99282d";
const WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);
const GENERATION: NonZeroU64 = NonZeroU64::MIN;
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn validation_before_first_launch_uses_real_socket_and_never_creates_host_runtime() -> TestResult {
    let fixture = TestCohort::new()?;
    let cohort = NativeCohort::load(fixture.path(), Instant::now() + Duration::from_secs(2))?;
    let available = fixture.path().join("available");
    fs::create_dir(&available)?;
    let missing = fixture.path().join("missing");
    let site_runtime = fixture.path().join("runtime");
    fs::create_dir(&site_runtime)?;
    fs::set_permissions(&site_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSiteRendezvous::host_runtime_directory(&site_runtime);
    let host_endpoint = ModeCheckedLocalSiteRendezvous::host_endpoint_path(&site_runtime);
    let (mut server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(Duration::from_secs(2)))?;
    server.set_write_timeout(Some(Duration::from_secs(2)))?;
    let worker_runtime = site_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(&worker, WORKER_ID, GENERATION, &cohort, &worker_runtime).is_err()
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
            TerminalCwdValidationOutcome::DirectoryUnavailable,
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
    let cohort = NativeCohort::load(fixture.path(), Instant::now() + Duration::from_secs(2))?;
    let directory = fixture.path().join("plain-directory");
    fs::create_dir(&directory)?;
    let site_runtime = fixture.path().join("runtime");
    fs::create_dir(&site_runtime)?;
    fs::set_permissions(&site_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSiteRendezvous::host_runtime_directory(&site_runtime);
    let host_endpoint = ModeCheckedLocalSiteRendezvous::host_endpoint_path(&site_runtime);
    let (mut server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(Duration::from_secs(5)))?;
    server.set_write_timeout(Some(Duration::from_secs(2)))?;
    let worker_runtime = site_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(&worker, WORKER_ID, GENERATION, &cohort, &worker_runtime).is_err()
    });

    let start = WorktreeJobRequest {
        operation_id: OperationId::from_bytes([9; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        action: WorktreeJobAction::Start {
            directory: directory.as_os_str().as_encoded_bytes().to_vec(),
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
    let cohort = NativeCohort::load(fixture.path(), Instant::now() + Duration::from_secs(2))?;
    // Pad the Site runtime until the derived `host/<id>.s` endpoint cannot fit
    // one socket address, while the Site directory itself is still creatable.
    let mut site_runtime = fixture.path().join("runtime");
    while nix::sys::socket::UnixAddr::new(&ModeCheckedLocalSiteRendezvous::host_endpoint_path(
        &site_runtime,
    ))
    .is_ok()
    {
        site_runtime = site_runtime.join("x");
    }
    fs::create_dir_all(&site_runtime)?;
    fs::set_permissions(&site_runtime, fs::Permissions::from_mode(0o700))?;
    let host_runtime = ModeCheckedLocalSiteRendezvous::host_runtime_directory(&site_runtime);
    let (mut server, worker) = UnixStream::pair()?;
    server.set_read_timeout(Some(Duration::from_secs(2)))?;
    server.set_write_timeout(Some(Duration::from_secs(2)))?;
    let worker_runtime = site_runtime.clone();
    let worker_thread = thread::spawn(move || {
        run(&worker, WORKER_ID, GENERATION, &cohort, &worker_runtime).is_err()
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
        let path = std::env::temp_dir().join(format!(
            "orchbee-worker-lazy-validation-{}-{identity}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        for artifact in ARTIFACTS {
            let artifact_path = path.join(artifact);
            fs::write(&artifact_path, ARTIFACT_CONTENT)?;
            fs::set_permissions(artifact_path, fs::Permissions::from_mode(0o700))?;
        }
        let manifest = path.join("cohort.json");
        fs::write(
            &manifest,
            format!(
                r#"{{"schemaVersion":1,"artifacts":{{"orchbee":"{}","orchbee-server":"{}","orchbee-worker":"{}","orchbee-terminal-host":"{}"}}}}"#,
                ARTIFACT_SHA256, ARTIFACT_SHA256, ARTIFACT_SHA256, ARTIFACT_SHA256
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

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
    AbsoluteCwd, CapabilityNodeMessage, OperationId, TerminalCwdValidationOutcome,
    TerminalCwdValidationRequest, WorkerGeneration, WorkerId as CapabilityWorkerId, read_message,
    write_message,
};
use orchbee_local_site_protocol::WorkerId;
use orchbee_local_site_runtime::NativeCohort;

use super::run;

const ARTIFACTS: [&str; 4] = [
    "orchbee",
    "orchbee-server",
    "orchbee-worker",
    "orchbee-terminal-host",
];
const ARTIFACT_CONTENT: &[u8] = b"native fixture\n";
const ARTIFACT_SHA256: &str = "396cfec2d1c795ce71dce2d9c92f29cddec638632050c17861d771936a99282d";
const HOST_ENDPOINT_ID: &str = "00000000000000000000000000000001";
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
    let host_runtime = site_runtime.join("host");
    let host_endpoint = host_runtime.join(format!("{HOST_ENDPOINT_ID}.s"));
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

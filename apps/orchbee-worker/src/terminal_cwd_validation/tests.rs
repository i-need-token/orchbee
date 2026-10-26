use std::{
    error::Error,
    fs::{self},
    io,
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, OperationId, TerminalCwdValidationOutcome, TerminalCwdValidationRequest,
    WorkerGeneration, WorkerId as CapabilityWorkerId,
};
use orchbee_local_worker_protocol::WorkerId;

use super::validate;

const WORKER_ID: WorkerId = WorkerId::from_bytes([7; 16]);
const GENERATION: NonZeroU64 = NonZeroU64::MIN;
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[cfg(unix)]
#[test]
fn validates_directory_metadata_without_a_host_and_preserves_path_spelling() -> TestResult {
    let fixture = TestDirectory::new()?;
    let target = fixture.path().join("target");
    fs::create_dir(&target)?;
    std::os::unix::fs::symlink(&target, fixture.path().join("linked"))?;
    std::fs::File::create(fixture.path().join("regular-file"))?;

    let spelled = fixture.path().join("target").join("..").join("linked");
    let available = validate(
        request(&spelled, WORKER_ID, GENERATION)?,
        WORKER_ID,
        GENERATION,
    );
    assert_eq!(
        available.outcome,
        TerminalCwdValidationOutcome::DirectoryAvailable
    );
    assert_eq!(
        available.request.cwd.as_str(),
        spelled
            .to_str()
            .ok_or_else(|| io::Error::other("non-UTF8 test path"))?
    );

    for path in [
        fixture.path().join("regular-file"),
        fixture.path().join("missing"),
    ] {
        assert_eq!(
            validate(
                request(&path, WORKER_ID, GENERATION)?,
                WORKER_ID,
                GENERATION
            )
            .outcome,
            TerminalCwdValidationOutcome::DirectoryUnavailable
        );
    }
    Ok(())
}

#[test]
fn rejects_the_wrong_worker_or_generation_before_directory_observation() -> TestResult {
    let fixture = TestDirectory::new()?;
    let missing = fixture.path().join("missing");
    let wrong_worker = WorkerId::from_bytes([8; 16]);
    let newer_generation =
        NonZeroU64::new(2).ok_or_else(|| io::Error::other("test generation must be nonzero"))?;

    assert_eq!(
        validate(
            request(&missing, wrong_worker, GENERATION)?,
            WORKER_ID,
            GENERATION,
        )
        .outcome,
        TerminalCwdValidationOutcome::WorkerUnavailable
    );
    assert_eq!(
        validate(
            request(&missing, WORKER_ID, newer_generation)?,
            WORKER_ID,
            GENERATION,
        )
        .outcome,
        TerminalCwdValidationOutcome::WorkerUnavailable
    );
    Ok(())
}

fn request(
    path: &Path,
    worker_id: WorkerId,
    generation: NonZeroU64,
) -> TestResult<TerminalCwdValidationRequest> {
    Ok(TerminalCwdValidationRequest {
        operation_id: OperationId::from_bytes([1; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(worker_id.as_bytes())?,
        worker_generation: WorkerGeneration::new(generation),
        cwd: AbsoluteCwd::new(
            path.to_str()
                .ok_or_else(|| io::Error::other("non-UTF8 test path"))?,
        )?,
    })
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> io::Result<Self> {
        let identity = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "orchbee-worker-cwd-validation-{}-{identity}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}

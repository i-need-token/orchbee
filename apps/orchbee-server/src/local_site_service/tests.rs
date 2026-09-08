use std::{
    error::Error,
    io,
    num::NonZeroU64,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, CapabilityNodeMessage, HostEpoch, LeaseExpiresAtUnixMilliseconds,
    LeaseId, OperationId, TerminalFence, TerminalId, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchRejection, TerminalLaunchResult, TerminalOperationAuthorization,
    TerminalOperationLease, WorkerGeneration, WorkerId as CapabilityWorkerId,
    read_message as read_capability_message, write_message as write_capability_message,
};
use orchbee_local_site_protocol::{
    LocalSiteMessage, RejectionReason, WorkerHello, WorkerId, read_message as read_site_message,
    write_message as write_site_message,
};

use crate::{
    worker_obcn_carrier::{WorkerOperationRequest, WorkerOperationResult},
    worker_registry::{WorkerRegistry, generation_is_retired},
};

use super::handle_candidate;

const COHORT: &str = "test-cohort";
const FIRST_WORKER: WorkerId = WorkerId::from_bytes([1; 16]);
const SECOND_WORKER: WorkerId = WorkerId::from_bytes([2; 16]);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(2);

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[test]
fn accepted_stream_becomes_one_generation_scoped_obcn_carrier() -> TestResult {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let (mut first_worker, first_generation) = admit_worker(&registry, FIRST_WORKER)?;
    let first_handle = request_handle(&registry)?;
    assert_eq!(first_handle.generation(), first_generation);

    let first_request = launch_request(FIRST_WORKER, first_generation, 1)?;
    let first_result = launch_result(&first_request)?;
    let first_peer = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_capability_message(&mut first_worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(first_request)
        );
        write_capability_message(
            &mut first_worker,
            &CapabilityNodeMessage::TerminalLaunchResult(first_result),
        )?;
        Ok(())
    });
    assert_eq!(
        first_handle.request(WorkerOperationRequest::Launch(launch_request(
            FIRST_WORKER,
            first_generation,
            1,
        )?))?,
        WorkerOperationResult::Launch(first_result)
    );
    first_peer
        .join()
        .map_err(|_panic| "first fake Worker panicked")??;
    wait_for_generation_retirement(&registry, first_generation)?;
    assert!(!first_handle.is_active());

    let (mut replacement_worker, replacement_generation) = admit_worker(&registry, SECOND_WORKER)?;
    assert!(replacement_generation > first_generation);
    let replacement_handle = request_handle(&registry)?;
    assert!(replacement_handle.is_active());

    let replacement_request = launch_request(SECOND_WORKER, replacement_generation, 2)?;
    let replacement_result = launch_result(&replacement_request)?;
    let replacement_peer = thread::spawn(move || -> TestResult {
        assert_eq!(
            read_capability_message(&mut replacement_worker)?,
            CapabilityNodeMessage::AuthorizedTerminalLaunch(replacement_request)
        );
        write_capability_message(
            &mut replacement_worker,
            &CapabilityNodeMessage::TerminalLaunchResult(replacement_result),
        )?;
        Ok(())
    });
    assert_eq!(
        replacement_handle.request(WorkerOperationRequest::Launch(launch_request(
            SECOND_WORKER,
            replacement_generation,
            2,
        )?))?,
        WorkerOperationResult::Launch(replacement_result)
    );
    replacement_peer
        .join()
        .map_err(|_panic| "replacement fake Worker panicked")??;
    wait_for_generation_retirement(&registry, replacement_generation)?;
    Ok(())
}

#[test]
fn rejects_an_unmappable_zero_worker_id_before_acceptance() -> TestResult {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let (mut worker, server) = UnixStream::pair()?;
    worker.set_read_timeout(Some(COMPLETION_TIMEOUT))?;
    worker.set_write_timeout(Some(COMPLETION_TIMEOUT))?;
    write_site_message(
        &mut worker,
        &LocalSiteMessage::WorkerHello(WorkerHello::new(COHORT, WorkerId::from_bytes([0; 16]))?),
    )?;

    handle_candidate(server, &registry, COHORT);

    assert_eq!(
        read_site_message(&mut worker)?,
        LocalSiteMessage::Rejected {
            reason: RejectionReason::MalformedFrame,
        }
    );
    assert_eq!(
        registry
            .lock()
            .map_err(|_poisoned| io::Error::other("Worker registry was poisoned"))?
            .admit(FIRST_WORKER),
        crate::worker_registry::WorkerAdmission::Accepted {
            generation: NonZeroU64::MIN,
        }
    );
    Ok(())
}

fn admit_worker(
    registry: &Arc<Mutex<WorkerRegistry>>,
    worker_id: WorkerId,
) -> TestResult<(UnixStream, NonZeroU64)> {
    let (mut worker, server) = UnixStream::pair()?;
    worker.set_read_timeout(Some(COMPLETION_TIMEOUT))?;
    worker.set_write_timeout(Some(COMPLETION_TIMEOUT))?;
    write_site_message(
        &mut worker,
        &LocalSiteMessage::WorkerHello(WorkerHello::new(COHORT, worker_id)?),
    )?;
    handle_candidate(server, registry, COHORT);
    let reply = read_site_message(&mut worker)?;
    let LocalSiteMessage::WorkerAccepted { generation } = reply else {
        return Err(io::Error::other(format!(
            "fake Worker {worker_id:?} was not accepted: {reply:?}"
        ))
        .into());
    };
    Ok((worker, generation))
}

fn request_handle(
    registry: &Arc<Mutex<WorkerRegistry>>,
) -> TestResult<crate::worker_obcn_carrier::WorkerRequestHandle> {
    registry
        .lock()
        .map_err(|_poisoned| io::Error::other("Worker registry was poisoned"))?
        .request_handle()
        .ok_or_else(|| io::Error::other("Worker request handle was not registered").into())
}

fn wait_for_generation_retirement(
    registry: &Arc<Mutex<WorkerRegistry>>,
    generation: NonZeroU64,
) -> TestResult {
    let deadline = Instant::now() + COMPLETION_TIMEOUT;
    loop {
        let registry_guard = registry
            .lock()
            .map_err(|_poisoned| io::Error::other("Worker registry was poisoned"))?;
        let retired = generation_is_retired(&registry_guard, generation);
        drop(registry_guard);
        if retired {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Worker generation {generation} retirement did not finish"),
            )
            .into());
        }
        thread::yield_now();
    }
}

fn launch_request(
    worker_id: WorkerId,
    generation: NonZeroU64,
    operation: u8,
) -> TestResult<AuthorizedTerminalLaunch> {
    let authorization = TerminalOperationAuthorization::new(
        TerminalId::from_bytes([operation; 16])?,
        OperationId::from_bytes([operation; 16])?,
        CapabilityWorkerId::from_bytes(worker_id.as_bytes())?,
        WorkerGeneration::new(generation),
        TerminalFence::new(NonZeroU64::MIN),
        TerminalOperationLease::new(
            LeaseId::from_bytes([operation; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
        ),
        orchbee_capability_node_protocol::TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    Ok(AuthorizedTerminalLaunch::new(
        authorization,
        None,
        TerminalLaunchMode::Deferred,
    ))
}

fn launch_result(request: &AuthorizedTerminalLaunch) -> TestResult<TerminalLaunchResult> {
    Ok(TerminalLaunchResult::new(
        request.authorization(),
        TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
            host_epoch: HostEpoch::from_bytes([9; 16])?,
        }),
    )?)
}

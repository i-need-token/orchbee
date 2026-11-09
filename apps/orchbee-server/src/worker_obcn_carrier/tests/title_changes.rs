//! Title change reads never go ahead of other work, notes reach the registry while idle and while a
//! response is awaited, and a read's answer must echo its request.
use super::*;
use crate::terminal_titles::{NextRead, TerminalTitles};
use orchbee_capability_node_protocol::{
    TerminalTitleChangePage, TerminalTitleChangesOutcome, TerminalTitleChangesRequest,
    TerminalTitleChangesResult, TerminalTitlesChanged, TerminalTitlesRequest,
};

fn read_request(value: u8, after: u64) -> TestResult<TerminalTitleChangesRequest> {
    Ok(TerminalTitleChangesRequest {
        request_id: OperationId::from_bytes([value; 16])?,
        worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        worker_generation: WorkerGeneration::new(GENERATION),
        after,
    })
}

fn note(through: u64) -> TestResult<CapabilityNodeMessage> {
    Ok(CapabilityNodeMessage::TerminalTitlesChanged(
        TerminalTitlesChanged {
            through: nonzero(through)?,
        },
    ))
}

#[test]
fn a_title_change_read_waits_until_both_lanes_are_empty() -> TestResult {
    let (handle, owner, _shutdown, _server, _worker) = test_carrier()?;
    let _read = handle.admit_title_changes(read_request(1, 0)?)?;
    // One read is outstanding at a time; another is backpressure, never a wait.
    assert!(matches!(
        handle.admit_title_changes(read_request(2, 0)?),
        Err(WorkerRequestError::Backpressure)
    ));
    let _low = enqueue_request(
        &handle,
        WorkerOperationRequest::Titles(TerminalTitlesRequest {
            request_id: OperationId::from_bytes([3; 16])?,
            worker_id: CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
            worker_generation: WorkerGeneration::new(GENERATION),
            identities: vec![host_identity(1)?],
        }),
    )?;
    let _high = enqueue_request(
        &handle,
        WorkerOperationRequest::CwdValidation(cwd_validation_request(
            4,
            GENERATION.get(),
            LOCAL_WORKER_ID,
            if cfg!(windows) { "C:\\work" } else { "/work" },
        )?),
    )?;
    let order: Vec<RequestLane> = (0..3)
        .map(|_| take_dispatched(&owner).map(|pending| pending.request.lane()))
        .collect::<TestResult<_>>()?;
    assert_eq!(
        order,
        [RequestLane::High, RequestLane::Low, RequestLane::Idle]
    );
    assert!(owner.queue.dispatch_next().is_none());
    Ok(())
}

#[test]
fn notes_reach_the_registry_while_idle_and_while_a_read_is_answered() -> TestResult {
    let titles = Arc::new(TerminalTitles::default());
    titles.begin(
        CapabilityWorkerId::from_bytes(LOCAL_WORKER_ID.as_bytes())?,
        GENERATION,
    );
    let start = Instant::now();
    let empty = |next| {
        Some(TerminalTitleChangesOutcome::Page(TerminalTitleChangePage {
            rows: Vec::new(),
            next,
            complete: true,
        }))
    };
    let NextRead::Read(fresh) = titles.next_read(start) else {
        return Err("a fresh read was due".into());
    };
    titles.apply(&fresh, empty(0), start);
    let (handle, owner, _shutdown, server, mut worker) = test_carrier()?;
    let (exit_sender, _exit_receiver) = mpsc::channel();
    let owner_thread = spawn_owner(
        owner.with_titles(Arc::clone(&titles), GENERATION),
        server,
        exit_sender,
    );
    let due = |now: Instant| -> TestResult<TerminalTitleChangesRequest> {
        let deadline = Instant::now() + TEST_COMPLETION_TIMEOUT;
        loop {
            if let NextRead::Read(request) = titles.next_read(now) {
                return Ok(request);
            }
            if Instant::now() >= deadline {
                return Err("the note did not reach the registry".into());
            }
            thread::yield_now();
        }
    };

    write_message(&mut worker, &note(3)?)?;
    let noted = due(start + Duration::from_secs(1))?;
    assert_eq!(noted.after, 0);
    titles.apply(&noted, empty(3), start + Duration::from_secs(1));

    let request = read_request(5, 3)?;
    let answer = TerminalTitleChangesResult {
        request,
        outcome: TerminalTitleChangesOutcome::Reset,
    };
    let response = answer.clone();
    let worker_thread = thread::spawn(move || -> TestResult<UnixStream> {
        assert_eq!(
            read_message(&mut worker)?,
            CapabilityNodeMessage::TerminalTitleChangesRequest(request)
        );
        write_message(&mut worker, &note(4)?)?;
        write_message(
            &mut worker,
            &CapabilityNodeMessage::TerminalTitleChangesResult(response),
        )?;
        Ok(worker)
    });
    assert_eq!(
        handle.admit_title_changes(request)?.recv(),
        Ok(WorkerOperationResult::TitleChanges(answer))
    );
    let worker = worker_thread.join().map_err(|_panic| "worker panicked")??;
    assert_eq!(due(start + Duration::from_secs(2))?.after, 3);
    drop(worker);
    owner_thread
        .join()
        .map_err(|_panic| "wire owner panicked")?;
    Ok(())
}

#[test]
fn a_read_answer_echoing_another_request_retires_the_carrier() -> TestResult {
    let request = read_request(6, 4)?;
    assert_post_delivery_failure(
        WorkerOperationRequest::TitleChanges(request),
        CapabilityNodeMessage::TerminalTitleChangesResult(TerminalTitleChangesResult {
            request: TerminalTitleChangesRequest {
                after: 3,
                ..request
            },
            outcome: TerminalTitleChangesOutcome::Reset,
        }),
        WorkerResponseFailure::MismatchedCorrelation,
    )
}

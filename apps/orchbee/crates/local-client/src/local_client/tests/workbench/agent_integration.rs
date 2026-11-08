use super::*;
use crate::local_client::workbench::agent_integration::AgentIntegrationJobEnd as End;
use orchbee_agent_integration::{IntegrationOperation, IntegrationReport, IntegrationRequest};
use orchbee_workbench_protocol::{
    AgentIntegrationAction as Action, AgentIntegrationJob, AgentIntegrationOutcome as Outcome,
    AgentIntegrationResult,
};
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Start,
    Poll,
    Cancel,
    /// A pause the Client asked its caller for.
    Pause(Duration),
}

/// How one job ended, the log of its steps, and the deadline it ran under.
struct Run {
    end: End,
    log: Vec<(Step, Instant)>,
    deadline: Instant,
}

/// Runs one job against a scripted Server, which answers each request from the steps logged so
/// far and may interrupt the caller while that answer is in flight. Waits sleep, then report the
/// interruption. The log interleaves the requests the Server read with the Client's pauses in
/// causal order.
fn run_job(
    budget: Duration,
    mut script: impl FnMut(Step, &[(Step, Instant)]) -> (Outcome, bool) + Send + 'static,
) -> Result<Run, Box<dyn std::error::Error>> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let interrupted = Arc::new(AtomicBool::new(false));
    let server = {
        let (log, interrupted) = (Arc::clone(&log), Arc::clone(&interrupted));
        FakeServer::start(move |mut stream| {
            handshake::expect_hello(&mut stream)?;
            admit(&mut stream, 70)?;
            loop {
                let request = match read_workbench_request(&mut stream) {
                    Ok(WorkbenchRequest::AgentIntegration(request)) => request,
                    Err(WorkbenchWireError::Io(error))
                        if error.kind() == io::ErrorKind::UnexpectedEof =>
                    {
                        return Ok(());
                    }
                    other => return Err(io::Error::other(format!("{other:?}"))),
                };
                let step = match request.action {
                    Action::Start { .. } => Step::Start,
                    Action::Poll { .. } => Step::Poll,
                    Action::Cancel { .. } => Step::Cancel,
                };
                let (outcome, interrupt) = {
                    let mut log = log.lock().unwrap();
                    log.push((step, Instant::now()));
                    script(step, &log)
                };
                interrupted.fetch_or(interrupt, Ordering::SeqCst);
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::AgentIntegration(AgentIntegrationResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        session_id: request.session_id,
                        outcome,
                    }),
                )
                .map_err(io::Error::other)?;
            }
        })?
    };
    let mut client = LocalClient::connect(server.endpoint())?;
    let mut ids = 0x100_u128..;
    let deadline = Instant::now() + budget;
    let end = client.agent_integration_job(
        &SessionId::parse(&id(2)).unwrap(),
        IntegrationRequest {
            operation: IntegrationOperation::List { provider: None },
            adapter_source: None,
            budget_milliseconds: None,
        },
        deadline,
        || ids.next().map(|value| workbench_id(&id(value))),
        |pause| {
            log.lock()
                .unwrap()
                .push((Step::Pause(pause), Instant::now()));
            std::thread::sleep(pause);
            interrupted.load(Ordering::SeqCst)
        },
    );
    drop(client);
    server.finish()?;
    let log = log.lock().unwrap().clone();
    Ok(Run { end, log, deadline })
}

fn job() -> AgentIntegrationJob {
    serde_json::from_value(serde_json::json!({"workerId":id(4),"generation":"1","jobId":id(9)}))
        .unwrap()
}

fn completed() -> Outcome {
    Outcome::Completed {
        job: job(),
        report: Box::new(IntegrationReport::HomeUnavailable {}),
    }
}

fn cancelled(log: &[(Step, Instant)]) -> bool {
    log.iter().any(|(step, _)| *step == Step::Cancel)
}

#[test]
fn a_job_pending_at_the_settlement_is_cancelled_in_time_and_polled_to_its_end() -> TestResult {
    // Whether the Worker registers the cancellation or had already settled the job, and whether
    // the job then reports before the deadline.
    for (registered, reports) in [(true, true), (false, true), (true, false)] {
        let Run { end, log, deadline } = run_job(Duration::from_millis(600), move |step, log| {
            let outcome = match step {
                Step::Start => Outcome::Accepted { job: job() },
                Step::Cancel if registered => Outcome::CancelRegistered { job: job() },
                Step::Cancel => Outcome::AlreadyTerminal { job: job() },
                Step::Poll if reports && cancelled(log) => completed(),
                Step::Poll | Step::Pause(_) => Outcome::Pending { job: job() },
            };
            (outcome, false)
        })?;
        let case = format!("registered {registered}, reports {reports}: {log:?}");
        let requests: Vec<_> = log
            .iter()
            .filter(|(step, _)| !matches!(step, Step::Pause(_)))
            .collect();
        let cancels: Vec<_> = requests
            .iter()
            .filter(|(step, _)| *step == Step::Cancel)
            .collect();
        // The settlement is the last half of this short budget.
        assert_eq!(cancels.len(), 1, "{case}");
        assert!(
            cancels[0].1 >= deadline - Duration::from_millis(300) && cancels[0].1 < deadline,
            "{case}"
        );
        assert_eq!(
            requests.last().map(|(step, _)| *step),
            Some(Step::Poll),
            "{case}"
        );
        match end {
            End::Completed {
                cancel_registered, ..
            } if reports => assert_eq!(cancel_registered, registered, "{case}"),
            End::Deadline if !reports => {}
            end => panic!("{case}: {end:?}"),
        }
    }
    Ok(())
}

#[test]
fn interruptions_cancel_once_their_exchange_answers_unless_the_job_reported() -> TestResult {
    for raised_by in [Step::Start, Step::Poll] {
        let Run { end, log, .. } = run_job(Duration::from_secs(10), move |step, log| match step {
            Step::Start => (Outcome::Accepted { job: job() }, raised_by == Step::Start),
            Step::Cancel => (Outcome::CancelRegistered { job: job() }, false),
            Step::Poll if cancelled(log) => (completed(), false),
            Step::Poll | Step::Pause(_) => {
                (Outcome::Pending { job: job() }, raised_by == Step::Poll)
            }
        })?;
        // Between the answer that carried the interruption and the cancellation: no poll and no
        // backoff, only a probe of the interruption.
        let raised = log.iter().position(|(step, _)| *step == raised_by).unwrap();
        let cancel = log.iter().position(|(step, _)| *step == Step::Cancel);
        let between = cancel.map(|cancel| &log[raised + 1..cancel]);
        assert!(
            between.is_some_and(|steps| steps.iter().all(|(step, _)| {
                matches!(step, Step::Pause(pause) if *pause <= Duration::from_millis(1))
            })),
            "{raised_by:?}: {log:?}"
        );
        assert!(
            matches!(
                end,
                End::Completed {
                    cancel_registered: true,
                    ..
                }
            ),
            "{raised_by:?}: {end:?}"
        );
    }
    // A final report answering as the interruption arrives stands, and nothing is cancelled.
    let Run { end, log, .. } = run_job(Duration::from_secs(10), |step, _| match step {
        Step::Start => (Outcome::Accepted { job: job() }, false),
        Step::Poll | Step::Cancel | Step::Pause(_) => (completed(), true),
    })?;
    assert!(!cancelled(&log), "{log:?}");
    assert!(
        matches!(
            end,
            End::Completed {
                cancel_registered: false,
                ..
            }
        ),
        "{end:?}"
    );
    Ok(())
}

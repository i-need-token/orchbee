use super::*;

/// The send path's total budget is shared, not additive: the preamble's
/// stage deadline is the earlier of its own budget and the command's
/// absolute deadline — pinned without any clock or socket.
#[test]
fn bounded_deadline_takes_the_earlier_bound() {
    let now = Instant::now();
    // A nearly spent total budget leaves the preamble almost nothing.
    let ceiling = now + Duration::from_millis(50);
    assert_eq!(
        bounded_deadline(now, COHORT_BUDGET, ceiling).unwrap(),
        ceiling
    );
    // A far ceiling keeps the stage's own budget intact.
    assert_eq!(
        bounded_deadline(now, COHORT_BUDGET, now + Duration::from_secs(60)).unwrap(),
        now + COHORT_BUDGET
    );
}

fn worktree_directory() -> WorkbenchDirectoryId {
    WorkbenchDirectoryId::parse("0123456789abcdef0123456789abcdef").unwrap()
}

/// Post-delivery response loss is transport-loss (exit 5) under the
/// dedicated worktree envelope — the read-only family has no
/// outcome-unknown kind.
#[test]
fn worktree_transport_loss_is_exit_five() {
    let failure = classify_worktree_outcome(
        WorkbenchWorktreeListOutcome::TransportLoss {},
        &worktree_directory(),
    );
    assert_eq!(failure.kind, FailureKind::TransportLoss);
    assert_eq!(failure.kind.exit_code(), 5);
    assert_eq!(
        failure.failure_schema,
        Some(crate::cli::schema::WORKTREE_FAILURE_ID)
    );
    let details = failure.details.expect("transport-loss details");
    assert_eq!(details["stage"], serde_json::json!("worktree-list"));
    assert_eq!(details.len(), 1);
}

/// Both spawn arms are the Worker's process-start failure:
/// service-unavailable (exit 4) with the worker-unavailable reason —
/// never a Git check failure.
#[test]
fn worktree_spawn_failures_are_worker_unavailable() {
    for outcome in [
        WorkbenchWorktreeListOutcome::SpawnUnavailable {
            stage: WorktreeListStage::Probe,
        },
        WorkbenchWorktreeListOutcome::SpawnFailed {
            stage: WorktreeListStage::List,
        },
    ] {
        let failure = classify_worktree_outcome(outcome, &worktree_directory());
        assert_eq!(failure.kind, FailureKind::ServiceUnavailable);
        assert_eq!(failure.kind.exit_code(), 4);
        assert_eq!(
            failure.failure_schema,
            Some(crate::cli::schema::WORKTREE_FAILURE_ID)
        );
        let details = failure.details.expect("unavailable details");
        assert_eq!(details["stage"], serde_json::json!("worktree-list"));
        assert_eq!(details["reason"], serde_json::json!("worker-unavailable"));
    }
}

/// Git failures always carry the status key: the exact exit integer, or
/// an explicit JSON null when the process was signaled. Malformed
/// listings carry no invented status at all.
#[test]
fn worktree_git_status_is_always_explicit() {
    let failure = classify_worktree_outcome(
        WorkbenchWorktreeListOutcome::GitFailed {
            stage: WorktreeListStage::Probe,
            status: Some(1),
        },
        &worktree_directory(),
    );
    assert_eq!(failure.kind, FailureKind::CheckFailed);
    assert_eq!(failure.kind.exit_code(), 1);
    let details = failure.details.expect("git details");
    assert_eq!(details["stage"], serde_json::json!("probe"));
    assert_eq!(details["status"], serde_json::json!(1));

    let failure = classify_worktree_outcome(
        WorkbenchWorktreeListOutcome::GitFailed {
            stage: WorktreeListStage::List,
            status: None,
        },
        &worktree_directory(),
    );
    let details = failure.details.expect("signaled git details");
    assert_eq!(details["stage"], serde_json::json!("list"));
    assert_eq!(details["status"], serde_json::Value::Null);
    assert!(details.contains_key("status"));

    let failure = classify_worktree_outcome(
        WorkbenchWorktreeListOutcome::MalformedListing {},
        &worktree_directory(),
    );
    assert_eq!(failure.kind, FailureKind::CheckFailed);
    let details = failure.details.expect("parse details");
    assert_eq!(details["stage"], serde_json::json!("parse"));
    assert!(!details.contains_key("status"));
}

/// Worker-side physical uncertainty surfaces as worker-unavailable; the
/// not-found arm keeps its directory resource identity.
#[test]
fn worktree_unavailable_reasons_and_not_found_stay_typed() {
    let failure = classify_worktree_outcome(
        WorkbenchWorktreeListOutcome::Unavailable {
            reason: WorktreeListUnavailableReason::WorkerUnavailable,
        },
        &worktree_directory(),
    );
    assert_eq!(failure.kind, FailureKind::ServiceUnavailable);
    let details = failure.details.expect("unavailable details");
    assert_eq!(details["reason"], serde_json::json!("worker-unavailable"));

    let failure = classify_worktree_outcome(
        WorkbenchWorktreeListOutcome::NotFound {},
        &worktree_directory(),
    );
    assert_eq!(failure.kind, FailureKind::NotFound);
    assert_eq!(failure.kind.exit_code(), 3);
    let details = failure.details.expect("not-found details");
    assert_eq!(details["resource"], serde_json::json!("directory"));
    assert_eq!(
        details["target"],
        serde_json::json!("0123456789abcdef0123456789abcdef")
    );
}

/// socket.
#[test]
fn agent_settle_reports_timeout_only_at_or_past_total() {
    assert!(
        agent_settle(
            Instant::now() + Duration::from_secs(60),
            AgentReadOperation::List
        )
        .is_ok()
    );
    let failure = agent_settle(Instant::now(), AgentReadOperation::Get)
        .expect_err("an expired total settles as the Agent timeout");
    assert_eq!(failure.kind, FailureKind::Timeout);
    let details = failure.details.expect("agent timeout details");
    assert_eq!(details["stage"], serde_json::json!("agent-read"));
    assert_eq!(details["operation"], serde_json::json!("get"));
    assert_eq!(details["timeoutMs"], serde_json::json!(5000));
}

/// An already-expired total fails before any subsequent stage IO: the
/// preamble never gets a usable window and the connect is never
/// attempted — the call returns promptly with a failure, deterministic
/// on any machine.
#[test]
fn agent_session_until_expired_total_fails_before_io() {
    let started = Instant::now();
    assert!(agent_session_until(Instant::now(), None).is_err());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "an expired total fails fast, never starting the next IO"
    );
}

/// A provisional wake marker held into `Clear` classification is a
/// contradiction: at every transport I/O position it must land on
/// `internal-failure`, never a transport-loss or timeout verdict.
#[test]
fn agent_markers_classify_internal_at_every_io_position() {
    let request_id = ObapRequestId::parse("11111111111111111111111111111111").expect("request id");
    for marker in [
        crate::local_client::wake_ready_io,
        crate::local_client::wake_fault_io,
    ] {
        for operation in [
            AgentReadOperation::List,
            AgentReadOperation::Get,
            AgentReadOperation::Explain,
        ] {
            let not_submitted =
                classify_agent_read(AgentExchangeError::RequestNotSubmitted(marker()), operation);
            assert_eq!(not_submitted.kind, FailureKind::InternalFailure);
            for reason in [
                AgentExchangeFailure::Write(marker()),
                AgentExchangeFailure::ResponseDeadline(marker()),
                AgentExchangeFailure::Response(AgentWireError::Io(marker())),
            ] {
                let failure = classify_agent_read(
                    AgentExchangeError::Failed {
                        request_id: request_id.clone(),
                        reason,
                    },
                    operation,
                );
                assert_eq!(
                    failure.kind,
                    FailureKind::InternalFailure,
                    "{operation:?} marker must never classify as transport/timeout"
                );
                assert!(
                    failure.details.is_none(),
                    "internal-failure carries no agent-read details"
                );
            }
        }
    }
}

/// A recording watch wrapping a real armed INT/TERM guard: `finish`
/// first waits for the peer's disconnect observation, then runs the real
/// arbitration read — so it only succeeds when the admitted session's
/// socket was already dropped before the finish read.
struct OrderWatch {
    inner: ArmedSignals,
    eof: std::sync::mpsc::Receiver<()>,
}

impl AgentWatch for OrderWatch {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        SignalWatch::poll_fd(&self.inner)
    }

    fn finish(self) -> Result<Finish, crate::cli::source::SignalError> {
        self.eof
            .recv_timeout(Duration::from_secs(5))
            .expect("the admitted session must be dropped before finish");
        self.inner.finish()
    }
}

/// Owner-model evidence for the drop-before-finish order: driving the
/// real `agent_armed_finish` against a live admitted session, the watch
/// observes the peer's EOF at finish entry — the Client Session socket
/// was closed before the signal arbitration read ran.
#[test]
fn session_socket_drops_before_finish_arbitration() {
    use orchbee_agent_observation_protocol::{AgentResult, read_agent_request, write_agent_result};
    use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};

    let (eof, eof_rx) = std::sync::mpsc::channel();
    let server = crate::local_client::test_server::FakeServer::start(move |mut stream| {
            match read_message(&mut stream).map_err(std::io::Error::other)? {
                LocalClientMessage::ClientHello(_) => {}
                _ => {
                    return Err(std::io::Error::other("expected ClientHello"));
                }
            }
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: std::num::NonZeroU64::new(1).expect("generation"),
                },
            )
            .map_err(std::io::Error::other)?;
            let request = read_agent_request(&mut stream).map_err(std::io::Error::other)?;
            let result: AgentReadResult = serde_json::from_value(serde_json::json!({
                "kind": "agent.session.list.result",
                "requestId": request.request_id().as_str(),
                "result": {"outcome": {"kind": "listed", "items": [], "nextCursor": null, "revision": 7}}
            }))
            .map_err(std::io::Error::other)?;
            write_agent_result(&mut stream, &AgentResult::Read(result))
                .map_err(std::io::Error::other)?;
            // Read until the client session closes — EOF is the drop mark.
            let mut sink = [0_u8; 64];
            let _ = std::io::Read::read(&mut stream, &mut sink);
            let _ = eof.send(());
            Ok(())
        })
        .expect("start fake server");
    let client =
        LocalClient::connect(server.endpoint(), "test-cohort").expect("connect admitted client");
    let armed = ArmedSignals::arm_interrupts().expect("arm interrupts");
    let watch = OrderWatch {
        inner: armed,
        eof: eof_rx,
    };
    let request_id = ObapRequestId::parse("11111111111111111111111111111111").expect("request id");
    let outcome = agent_armed_finish(
        AgentReadOperation::List,
        |_, request_id, _, _| {
            Ok(AgentReadRequest::List {
                request_id,
                include_ended: false,
                limit: orchbee_agent_observation_protocol::AgentSafeInteger::new(50),
                cursor: None,
            })
        },
        Instant::now() + Duration::from_secs(10),
        request_id,
        watch,
        move |_total, _wake| Ok(client),
    );
    // The finish read ran only after the peer's EOF — and the cleared
    // span hands back the exchanged result for normal classification.
    let (_id, attempt) =
        outcome.unwrap_or_else(|_| panic!("clear arbitration hands back the attempt"));
    assert!(
        matches!(attempt, AgentAttempt::Exchanged(Ok(_))),
        "the exchange completed before finish"
    );
    server.finish().expect("peer finished");
}

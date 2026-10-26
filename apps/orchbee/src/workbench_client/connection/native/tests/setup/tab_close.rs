use super::super::commands::drive_controlling_ready_from_snapshot;
use super::*;
use crate::workbench_client::mailbox::UiWaitKey;
use crate::workbench_client::state::{
    CloseFlightKey, CloseLeaseAcknowledgement, CloseOperation, CloseOperationKind, CloseReadMode,
    CloseResult, CloseSelection, CloseTarget, PendingCloseResult,
};
use orchbee_workbench_protocol::{
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchRequestId,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePublicationReason,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
};

#[derive(Clone, Copy, Debug)]
enum Family {
    Close,
    Read,
    Resume,
}

const FAMILIES: [Family; 3] = [Family::Close, Family::Read, Family::Resume];

fn canonical_id() -> TestResult<WorkbenchRequestId> {
    Ok(WorkbenchRequestId::parse(
        "000000000000000000000000000000f0",
    )?)
}

fn steps(exchanges: usize) -> Vec<WireKind> {
    let mut result = vec![
        WireKind::Workbench,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
        WireKind::Obcs,
    ];
    result.extend(std::iter::repeat_n(WireKind::Workbench, exchanges));
    result
}

fn ready(harness: &mut Harness) -> TestResult {
    drive_controlling_ready_from_snapshot(harness, snapshot(2, 1), fixture_geometry()?)?;
    Ok(())
}

// This uses actual mailbox/executor transport. Confirmation and canonical Close settlement
// belong to the state/UI owner's tests, just as in the adjacent Move transport fixture.
fn admit(harness: &mut Harness, family: Family) -> TestResult<CloseOperation> {
    let state = harness.ui.state();
    let selected = state.active().ok_or("Close fixture has no active member")?;
    let tab = state
        .tab(selected.index())
        .ok_or("Close fixture member missing")?;
    let selection = CloseSelection::new(
        CloseTarget::new(
            state.cache().site_id().clone(),
            state.cache().terminal_context_id().unwrap().clone(),
            selected.key().tab_id().clone(),
        ),
        tab.tab_label().clone(),
        tab.binding().unwrap().clone(),
        Some(selected.key().anchor().clone().clone()),
        state.cache().revision(),
    );
    let kind = match family {
        Family::Close => CloseOperationKind::Close,
        Family::Read => CloseOperationKind::Read {
            expected_operation: Some(canonical_id()?),
            mode: CloseReadMode::Manual,
        },
        Family::Resume => CloseOperationKind::Resume {
            operation: canonical_id()?,
            generation: WorkbenchTabCloseAttemptGeneration::new(4)?,
        },
    };
    let clock = harness.ui.clock.clone();
    let deadline = clock
        .now()
        .checked_add(Duration::from_secs(50))
        .ok_or("Close cutoff")?;
    harness
        .mailbox
        .try_enqueue_close_for(
            &mut (),
            deadline,
            &clock,
            |_unit, request_id, _admitted_at| {
                Ok::<_, &'static str>(CloseOperation::new(request_id, selection, kind, deadline))
            },
            |_unit, _operation, _admitted_at| Ok(()),
        )
        .map_err(|error| test_error(&format!("Close transport admission: {error:?}")))
}

fn admit_close_set(harness: &mut Harness) -> TestResult<CloseOperation> {
    let clock = harness.ui.clock.clone();
    let deadline = clock.now() + Duration::from_secs(50);
    let state = harness.ui.state_mut();
    let ids = state
        .cache()
        .tabs()
        .iter()
        .map(|tab| tab.tab_id().clone())
        .collect::<Vec<_>>();
    state.open_close_set(&ids).unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    harness
        .mailbox
        .try_enqueue_close_for(
            harness.ui.state_mut(),
            deadline,
            &clock,
            |state, request, _| state.plan_close_confirmation(request, deadline),
            WorkbenchClientState::begin_close_exchange,
        )
        .map_err(|error| test_error(&format!("Close set admission: {error:?}")))
}

#[test]
fn close_set_full_preflight_cannot_reserve_cas_and_revision_race_is_never_retried() -> TestResult {
    for changed_before_read in [false, true] {
        let mut harness = Harness::start(
            steps(if changed_before_read { 1 } else { 2 }),
            fixture_geometry()?,
        )?;
        ready(&mut harness)?;
        let mut source = harness.ui.state().cache().canonical_snapshot().clone();
        let desired = harness.ui.state().desired().cloned();
        let operation = admit_close_set(&mut harness)?;
        let read = harness.carrier.take_read()?;
        if changed_before_read {
            source.revision = WorkbenchRevision::new(2);
        }
        harness.carrier.respond_read(
            &read,
            WorkbenchSnapshotOutcome::Available { snapshot: source },
        )?;
        if !changed_before_read {
            receive(&harness, Family::Close, &operation)?;
            harness
                .carrier
                .respond_workbench(WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: operation.key().request_id().clone(),
                    outcome: WorkbenchCommandOutcome::Rejected {
                        revision: WorkbenchRevision::new(2),
                        reason:
                            orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
                    },
                }))?;
        }
        let (wait, actual, outcome) = take_result(&harness)?;
        assert_eq!(actual, operation);
        if changed_before_read {
            assert_eq!(outcome, CloseResult::NotSubmitted);
        } else {
            assert!(matches!(
                outcome,
                CloseResult::Precommit(
                    crate::local_client::workbench::WorkbenchTabClosePrecommit::Rejected {
                        reason:
                            orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
                        ..
                    }
                )
            ));
        }
        harness
            .ui
            .state_mut()
            .settle_close_exchange(&operation, &outcome)
            .unwrap();
        assert!(harness.ui.state().close_set_status().unwrap().halted);
        assert_eq!(harness.ui.state().desired(), desired.as_ref());
        assert_eq!(harness.ui.state().cache().tabs().len(), 2);
        assert!(!harness.ui.state().close_set_ready());
        settle(&harness, &wait, operation.key())?;
        harness.finish()?;
    }
    Ok(())
}

fn receive(harness: &Harness, family: Family, operation: &CloseOperation) -> TestResult {
    let request = harness.carrier.take_workbench()?;
    let target = operation.key().target();
    match (family, request) {
        (Family::Close, WorkbenchRequest::Command(command)) => {
            assert_eq!(&command.request_id, operation.key().request_id());
            assert_eq!(&command.site_id, target.site_id());
            assert_eq!(command.expected_revision, WorkbenchRevision::new(1));
            assert_eq!(
                command.action,
                WorkbenchAction::TabClose {
                    terminal_context_id: target.terminal_context_id().clone(),
                    tab_id: target.tab_id().clone(),
                }
            );
        }
        (Family::Read, WorkbenchRequest::TabCloseRead(read)) => {
            assert_eq!(&read.request_id, operation.key().request_id());
            assert_eq!(&read.site_id, target.site_id());
            assert_eq!(&read.terminal_context_id, target.terminal_context_id());
            assert_eq!(&read.tab_id, target.tab_id());
        }
        (Family::Resume, WorkbenchRequest::TabCloseResume(resume)) => {
            assert_eq!(&resume.request_id, operation.key().request_id());
            assert_eq!(&resume.site_id, target.site_id());
            assert_eq!(&resume.terminal_context_id, target.terminal_context_id());
            assert_eq!(&resume.tab_id, target.tab_id());
            assert_eq!(resume.close_request_id, canonical_id()?);
            assert_eq!(resume.expected_attempt_generation.value(), 4);
        }
        _ => return Err(test_error("wrong captured Close family request")),
    }
    Ok(())
}

fn response(family: Family, operation: &CloseOperation) -> TestResult<WorkbenchResponse> {
    let target = operation.key().target();
    let request_id = operation.key().request_id().clone();
    Ok(match family {
        Family::Close => WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id,
            outcome: WorkbenchCommandOutcome::TabCloseAccepted {
                close_request_id: canonical_id()?,
                terminal_context_id: target.terminal_context_id().clone(),
                tab_id: target.tab_id().clone(),
                accepted_revision: WorkbenchRevision::new(1),
            },
        }),
        Family::Read => WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
            protocol_version: ProtocolVersion,
            request_id,
            outcome: WorkbenchTabCloseReadOutcome::Available {
                close_request_id: canonical_id()?,
                terminal_context_id: target.terminal_context_id().clone(),
                tab_id: target.tab_id().clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(5)?,
                status: WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(2),
                },
            },
        }),
        Family::Resume => WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
            protocol_version: ProtocolVersion,
            request_id,
            site_id: target.site_id().clone(),
            terminal_context_id: target.terminal_context_id().clone(),
            tab_id: target.tab_id().clone(),
            close_request_id: canonical_id()?,
            outcome: WorkbenchTabCloseResumeOutcome::Resumed {
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(5)?,
            },
        }),
    })
}

fn take_result(harness: &Harness) -> TestResult<(UiWaitKey, CloseOperation, CloseResult)> {
    let (wait, event) = harness.ui.take_next()?.into_parts();
    let WorkerEvent::CloseResult { operation, outcome } = event else {
        return Err(test_error("expected exact typed Close result"));
    };
    Ok((
        wait.ok_or("Close result has no UI correlation")?,
        operation,
        outcome,
    ))
}

fn settle(harness: &Harness, wait: &UiWaitKey, key: &CloseFlightKey) -> TestResult {
    require_applied(harness.mailbox.deposit_ui_reply_before(
        wait,
        &harness.ui.clock,
        |_transaction| Ok(UiReply::Close(key.clone())),
    ))
}

fn retained(harness: Harness, failure: CarrierFailure) -> TestResult<PendingCloseResult> {
    harness
        .finish_after_worker_failure_with_exit(failure)?
        .into_pending_close()
        .ok_or_else(|| test_error("exact Close result was lost at worker exit"))
}

#[test]
fn close_family_captured_exchange_and_typed_receipt_survive_late_ui_consumption() -> TestResult {
    for family in FAMILIES {
        let mut harness = Harness::start(steps(2), fixture_geometry()?)?;
        ready(&mut harness)?;
        let operation = admit(&mut harness, family)?;
        receive(&harness, family, &operation)?;
        harness
            .carrier
            .respond_workbench(response(family, &operation)?)?;
        let (wait, actual, outcome) = take_result(&harness)?;
        assert_eq!(actual, operation);
        assert!(matches!(
            (family, &outcome),
            (Family::Close, CloseResult::Accepted(_))
                | (
                    Family::Read,
                    CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                        status: WorkbenchTabCloseStatus::Closed { .. },
                        ..
                    })
                )
                | (
                    Family::Resume,
                    CloseResult::Resume(WorkbenchTabCloseResumeOutcome::Resumed { .. })
                )
        ));
        harness.elapsed_millis.store(70_000, Ordering::SeqCst);
        harness.mailbox.wake_waiters();
        settle(&harness, &wait, operation.key())?;
        // A fresh explicit observation proves the prior ACK released the executor, without
        // requiring any mutation replay, Snapshot, attachment effect, or automatic scheduler.
        let next = admit(&mut harness, Family::Read)?;
        receive(&harness, Family::Read, &next)?;
        assert_ne!(next.key(), operation.key());
        // Production ticks consume each finished Close lease acknowledgement; this driver
        // deposits replies without ticking, so drain the exact hand-off explicitly or the
        // next close-family finish_lease observes the stale record as a protocol violation.
        let acknowledgement = harness
            .mailbox
            .take_close_acknowledgement()
            .ok_or_else(|| test_error("first Close lease acknowledgement was not handed off"))?;
        let CloseLeaseAcknowledgement::Exchange(key) = &acknowledgement else {
            return Err(test_error(
                "first Close lease acknowledgement was not an exchange",
            ));
        };
        assert_eq!(key, operation.key());
        harness
            .ui
            .state_mut()
            .acknowledge_close_lease(acknowledgement);
        harness
            .carrier
            .respond_workbench(response(Family::Read, &next)?)?;
        let (wait, actual, _) = take_result(&harness)?;
        assert_eq!(actual, next);
        settle(&harness, &wait, next.key())?;
        harness.finish()?;
    }
    Ok(())
}

#[test]
fn close_family_prewrite_expiry_retains_not_submitted_and_sends_no_request() -> TestResult {
    for family in FAMILIES {
        let mut harness = Harness::start_with_prewrite_gate(steps(0), fixture_geometry()?)?;
        ready(&mut harness)?;
        harness.arm_prewrite_gate()?;
        let operation = admit(&mut harness, family)?;
        harness.await_prewrite_blocked()?;
        harness.elapsed_millis.store(50_000, Ordering::SeqCst);
        harness.release_prewrite_gate()?;
        let pending = retained(harness, CarrierFailure::Deadline)?;
        assert_eq!(pending.operation(), &operation);
        assert_eq!(pending.result(), &CloseResult::NotSubmitted);
    }
    Ok(())
}

#[test]
fn close_family_response_after_handoff_cutoff_retires_without_replay() -> TestResult {
    for family in FAMILIES {
        let mut harness = Harness::start(steps(1), fixture_geometry()?)?;
        ready(&mut harness)?;
        let operation = admit(&mut harness, family)?;
        receive(&harness, family, &operation)?;
        harness.elapsed_millis.store(50_000, Ordering::SeqCst);
        harness
            .carrier
            .respond_workbench(response(family, &operation)?)?;
        let failure = if matches!(family, Family::Read) {
            CarrierFailure::Deadline
        } else {
            CarrierFailure::OutcomeUnknown
        };
        let pending = retained(harness, failure)?;
        assert_eq!(pending.operation(), &operation);
        assert_eq!(
            pending.result(),
            &if matches!(family, Family::Read) {
                CloseResult::ObservationUnavailable
            } else {
                CloseResult::Ambiguous
            }
        );
    }
    Ok(())
}

#[test]
fn close_family_wrong_wire_correlation_never_becomes_a_typed_success() -> TestResult {
    for family in FAMILIES {
        let mut harness = Harness::start(steps(1), fixture_geometry()?)?;
        ready(&mut harness)?;
        let operation = admit(&mut harness, family)?;
        receive(&harness, family, &operation)?;
        let mut wrong = response(family, &operation)?;
        let wrong_id = canonical_id()?;
        assert_ne!(&wrong_id, operation.key().request_id());
        match &mut wrong {
            WorkbenchResponse::Command(result) => result.request_id = wrong_id,
            WorkbenchResponse::TabCloseRead(result) => result.request_id = wrong_id,
            WorkbenchResponse::TabCloseResume(result) => result.request_id = wrong_id,
            _ => return Err(test_error("wrong response fixture")),
        }
        harness.carrier.respond_workbench(wrong)?;
        let failure = if matches!(family, Family::Read) {
            CarrierFailure::Protocol
        } else {
            CarrierFailure::OutcomeUnknown
        };
        let pending = retained(harness, failure)?;
        assert_eq!(pending.operation(), &operation);
        assert_eq!(
            pending.result(),
            &if matches!(family, Family::Read) {
                CloseResult::ObservationUnavailable
            } else {
                CloseResult::Ambiguous
            }
        );
    }
    Ok(())
}

#[test]
fn accepted_disposed_and_closed_results_survive_retirement_before_ui_ack() -> TestResult {
    for status in [
        None,
        Some(WorkbenchTabCloseStatus::DisposedAwaitingPublication {
            reason: WorkbenchTabClosePublicationReason::Preparing,
            resumable: false,
        }),
        Some(WorkbenchTabCloseStatus::Closed {
            revision: WorkbenchRevision::new(2),
        }),
    ] {
        let family = if status.is_some() {
            Family::Read
        } else {
            Family::Close
        };
        let mut harness = Harness::start(steps(1), fixture_geometry()?)?;
        ready(&mut harness)?;
        let operation = admit(&mut harness, family)?;
        receive(&harness, family, &operation)?;
        let mut reply = response(family, &operation)?;
        if let (Some(status), WorkbenchResponse::TabCloseRead(read)) = (status, &mut reply)
            && let WorkbenchTabCloseReadOutcome::Available { status: actual, .. } =
                &mut read.outcome
        {
            *actual = status;
        }
        harness.carrier.respond_workbench(reply)?;
        let (_wait, actual, outcome) = take_result(&harness)?;
        assert_eq!(actual, operation);
        harness.mailbox.close(CarrierFailure::Closed);
        harness.shutdown.close()?;
        let pending = retained(harness, CarrierFailure::Closed)?;
        assert_eq!(pending.operation(), &operation);
        assert_eq!(pending.result(), &outcome);
    }
    Ok(())
}

#[test]
fn wrong_ui_flight_ack_preserves_the_original_closed_receipt() -> TestResult {
    let mut harness = Harness::start(steps(1), fixture_geometry()?)?;
    ready(&mut harness)?;
    let operation = admit(&mut harness, Family::Read)?;
    receive(&harness, Family::Read, &operation)?;
    harness
        .carrier
        .respond_workbench(response(Family::Read, &operation)?)?;
    let (wait, actual, outcome) = take_result(&harness)?;
    assert_eq!(actual, operation);
    let wrong = CloseFlightKey::new(canonical_id()?, operation.key().target().clone());
    assert_ne!(&wrong, operation.key());
    let result =
        harness
            .mailbox
            .deposit_ui_reply_before(&wait, &harness.ui.clock, |_transaction| {
                Ok(UiReply::Close(wrong))
            });
    assert!(
        matches!(result, Err(EventError::Closed(closed)) if closed.failure() == CarrierFailure::Protocol)
    );
    let pending = retained(harness, CarrierFailure::Protocol)?;
    assert_eq!(pending.operation(), &operation);
    assert_eq!(pending.result(), &outcome);
    Ok(())
}

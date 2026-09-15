use super::membership_tests::snapshot;
use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::{
    mailbox::{CommandLease, EventDeadline},
    state::{CloseWorkerReturn, retired_membership::UnavailablePoll},
    workbench_projection::project_native_standalone_tabs,
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchSnapshotOutcome};
use std::sync::atomic::{AtomicU64, Ordering};

fn ready(
    elapsed: Arc<AtomicU64>,
) -> (NativeUi, Arc<CommandMailbox>, CommandLease, UnavailablePoll) {
    let clock = adjustable_operation_clock(elapsed);
    let mailbox = Arc::new(CommandMailbox::new(
        800,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                cache: project_native_standalone_tabs(&snapshot(3, 1)).unwrap(),
            },
            EventDeadline::Startup(clock.now() + super::super::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let old = state.desired().unwrap().clone();
    assert!(state.install_projection(&old));
    assert!(state.record_setup_configuration(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    ui.ordinary_poll_serial = Some(
        mailbox
            .try_enqueue(QueuedCommand::Poll(Tagged::new(old, ())))
            .unwrap(),
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let evidence = UnavailablePoll {
        request_id: mailbox.allocate_obcs_request_id().unwrap(),
        poll: lease.poll_key().unwrap(),
    };
    (ui, mailbox, lease, evidence)
}

fn retire(ui: &mut NativeUi, evidence: Option<UnavailablePoll>, clean: bool) {
    ui.retire(CarrierFailure::Protocol);
    ui.finish_retired_peer_join(evidence, clean);
    ui.finalize_retirement(None, None, None, CloseWorkerReturn::Returned(None));
    ui.release_mailbox();
}

fn removed(count: usize) -> Snapshot {
    let mut value = snapshot(count, 2);
    if count != 0 {
        value.terminal_contexts[0].tabs.remove(0);
    }
    value
}

#[test]
fn stale_poll_close_confirmation_structural_flight_and_missing_return_never_grant_recovery() {
    for conflict in 0..5 {
        let (mut ui, mailbox, _lease, evidence) = ready(Arc::new(AtomicU64::new(0)));
        let state = ui.state.as_mut().unwrap();
        match conflict {
            0 => {
                state.open_close_confirmation().unwrap();
            }
            1 => {
                state.open_rename_draft().unwrap();
            }
            2 => {
                let deadline = ui.clock.now() + super::super::state::OPERATION_TOTAL;
                mailbox
                    .try_enqueue_refresh_for(
                        state,
                        deadline,
                        |state, generation| {
                            state
                                .start_pending_refresh(generation)
                                .then_some(())
                                .ok_or(())
                        },
                        |_, _| {},
                    )
                    .unwrap();
            }
            3 => {
                ui.ordinary_poll_serial = Some(evidence.poll.serial() + 1);
            }
            _ => {}
        }
        retire(&mut ui, Some(evidence), conflict != 4);
        assert!(
            ui.begin_retired_membership_recovery().is_none(),
            "conflict {conflict}"
        );
        assert!(ui.mailbox.is_none());
    }
}

#[test]
fn recovery_checks_original_forward_deadline_and_installs_empty_or_one_successor_atomically() {
    for count in [0, 3] {
        for offset in [0, 44_999, 45_000, 49_999, 50_000] {
            let elapsed = Arc::new(AtomicU64::new(0));
            let (mut ui, _old, _lease, evidence) = ready(Arc::clone(&elapsed));
            let old_key = evidence.poll.attachment().clone();
            retire(&mut ui, Some(evidence), true);
            let baseline = ui.state.as_ref().unwrap().cache().clone();
            let (mailbox, clock, cutoff) = ui.begin_retired_membership_recovery().unwrap();
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::Refresh(marker) = lease.record() else {
                panic!("Snapshot flight");
            };
            let generation = marker.generation();
            assert_eq!(
                mailbox.mark_refresh_in_flight(generation).unwrap(),
                MutationResult::Applied
            );
            let wait = mailbox
                .publish_refresh_result_before(
                    generation,
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: removed(count),
                    },
                    cutoff,
                    &clock,
                )
                .unwrap();
            elapsed.store(offset, Ordering::SeqCst);
            ui.tick();
            if offset < 45_000 {
                let UiReply::Refresh(reply) = mailbox.take_deposited_reply(&wait).unwrap().unwrap()
                else {
                    panic!("membership settlement");
                };
                let state = ui.state.as_ref().unwrap();
                assert_eq!(
                    state.cache().revision(),
                    orchbee_workbench_protocol::WorkbenchRevision::new(2)
                );
                if count == 0 {
                    assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
                    assert!(reply.desired().is_none());
                    assert!(state.desired().is_none());
                } else {
                    let RefreshDecision::AttachRecovered(target) = reply.decision() else {
                        panic!("neighbor Attach");
                    };
                    assert_eq!(
                        target.selected().key().generation(),
                        old_key.generation() + 1
                    );
                    assert_eq!(state.desired(), Some(target.selected().key()));
                    assert_eq!(
                        target.deadlines().forward_cutoff(),
                        cutoff,
                        "UI wait cannot renew setup"
                    );
                }
                mailbox.finish_lease(lease).unwrap();
                assert!(
                    ui.retired_peer.is_none(),
                    "committed candidate cannot restore the old origin"
                );
            } else {
                assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
                assert!(ui.retired_peer.is_some());
                assert!(mailbox.execution_fence(&lease).desired().key().is_none());
            }
        }
    }
}

#[test]
fn unavailable_present_and_invalid_candidates_preserve_origin_for_fresh_explicit_retry() {
    for kind in 0..4 {
        let (mut ui, _old, _lease, evidence) = ready(Arc::new(AtomicU64::new(0)));
        retire(&mut ui, Some(evidence), true);
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let (mailbox, clock, deadline) = ui.begin_retired_membership_recovery().unwrap();
        assert!(
            ui.begin_retired_membership_recovery().is_none(),
            "single active attempt"
        );
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Refresh(marker) = lease.record() else {
            panic!("Refresh");
        };
        let first = marker.generation();
        let outcome = match kind {
            0 => WorkbenchSnapshotOutcome::Denied {},
            1 => WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(3, 2),
            },
            2 => {
                let mut value = removed(3);
                value.site_id =
                    orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", 99)).unwrap();
                WorkbenchSnapshotOutcome::Available { snapshot: value }
            }
            _ => {
                let mut value = removed(3);
                value.revision = orchbee_workbench_protocol::WorkbenchRevision::new(1);
                WorkbenchSnapshotOutcome::Available { snapshot: value }
            }
        };
        mailbox.mark_refresh_in_flight(first).unwrap();
        mailbox
            .publish_refresh_result_before(first, outcome, deadline, &clock)
            .unwrap();
        ui.tick();
        assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
        assert!(mailbox.execution_fence(&lease).desired().key().is_none());
        ui.finish_retired_peer_join(None, true);
        ui.finalize_retirement(None, None, None, CloseWorkerReturn::Returned(None));
        ui.release_mailbox();
        let (retry, clock, deadline) = ui.begin_retired_membership_recovery().unwrap();
        let retry_lease = retry.try_recv().unwrap().unwrap();
        let QueuedCommand::Refresh(marker) = retry_lease.record() else {
            panic!("retry Refresh");
        };
        assert!(marker.generation() > first);
        retry.mark_refresh_in_flight(marker.generation()).unwrap();
        let wait = retry
            .publish_refresh_result_before(
                marker.generation(),
                WorkbenchSnapshotOutcome::Available {
                    snapshot: removed(0),
                },
                deadline,
                &clock,
            )
            .unwrap();
        ui.tick();
        assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
        retry.take_deposited_reply(&wait).unwrap().unwrap();
        retry.finish_lease(retry_lease).unwrap();
    }
}

#[test]
fn cancelled_old_reply_cannot_install_into_a_later_explicit_attempt() {
    let (mut ui, _old, _lease, evidence) = ready(Arc::new(AtomicU64::new(0)));
    retire(&mut ui, Some(evidence), true);
    let baseline = ui.state.as_ref().unwrap().cache().clone();
    let (first, clock, deadline) = ui.begin_retired_membership_recovery().unwrap();
    let lease = first.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("first Refresh");
    };
    first.mark_refresh_in_flight(marker.generation()).unwrap();
    first
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available {
                snapshot: removed(0),
            },
            deadline,
            &clock,
        )
        .unwrap();
    let stale = first.take_event_before(&clock).unwrap().unwrap();
    ui.retire(CarrierFailure::Closed);
    ui.finish_retired_peer_join(None, true);
    ui.finalize_retirement(None, None, None, CloseWorkerReturn::Returned(None));
    ui.release_mailbox();
    let (second, _, _) = ui.begin_retired_membership_recovery().unwrap();
    let cursor = second.cursors();
    assert!(!ui.apply_event(&first, stale));
    assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
    assert_eq!(second.cursors(), cursor);
    assert!(ui.retired_peer_attempt.is_some());
    ui.retire(CarrierFailure::Closed);
}

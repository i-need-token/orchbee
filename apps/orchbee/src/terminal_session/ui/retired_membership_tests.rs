use super::membership_tests::snapshot;
use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_session::{
    mailbox::{CommandLease, EventDeadline},
    state::{CloseWorkerReturn, retired_membership::UnavailablePoll},
    workbench_projection::project_native_standalone_tabs,
};
use orchbee_client_server_protocol::ControllerId;
use orchbee_workbench_protocol::{
    PanelNode, Snapshot, WorkbenchSnapshotOutcome, WorkbenchTerminalContextId,
};
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
    assert!(state.install_projection_t(&old));
    assert!(state.record_setup_configuration_t(&old, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&old, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    let old_ref = state.pane_ref_for(&old).unwrap();
    ui.ordinary_poll_serial = Some(
        mailbox
            .try_enqueue(QueuedCommand::Poll(Tagged::new(old_ref, ())))
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

/// The origin's whole terminal context is absent: navigation names a
/// different surviving standalone context whose tabs never reuse the removed
/// context's identities.
fn context_removed(count: usize) -> Snapshot {
    let mut value = snapshot(count.max(1), 2);
    let landing = WorkbenchTerminalContextId::parse(&format!("{:032x}", 9)).unwrap();
    value.navigation.standalone_terminal_context_id = landing.clone();
    let context = &mut value.terminal_contexts[0];
    context.terminal_context_id = landing;
    for (index, tab) in context.tabs.iter_mut().enumerate() {
        let seed = 5_000 + index * 10;
        tab.tab_id = WorkbenchTabId::parse(&format!("{seed:032x}")).unwrap();
        let PanelNode::Panel { panel_id, .. } = &mut tab.root else {
            unreachable!()
        };
        *panel_id = WorkbenchPanelId::parse(&format!("{:032x}", seed + 1)).unwrap();
    }
    value.terminal_contexts[0].tabs.truncate(count);
    value
}

#[test]
fn removed_origin_context_lands_on_the_surviving_context_and_consumes_the_permit() {
    for count in [0, 2] {
        let elapsed = Arc::new(AtomicU64::new(0));
        let (mut ui, _old, _lease, evidence) = ready(Arc::clone(&elapsed));
        let old_key = evidence.poll.attachment().clone();
        retire(&mut ui, Some(evidence), true);
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
                    snapshot: context_removed(count),
                },
                cutoff,
                &clock,
            )
            .unwrap();
        ui.tick();
        let UiReply::Refresh(reply) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
            panic!("membership settlement")
        };
        let state = ui.state.as_ref().unwrap();
        assert_eq!(
            state.cache().terminal_context_id(),
            &WorkbenchTerminalContextId::parse(&format!("{:032x}", 9)).unwrap()
        );
        if count == 0 {
            assert_eq!(ui.view().phase(), NativeDisplayPhase::Empty);
            assert!(state.desired().is_none());
        } else {
            let RefreshDecision::AttachRecovered(target) = reply.decision() else {
                panic!("landing attach")
            };
            assert_eq!(
                target.selected().key().generation(),
                old_key.generation() + 1
            );
            assert_eq!(state.desired(), Some(target.selected().key()));
        }
        // The permit is spent inside this settlement: the attempt marker is
        // gone, the consumed origin never re-arms, and the removed context
        // stays absent rather than parking back as retained evidence.
        assert!(ui.retired_peer_attempt.is_none());
        assert!(ui.retired_peer.is_none());
        assert!(ui.retirement_failure.is_none());
        mailbox.finish_lease(lease).unwrap();
        ui.retire(CarrierFailure::Closed);
    }
}

#[test]
fn malformed_removal_snapshot_parks_the_attempt_truthfully() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, _old, _lease, evidence) = ready(Arc::clone(&elapsed));
    retire(&mut ui, Some(evidence), true);
    let baseline = ui.state.as_ref().unwrap().cache().clone();
    let (mailbox, clock, cutoff) = ui.begin_retired_membership_recovery().unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("Snapshot flight");
    };
    mailbox.mark_refresh_in_flight(marker.generation()).unwrap();
    // Navigation still names the removed context as standalone while the
    // context payload is gone — never a permit, never a landing.
    let mut broken = context_removed(1);
    broken.navigation.standalone_terminal_context_id =
        WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
    mailbox
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available { snapshot: broken },
            cutoff,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert_eq!(
        ui.retirement_failure,
        Some(CarrierFailure::AuthorityDivergence)
    );
    assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
    assert!(ui.retired_peer.is_some());
    mailbox.finish_lease(lease).unwrap();
    ui.retire(CarrierFailure::Closed);
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
        if conflict <= 1 {
            // Retirement drops the armed capture outright — nothing pending
            // survives to block membership recovery.
            let state = ui.state.as_ref().unwrap();
            assert!(match conflict {
                0 => state.view().close_confirmation().is_none(),
                _ => !state.rename_draft_blocks_structure(),
            });
            assert!(
                ui.begin_retired_membership_recovery().is_some(),
                "conflict {conflict}"
            );
        } else {
            assert!(
                ui.begin_retired_membership_recovery().is_none(),
                "conflict {conflict}"
            );
            assert!(ui.mailbox.is_none());
        }
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

fn ready_controlling(
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
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let mode = ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 8)).unwrap());
    let state = ui.state.as_mut().unwrap();
    let old = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&old));
    assert!(state.record_setup_configuration_t(&old, mode.clone(), Some(geometry)));
    assert!(state.stage_drawn_frame_t(&old, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&old, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&old, FrameIdentity::new(1), mode, Some(geometry)));
    let old_ref = state.pane_ref_for(&old).unwrap();
    ui.ordinary_poll_serial = Some(
        mailbox
            .try_enqueue(QueuedCommand::Poll(Tagged::new(old_ref, ())))
            .unwrap(),
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let evidence = UnavailablePoll {
        request_id: mailbox.allocate_obcs_request_id().unwrap(),
        poll: lease.poll_key().unwrap(),
    };
    (ui, mailbox, lease, evidence)
}

#[test]
fn armed_attempt_suppresses_tick_scheduling_and_keeps_refresh_only_queue() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, _old, _lease, evidence) = ready(Arc::clone(&elapsed));
    retire(&mut ui, Some(evidence), true);
    let panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    let (mailbox, _clock, _cutoff) = ui.begin_retired_membership_recovery().unwrap();
    // The retained pane set diverges from live attachments exactly like the
    // close witness: the old anchor owes a reconcile episode whose anchor the
    // recovery mailbox must reject, not see.
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    assert!(
        ui.state
            .as_mut()
            .unwrap()
            .sync_pane_set(&[(panel, 0, geometry)], ui.clock.now())
    );
    assert!(ui.state.as_ref().unwrap().reconcile_pending());
    // Repeated ticks inside the armed window admit no ordinary work and do
    // not retire on the stale-anchor reconcile.
    ui.tick();
    ui.tick();
    assert!(ui.retirement_failure.is_none());
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Refresh(_)));
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.retire(CarrierFailure::Closed);
}

#[test]
fn armed_attempt_suppresses_observed_geometry_and_pane_reconcile_admission() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, _old, _lease, evidence) = ready_controlling(Arc::clone(&elapsed));
    let panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    retire(&mut ui, Some(evidence), true);
    let (mailbox, _clock, _cutoff) = ui.begin_retired_membership_recovery().unwrap();
    let moved = Geometry::new(100, 30, 8, 16).unwrap();
    let resized = Geometry::new(90, 28, 8, 16).unwrap();
    // Redraw-driven inputs inside the armed window mint nothing: the pane
    // set is canonical state, not bookkeeping — a minted record would break
    // the install-time origin invariant, and an admitted Resize or reconcile
    // would collide with the Refresh-only queue.
    ui.observe_geometry(moved);
    ui.observe_panes(&[(panel.clone(), 0, resized)]);
    ui.observe_panes(&[]);
    assert!(ui.retirement_failure.is_none());
    assert!(ui.state.as_ref().unwrap().pane_by_panel(&panel).is_none());
    assert!(!ui.state.as_ref().unwrap().reconcile_pending());
    ui.tick();
    assert!(ui.retirement_failure.is_none());
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Refresh(_)));
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.retire(CarrierFailure::Closed);
}

#[test]
fn armed_attempt_expired_wait_and_failed_refresh_settle_truthfully_without_zombies() {
    for expired in [true, false] {
        let elapsed = Arc::new(AtomicU64::new(0));
        let (mut ui, _old, _lease, evidence) = ready(Arc::clone(&elapsed));
        retire(&mut ui, Some(evidence), true);
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let (mailbox, clock, cutoff) = ui.begin_retired_membership_recovery().unwrap();
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Refresh(marker) = lease.record() else {
            panic!("Refresh");
        };
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap();
        if expired {
            elapsed.store(50_000, Ordering::SeqCst);
            // No result ever arrives: repeated waiting ticks admit no ordinary
            // work; the worker-side deadline then closes the carrier and the
            // UI retires truthfully with the delivered failure — no zombie
            // admission behind the closed queue.
            ui.tick();
            ui.tick();
            assert!(ui.retirement_failure.is_none());
            assert!(mailbox.try_recv().unwrap().is_none());
            mailbox.close(CarrierFailure::Deadline);
            ui.tick();
            assert_eq!(ui.retirement_failure, Some(CarrierFailure::Deadline));
            ui.tick();
        } else {
            // A late, failed result still settles truthfully: the attempt's
            // own cutoff makes the install deposit a Retire decision, the
            // carrier reports its failure, and nothing ordinary was admitted.
            mailbox
                .publish_refresh_result_before(
                    marker.generation(),
                    WorkbenchSnapshotOutcome::Denied {},
                    cutoff,
                    &clock,
                )
                .unwrap();
            elapsed.store(50_000, Ordering::SeqCst);
            ui.tick();
            assert_eq!(
                ui.retirement_failure,
                Some(CarrierFailure::AuthorityDivergence)
            );
            ui.tick();
        }
        assert!(mailbox.try_recv().is_err());
        assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
        assert!(ui.retired_peer.is_some());
        ui.retire(CarrierFailure::Closed);
    }
}

#[test]
fn installed_survivor_clears_marker_and_resumes_ordinary_admission() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, _old, _lease, evidence) = ready(Arc::clone(&elapsed));
    retire(&mut ui, Some(evidence), true);
    let (mailbox, clock, cutoff) = ui.begin_retired_membership_recovery().unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("Refresh");
    };
    let generation = marker.generation();
    mailbox.mark_refresh_in_flight(generation).unwrap();
    mailbox
        .publish_refresh_result_before(
            generation,
            WorkbenchSnapshotOutcome::Available {
                snapshot: removed(3),
            },
            cutoff,
            &clock,
        )
        .unwrap();
    ui.tick();
    // The atomic install cleared the attempt marker inside this same tick:
    // the retained origin is consumed, the desired anchor is the survivor's,
    // and the untouched canonical pane set kept the origin check exact.
    assert!(ui.retired_peer_attempt.is_none());
    assert!(ui.retirement_failure.is_none());
    assert!(ui.retired_peer.is_none());
    let anchor = ui
        .state
        .as_ref()
        .unwrap()
        .desired()
        .unwrap()
        .anchor()
        .clone();
    // The gate released with the marker: the next ordinary input admits
    // against the new epoch — a fresh leaf report mints its record and the
    // reconcile enqueue targets the new anchor, never the retired one.
    let fresh = WorkbenchPanelId::parse(&format!("{:032x}", 9_001)).unwrap();
    ui.observe_panes(&[(fresh, 0, Geometry::new(80, 24, 8, 16).unwrap())]);
    // The Refresh lease must settle before further commands can lease out.
    mailbox.finish_lease(lease).unwrap();
    // A full queue can defer the first enqueue attempt by one tick; drive a
    // few ordinary ticks while draining so the retry lands.
    let mut reconciled = false;
    'admit: for _ in 0..12 {
        ui.tick();
        while let Ok(Some(next)) = mailbox.try_recv() {
            let hit = matches!(next.record(), QueuedCommand::ReconcilePanes(marker) if marker.anchor() == &anchor);
            let is_reconcile = matches!(next.record(), QueuedCommand::ReconcilePanes(_));
            mailbox.finish_lease(next).unwrap();
            if is_reconcile {
                assert!(hit, "reconcile must target the installed anchor");
                reconciled = true;
                break 'admit;
            }
        }
        assert!(ui.retirement_failure.is_none());
    }
    assert!(
        reconciled,
        "post-install admission must resume for the new anchor"
    );
    ui.retire(CarrierFailure::Closed);
}

#[test]
fn empty_install_stays_inert_and_ready_control_admits_pending_reconcile() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, _old, _lease, evidence) = ready(Arc::clone(&elapsed));
    retire(&mut ui, Some(evidence), true);
    let (mailbox, clock, cutoff) = ui.begin_retired_membership_recovery().unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("Refresh");
    };
    mailbox.mark_refresh_in_flight(marker.generation()).unwrap();
    mailbox
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available {
                snapshot: removed(0),
            },
            cutoff,
            &clock,
        )
        .unwrap();
    ui.tick();
    // Empty installs desired=None: every ordinary scheduler is inert even
    // though the attempt marker is gone.
    assert!(ui.retired_peer_attempt.is_none());
    assert!(ui.state.as_ref().unwrap().desired().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
    mailbox.finish_lease(lease).unwrap();
    ui.retire(CarrierFailure::Closed);

    // Ready positive control: the same pending divergence on an ordinary
    // mailbox admits the reconcile enqueue immediately.
    let (mut control, control_mailbox, control_lease, _control_evidence) =
        ready(Arc::new(AtomicU64::new(0)));
    control_mailbox.finish_lease(control_lease).unwrap();
    assert!(
        control
            .state
            .as_mut()
            .unwrap()
            .sync_pane_set(&[], control.clock.now())
    );
    let mut admitted = false;
    'admit: for _ in 0..12 {
        control.tick();
        while let Ok(Some(next)) = control_mailbox.try_recv() {
            let is_reconcile = matches!(next.record(), QueuedCommand::ReconcilePanes(_));
            control_mailbox.finish_lease(next).unwrap();
            if is_reconcile {
                admitted = true;
                break 'admit;
            }
        }
        assert!(control.retirement_failure.is_none());
    }
    assert!(admitted, "Ready session must admit the pending reconcile");
    control.retire(CarrierFailure::Closed);
}

#[test]
fn status_only_carrier_suppresses_ordinary_reconcile_until_install() {
    use crate::local_client::workbench::WorkbenchTabCloseAccepted;
    use crate::terminal_session::state::CloseResult;
    use orchbee_workbench_protocol::{
        WorkbenchRequestId, WorkbenchRevision, WorkbenchTabCloseAttemptGeneration,
        WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus,
    };
    use std::time::Duration;

    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, _session_mailbox, _lease, evidence) = ready(Arc::clone(&elapsed));
    // A real retained close: confirmed intent, accepted exchange, Closed read.
    let (closed_tab, close_request_id) = {
        let state = ui.state.as_mut().unwrap();
        state.open_close_confirmation().unwrap();
        let token = state.view().close_confirmation().unwrap().token();
        state.acknowledge_close_draw(Some(token), None);
        let now = ui.clock.now();
        let close = state
            .plan_close_confirmation(
                WorkbenchRequestId::parse(&format!("{:032x}", 901)).unwrap(),
                now + Duration::from_secs(50),
            )
            .unwrap();
        state.begin_close_exchange(&close, now).unwrap();
        state
            .settle_close_exchange(
                &close,
                &CloseResult::Accepted(WorkbenchTabCloseAccepted {
                    close_request_id: close.key().request_id().clone(),
                    terminal_context_id: close.key().target().terminal_context_id().clone(),
                    tab_id: close.key().target().tab_id().clone(),
                    accepted_revision: close.selection().base_revision(),
                }),
            )
            .unwrap();
        let read = state
            .plan_close_read(
                WorkbenchRequestId::parse(&format!("{:032x}", 902)).unwrap(),
                now + Duration::from_secs(50),
                now,
            )
            .unwrap();
        state.begin_close_exchange(&read, now).unwrap();
        state
            .settle_close_exchange(
                &read,
                &CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                    close_request_id: close.key().request_id().clone(),
                    terminal_context_id: close.key().target().terminal_context_id().clone(),
                    tab_id: close.key().target().tab_id().clone(),
                    attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                    status: WorkbenchTabCloseStatus::Closed {
                        revision: WorkbenchRevision::new(2),
                    },
                }),
            )
            .unwrap();
        (
            close.key().target().tab_id().clone(),
            close.key().request_id().clone(),
        )
    };
    retire(&mut ui, Some(evidence), true);
    let (mailbox, clock) = ui.begin_status_recovery().expect("status-only carrier");
    assert!(ui.state.as_ref().unwrap().close_status_only());

    // A redraw inside the status-only window reports a real leaf of the
    // retained anchor: the carrier predicate keeps the observation outside
    // the canonical pane set — no record mint, no pending divergence, no
    // ordinary command — the exact input that previously walked into
    // StaleAttachment → Protocol and poisoned the Retired-phase shape.
    let leaf = ui.state.as_ref().unwrap().cache().tabs()[0]
        .panel(0)
        .unwrap()
        .panel_id()
        .clone();
    ui.observe_panes(&[(leaf.clone(), 0, Geometry::new(80, 24, 8, 16).unwrap())]);
    assert!(!ui.state.as_ref().unwrap().reconcile_pending());
    assert_eq!(ui.state.as_ref().unwrap().pane_facts().count(), 0);
    // The manual pass is throttled by the ordinary read interval; advance the
    // clock so the carrier's own Read becomes schedulable.
    elapsed.store(1_000, Ordering::Relaxed);
    ui.tick();
    assert!(ui.retirement_failure.is_none());

    // Legitimate close scheduling stays live: the fresh carrier admits the
    // status Read while the ordinary ReconcilePanes never appears.
    let mut read_lease = None;
    'read: for _ in 0..8 {
        ui.tick();
        while let Ok(Some(next)) = mailbox.try_recv() {
            assert!(
                !matches!(next.record(), QueuedCommand::ReconcilePanes(_)),
                "status-only must not admit the ordinary reconcile"
            );
            if matches!(next.record(), QueuedCommand::Close(_)) {
                read_lease = Some(next);
                break 'read;
            }
            mailbox.finish_lease(next).unwrap();
        }
        assert!(ui.retirement_failure.is_none());
    }
    let read_lease = read_lease.expect("the status Read must remain admissible");
    let QueuedCommand::Close(marker) = read_lease.record() else {
        unreachable!()
    };
    let operation = marker.operation().clone();
    assert_eq!(
        mailbox.mark_close_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_close_result_before(
            operation.clone(),
            CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id,
                terminal_context_id: operation.key().target().terminal_context_id().clone(),
                tab_id: closed_tab.clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                status: WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(2),
                },
            }),
            operation.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.finish_lease(read_lease).unwrap();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    assert!(
        ui.retirement_failure.is_none(),
        "the carrier's own read settlement must survive untouched pane facts"
    );
    assert_eq!(ui.state.as_ref().unwrap().pane_facts().count(), 0);

    // MembershipRecoveryPending stays quiet the same way: the carrier swap
    // keeps the funnel closed while its own Snapshot admission stays live.
    let token = ui
        .state
        .as_ref()
        .unwrap()
        .view()
        .close_status()
        .unwrap()
        .token();
    ui.state
        .as_mut()
        .unwrap()
        .acknowledge_close_draw(None, Some(token));
    ui.state
        .as_mut()
        .unwrap()
        .request_close_membership_recovery()
        .unwrap();
    assert!(ui.state.as_ref().unwrap().close_status_only());
    ui.observe_panes(&[(leaf, 0, Geometry::new(80, 24, 8, 16).unwrap())]);
    assert!(
        !ui.state.as_ref().unwrap().reconcile_pending(),
        "MembershipRecoveryPending keeps observations outside the pane set"
    );
    let mut reconciliation_lease = None;
    'reconcile: for _ in 0..8 {
        ui.tick();
        while let Ok(Some(next)) = mailbox.try_recv() {
            assert!(
                !matches!(next.record(), QueuedCommand::ReconcilePanes(_)),
                "MembershipRecoveryPending must not admit the ordinary reconcile"
            );
            if matches!(next.record(), QueuedCommand::ReconcileClose(_)) {
                reconciliation_lease = Some(next);
                break 'reconcile;
            }
            mailbox.finish_lease(next).unwrap();
        }
        assert!(ui.retirement_failure.is_none());
    }
    let reconciliation_lease =
        reconciliation_lease.expect("the membership Snapshot must remain admissible");
    let QueuedCommand::ReconcileClose(marker) = reconciliation_lease.record() else {
        unreachable!()
    };
    assert!(marker.membership_recovery());
    let marker = marker.clone();
    assert_eq!(
        mailbox
            .mark_close_reconciliation_in_flight(&marker)
            .unwrap(),
        MutationResult::Applied
    );
    let mut snapshot = snapshot(3, 2);
    snapshot.terminal_contexts[0]
        .tabs
        .retain(|tab| tab.tab_id != closed_tab);
    let wait = mailbox
        .publish_close_reconciliation_before(
            marker.clone(),
            WorkbenchSnapshotOutcome::Available { snapshot },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.finish_lease(reconciliation_lease).unwrap();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    assert!(ui.retirement_failure.is_none());
    assert!(
        !ui.state.as_ref().unwrap().close_status_only(),
        "the atomic consumption commit restores the session carrier"
    );
    let anchor = ui
        .state
        .as_ref()
        .unwrap()
        .desired()
        .unwrap()
        .anchor()
        .clone();

    // The carrier predicate released with the consumption commit: the next
    // ordinary input admits against the installed anchor again.
    let fresh = WorkbenchPanelId::parse(&format!("{:032x}", 9_777)).unwrap();
    ui.observe_panes(&[(fresh, 0, Geometry::new(80, 24, 8, 16).unwrap())]);
    let mut resumed = false;
    'resume: for _ in 0..12 {
        ui.tick();
        while let Ok(Some(next)) = mailbox.try_recv() {
            let is_reconcile = matches!(next.record(), QueuedCommand::ReconcilePanes(_));
            if is_reconcile {
                let QueuedCommand::ReconcilePanes(marker) = next.record() else {
                    unreachable!()
                };
                assert_eq!(marker.anchor(), &anchor);
            }
            mailbox.finish_lease(next).unwrap();
            if is_reconcile {
                resumed = true;
                break 'resume;
            }
        }
        assert!(ui.retirement_failure.is_none());
    }
    assert!(
        resumed,
        "ordinary admission must resume after the atomic install"
    );
    ui.retire(CarrierFailure::Closed);
}

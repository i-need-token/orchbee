//! Frozen-set lifecycle traces exercise the ordinary Close and membership commit owners.
use super::*;
use crate::workbench_client::state::{
    CloseLeaseAcknowledgement, CloseOperationKind, CloseSetKind, ControllerMode, FrameIdentity,
    MembershipInstallDecision, OperationDeadlines, close_set_targets,
};
use orchbee_workbench_protocol::{
    Snapshot, WorkbenchTabCloseReadOutcome, WorkbenchTabGroupId, WorkbenchTabId,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    name: String,
    tabs: Vec<TabInput>,
    selected: Vec<String>,
    target: String,
    expected: Expected,
}
#[derive(Deserialize)]
struct TabInput {
    id: String,
    pinned: bool,
    group: Option<String>,
}
#[derive(Deserialize)]
struct Expected {
    selected: Vec<String>,
    other: Vec<String>,
    trailing: Vec<String>,
    group: Vec<String>,
}
fn id(value: &str) -> WorkbenchTabId {
    WorkbenchTabId::parse(&format!("{:032x}", value.parse::<u128>().unwrap())).unwrap()
}

#[test]
fn close_set_target_corpus_matches_actual_gui_menu_owner() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/tab-strip/src/tab-strip/model/__tests__/fixtures/tab-close-targets.json"
    )))
    .unwrap();
    for case in cases {
        let mut source = snapshot(case.tabs.len(), 1, None);
        let tabs = &mut source.terminal_contexts[0].tabs;
        for (tab, input) in tabs.iter_mut().zip(&case.tabs) {
            tab.tab_id = id(&input.id);
            tab.pinned = input.pinned;
            tab.group_id = input.group.as_ref().map(|value| {
                WorkbenchTabGroupId::parse(&format!(
                    "{:032x}",
                    1000 + value.parse::<u128>().unwrap()
                ))
                .unwrap()
            });
        }
        let selected: Vec<_> = case.selected.iter().map(|value| id(value)).collect();
        for (kind, expected) in [
            (CloseSetKind::Selected, case.expected.selected),
            (CloseSetKind::Other, case.expected.other),
            (CloseSetKind::Trailing, case.expected.trailing),
            (CloseSetKind::Group, case.expected.group),
        ] {
            assert_eq!(
                close_set_targets(tabs, &selected, &id(&case.target), kind),
                expected.iter().map(|value| id(value)).collect::<Vec<_>>(),
                "{} {kind:?}",
                case.name
            );
        }
    }
}

fn open(state: &mut WorkbenchClientState, indexes: &[usize], now: Instant) -> CloseOperation {
    let ids: Vec<_> = indexes
        .iter()
        .map(|index| state.cache().tabs()[*index].tab_id().clone())
        .collect();
    state.open_close_set(&ids).unwrap();
    assert_eq!(
        state.plan_close_confirmation(workbench_request_id(8000), now + Duration::from_secs(50)),
        Err(CloseStateError::NotDisplayed)
    );
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let operation = state
        .plan_close_confirmation(workbench_request_id(8000), now + Duration::from_secs(50))
        .unwrap();
    state.begin_close_exchange(&operation, now).unwrap();
    operation
}

fn observe_closed(
    state: &mut WorkbenchClientState,
    close: &CloseOperation,
    now: Instant,
) -> CloseOperation {
    // Accepted can name an already admitted primary. The request ID need not be that identity.
    let alias = workbench_request_id(8888);
    assert_ne!(close.key().request_id(), &alias);
    state
        .settle_close_exchange(
            close,
            &CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: alias.clone(),
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                accepted_revision: close.selection().base_revision(),
            }),
        )
        .unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(close.key().clone()));
    let read = state
        .plan_close_read(
            workbench_request_id(8001 + u128::from(state.cache().revision().value())),
            state
                .close_set_deadline()
                .unwrap_or(now + Duration::from_secs(50)),
            now,
        )
        .unwrap();
    assert!(
        matches!(read.kind(), CloseOperationKind::Read { expected_operation: Some(known), .. } if known == &alias)
    );
    state.begin_close_exchange(&read, now).unwrap();
    state
        .settle_close_exchange(
            &read,
            &CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id: alias,
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                status: WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(state.cache().revision().value() + 1),
                },
            }),
        )
        .unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(read.key().clone()));
    read
}

fn removed(state: &WorkbenchClientState, tab: &WorkbenchTabId) -> Snapshot {
    let mut source = state.cache().canonical_snapshot().clone();
    source.revision = WorkbenchRevision::new(source.revision.value() + 1);
    let context = &mut source.terminal_contexts[0];
    let group = context
        .tabs
        .iter()
        .find(|member| &member.tab_id == tab)
        .unwrap()
        .group_id
        .clone();
    context.tabs.retain(|member| &member.tab_id != tab);
    context.groups.retain(|candidate| {
        Some(&candidate.group_id) != group.as_ref()
            || context
                .tabs
                .iter()
                .any(|member| member.group_id.as_ref() == Some(&candidate.group_id))
    });
    source
}

fn install(
    state: &mut WorkbenchClientState,
    close: &CloseOperation,
    source: Snapshot,
    now: Instant,
) {
    let marker = state
        .plan_close_reconciliation(
            workbench_request_id(9000 + source.revision.value() as u128),
            close.deadline(),
        )
        .unwrap();
    state.begin_close_reconciliation(&marker).unwrap();
    let prepared = state
        .prepare_close_membership_candidate(
            &marker,
            WorkbenchSnapshotOutcome::Available { snapshot: source },
        )
        .unwrap()
        .unwrap();
    let decision = if prepared.preserves_selection() {
        state
            .prepare_close_membership_install(
                &marker,
                prepared,
                state.desired().map(|key| key.anchor().clone()),
                None,
            )
            .unwrap()
            .commit()
    } else {
        let fresh = prepared
            .selected_tab_id()
            .map(|tab| AttachmentKey::new(100 + state.cache().revision().value(), tab.clone()));
        let cleanup = state
            .stage_close_membership_cleanup(
                &marker,
                prepared,
                OperationDeadlines::from_sample(now).unwrap(),
            )
            .unwrap();
        let candidate = state
            .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
            .unwrap();
        state
            .prepare_cleaned_membership_install(&cleanup, candidate, fresh)
            .unwrap()
            .commit()
    };
    if let MembershipInstallDecision::Attach(target) = decision {
        let key = target.selected().key().clone();
        assert!(state.install_projection_t(&key));
        assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
        assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
        assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
        assert!(state.publish_ready_t(&key, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    }
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Reconciliation(Box::new(marker)));
    assert!(state.invariant_holds());
}

#[test]
fn close_set_background_targets_never_activate_and_alias_receipts_advance_only_after_membership_commit()
 {
    let now = Instant::now();
    let mut state = ready(3);
    let desired = state.desired().cloned();
    let second = state.cache().tabs()[2].tab_id().clone();
    let close = open(&mut state, &[1, 2], now);
    assert_eq!(close.selection().attachment(), None);
    assert_eq!(close.selection().binding(), None);
    assert_eq!(state.desired().cloned(), desired);
    assert!(close.set_source().is_some());
    observe_closed(&mut state, &close, now);
    assert!(
        !state.close_set_ready(),
        "Closed status cannot stand in for canonical membership"
    );
    let source = removed(&state, close.key().target().tab_id());
    install(&mut state, &close, source, now);
    assert_eq!(state.desired().cloned(), desired);
    assert!(state.close_set_ready());
    let next = state
        .plan_next_close_set(workbench_request_id(9999), close.deadline())
        .unwrap();
    assert_eq!(next.key().target().tab_id(), &second);
    assert_eq!(next.deadline(), close.deadline());
    assert_eq!(state.close_set_status().unwrap().completed, 1);
}

#[test]
fn close_set_allows_only_own_survivor_selection_and_finishes_the_original_set() {
    let now = Instant::now();
    let mut state = ready(3);
    let mut close = open(&mut state, &[0, 1, 2], now);
    for completed in 1..=3 {
        let sampled = now + Duration::from_secs(completed);
        observe_closed(&mut state, &close, sampled);
        let source = removed(&state, close.key().target().tab_id());
        install(&mut state, &close, source, sampled);
        assert_eq!(state.settle_close_set(sampled), completed == 3);
        assert_eq!(
            state.close_set_status().unwrap().completed,
            completed as usize
        );
        if completed < 3 {
            close = state
                .plan_next_close_set(
                    workbench_request_id(10000 + completed as u128),
                    close.deadline(),
                )
                .unwrap();
            state.begin_next_close_set(&close, sampled).unwrap();
        }
    }
    assert!(state.cache().tabs().is_empty());
    assert_eq!(state.close_set_deadline(), None);
    assert!(!state.close_set_status().unwrap().halted);
}

#[test]
fn close_set_partial_completion_halts_on_unrelated_snapshot_change_or_unknown_next_close() {
    for drift in [false, true] {
        let now = Instant::now();
        let mut state = ready(3);
        let close = open(&mut state, &[1, 2], now);
        observe_closed(&mut state, &close, now);
        let mut source = removed(&state, close.key().target().tab_id());
        if drift {
            source.terminal_contexts[0].tabs[0].label = "peer edit".into();
        }
        install(&mut state, &close, source, now);
        if !drift {
            let next = state
                .plan_next_close_set(workbench_request_id(10001), close.deadline())
                .unwrap();
            state.begin_next_close_set(&next, now).unwrap();
            state
                .settle_close_exchange(&next, &CloseResult::Ambiguous)
                .unwrap();
            assert_eq!(state.uncertain_close_identity().unwrap().0, next.key());
        }
        let status = state.close_set_status().unwrap();
        assert!(status.halted);
        assert_eq!((status.completed, status.total), (1, 2));
        assert!(
            state
                .plan_next_close_set(workbench_request_id(10002), close.deadline())
                .is_err()
        );
    }
}

#[test]
fn close_set_completed_summary_expires_or_yields_to_the_next_notice_without_erasing_facts() {
    use crate::{
        client_view::ClientView,
        localization::Locale,
        tui_pointer::HitMapBuilder,
        tui_theme::Theme,
        workbench_client::{NativeProjectionStatus, WorkbenchClientView},
    };
    use ratatui::{Terminal, backend::TestBackend};
    let render = |state: &WorkbenchClientState, locale| {
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &ClientView::new(120),
                    locale,
                    Some(WorkbenchClientView::test_view(
                        state,
                        &[],
                        NativeProjectionStatus::Empty,
                    )),
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..40 {
            let mut x = 0;
            while x < 120 {
                let symbol = buffer[(x, y)].symbol();
                text.push_str(symbol);
                x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
            }
            text.push('\n');
        }
        text
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for next in ["timeout", "notice", "action", "halted-notice"] {
            let now = Instant::now();
            let mut state = ready(3);
            let close = open(&mut state, &[1], now);
            if next == "halted-notice" {
                state
                    .settle_close_exchange(&close, &CloseResult::Ambiguous)
                    .unwrap();
                state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(
                    close.key().clone(),
                ));
            } else {
                observe_closed(&mut state, &close, now);
                let source = removed(&state, close.key().target().tab_id());
                install(&mut state, &close, source, now);
                assert!(state.settle_close_set(now));
                assert_eq!(
                    state.close_set_summary_deadline(),
                    Some(now + Duration::from_secs(5))
                );
                assert!(!state.settle_close_set(now + Duration::from_secs(4)));
                assert!(
                    render(&state, locale)
                        .contains(&locale.close_set_status(state.close_set_status().unwrap()))
                );
            }
            let facts = state.close_set_status();
            let recovery = state
                .uncertain_close_identity()
                .map(|(key, operation)| (key.clone(), operation.cloned()));
            match next {
                "timeout" => {
                    assert!(state.settle_close_set(now + Duration::from_secs(5)));
                }
                "action" => state.dismiss_close_set_summary(),
                _ => {
                    state.set_notice(ClientNotice::CloseRequestNotApplied);
                    let screen = render(&state, locale);
                    assert!(
                        screen.contains(&locale.native_notice(
                            crate::workbench_client::WorkbenchClientNotice::CloseRequestNotApplied
                        )),
                        "{screen}"
                    );
                    assert!(!screen.contains(&locale.close_set_status(facts.unwrap())));
                    state.clear_notice();
                }
            }
            assert_eq!(state.close_set_status(), facts);
            assert_eq!(
                state
                    .uncertain_close_identity()
                    .map(|(key, operation)| (key.clone(), operation.cloned())),
                recovery
            );
            if next == "halted-notice" {
                assert!(state.view().close_set_status().unwrap().halted);
                assert!(!state.close_set_ready());
                assert!(
                    state
                        .plan_next_close_set(
                            workbench_request_id(90000),
                            now + Duration::from_secs(50)
                        )
                        .is_err()
                );
            } else {
                assert!(state.view().close_set_status().is_none());
                assert!(state.close_set_summary_deadline().is_none());
                assert!(!render(&state, locale).contains(&locale.close_set_status(facts.unwrap())));
                assert!(!state.settle_close_set(now + Duration::from_secs(60)));
            }
        }
    }
}

#[test]
fn close_set_cutoff_halts_new_work_but_a_later_closed_receipt_keeps_truth_without_resuming() {
    let now = Instant::now();
    let mut state = ready(3);
    let close = open(&mut state, &[1, 2], now);
    assert!(state.settle_close_set(close.deadline()));
    assert!(state.close_set_status().unwrap().halted);
    observe_closed(&mut state, &close, now + Duration::from_secs(51));
    let status = state.close_set_status().unwrap();
    assert_eq!(
        (status.completed, status.total, status.halted),
        (1, 2, true)
    );
    assert!(!state.close_set_ready());
}

#[test]
fn close_set_same_tick_local_drift_before_owned_membership_install_cannot_be_laundered() {
    let now = Instant::now();
    let mut state = ready(3);
    let id = state.cache().tabs()[2].tab_id().clone();
    state
        .mark_tabs(
            &id,
            crate::workbench_client::state::TabSelectionChange::Toggle,
        )
        .unwrap();
    assert_eq!(state.selected_tab_ids().len(), 2);
    let close = open(&mut state, &[1, 2], now);
    observe_closed(&mut state, &close, now);
    // This is the same clear performed by ordinary navigation. No coordinator tick runs
    // between the local change and the canonical close installation.
    state.clear_tab_selection();
    let source = removed(&state, close.key().target().tab_id());
    install(&mut state, &close, source, now);
    let status = state.close_set_status().unwrap();
    assert_eq!(
        (status.completed, status.total, status.halted),
        (1, 2, true)
    );
    assert!(!state.close_set_ready());
    assert!(
        state
            .plan_next_close_set(workbench_request_id(10002), close.deadline())
            .is_err()
    );
    assert!(state.cache().tab_index(&id).is_some());
}

#[test]
fn close_set_precommit_race_halts_without_retarget_or_mutation_replay() {
    let now = Instant::now();
    let mut state = ready(3);
    let source = state.cache().canonical_snapshot().clone();
    let desired = state.desired().cloned();
    let close = open(&mut state, &[1, 2], now);
    state
        .settle_close_exchange(
            &close,
            &CloseResult::Precommit(
                crate::local_client::workbench::WorkbenchTabClosePrecommit::Rejected {
                    revision: WorkbenchRevision::new(2),
                    reason: orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
                },
            ),
        )
        .unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(close.key().clone()));
    let status = state.close_set_status().unwrap();
    assert_eq!(
        (status.completed, status.total, status.halted),
        (0, 2, true)
    );
    assert!(!state.has_retained_close_records());
    assert_eq!(state.cache().canonical_snapshot(), &source);
    assert_eq!(state.desired(), desired.as_ref());
    assert!(
        state
            .plan_next_close_set(workbench_request_id(10002), close.deadline())
            .is_err()
    );
}

fn rebind_same_focused_pane(state: &mut WorkbenchClientState, now: Instant) {
    let before = state.focus_record().unwrap().pane_ref();
    let panel = before.pane().panel().clone();
    let desired = state.desired().cloned();
    let mode = state.controller_mode().cloned().unwrap();
    assert!(state.sync_pane_set(&[], now));
    assert!(state.settle_pane_detached(&before));
    assert!(state.sync_pane_set(&[(panel, 0, state.latest_geometry())], now));
    let after = state.focus_record().unwrap().pane_ref();
    assert_eq!(before.pane(), after.pane());
    assert_ne!(
        before, after,
        "the real membership owner minted a new incarnation"
    );
    // Complete the record-level fixture too: an unattached record underneath Ready would
    // be an invalid input and only prove the generic membership invariant, not this fence.
    assert!(state.mark_pane_attached(&after));
    assert!(state.record_setup_configuration(&after, mode, Some(state.latest_geometry())));
    assert!(state.stage_drawn_frame(&after, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&after, FrameIdentity::new(2)));
    assert!(state.invariant_holds());
    assert_eq!(state.desired(), desired.as_ref());
}

#[test]
fn close_set_targeted_resume_rejects_same_key_rebind_at_plan_and_final_admission() {
    for rebind_before_plan in [true, false] {
        let now = Instant::now();
        let mut state = ready(3);
        let selected = selection(1);
        state.close.retain_submission(selected.clone()).unwrap();
        observe(&mut state.close, &selected, 7, parked());
        let status = state.close.status(selected.target()).unwrap().token();
        state.acknowledge_close_draw(None, Some(status));
        state
            .open_tab_close_resume(selected.target().tab_id())
            .unwrap();
        let prompt = state.view().close_confirmation().unwrap().token();
        state.acknowledge_close_draw(Some(prompt), Some(status));
        let operation = state
            .plan_close_confirmation(workbench_request_id(8100), now + Duration::from_secs(50))
            .unwrap();
        assert_eq!(operation.selection().binding(), None);
        assert_eq!(operation.selection().attachment(), None);
        assert!(
            matches!(operation.kind(), CloseOperationKind::Resume { operation, generation }
            if operation == &workbench_request_id(701) && generation.value() == 7)
        );
        rebind_same_focused_pane(&mut state, now);
        if rebind_before_plan {
            assert_eq!(
                state.plan_close_confirmation(workbench_request_id(8100), operation.deadline()),
                Err(CloseStateError::StaleConfirmation)
            );
        } else {
            assert_eq!(
                state.begin_close_exchange(&operation, now),
                Err(CloseStateError::StaleConfirmation)
            );
        }
        assert!(state.settle_stale_close_confirmations());
        assert!(state.view().close_confirmation().is_none());
    }
}

#[test]
fn close_set_background_resume_draw_uses_its_exact_record_when_active_record_competes() {
    use crate::workbench_client::ui::NativeProjectionStatus;
    use crate::workbench_client::{ModalPresentation, NativeDrawReceipt, WorkbenchClientView};
    use crate::{client_view::ClientView, localization::Locale, tui_theme::Theme};
    use ratatui::{Terminal, backend::TestBackend};

    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24)] {
            let mut state = ready(3);
            let selected = selection(1);
            state.close.retain_submission(selected.clone()).unwrap();
            observe(&mut state.close, &selected, 7, parked());
            let status = state.close.status(selected.target()).unwrap().token();
            state.acknowledge_close_draw(None, Some(status));
            state
                .open_tab_close_resume(selected.target().tab_id())
                .unwrap();
            let competing = selection(0);
            state.close.retain_submission(competing.clone()).unwrap();
            observe(&mut state.close, &competing, 1, parked());
            let other = state.close.status(competing.target()).unwrap().token();
            assert_ne!(status, other);
            let prompt = state.view().close_confirmation().unwrap().token();
            // A different record's complete draw can never authorize this Resume.
            state.acknowledge_close_draw(Some(prompt), Some(other));
            assert_eq!(
                state.plan_close_confirmation(
                    workbench_request_id(8101),
                    Instant::now() + Duration::from_secs(50)
                ),
                Err(CloseStateError::NotDisplayed)
            );
            assert_eq!(state.view().close_status().unwrap().token(), status);

            let view = WorkbenchClientView::test_view(&state, &[], NativeProjectionStatus::Empty);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut receipt = NativeDrawReceipt::default();
            terminal
                .draw(|frame| {
                    receipt = crate::tui_render::render_terminal_shell(
                        frame,
                        &ClientView::new(width),
                        locale,
                        Some(view),
                        &Theme::terminal_native(),
                        &mut crate::tui_pointer::HitMapBuilder::new(),
                    );
                })
                .unwrap();
            assert_eq!(
                receipt,
                NativeDrawReceipt::new(Some(ModalPresentation::Presented(prompt)), Some(status))
                    .with_position(receipt.position().cloned())
            );
            state.acknowledge_close_draw(Some(prompt), Some(status));
            let operation = state
                .plan_close_confirmation(
                    workbench_request_id(8101),
                    Instant::now() + Duration::from_secs(50),
                )
                .unwrap();
            state
                .begin_close_exchange(&operation, Instant::now())
                .unwrap();
            assert_eq!(operation.key().target(), selected.target());
            assert!(
                matches!(operation.kind(), CloseOperationKind::Resume { operation, generation }
                if operation == &workbench_request_id(701) && generation.value() == 7)
            );
        }
    }
}

#[test]
fn close_set_same_pane_key_rebind_revokes_confirmation_waiting_and_preinstall_premises() {
    for stage in 0..3 {
        let now = Instant::now();
        let mut state = ready(3);
        let source = state.cache().canonical_snapshot().clone();
        let ids = vec![
            state.cache().tabs()[1].tab_id().clone(),
            state.cache().tabs()[2].tab_id().clone(),
        ];
        if stage == 0 {
            state.open_close_set(&ids).unwrap();
            let token = state.view().close_confirmation().unwrap().token();
            state.acknowledge_close_draw(Some(token), None);
            rebind_same_focused_pane(&mut state, now);
            assert!(
                state
                    .plan_close_confirmation(
                        workbench_request_id(8010),
                        now + Duration::from_secs(50)
                    )
                    .is_err()
            );
            assert!(state.settle_stale_close_confirmations());
            assert!(state.view().close_confirmation().is_none());
        } else {
            let close = open(&mut state, &[1, 2], now);
            if stage == 2 {
                observe_closed(&mut state, &close, now);
            }
            rebind_same_focused_pane(&mut state, now);
            if stage == 1 {
                assert!(state.settle_close_set(now));
            } else {
                // Deliberately no tick before installation, just as for the mark-race witness.
                let candidate = removed(&state, close.key().target().tab_id());
                install(&mut state, &close, candidate, now);
            }
            assert!(state.close_set_status().unwrap().halted);
            assert!(!state.close_set_ready());
        }
        if stage != 2 {
            assert_eq!(state.cache().canonical_snapshot(), &source);
        }
    }
}

#[test]
fn close_set_expected_removal_preserves_other_contexts_and_unrelated_empty_groups() {
    use crate::workbench_client::workbench_projection::project_preferred_workspace;
    use orchbee_workbench_protocol::{TabGroup, TabGroupColor};
    for drift in ["none", "context", "group", "revision"] {
        let now = Instant::now();
        let mut source = super::super::context_selection::contexts(1, 1);
        let source_group = TabGroup {
            group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 18000)).unwrap(),
            label: "source".into(),
            color: TabGroupColor::Green,
        };
        let unrelated = TabGroup {
            group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 18001)).unwrap(),
            label: "unrelated empty".into(),
            color: TabGroupColor::Blue,
        };
        for tab in &mut source.terminal_contexts[0].tabs {
            tab.group_id = Some(source_group.group_id.clone());
        }
        source.terminal_contexts[0].groups = vec![source_group.clone(), unrelated.clone()];
        let other_context = source.terminal_contexts[1].clone();
        let mut state =
            super::super::ready_with_cache(project_preferred_workspace(&source).unwrap());
        let first = open(&mut state, &[0, 1], now);
        observe_closed(&mut state, &first, now);
        let mut candidate = removed(&state, first.key().target().tab_id());
        match drift {
            "context" => candidate.terminal_contexts[1].tabs[0].label = "peer label".into(),
            "group" => candidate.terminal_contexts[0].groups[1].color = TabGroupColor::Red,
            "revision" => {
                candidate.revision = WorkbenchRevision::new(candidate.revision.value() + 1)
            }
            "none" => {}
            _ => unreachable!(),
        }
        install(&mut state, &first, candidate, now);
        if drift != "none" {
            assert!(
                state.close_set_status().unwrap().halted,
                "{drift} is not an owned revision advance"
            );
            assert!(!state.close_set_ready());
            continue;
        }
        assert_eq!(
            state.cache().canonical_snapshot().terminal_contexts[0].groups,
            vec![source_group, unrelated.clone()]
        );
        let second = state
            .plan_next_close_set(workbench_request_id(10001), first.deadline())
            .unwrap();
        state
            .begin_next_close_set(&second, now + Duration::from_secs(1))
            .unwrap();
        observe_closed(&mut state, &second, now + Duration::from_secs(1));
        let candidate = removed(&state, second.key().target().tab_id());
        install(&mut state, &second, candidate, now + Duration::from_secs(1));
        assert!(!state.close_set_status().unwrap().halted);
        assert_eq!(
            state.cache().canonical_snapshot().terminal_contexts[0].groups,
            vec![unrelated]
        );
        assert_eq!(
            state.cache().canonical_snapshot().terminal_contexts[1],
            other_context
        );
    }
}

#[test]
fn close_set_unseen_cancelled_resized_and_stale_prompts_never_admit() {
    for change in 0..4 {
        let now = Instant::now();
        let mut state = ready(3);
        let ids = vec![
            state.cache().tabs()[1].tab_id().clone(),
            state.cache().tabs()[2].tab_id().clone(),
        ];
        state.open_close_set(&ids).unwrap();
        let token = state.view().close_confirmation().unwrap().token();
        state.acknowledge_close_draw(Some(token), None);
        match change {
            0 => state.cancel_close_confirmation(),
            1 => state.invalidate_close_display(),
            2 => {
                let _ = state.settle_close_confirmation_not_presented();
            }
            3 => {
                let mut peer = state.cache().canonical_snapshot().clone();
                peer.revision = WorkbenchRevision::new(2);
                state.cache = state.cache.project_snapshot(&peer).unwrap();
                assert!(state.settle_stale_close_confirmations());
            }
            _ => unreachable!(),
        }
        assert!(
            state
                .plan_close_confirmation(workbench_request_id(8000), now + Duration::from_secs(50))
                .is_err()
        );
        assert!(!state.has_retained_close_records());
        assert!(state.close_set_status().is_none());
    }
}

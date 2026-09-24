use super::*;
use crate::{
    client_view::ClientAction,
    local_client::test_server::disconnected_origin,
    terminal_session::ui::{
        move_membership_tests::ready_ui, publication_hint_tests::ready_split_ui,
    },
};
use orchbee_workbench_protocol::{
    ClientViewControlStatus, ClientViewIntent, ClientViewOperation, WorkbenchClientId,
    WorkbenchRequestId,
};

fn controller(clock: LocalOperationClock) -> ClientControl {
    let mut control = ClientControl::new(disconnected_origin(), clock);
    control.lane = Some(ClientControlLane::for_test());
    control
}
fn command(control: &ClientControl, zoom: Option<bool>) -> ClientViewControl {
    let view = control.observed.as_ref().unwrap();
    let selection = view.selection.as_ref().unwrap();
    let context = selection.terminal_context_id.clone();
    let tab = selection.tab_id.clone().unwrap();
    let panel = selection.panel_id.clone().unwrap();
    ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 801)).unwrap(),
        client_id: WorkbenchClientId::parse(&format!("{:032x}", 802)).unwrap(),
        expected_view_revision: ClientViewSequence::new(control.revision).unwrap(),
        minimum_workbench_revision: view.workbench_revision,
        intent: zoom.map_or(
            ClientViewIntent::Panel {
                terminal_context_id: context.clone(),
                tab_id: tab.clone(),
                panel_id: panel.clone(),
            },
            |enabled| ClientViewIntent::Zoom {
                terminal_context_id: context,
                tab_id: tab,
                panel_id: panel,
                enabled,
            },
        ),
    }
}
fn inject(control: &ClientControl, command: ClientViewControl) {
    control.lane.as_ref().unwrap().inject_for_test(command);
}
fn completion(control: &ClientControl) -> Option<(Sample, ClientViewCompletion)> {
    control.lane.as_ref().unwrap().completion_for_test()
}

#[test]
fn an_observed_target_is_not_acknowledged_until_successful_draw() {
    let (mut native, _mailbox, clock, _elapsed) = ready_ui();
    let mut control = controller(clock);
    assert!(!control.tick(&mut native, false));
    let command = command(&control, None);
    inject(&control, command.clone());
    assert!(control.tick(&mut native, false));
    assert!(
        completion(&control).is_none(),
        "selected or queued is not painted"
    );
    control.after_draw(&native);
    let (sample, completion) = completion(&control).unwrap();
    assert_eq!(completion.outcome, ClientViewCompletionOutcome::Applied {});
    ClientViewOperation {
        command,
        status: ClientViewControlStatus::Applied { view: sample.view },
    }
    .validate()
    .unwrap();
    assert!(control.pending.is_none());
}

#[test]
fn zoom_changes_geometry_then_waits_for_paint_and_single_panel_has_explicit_noop() {
    for single in [false, true] {
        let (mut native, _mailbox, clock, _elapsed) =
            if single { ready_ui() } else { ready_split_ui() };
        let mut control = controller(clock);
        control.tick(&mut native, false);
        let command = command(&control, Some(true));
        inject(&control, command.clone());
        assert!(control.tick(&mut native, false));
        assert!(completion(&control).is_none());
        assert!(control.tick(&mut native, false));
        assert!(completion(&control).is_none());
        control.after_draw(&native);
        let (sample, completion) = completion(&control).unwrap();
        assert_eq!(sample.view.selection.as_ref().unwrap().zoomed, !single);
        let status = if single {
            assert_eq!(
                completion.outcome,
                ClientViewCompletionOutcome::ZoomUnchanged {}
            );
            ClientViewControlStatus::ZoomUnchanged { view: sample.view }
        } else {
            assert_eq!(completion.outcome, ClientViewCompletionOutcome::Applied {});
            ClientViewControlStatus::Applied { view: sample.view }
        };
        ClientViewOperation { command, status }.validate().unwrap();
    }
}

#[test]
fn human_intent_interrupts_without_claiming_a_zoom_was_undone() {
    let (mut native, _mailbox, clock, _elapsed) = ready_split_ui();
    let mut control = controller(clock);
    control.tick(&mut native, false);
    inject(&control, command(&control, Some(true)));
    assert!(control.tick(&mut native, false));
    control.interrupt();
    control.tick(&mut native, false);
    let (sample, completion) = completion(&control).unwrap();
    assert_eq!(
        completion.outcome,
        ClientViewCompletionOutcome::Incomplete {
            reason: Reason::Interrupted
        }
    );
    assert!(sample.view.selection.unwrap().zoomed);
    assert!(control.pending.is_none());
}

#[test]
fn stale_view_and_blocking_chrome_refuse_before_any_remote_effect() {
    for shell_blocked in [false, true] {
        let (mut native, _mailbox, clock, _elapsed) = ready_split_ui();
        let mut control = controller(clock);
        control.tick(&mut native, false);
        let command = command(&control, Some(false));
        if !shell_blocked {
            native.apply_client_action(ClientAction::ToggleZoom);
        }
        inject(&control, command);
        control.tick(&mut native, shell_blocked);
        let (sample, completion) = completion(&control).unwrap();
        assert_eq!(
            completion.outcome,
            ClientViewCompletionOutcome::Rejected {
                reason: Reason::StaleView
            }
        );
        assert_eq!(sample.view.selection.unwrap().zoomed, !shell_blocked);
        assert!(control.pending.is_none());
    }
}

#[test]
fn completed_draw_sample_cannot_be_replaced_by_later_navigation() {
    let (mut native, _mailbox, clock, _elapsed) = ready_split_ui();
    let mut control = controller(clock);
    control.tick(&mut native, false);
    inject(&control, command(&control, None));
    control.tick(&mut native, false);
    control.after_draw(&native);
    let (before, completion_before) = completion(&control).unwrap();
    native.apply_client_action(ClientAction::ToggleZoom);
    control.tick(&mut native, false);
    let (after, completion_after) = completion(&control).unwrap();
    assert_eq!(before.view, after.view);
    assert_eq!(before.heartbeat, after.heartbeat);
    assert_eq!(completion_before, completion_after);
    assert!(!after.view.selection.unwrap().zoomed);
    assert!(
        control
            .observed
            .as_ref()
            .unwrap()
            .selection
            .as_ref()
            .unwrap()
            .zoomed
    );
}

#[test]
fn targeted_toggle_captures_once_and_only_acknowledges_the_opposite_drawn_state() {
    for previously_zoomed in [false, true] {
        let (mut native, _mailbox, clock, _elapsed) = ready_split_ui();
        if previously_zoomed {
            native.apply_client_action(ClientAction::ToggleZoom);
        }
        let mut control = controller(clock);
        control.tick(&mut native, false);
        let mut command = command(&control, None);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = command.intent
        else {
            unreachable!()
        };
        command.intent = ClientViewIntent::ToggleZoom {
            terminal_context_id,
            tab_id,
            panel_id,
        };
        inject(&control, command.clone());
        for _ in 0..4 {
            assert!(control.tick(&mut native, false));
        }
        assert!(completion(&control).is_none());
        control.after_draw(&native);
        let (sample, completion) = completion(&control).unwrap();
        assert_eq!(
            completion.outcome,
            ClientViewCompletionOutcome::ZoomToggled { previously_zoomed }
        );
        assert_eq!(
            sample.view.selection.as_ref().unwrap().zoomed,
            !previously_zoomed
        );
        ClientViewOperation {
            command,
            status: ClientViewControlStatus::ZoomToggled {
                previously_zoomed,
                view: sample.view,
            },
        }
        .validate()
        .unwrap();
    }
}

#[test]
fn layout_read_completes_without_draw_resize_or_selection_and_pins_its_sample() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::publication_hint_tests::split_snapshot,
    };
    use ratatui::layout::Rect;
    for available in [false, true] {
        let (mut native, mailbox, clock, _elapsed) = ready_split_ui();
        let initial = mailbox.try_recv().unwrap().unwrap();
        assert!(matches!(
            initial.record(),
            super::super::mailbox::QueuedCommand::Poll(_)
        ));
        mailbox.finish_lease(initial).unwrap();
        let mut control = controller(clock);
        control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
        assert!(!control.tick(&mut native, false));
        let before = native.control_observation(false).unwrap();
        let mut command = command(&control, None);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = command.intent
        else {
            panic!("panel");
        };
        command.intent = ClientViewIntent::Layout {
            terminal_context_id,
            tab_id,
            panel_id,
        };
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command: command.clone(),
                layout_snapshot: available.then(|| split_snapshot(1)),
            });
        assert!(
            !control.tick(&mut native, false),
            "a read must not ask the shell to resize"
        );
        let (sample, completed) = completion(&control).unwrap();
        let status = match completed.outcome {
            ClientViewCompletionOutcome::LayoutRead { layout } if available => {
                assert!(layout.selected);
                assert_eq!(layout.viewport.width, 120);
                ClientViewControlStatus::LayoutRead {
                    layout,
                    view: sample.view.clone(),
                }
            }
            ClientViewCompletionOutcome::Rejected { reason } if !available => {
                assert_eq!(reason, Reason::ProjectionUnavailable);
                ClientViewControlStatus::Rejected { reason }
            }
            other => panic!("unexpected completion {other:?}"),
        };
        ClientViewOperation { command, status }.validate().unwrap();
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(control.pending.is_none());
        control.observe_geometry(Rect::new(0, 0, 80, 25), SidebarSurface::Docked);
        assert!(!control.tick(&mut native, false));
        assert_eq!(completion(&control).unwrap().0.view, sample.view);
        if let ClientViewCompletionOutcome::LayoutRead { layout } =
            completion(&control).unwrap().1.outcome
        {
            assert_eq!(
                layout.viewport.width, 120,
                "later geometry cannot rewrite the retained read"
            );
        }
    }
}

fn directional(
    command: &mut ClientViewControl,
    direction: orchbee_workbench_protocol::ClientPanelDirection,
) {
    let ClientViewIntent::Panel {
        terminal_context_id,
        tab_id,
        panel_id,
    } = command.intent.clone()
    else {
        panic!("panel");
    };
    command.intent = ClientViewIntent::FocusDirection {
        terminal_context_id,
        tab_id,
        panel_id,
        direction,
    };
}

#[test]
fn directional_focus_pins_one_neighbor_preserves_zoom_and_requires_draw() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::publication_hint_tests::split_snapshot,
    };
    use orchbee_workbench_protocol::ClientPanelDirection;
    use ratatui::layout::Rect;
    for interrupted in [false, true] {
        let (mut native, mailbox, clock, _elapsed) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        native.apply_client_action(ClientAction::ToggleZoom);
        let mut control = controller(clock.clone());
        control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
        control.tick(&mut native, false);
        let mut command = command(&control, None);
        directional(&mut command, ClientPanelDirection::Right);
        let snapshot = split_snapshot(1);
        let layout = native
            .read_client_layout(
                &command,
                &snapshot,
                Rect::new(0, 0, 120, 40),
                SidebarSurface::Hidden,
            )
            .unwrap();
        let destination = layout.neighbors.right.unwrap();
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command: command.clone(),
                layout_snapshot: Some(snapshot),
            });
        assert!(!control.tick(&mut native, false));
        assert!(
            completion(&control).is_none(),
            "pending sibling setup is not failure"
        );
        let deadline = control.pending.as_ref().unwrap().deadline;
        crate::terminal_session::ui::publication_hint_tests::attach_split_peer(
            &mut native,
            &mailbox,
            &clock,
        );
        assert!(control.tick(&mut native, false));
        assert_eq!(control.pending.as_ref().unwrap().deadline, deadline);
        assert!(completion(&control).is_none());
        let selection = native
            .control_observation(false)
            .unwrap()
            .1
            .selection
            .unwrap();
        assert_eq!(selection.panel_id.as_ref(), Some(&destination));
        assert!(selection.zoomed);
        // Resizing hides the original source; the already captured destination still completes.
        control.observe_geometry(Rect::new(0, 0, 40, 10), SidebarSurface::Hidden);
        if interrupted {
            control.interrupt();
        }
        assert!(control.tick(&mut native, false));
        if interrupted {
            assert_eq!(
                completion(&control).unwrap().1.outcome,
                ClientViewCompletionOutcome::Incomplete {
                    reason: Reason::Interrupted
                }
            );
        } else {
            assert!(completion(&control).is_none());
            control.after_draw(&native);
            let (sample, done) = completion(&control).unwrap();
            assert_eq!(
                done.outcome,
                ClientViewCompletionOutcome::DirectionFocused {
                    panel_id: destination.clone()
                }
            );
            ClientViewOperation {
                command,
                status: ClientViewControlStatus::DirectionFocused {
                    panel_id: destination,
                    view: sample.view,
                },
            }
            .validate()
            .unwrap();
            assert!(control.pending.is_none());
        }
    }
}

#[test]
fn no_neighbor_in_an_inactive_tab_has_no_selection_attachment_resize_or_draw_effect() {
    use crate::{
        client_view::SidebarSurface,
        terminal_session::{
            ui::membership_tests::snapshot, workbench_projection::project_native_standalone_tabs,
        },
    };
    use orchbee_workbench_protocol::ClientPanelDirection;
    use ratatui::layout::Rect;
    for direction in [
        ClientPanelDirection::Left,
        ClientPanelDirection::Right,
        ClientPanelDirection::Up,
        ClientPanelDirection::Down,
    ] {
        let (mut native, mailbox, clock, _elapsed) = ready_ui();
        let before = native.control_observation(false).unwrap();
        let mut control = controller(clock);
        control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
        control.tick(&mut native, false);
        // The source is in a newer Snapshot of an inactive Tab; current cache stays at revision 1.
        let snapshot = snapshot(2, 2);
        let cache = project_native_standalone_tabs(&snapshot).unwrap();
        let tab = &cache.tabs()[1];
        let mut command = command(&control, None);
        command.minimum_workbench_revision = snapshot.revision;
        command.intent = ClientViewIntent::FocusDirection {
            terminal_context_id: cache.terminal_context_id().clone(),
            tab_id: tab.tab_id().clone(),
            panel_id: tab.panel(0).unwrap().panel_id().clone(),
            direction,
        };
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command: command.clone(),
                layout_snapshot: Some(snapshot.clone()),
            });
        assert!(
            !control.tick(&mut native, false),
            "no-neighbor cannot ask shell to synchronize sizes"
        );
        let (sample, done) = completion(&control).unwrap();
        assert_eq!(
            done.outcome,
            ClientViewCompletionOutcome::DirectionUnchanged {
                layout_revision: snapshot.revision
            }
        );
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert_eq!(sample.view, before.1);
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(control.pending.is_none());
        ClientViewOperation {
            command,
            status: ClientViewControlStatus::DirectionUnchanged {
                layout_revision: snapshot.revision,
                view: sample.view,
            },
        }
        .validate()
        .unwrap();
        assert!(!control.tick(&mut native, false));
        assert_eq!(completion(&control).unwrap().1, done, "status stays pinned");
    }
}

#[test]
fn directional_unknown_geometry_never_claims_no_neighbor_or_changes_selection() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::publication_hint_tests::split_snapshot,
    };
    use orchbee_workbench_protocol::{ClientPanelDirection, WorkbenchRevision};
    use ratatui::layout::Rect;
    for case in [
        "missing-snapshot",
        "hidden-source",
        "stale-membership",
        "stale-view",
    ] {
        let (mut native, mailbox, clock, _elapsed) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock);
        control.observe_geometry(
            if case == "hidden-source" {
                Rect::new(0, 0, 40, 10)
            } else {
                Rect::new(0, 0, 120, 40)
            },
            SidebarSurface::Hidden,
        );
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let mut command = command(&control, None);
        directional(&mut command, ClientPanelDirection::Left);
        if case == "hidden-source" {
            let layout = native
                .read_client_layout(
                    &command,
                    &split_snapshot(1),
                    Rect::new(0, 0, 120, 40),
                    SidebarSurface::Hidden,
                )
                .unwrap();
            if let ClientViewIntent::FocusDirection { panel_id, .. } = &mut command.intent {
                *panel_id = layout.panels[1].panel_id.clone();
            }
        }
        if case == "stale-membership" {
            command.minimum_workbench_revision = WorkbenchRevision::new(2);
        }
        if case == "stale-view" {
            command.expected_view_revision = ClientViewSequence::new(control.revision + 1).unwrap();
        }
        let snapshot = (case != "missing-snapshot").then(|| split_snapshot(2));
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command,
                layout_snapshot: snapshot,
            });
        assert!(!control.tick(&mut native, false));
        assert_eq!(
            completion(&control).unwrap().1.outcome,
            ClientViewCompletionOutcome::Rejected {
                reason: if case == "stale-view" {
                    Reason::StaleView
                } else {
                    Reason::ProjectionUnavailable
                }
            }
        );
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(control.pending.is_none());
    }
}

#[test]
fn a_directional_destination_that_never_attaches_keeps_the_original_deadline() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::publication_hint_tests::split_snapshot,
    };
    use orchbee_workbench_protocol::ClientPanelDirection;
    use ratatui::layout::Rect;
    use std::sync::atomic::Ordering;
    let (mut native, _mailbox, clock, elapsed) = ready_split_ui();
    let mut control = controller(clock);
    control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
    control.tick(&mut native, false);
    let before = native.control_observation(false).unwrap();
    let mut command = command(&control, None);
    directional(&mut command, ClientPanelDirection::Right);
    control
        .lane
        .as_ref()
        .unwrap()
        .inject_delivery_for_test(lane::Delivery {
            command,
            layout_snapshot: Some(split_snapshot(1)),
        });
    assert!(!control.tick(&mut native, false));
    let deadline = control.pending.as_ref().unwrap().deadline;
    elapsed.store(49_000, Ordering::SeqCst);
    assert!(!control.tick(&mut native, false));
    assert_eq!(control.pending.as_ref().unwrap().deadline, deadline);
    elapsed.store(51_000, Ordering::SeqCst);
    assert!(control.tick(&mut native, false));
    assert_eq!(
        completion(&control).unwrap().1.outcome,
        ClientViewCompletionOutcome::Rejected {
            reason: Reason::Expired
        }
    );
    assert_eq!(native.control_observation(false).unwrap(), before);
}

fn ratio_command(control: &ClientControl, ratio: f64) -> ClientViewControl {
    use orchbee_workbench_protocol::{ClientSplitPath, ClientSplitRatio};
    let mut command = command(control, None);
    let ClientViewIntent::Panel {
        terminal_context_id,
        tab_id,
        ..
    } = command.intent
    else {
        panic!()
    };
    command.intent = ClientViewIntent::SetSplitRatio {
        terminal_context_id,
        tab_id,
        path: ClientSplitPath::new(vec![]).unwrap(),
        ratio: ClientSplitRatio::new(ratio).unwrap(),
    };
    command
}

#[test]
fn resize_commits_once_preserves_zoom_and_only_draw_or_interruption_settles_it() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::publication_hint_tests::split_snapshot,
    };
    use orchbee_workbench_protocol::{
        ClientLayoutChange, ClientPanelDirection, ClientResizeAmount,
    };
    use ratatui::layout::Rect;
    for ending in ["draw", "interrupt", "expire"] {
        let (mut native, mailbox, clock, elapsed) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        native.apply_client_action(ClientAction::ToggleZoom);
        let mut control = controller(clock);
        control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let mut command = command(&control, None);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = command.intent.clone()
        else {
            panic!()
        };
        command.intent = ClientViewIntent::Resize {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            direction: ClientPanelDirection::Right,
            amount: ClientResizeAmount::new(0.1).unwrap(),
        };
        let snapshot = split_snapshot(1);
        let mut query = command.clone();
        query.intent = ClientViewIntent::Layout {
            terminal_context_id,
            tab_id,
            panel_id,
        };
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command: command.clone(),
                layout_snapshot: Some(snapshot.clone()),
            });
        assert!(control.tick(&mut native, false));
        assert!(completion(&control).is_none());
        let deadline = control.pending.as_ref().unwrap().deadline;
        for _ in 0..3 {
            assert!(control.tick(&mut native, false));
            let layout = native
                .read_client_layout(
                    &query,
                    &snapshot,
                    Rect::new(0, 0, 120, 40),
                    SidebarSurface::Hidden,
                )
                .unwrap();
            assert_eq!(
                layout.splits[0].ratio.value(),
                0.6,
                "one invocation, one delta"
            );
            assert!(layout.zoomed);
            assert_eq!(native.control_observation(false).unwrap(), before);
            assert_eq!(control.pending.as_ref().unwrap().deadline, deadline);
            assert!(completion(&control).is_none());
        }
        if ending == "draw" {
            control.after_draw(&native);
            let (sample, done) = completion(&control).unwrap();
            let ClientViewCompletionOutcome::LayoutAdjusted { adjustment } = done.outcome else {
                panic!()
            };
            let ClientLayoutChange::Split { split } = &adjustment.change else {
                panic!()
            };
            assert_eq!(split.ratio.value(), 0.6);
            ClientViewOperation {
                command,
                status: ClientViewControlStatus::LayoutAdjusted {
                    adjustment,
                    view: sample.view,
                },
            }
            .validate()
            .unwrap();
        } else {
            if ending == "interrupt" {
                control.interrupt();
            } else {
                elapsed.store(51_000, std::sync::atomic::Ordering::SeqCst);
            }
            control.tick(&mut native, false);
            assert_eq!(
                completion(&control).unwrap().1.outcome,
                ClientViewCompletionOutcome::Incomplete {
                    reason: if ending == "interrupt" {
                        Reason::Interrupted
                    } else {
                        Reason::Expired
                    }
                }
            );
            assert_eq!(
                native
                    .read_client_layout(
                        &query,
                        &snapshot,
                        Rect::new(0, 0, 120, 40),
                        SidebarSurface::Hidden
                    )
                    .unwrap()
                    .splits[0]
                    .ratio
                    .value(),
                0.6
            );
        }
        assert!(control.pending.is_none());
    }
}

#[test]
fn unchanged_ratio_and_no_resize_border_do_not_request_draw_or_queue_native_work() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::publication_hint_tests::split_snapshot,
    };
    use orchbee_workbench_protocol::{ClientPanelDirection, ClientResizeAmount};
    use ratatui::layout::Rect;
    for no_border in [false, true] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock);
        control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let mut command = ratio_command(&control, 0.5);
        if no_border {
            let mut source = self::command(&control, None);
            let ClientViewIntent::Panel {
                terminal_context_id,
                tab_id,
                panel_id,
            } = source.intent
            else {
                panic!()
            };
            source.intent = ClientViewIntent::Resize {
                terminal_context_id,
                tab_id,
                panel_id,
                direction: ClientPanelDirection::Up,
                amount: ClientResizeAmount::new(0.1).unwrap(),
            };
            command = source;
        }
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command: command.clone(),
                layout_snapshot: Some(split_snapshot(1)),
            });
        assert!(!control.tick(&mut native, false));
        let (sample, done) = completion(&control).unwrap();
        let ClientViewCompletionOutcome::LayoutAdjusted { adjustment } = done.outcome else {
            panic!()
        };
        assert!(!adjustment.change.changed());
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(control.pending.is_none());
        ClientViewOperation {
            command,
            status: ClientViewControlStatus::LayoutAdjusted {
                adjustment,
                view: sample.view,
            },
        }
        .validate()
        .unwrap();
    }
}

#[test]
fn inactive_and_unvisited_ratio_changes_store_without_selection_attachment_or_draw() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::publication_hint_tests::split_snapshot,
    };
    use orchbee_workbench_protocol::{
        ClientSplitPath, ClientSplitRatio, PanelNode, Snapshot, WorkbenchTerminalContextId,
    };
    use ratatui::layout::Rect;
    fn new_identity(value: &mut serde_json::Value, next: &mut u32) {
        match value {
            serde_json::Value::Object(fields) => {
                for child in fields.values_mut() {
                    new_identity(child, next);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    new_identity(child, next);
                }
            }
            serde_json::Value::String(text)
                if text.len() == 32 && text.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
            {
                *text = format!("{:032x}", *next);
                *next += 1;
            }
            _ => {}
        }
    }
    for unvisited in [false, true] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut snapshot = split_snapshot(2);
        let mut tab = serde_json::to_value(&snapshot.terminal_contexts[0].tabs[0]).unwrap();
        new_identity(&mut tab, &mut 10_000);
        let target: orchbee_workbench_protocol::Tab = serde_json::from_value(tab.clone()).unwrap();
        let mut value = serde_json::to_value(&snapshot).unwrap();
        let context = if unvisited {
            let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 991)).unwrap();
            value["terminalContexts"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"terminalContextId":context,"groups":[],"tabs":[tab]}));
            value["navigation"]["projects"] = serde_json::json!([{"projectId":format!("{:032x}",992),"label":"new","directories":[],"workspaces":[
                {"workspaceId":format!("{:032x}",993),"label":"unvisited","primaryDirectoryId":null,"terminalContextId":context}]}]);
            context
        } else {
            value["terminalContexts"][0]["tabs"]
                .as_array_mut()
                .unwrap()
                .push(tab);
            snapshot.terminal_contexts[0].terminal_context_id.clone()
        };
        snapshot = serde_json::from_value::<Snapshot>(value).unwrap();
        // Ensure the test exercises an admissible Snapshot, not a partial cache shaped as one.
        let document =
            orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot.clone());
        orchbee_workbench_protocol::parse_snapshot(&serde_json::to_vec(&document).unwrap())
            .unwrap();
        let mut control = controller(clock);
        control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let mut command = command(&control, None);
        command.minimum_workbench_revision = snapshot.revision;
        command.intent = ClientViewIntent::SetSplitRatio {
            terminal_context_id: context.clone(),
            tab_id: target.tab_id.clone(),
            path: ClientSplitPath::new(vec![]).unwrap(),
            ratio: ClientSplitRatio::new(0.7).unwrap(),
        };
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command: command.clone(),
                layout_snapshot: Some(snapshot.clone()),
            });
        assert!(
            !control.tick(&mut native, false),
            "inactive store cannot request shell geometry synchronization"
        );
        let (sample, done) = completion(&control).unwrap();
        let ClientViewCompletionOutcome::LayoutAdjusted { adjustment } = done.outcome else {
            panic!("{:?}", done.outcome)
        };
        assert!(!adjustment.selected);
        assert!(adjustment.change.changed());
        assert_eq!(sample.view, before.1);
        ClientViewOperation {
            command: command.clone(),
            status: ClientViewControlStatus::LayoutAdjusted {
                adjustment,
                view: sample.view,
            },
        }
        .validate()
        .unwrap();
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
        let PanelNode::Split { children, .. } = &target.root else {
            panic!()
        };
        let PanelNode::Panel { panel_id, .. } = &children[0].node else {
            panic!()
        };
        command.intent = ClientViewIntent::Layout {
            terminal_context_id: context,
            tab_id: target.tab_id,
            panel_id: panel_id.clone(),
        };
        let layout = native
            .read_client_layout(
                &command,
                &snapshot,
                Rect::new(0, 0, 120, 40),
                SidebarSurface::Hidden,
            )
            .unwrap();
        assert_eq!(layout.splits[0].ratio.value(), 0.7);
        assert_eq!(layout.panels[0].unzoomed.unwrap().width, 82);
        assert!(layout.focused_panel_id.is_none());
    }
}

#[test]
fn ratio_paths_require_matching_rendered_and_canonical_split_identity_before_commit() {
    use crate::terminal_session::ui::publication_hint_tests::split_snapshot;
    use orchbee_workbench_protocol::{ClientSplitPath, PanelNode};
    for case in ["leaf", "reordered-slots", "missing-snapshot"] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock);
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let mut command = ratio_command(&control, 0.7);
        let mut snapshot = split_snapshot(2);
        if case == "leaf" {
            let ClientViewIntent::SetSplitRatio { path, .. } = &mut command.intent else {
                panic!()
            };
            *path = ClientSplitPath::new(vec![0]).unwrap();
        } else if case == "reordered-slots" {
            let PanelNode::Split { children, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root
            else {
                panic!()
            };
            children.swap(0, 1);
        }
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command,
                layout_snapshot: (case != "missing-snapshot").then_some(snapshot),
            });
        assert!(!control.tick(&mut native, false));
        assert!(matches!(
            completion(&control).unwrap().1.outcome,
            ClientViewCompletionOutcome::Rejected { .. }
        ));
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

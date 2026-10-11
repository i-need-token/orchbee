use super::*;
use crate::client_view::{Notice, PanelDirection, SidebarSurface};
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSplitRatio,
};
use ratatui::layout::Rect;

#[test]
fn painted_resize_rejects_changed_source_or_geometry_and_uses_the_keyboard_lane() {
    for changed in [
        "current", "site", "context", "tab", "focus", "revision", "viewport", "sidebar",
    ] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock);
        let viewport = Rect::new(0, 0, 120, 40);
        control.observe_geometry(viewport, SidebarSurface::Hidden);
        control.tick(&mut native, false);
        let mut target = crate::tui_layout::DrawnResize {
            source: native.view().divider_source().unwrap(),
            viewport,
            sidebar: SidebarSurface::Hidden,
        };
        let other = format!("{:032x}", 987);
        match changed {
            "site" => {
                target.source.site = orchbee_workbench_protocol::SiteId::parse(&other).unwrap()
            }
            "context" => {
                target.source.context =
                    orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&other).unwrap()
            }
            "tab" => {
                target.source.tab =
                    orchbee_workbench_protocol::WorkbenchTabId::parse(&other).unwrap()
            }
            "focus" => {
                target.source.focus =
                    orchbee_workbench_protocol::WorkbenchPanelId::parse(&other).unwrap()
            }
            "revision" => target.source.revision = WorkbenchRevision::new(98),
            "viewport" => target.viewport.width -= 1,
            "sidebar" => {
                target.sidebar = SidebarSurface::Docked(crate::tui_layout::SidebarWidth::DEFAULT)
            }
            _ => {}
        }
        control.resize_drawn(&native, &target, PanelDirection::Right, true);
        let mutation = control.lane.as_ref().unwrap().take_layout_for_test();
        if changed == "current" {
            assert!(
                matches!(mutation.unwrap().action, WorkbenchAction::SplitRatioSet { ratio, .. } if (ratio.value() - 0.51).abs() < 1e-9)
            );
        } else {
            assert!(mutation.is_none(), "{changed}");
            assert!(matches!(
                control.take_local_resize_feedback(),
                Some(LocalResizeFeedback::Notice(Notice::ResizeFailed))
            ));
        }
    }
}

#[test]
fn known_keyboard_commit_waits_through_an_older_refresh_without_resubmission_until_expiry() {
    use crate::terminal_session::ui::tests::publication_hint::{
        settle_queued_refresh, split_snapshot,
    };
    use std::sync::atomic::Ordering;
    let (mut native, mailbox, clock, elapsed) = ready_split_ui();
    let poll = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(poll).unwrap();
    let mut control = controller(clock);
    control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
    control.tick(&mut native, false);
    control.resize(&native, PanelDirection::Right, false);
    let mutation = control
        .lane
        .as_ref()
        .unwrap()
        .take_layout_for_test()
        .unwrap();
    control
        .lane
        .as_ref()
        .unwrap()
        .finish_layout_for_test(lane::LayoutCompletion {
            operation_id: mutation.request_id,
            reply: Ok(WorkbenchRevision::new(2)),
        });
    control.tick(&mut native, true);
    settle_queued_refresh(
        &mut native,
        &mailbox,
        &control.clock,
        WorkbenchSnapshotOutcome::Available {
            snapshot: split_snapshot(1),
        },
    );
    for _ in 0..3 {
        control.tick(&mut native, true);
        control.after_draw(&mut native);
        assert!(
            control.local_resize_pending(),
            "an older read is not proof of commit failure"
        );
        control.resize(&native, PanelDirection::Right, false);
        assert!(
            control
                .lane
                .as_ref()
                .unwrap()
                .take_layout_for_test()
                .is_none()
        );
        assert!(completion(&control).is_none());
    }
    elapsed.fetch_add(50_001, Ordering::SeqCst);
    control.tick(&mut native, true);
    assert!(!control.local_resize_pending());
    assert!(matches!(
        control.take_local_resize_feedback(),
        Some(LocalResizeFeedback::Notice(Notice::ResizeUnknown))
    ));
    assert!(
        control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .is_none()
    );
}

#[test]
fn keyboard_geometry_uses_shared_resolver_for_both_slots_zoom_fine_steps_and_hidden_sources() {
    use orchbee_workbench_protocol::{ClientLayoutChange, ClientPanelDirection};
    for zoom in [false, true] {
        for second in [false, true] {
            let (mut native, mailbox, clock, _) = ready_split_ui();
            let poll = mailbox.try_recv().unwrap().unwrap();
            mailbox.finish_lease(poll).unwrap();
            if second {
                crate::terminal_session::ui::tests::publication_hint::attach_split_peer(
                    &mut native,
                    &mailbox,
                    &clock,
                );
                native.apply_client_action(ClientAction::MovePanelFocus(PanelDirection::Right));
            }
            if zoom {
                native.apply_client_action(ClientAction::ToggleZoom);
            }
            let before = native.control_observation(false).unwrap();
            for (direction, fine, expected) in [
                (ClientPanelDirection::Left, false, 0.45),
                (ClientPanelDirection::Right, false, 0.55),
                (ClientPanelDirection::Left, true, 0.49),
                (ClientPanelDirection::Right, true, 0.51),
            ] {
                let (_, adjustment, observation) = native
                    .prepare_keyboard_adjustment(
                        direction,
                        fine,
                        Some((Rect::new(0, 0, 120, 40), SidebarSurface::Hidden)),
                    )
                    .unwrap();
                let ClientLayoutChange::Split { split } = adjustment.change else {
                    panic!()
                };
                assert!((split.ratio.value() - expected).abs() < 1e-9);
                assert_eq!(observation, before.1);
                assert!(adjustment.selected);
            }
            assert!(
                native
                    .prepare_keyboard_adjustment(
                        ClientPanelDirection::Right,
                        false,
                        Some((Rect::new(0, 0, 10, 3), SidebarSurface::Hidden))
                    )
                    .is_err()
            );
            assert_eq!(native.control_observation(false).unwrap(), before);
        }
    }
}

#[test]
fn keyboard_submits_once_drops_repeats_and_owns_no_remote_receipt_until_shared_draw() {
    use crate::terminal_session::ui::tests::publication_hint::{
        settle_queued_refresh, split_snapshot,
    };
    let (mut native, mailbox, clock, _) = ready_split_ui();
    let poll = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(poll).unwrap();
    let mut control = controller(clock.clone());
    control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
    control.tick(&mut native, false);
    let before = native.control_observation(false).unwrap();
    control.resize(&native, PanelDirection::Right, false);
    assert!(control.local_resize_pending());
    let mutation = control
        .lane
        .as_ref()
        .unwrap()
        .take_layout_for_test()
        .unwrap();
    assert_eq!(mutation.expected_revision, before.1.workbench_revision);
    assert!(
        matches!(mutation.action, WorkbenchAction::SplitRatioSet { ratio, .. } if (ratio.value() - 0.55).abs() < 1e-9)
    );
    for _ in 0..6 {
        control.resize(&native, PanelDirection::Right, false);
    }
    assert!(matches!(
        control.take_local_resize_feedback(),
        Some(LocalResizeFeedback::Notice(Notice::ResizeBusy))
    ));
    assert!(
        control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .is_none()
    );
    control.tick(&mut native, true); // resize mode is shell-blocked for remote commands, not for its own completion
    control.after_draw(&mut native);
    assert!(control.local_resize_pending());
    assert_eq!(native.control_observation(false).unwrap(), before);
    assert!(completion(&control).is_none());
    let command = command(&control, None);
    inject(&control, command);
    control.tick(&mut native, true);
    assert_eq!(
        completion(&control).unwrap().1.outcome,
        ClientViewCompletionOutcome::Rejected {
            reason: Reason::Busy
        }
    );
    assert!(control.local_resize_pending());
    control
        .lane
        .as_ref()
        .unwrap()
        .finish_layout_for_test(lane::LayoutCompletion {
            operation_id: mutation.request_id,
            reply: Ok(WorkbenchRevision::new(2)),
        });
    assert!(control.tick(&mut native, true));
    control.after_draw(&mut native);
    assert!(control.local_resize_pending(), "commit alone is not drawn");
    let mut snapshot = split_snapshot(3);
    let PanelNode::Split { ratio, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    *ratio = WorkbenchSplitRatio::new(0.55).unwrap();
    settle_queued_refresh(
        &mut native,
        &mailbox,
        &clock,
        WorkbenchSnapshotOutcome::Available { snapshot },
    );
    assert!(control.tick(&mut native, true));
    assert!(control.local_resize_pending());
    control.after_draw(&mut native);
    assert!(!control.local_resize_pending());
    assert!(matches!(
        control.take_local_resize_feedback(),
        Some(LocalResizeFeedback::Clear)
    ));
    assert_eq!(
        completion(&control).unwrap().1.outcome,
        ClientViewCompletionOutcome::Rejected {
            reason: Reason::Busy
        },
        "local success must not overwrite the remote rejection"
    );
    assert_eq!(
        native.control_observation(false).unwrap().1.selection,
        before.1.selection
    );
}

#[test]
fn failed_interrupted_and_stale_keyboard_completions_never_replay_or_complete_another_owner() {
    use crate::local_client::workbench::split_ratio::SplitRatioFailure;
    for ending in ["rejected", "unknown", "interrupted", "stale"] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock);
        control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
        control.tick(&mut native, false);
        control.resize(&native, PanelDirection::Right, true);
        let mutation = control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .unwrap();
        if ending == "interrupted" {
            control.interrupt();
            control.tick(&mut native, false);
        }
        control
            .lane
            .as_ref()
            .unwrap()
            .finish_layout_for_test(lane::LayoutCompletion {
                operation_id: if ending == "stale" {
                    WorkbenchRequestId::parse(&format!("{:032x}", 9999)).unwrap()
                } else {
                    mutation.request_id
                },
                reply: Err(SplitRatioFailure {
                    may_have_committed: ending == "unknown",
                    reason: Reason::StaleView,
                }),
            });
        control.tick(&mut native, false);
        if ending == "stale" {
            assert!(control.local_resize_pending());
            control.interrupt();
            control.tick(&mut native, false);
        }
        assert!(!control.local_resize_pending());
        assert!(
            matches!(control.take_local_resize_feedback(), Some(LocalResizeFeedback::Notice(n)) if n == if ending == "rejected" { Notice::ResizeFailed } else { Notice::ResizeUnknown })
        );
        assert!(completion(&control).is_none());
        assert!(
            control
                .lane
                .as_ref()
                .unwrap()
                .take_layout_for_test()
                .is_none()
        );
    }
}

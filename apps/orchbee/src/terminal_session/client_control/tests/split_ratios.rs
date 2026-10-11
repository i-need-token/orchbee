use super::*;
use orchbee_workbench_protocol::WorkbenchRevision;
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
        expected_split: None,
        terminal_context_id,
        tab_id,
        path: ClientSplitPath::new(vec![]).unwrap(),
        ratio: ClientSplitRatio::new(ratio).unwrap(),
    };
    command
}

#[test]
fn resize_submits_once_preserves_zoom_and_only_shared_draw_or_interruption_settles_it() {
    use crate::{
        client_view::SidebarSurface,
        terminal_session::ui::tests::publication_hint::{settle_queued_refresh, split_snapshot},
    };
    use orchbee_workbench_protocol::{
        ClientLayoutChange, ClientPanelDirection, ClientResizeAmount, PanelNode, WorkbenchAction,
        WorkbenchSnapshotOutcome, WorkbenchSplitRatio,
    };
    use ratatui::layout::Rect;
    for ending in ["draw", "interrupt", "expire"] {
        let (mut native, mailbox, clock, elapsed) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        native.apply_client_action(ClientAction::ToggleZoom);
        let mut control = controller(clock.clone());
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
        let mut snapshot = split_snapshot(1);
        let mut query = command.clone();
        query.intent = ClientViewIntent::Layout {
            terminal_context_id,
            tab_id,
            panel_id,
        };
        let lane = control.lane.as_ref().unwrap();
        lane.inject_delivery_for_test(lane::Delivery {
            command: command.clone(),
            workbench_snapshot: Some(snapshot.clone()),
        });
        assert!(!control.tick(&mut native, false));
        let mutation = control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .unwrap();
        assert_eq!(mutation.request_id, command.operation_id);
        assert_eq!(mutation.expected_revision, snapshot.revision);
        let WorkbenchAction::SplitRatioSet {
            previous_ratio,
            ratio,
            ..
        } = mutation.action
        else {
            panic!("canonical mutation")
        };
        assert_eq!(previous_ratio.value(), 0.5);
        assert_eq!(ratio.value(), 0.6);
        let deadline = control.pending.as_ref().unwrap().deadline;
        for _ in 0..3 {
            assert!(!control.tick(&mut native, false));
            control.after_draw(&mut native);
            assert!(completion(&control).is_none(), "no optimistic success");
            assert_eq!(native.control_observation(false).unwrap(), before);
            assert!(
                control
                    .lane
                    .as_ref()
                    .unwrap()
                    .take_layout_for_test()
                    .is_none()
            );
            assert!(mailbox.try_recv().unwrap().is_none());
        }
        control
            .lane
            .as_ref()
            .unwrap()
            .finish_layout_for_test(lane::LayoutCompletion {
                operation_id: command.operation_id.clone(),
                reply: Ok(WorkbenchRevision::new(2)),
            });
        assert!(
            control.tick(&mut native, false),
            "publication refresh queued"
        );
        control.after_draw(&mut native);
        assert!(
            completion(&control).is_none(),
            "backend commit is not a draw"
        );
        snapshot.revision = WorkbenchRevision::new(3); // unrelated metadata may advance revision
        let PanelNode::Split { ratio, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
            panic!()
        };
        *ratio = WorkbenchSplitRatio::new(0.6).unwrap();
        settle_queued_refresh(
            &mut native,
            &mailbox,
            &clock,
            WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot.clone(),
            },
        );
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
            let view = native.control_observation(false).unwrap().1;
            assert_eq!(view.selection, before.1.selection);
            assert_eq!(view.workbench_revision, snapshot.revision);
            assert_eq!(view.split_revision, Some(snapshot.revision));
            assert_eq!(control.pending.as_ref().unwrap().deadline, deadline);
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
        if ending == "draw" {
            control.after_draw(&mut native);
            let (sample, done) = completion(&control).unwrap();
            let ClientViewCompletionOutcome::LayoutAdjusted { adjustment } = done.outcome else {
                panic!()
            };
            let ClientLayoutChange::Split { split } = &adjustment.change else {
                panic!()
            };
            assert_eq!(split.ratio.value(), 0.6);
            assert_eq!(adjustment.workbench_revision, snapshot.revision);
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
        client_view::SidebarSurface, terminal_session::ui::tests::publication_hint::split_snapshot,
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
                workbench_snapshot: Some(split_snapshot(1)),
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
fn inactive_and_unvisited_shared_changes_acknowledge_without_selection_attachment_or_draw() {
    use crate::{
        client_view::SidebarSurface, terminal_session::ui::tests::publication_hint::split_snapshot,
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
            value["navigation"]["projects"] = serde_json::json!([
                {"projectId":format!("{:032x}",992),"label":"new"}
            ]);
            value["navigation"]["workspaces"] = serde_json::json!([
                {"workspaceId":format!("{:032x}",993),"projectId":format!("{:032x}",992),"label":"unvisited","primaryDirectoryId":null,"terminalContextId":context}
            ]);
            value["navigation"]["workspaceOrder"] = serde_json::json!([format!("{:032x}", 993)]);
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
            expected_split: None,
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
                workbench_snapshot: Some(snapshot.clone()),
            });
        assert!(
            !control.tick(&mut native, false),
            "inactive store cannot request shell geometry synchronization"
        );
        assert!(completion(&control).is_none());
        assert_eq!(native.control_observation(false).unwrap(), before);
        let mutation = control
            .lane
            .as_ref()
            .unwrap()
            .take_layout_for_test()
            .unwrap();
        assert_eq!(mutation.expected_revision, snapshot.revision);
        control
            .lane
            .as_ref()
            .unwrap()
            .finish_layout_for_test(lane::LayoutCompletion {
                operation_id: command.operation_id.clone(),
                reply: Ok(WorkbenchRevision::new(3)),
            });
        assert!(!control.tick(&mut native, false));
        let (sample, done) = completion(&control).unwrap();
        let ClientViewCompletionOutcome::LayoutAdjusted { adjustment } = done.outcome else {
            panic!("{:?}", done.outcome)
        };
        assert_eq!(adjustment.workbench_revision, WorkbenchRevision::new(3));
        snapshot.revision = WorkbenchRevision::new(3);
        let changed_tab = snapshot
            .terminal_contexts
            .iter_mut()
            .flat_map(|c| c.tabs.iter_mut())
            .find(|tab| tab.tab_id == target.tab_id)
            .unwrap();
        let PanelNode::Split { ratio, .. } = &mut changed_tab.root else {
            panic!()
        };
        *ratio = ClientSplitRatio::new(0.7).unwrap();
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
        assert_eq!(layout.focused_panel_id.as_ref(), Some(panel_id));
        assert!(!layout.selected);
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn ratio_paths_require_matching_rendered_and_canonical_split_identity_before_commit() {
    use crate::terminal_session::ui::tests::publication_hint::split_snapshot;
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
                workbench_snapshot: (case != "missing-snapshot").then_some(snapshot),
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

#[test]
fn ratio_creation_guard_rejects_each_replaced_identity_before_any_effect() {
    use crate::terminal_session::ui::tests::publication_hint::split_snapshot;
    use orchbee_workbench_protocol::{ClientSplitIdentity, PanelNode};
    for changed in ["splitId", "firstSlotId", "secondSlotId"] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock);
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let snapshot = split_snapshot(1);
        let PanelNode::Split {
            split_id, children, ..
        } = &snapshot.terminal_contexts[0].tabs[0].root
        else {
            panic!();
        };
        let mut identity = serde_json::json!({"splitId":split_id,"firstSlotId":children[0].slot_id,"secondSlotId":children[1].slot_id});
        identity[changed] = serde_json::json!(format!("{:032x}", 9981));
        let mut command = ratio_command(&control, 0.7);
        let ClientViewIntent::SetSplitRatio { expected_split, .. } = &mut command.intent else {
            panic!();
        };
        *expected_split = Some(serde_json::from_value::<ClientSplitIdentity>(identity).unwrap());
        control
            .lane
            .as_ref()
            .unwrap()
            .inject_delivery_for_test(lane::Delivery {
                command,
                workbench_snapshot: Some(snapshot),
            });
        assert!(!control.tick(&mut native, false));
        assert_eq!(
            completion(&control).unwrap().1.outcome,
            ClientViewCompletionOutcome::Rejected {
                reason: Reason::StaleView
            }
        );
        assert_eq!(native.control_observation(false).unwrap(), before);
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(control.pending.is_none());
    }
}

mod outcomes;

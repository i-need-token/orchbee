use super::*;
use crate::{
    client_view::ClientAction,
    local_client::test_support::server::disconnected_origin,
    workbench_client::ui::{
        tests::move_membership::ready_ui, tests::publication_hint::ready_split_ui,
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

#[test]
fn plugin_overlay_control_reserves_before_focus_and_requires_an_uninterrupted_draw() {
    use crate::workbench_client::ui::tests::publication_hint::{attach_split_peer, split_snapshot};
    use orchbee_workbench_protocol::{ClientOverlayMember, PanelNode};
    for case in ["applied", "interrupted", "wrong-binding"] {
        let (mut native, mailbox, clock, _) = ready_split_ui();
        let poll = mailbox.try_recv().unwrap().unwrap();
        mailbox.finish_lease(poll).unwrap();
        let mut control = controller(clock.clone());
        control.tick(&mut native, false);
        let before = native.control_observation(false).unwrap();
        let mut command = command(&control, None);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = command.intent
        else {
            unreachable!()
        };
        let snapshot = split_snapshot(1);
        let PanelNode::Split { children, .. } = &snapshot.terminal_contexts[0].tabs[0].root else {
            unreachable!()
        };
        let PanelNode::Panel {
            primary_presentation: source,
            ..
        } = &children[0].node
        else {
            unreachable!()
        };
        let PanelNode::Panel {
            panel_id: target,
            primary_presentation: member,
            ..
        } = &children[1].node
        else {
            unreachable!()
        };
        let mut expected_binding = member.binding.clone().unwrap();
        if case == "wrong-binding" {
            expected_binding.terminal_id =
                orchbee_workbench_protocol::TerminalId::parse(&format!("{:032x}", 999)).unwrap();
        }
        command.intent = ClientViewIntent::Overlay {
            terminal_context_id,
            tab_id,
            panel_id: target.clone(),
            presentation_id: member.presentation_id.clone(),
            expected_binding,
            return_to: ClientOverlayMember {
                panel_id,
                presentation_id: source.presentation_id.clone(),
                binding: source.binding.clone(),
            },
            previously_zoomed: false,
        };
        let target = target.clone();
        control.lane.as_ref().unwrap().inject_delivery_for_test(
            crate::workbench_client::client_control::Delivery {
                command,
                workbench_snapshot: Some(snapshot),
            },
        );
        control.tick(&mut native, false);
        if case == "wrong-binding" {
            assert_eq!(native.control_observation(false).unwrap(), before);
            assert_eq!(
                completion(&control).unwrap().1.outcome,
                ClientViewCompletionOutcome::Rejected {
                    reason: Reason::TargetUnavailable
                }
            );
            continue;
        }
        assert!(completion(&control).is_none());
        attach_split_peer(&mut native, &mailbox, &clock);
        // Focus, zoom and the observed draw candidate are distinct UI turns.
        for _ in 0..3 {
            control.tick(&mut native, false);
        }
        let selection = native
            .control_observation(false)
            .unwrap()
            .1
            .selection
            .unwrap();
        assert_eq!(selection.panel_id.as_ref(), Some(&target));
        assert!(selection.zoomed);
        assert!(completion(&control).is_none());
        if case == "interrupted" {
            control.interrupt();
            control.tick(&mut native, false);
        }
        control.after_draw(&mut native);
        assert_eq!(
            completion(&control).unwrap().1.outcome,
            if case == "applied" {
                ClientViewCompletionOutcome::Applied {}
            } else {
                ClientViewCompletionOutcome::Incomplete {
                    reason: Reason::Interrupted,
                }
            }
        );
    }
}
fn inject(control: &ClientControl, command: ClientViewControl) {
    let workbench_snapshot = matches!(command.intent, ClientViewIntent::Panel { .. }).then(|| {
        // These controller fixtures are either the two-Tab baseline or its split first Tab.
        let revision = command.minimum_workbench_revision.value();
        if control
            .observed
            .as_ref()
            .unwrap()
            .selection
            .as_ref()
            .unwrap()
            .panel_count
            == 2
        {
            crate::workbench_client::ui::tests::publication_hint::split_snapshot(revision)
        } else {
            crate::workbench_client::ui::tests::membership::snapshot(2, revision)
        }
    });
    control.lane.as_ref().unwrap().inject_delivery_for_test(
        crate::workbench_client::client_control::Delivery {
            command,
            workbench_snapshot,
        },
    );
}
fn completion(control: &ClientControl) -> Option<(Sample, ClientViewCompletion)> {
    control.lane.as_ref().unwrap().completion_for_test()
}

#[test]
fn decoded_local_input_reaches_pending_focus_before_terminal_routing() {
    use crate::terminal_input::InputDispatcher;
    use crate::workbench_client::{ClientBackend, WorkbenchClient};
    let cases: &[(&[u8], bool)] = &[
        (b"x", true),
        (b"\x1b[200~text\x1b[201~", true),
        (b"\x1b[5~", true),
        (b"\x02", true),
        (b"\x1b[<64;1;1M", true),
        (b"\x1b[<0;1;1M", true),
        (b"\x1b[<32;1;1M", true),
        (b"\x1b[<35;1;1M", false),
        (b"\x1b[<0;1;1m", false),
        (b"\x1b[97;1:3u", false),
        (b"\x1b[I\x1b[O", false),
        (b"\x1b[<0;0;0M", false),
    ];
    for focus_kind in ["agent", "panel", "zoom"] {
        for &(packet, intent) in cases {
            let (mut native, agent, clock) =
                crate::workbench_client::ui::focused_physical_agent_fixture();
            let mut control = controller(clock);
            control.tick(&mut native, false);
            let mut command = command(&control, None);
            if focus_kind == "zoom" {
                command.intent = match command.intent {
                    ClientViewIntent::Panel {
                        terminal_context_id,
                        tab_id,
                        panel_id,
                    } => ClientViewIntent::Zoom {
                        terminal_context_id,
                        tab_id,
                        panel_id,
                        enabled: false,
                    },
                    _ => unreachable!(),
                };
            } else if focus_kind == "agent" {
                let ClientViewIntent::Panel {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                } = command.intent
                else {
                    panic!()
                };
                command.intent = serde_json::from_value(serde_json::json!({
                    "kind":"agent-focus", "terminalContextId": terminal_context_id,
                    "tabId": tab_id, "panelId": panel_id, "presentationId":agent.binding.presentation_id,
                    "target":{"kind":"terminal","binding":agent.binding},
                })).unwrap();
            }
            inject(&control, command);
            control.tick(&mut native, false);
            assert!(control.pending.as_ref().unwrap().awaiting_draw);
            let (shutdown, _peer) = orchbee_local_ipc::LocalStream::pair().unwrap();
            let mut workbench_client = WorkbenchClient {
                position_writer: None,
                position_startup: None,
                configuration: None,
                notifications: None,
                window_title: None,
                client_control: Some(control),
                backend: ClientBackend::Native(Box::new(native)),
                task: None,
                shutdown: crate::local_client::test_support::server::shutdown_handle(shutdown),
                retirement_redraw_pending: false,
                origin: None,
                recovery_shutdown: None,
            };
            let mut view = crate::client_view::ClientView::new(120);
            InputDispatcher::default()
                .dispatch(
                    &mut view,
                    packet,
                    &mut workbench_client,
                    Instant::now(),
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
            assert_eq!(
                workbench_client
                    .client_control
                    .as_ref()
                    .unwrap()
                    .pending
                    .as_ref()
                    .unwrap()
                    .interrupted,
                focus_kind != "zoom" && intent,
                "focus_kind={focus_kind}, packet={packet:?}"
            );
            // Even an otherwise complete draw cannot acknowledge after the decoded intent.
            workbench_client.after_draw();
            if focus_kind != "zoom" && intent {
                let control = workbench_client.client_control.as_ref().unwrap();
                assert!(completion(control).is_none());
                assert!(control.pending.as_ref().unwrap().interrupted);
            }
        }
    }
}

#[test]
fn focus_mode_decisions_distinguish_owned_recovery_from_new_capture() {
    let mut modes = FocusModes::Unprepared;
    assert_eq!(modes.decide(true, true, true), ModeStep::Prepare);
    modes = FocusModes::Exiting;
    assert_eq!(modes.decide(false, false, true), ModeStep::Wait);
    assert_eq!(modes.decide(false, true, true), ModeStep::Interrupted);
    assert_eq!(modes.decide(true, false, false), ModeStep::Interrupted);
    assert_eq!(modes.decide(false, false, false), ModeStep::Navigate);
    modes = FocusModes::Finished;
    assert_eq!(modes.decide(false, false, true), ModeStep::Interrupted);
    assert_eq!(modes.decide(false, false, false), ModeStep::Navigate);
}

#[test]
fn panel_and_agent_focus_shell_exit_is_one_shot_and_retains_partial_outcomes() {
    for agent_focus in [false, true] {
        for result in [
            "applied",
            "new-mode",
            "input",
            "expired",
            "too-small",
            "history",
        ] {
            let (mut native, agent, clock) =
                crate::workbench_client::ui::focused_physical_agent_fixture();
            let mut control = controller(clock);
            if result == "history" {
                native.apply_client_action(ClientAction::EnterHistory);
                assert!(native.control_history());
            }
            control.observe_geometry(
                ratatui::layout::Rect::new(0, 0, if result == "too-small" { 1 } else { 120 }, 36),
                crate::client_view::SidebarSurface::Hidden,
            );
            control.tick(&mut native, true);
            let mut command = command(&control, None);
            if agent_focus {
                let ClientViewIntent::Panel {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                } = command.intent
                else {
                    panic!("Panel")
                };
                command.intent = ClientViewIntent::AgentFocus {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(
                        agent.binding.presentation_id.as_str(),
                    )
                    .unwrap(),
                    target: orchbee_agent_observation_protocol::AgentNameTarget::Terminal {
                        binding: agent.binding.clone(),
                    },
                };
            }
            inject(&control, command);
            control.tick(&mut native, true);
            assert_eq!(control.take_shell_mode_dismissal(), result != "too-small");
            assert!(!control.take_shell_mode_dismissal());
            control.after_draw(&mut native);
            if result != "too-small" {
                assert!(
                    completion(&control).is_none(),
                    "mode exit alone is not applied focus"
                );
                if result == "input" {
                    control.interrupt();
                }
                if result == "expired" {
                    control.pending.as_mut().unwrap().deadline = control.clock.now();
                }
                control.tick(&mut native, result == "new-mode");
                control.after_draw(&mut native);
                if result == "history" {
                    assert!(completion(&control).is_none());
                    assert!(native.control_history());
                    control.pending.as_mut().unwrap().deadline = control.clock.now();
                    control.tick(&mut native, false);
                }
            }
            let expected = match result {
                "applied" => ClientViewCompletionOutcome::Applied {},
                "too-small" => ClientViewCompletionOutcome::Rejected {
                    reason: Reason::Busy,
                },
                "expired" | "history" => ClientViewCompletionOutcome::Incomplete {
                    reason: Reason::Expired,
                },
                _ => ClientViewCompletionOutcome::Incomplete {
                    reason: Reason::Interrupted,
                },
            };
            assert_eq!(completion(&control).unwrap().1.outcome, expected);
            assert!(!control.take_shell_mode_dismissal());
            let attention = native
                .read_terminal_agent_attention(std::slice::from_ref(&agent.binding))
                .unwrap();
            assert_eq!(
                attention.entries[0].state.as_ref().unwrap().seen,
                result == "applied"
            );
        }
    }
}

#[test]
fn agent_focus_acknowledges_only_after_an_uninterrupted_unexpired_draw() {
    use orchbee_agent_observation_protocol::AgentNameTarget;
    for disposition in ["applied", "expired", "interrupted"] {
        let (mut native, agent, clock) =
            crate::workbench_client::ui::focused_physical_agent_fixture();
        native.observe_focus(false);
        let mut control = controller(clock);
        control.tick(&mut native, false);
        let mut command = command(&control, None);
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = command.intent
        else {
            panic!("Panel")
        };
        command.intent = ClientViewIntent::AgentFocus {
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(
                agent.binding.presentation_id.as_str(),
            )
            .unwrap(),
            target: AgentNameTarget::Terminal {
                binding: agent.binding.clone(),
            },
        };
        inject(&control, command);
        assert!(control.tick(&mut native, false));
        let seen = |native: &WorkbenchClientUi| {
            native
                .read_terminal_agent_attention(std::slice::from_ref(&agent.binding))
                .unwrap()
                .entries[0]
                .state
                .as_ref()
                .unwrap()
                .seen
        };
        assert!(!seen(&native));
        assert!(completion(&control).is_none());
        if disposition == "expired" {
            control.pending.as_mut().unwrap().deadline = control.clock.now();
        }
        if disposition == "interrupted" {
            control.interrupt();
        }
        control.after_draw(&mut native);
        if disposition == "interrupted" {
            assert!(!seen(&native));
            assert!(completion(&control).is_none());
            control.tick(&mut native, false);
        }
        let outcome = completion(&control).unwrap().1.outcome;
        assert_eq!(seen(&native), disposition == "applied");
        assert_eq!(
            outcome,
            match disposition {
                "applied" => ClientViewCompletionOutcome::Applied {},
                "expired" => ClientViewCompletionOutcome::Rejected {
                    reason: Reason::Expired
                },
                _ => ClientViewCompletionOutcome::Rejected {
                    reason: Reason::Interrupted
                },
            }
        );
        assert!(control.pending.is_none());
        control.after_draw(&mut native);
        assert_eq!(seen(&native), disposition == "applied");
    }
}

#[test]
fn ordinary_focus_acknowledges_an_unfocused_tab_only_after_a_valid_draw() {
    for kind in ["context", "tab", "panel"] {
        for ending in ["applied", "expired", "interrupted"] {
            let (mut native, agent, clock) =
                crate::workbench_client::ui::focused_physical_agent_fixture();
            native.observe_focus(false);
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
            command.intent = match kind {
                "context" => ClientViewIntent::Context {
                    terminal_context_id,
                },
                "tab" => ClientViewIntent::Tab {
                    terminal_context_id,
                    tab_id,
                },
                _ => ClientViewIntent::Panel {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                },
            };
            inject(&control, command);
            assert!(control.tick(&mut native, false));
            let seen = |native: &WorkbenchClientUi| {
                native
                    .read_terminal_agent_attention(std::slice::from_ref(&agent.binding))
                    .unwrap()
                    .entries[0]
                    .state
                    .as_ref()
                    .unwrap()
                    .seen
            };
            assert!(!seen(&native));
            assert!(completion(&control).is_none());
            if ending == "expired" {
                control.pending.as_mut().unwrap().deadline = control.clock.now();
            } else if ending == "interrupted" {
                control.interrupt();
            }
            control.after_draw(&mut native);
            if ending == "interrupted" {
                assert!(!seen(&native));
                control.tick(&mut native, false);
            }
            assert_eq!(seen(&native), ending == "applied", "{kind}/{ending}");
            assert_eq!(
                completion(&control).unwrap().1.outcome,
                match ending {
                    "applied" => ClientViewCompletionOutcome::Applied {},
                    "expired" => ClientViewCompletionOutcome::Rejected {
                        reason: Reason::Expired
                    },
                    _ => ClientViewCompletionOutcome::Rejected {
                        reason: Reason::Interrupted
                    },
                }
            );
            control.after_draw(&mut native);
            assert_eq!(seen(&native), ending == "applied");
        }
    }
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
    control.after_draw(&mut native);
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
        control.after_draw(&mut native);
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
    control.after_draw(&mut native);
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
        control.after_draw(&mut native);
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
        client_view::SidebarSurface, workbench_client::ui::tests::publication_hint::split_snapshot,
    };
    use ratatui::layout::Rect;
    for available in [false, true] {
        let (mut native, mailbox, clock, _elapsed) = ready_split_ui();
        let initial = mailbox.try_recv().unwrap().unwrap();
        assert!(matches!(
            initial.record(),
            crate::workbench_client::mailbox::QueuedCommand::Poll(_)
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
        control.lane.as_ref().unwrap().inject_delivery_for_test(
            crate::workbench_client::client_control::Delivery {
                command: command.clone(),
                workbench_snapshot: available.then(|| split_snapshot(1)),
            },
        );
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
        control.observe_geometry(
            Rect::new(0, 0, 80, 25),
            SidebarSurface::Docked(crate::tui_layout::SidebarWidth::DEFAULT),
        );
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
        client_view::SidebarSurface, workbench_client::ui::tests::publication_hint::split_snapshot,
    };
    use orchbee_workbench_protocol::ClientPanelDirection;
    use ratatui::layout::Rect;
    for shell_busy in [false, true] {
        for interrupted in [false, true] {
            let (mut native, mailbox, clock, _elapsed) = ready_split_ui();
            let poll = mailbox.try_recv().unwrap().unwrap();
            mailbox.finish_lease(poll).unwrap();
            native.apply_client_action(ClientAction::ToggleZoom);
            let mut control = controller(clock.clone());
            control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
            if shell_busy {
                assert!(native.request_rename_draft().is_some());
            }
            control.tick(&mut native, shell_busy);
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
            control.lane.as_ref().unwrap().inject_delivery_for_test(
                crate::workbench_client::client_control::Delivery {
                    command: command.clone(),
                    workbench_snapshot: Some(snapshot),
                },
            );
            assert_eq!(control.tick(&mut native, shell_busy), shell_busy);
            if shell_busy {
                assert!(control.take_shell_mode_dismissal());
                assert!(native.view().rename_draft().is_none());
                assert!(
                    control.tick(&mut native, false),
                    "mode exit publishes the changed view"
                );
                assert!(!control.pending.as_ref().unwrap().awaiting_draw);
            }
            assert!(
                completion(&control).is_none(),
                "pending sibling setup is not failure"
            );
            let deadline = control.pending.as_ref().unwrap().deadline;
            crate::workbench_client::ui::tests::publication_hint::attach_split_peer(
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
                control.after_draw(&mut native);
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
}

#[test]
fn no_neighbor_in_an_inactive_tab_has_no_selection_attachment_resize_or_draw_effect() {
    use crate::client_view::SidebarSurface;
    use crate::workbench_client::ui::tests::membership::snapshot;
    use crate::workbench_projection::project_preferred_workspace;
    use orchbee_workbench_protocol::ClientPanelDirection;
    use ratatui::layout::Rect;
    for shell_busy in [false, true] {
        for direction in [
            ClientPanelDirection::Left,
            ClientPanelDirection::Right,
            ClientPanelDirection::Up,
            ClientPanelDirection::Down,
        ] {
            let (mut native, mailbox, clock, _elapsed) = ready_ui();
            if shell_busy {
                assert!(native.request_rename_draft().is_some());
            }
            let before = native.control_observation(shell_busy).unwrap();
            let mut control = controller(clock);
            control.observe_geometry(Rect::new(0, 0, 120, 40), SidebarSurface::Hidden);
            control.tick(&mut native, shell_busy);
            // The source is in a newer Snapshot of an inactive Tab; current cache stays at revision 1.
            let snapshot = snapshot(2, 2);
            let cache = project_preferred_workspace(&snapshot).unwrap();
            let tab = &cache.tabs()[1];
            let mut command = command(&control, None);
            command.minimum_workbench_revision = snapshot.revision;
            command.intent = ClientViewIntent::FocusDirection {
                terminal_context_id: cache.terminal_context_id().unwrap().clone(),
                tab_id: tab.tab_id().clone(),
                panel_id: tab.panel(0).unwrap().panel_id().clone(),
                direction,
            };
            control.lane.as_ref().unwrap().inject_delivery_for_test(
                crate::workbench_client::client_control::Delivery {
                    command: command.clone(),
                    workbench_snapshot: Some(snapshot.clone()),
                },
            );
            assert!(
                !control.tick(&mut native, shell_busy),
                "no-neighbor cannot ask shell to synchronize sizes"
            );
            let (sample, done) = completion(&control).unwrap();
            assert_eq!(
                done.outcome,
                ClientViewCompletionOutcome::DirectionUnchanged {
                    layout_revision: snapshot.revision
                }
            );
            assert_eq!(native.control_observation(shell_busy).unwrap(), before);
            assert_eq!(sample.view, before.1);
            assert!(mailbox.try_recv().unwrap().is_none());
            assert!(control.pending.is_none());
            assert!(!control.take_shell_mode_dismissal());
            assert_eq!(native.view().rename_draft().is_some(), shell_busy);
            ClientViewOperation {
                command,
                status: ClientViewControlStatus::DirectionUnchanged {
                    layout_revision: snapshot.revision,
                    view: sample.view,
                },
            }
            .validate()
            .unwrap();
            assert!(!control.tick(&mut native, shell_busy));
            assert_eq!(completion(&control).unwrap().1, done, "status stays pinned");
        }
    }
}

#[test]
fn directional_unknown_geometry_never_claims_no_neighbor_or_changes_selection() {
    use crate::{
        client_view::SidebarSurface, workbench_client::ui::tests::publication_hint::split_snapshot,
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
        control.lane.as_ref().unwrap().inject_delivery_for_test(
            crate::workbench_client::client_control::Delivery {
                command,
                workbench_snapshot: snapshot,
            },
        );
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
        client_view::SidebarSurface, workbench_client::ui::tests::publication_hint::split_snapshot,
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
    control.lane.as_ref().unwrap().inject_delivery_for_test(
        crate::workbench_client::client_control::Delivery {
            command,
            workbench_snapshot: Some(split_snapshot(1)),
        },
    );
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

#[test]
fn presentation_read_completes_without_draw_or_navigation_in_ready_and_busy() {
    use crate::workbench_client::ui::tests::publication_hint::split_snapshot;
    for available in [false, true] {
        for shell_busy in [false, true] {
            let (mut native, mailbox, clock, _) = ready_split_ui();
            let initial = mailbox.try_recv().unwrap().unwrap();
            assert!(matches!(
                initial.record(),
                crate::workbench_client::mailbox::QueuedCommand::Poll(_)
            ));
            mailbox.finish_lease(initial).unwrap();
            let mut control = controller(clock);
            assert!(!control.tick(&mut native, shell_busy));
            let before = native.control_observation(shell_busy).unwrap();
            let mut command = command(&control, None);
            let ClientViewIntent::Panel { panel_id, .. } = command.intent else {
                panic!()
            };
            command.intent = ClientViewIntent::Presentation {
                panel_id: panel_id.clone(),
            };
            control.lane.as_ref().unwrap().inject_delivery_for_test(
                crate::workbench_client::client_control::Delivery {
                    command: command.clone(),
                    workbench_snapshot: available.then(|| split_snapshot(1)),
                },
            );
            assert!(!control.tick(&mut native, shell_busy));
            let (sample, completed) = completion(&control).unwrap();
            let status = match completed.outcome {
                ClientViewCompletionOutcome::PresentationRead { presentation } if available => {
                    assert_eq!(presentation.panel_id, panel_id);
                    assert!(presentation.presentation.is_some());
                    ClientViewControlStatus::PresentationRead {
                        presentation,
                        view: sample.view,
                    }
                }
                ClientViewCompletionOutcome::Rejected { reason } if !available => {
                    assert_eq!(reason, Reason::ProjectionUnavailable);
                    ClientViewControlStatus::Rejected { reason }
                }
                other => panic!("unexpected completion {other:?}"),
            };
            ClientViewOperation { command, status }.validate().unwrap();
            assert_eq!(native.control_observation(shell_busy).unwrap(), before);
            assert!(control.pending.is_none());
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}

#[test]
fn panel_focus_validates_canonical_target_before_cancelling_modes() {
    use crate::workbench_client::ui::tests::membership::snapshot;
    use orchbee_workbench_protocol::{SessionId, WorkbenchRevision};
    for case in [
        "valid",
        "missing",
        "wrong-session",
        "stale",
        "unknown-panel",
        "wrong-tab",
        "wrong-context",
    ] {
        let (mut native, mailbox, clock, _) = ready_ui();
        assert!(native.request_rename_draft().is_some());
        let mut control = controller(clock);
        control.tick(&mut native, true);
        let before = native.control_observation(true).unwrap();
        let mut command = command(&control, None);
        let mut canonical = snapshot(2, 1);
        match case {
            "wrong-session" => {
                canonical.session_id = SessionId::parse(&format!("{:032x}", 99)).unwrap()
            }
            "stale" => command.minimum_workbench_revision = WorkbenchRevision::new(2),
            "unknown-panel" | "wrong-tab" | "wrong-context" => {
                let ClientViewIntent::Panel {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                } = &mut command.intent
                else {
                    panic!()
                };
                match case {
                    "unknown-panel" => {
                        *panel_id =
                            serde_json::from_value(serde_json::json!(format!("{:032x}", 99)))
                                .unwrap()
                    }
                    "wrong-tab" => *tab_id = canonical.terminal_contexts[0].tabs[1].tab_id.clone(),
                    _ => {
                        *terminal_context_id =
                            serde_json::from_value(serde_json::json!(format!("{:032x}", 99)))
                                .unwrap()
                    }
                }
            }
            _ => {}
        }
        control.lane.as_ref().unwrap().inject_delivery_for_test(
            crate::workbench_client::client_control::Delivery {
                command,
                workbench_snapshot: (case != "missing").then_some(canonical),
            },
        );
        control.tick(&mut native, true);
        assert_eq!(
            control.take_shell_mode_dismissal(),
            case == "valid",
            "{case}"
        );
        assert_eq!(
            native.view().rename_draft().is_some(),
            case != "valid",
            "{case}"
        );
        if case == "valid" {
            assert!(completion(&control).is_none());
            control.tick(&mut native, false);
            control.after_draw(&mut native);
            assert_eq!(
                completion(&control).unwrap().1.outcome,
                ClientViewCompletionOutcome::Applied {}
            );
        } else {
            let reason = if matches!(case, "missing" | "wrong-session" | "stale") {
                Reason::ProjectionUnavailable
            } else {
                Reason::TargetUnavailable
            };
            assert_eq!(
                completion(&control).unwrap().1.outcome,
                ClientViewCompletionOutcome::Rejected { reason },
                "{case}"
            );
            assert_eq!(native.control_observation(true).unwrap(), before);
            assert!(control.pending.is_none());
        }
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn panel_focus_preflight_accepts_a_new_canonical_target_before_local_refresh() {
    use crate::workbench_client::ui::tests::membership::snapshot;
    use crate::workbench_projection::project_preferred_workspace;
    let (native, _, clock, _) = ready_ui();
    let mut control = controller(clock);
    control.observed = Some(native.control_observation(false).unwrap().1);
    control.revision = 1;
    let mut target = command(&control, None);
    let canonical = snapshot(3, 2);
    let cache = project_preferred_workspace(&canonical).unwrap();
    let tab = &cache.tabs()[2];
    target.minimum_workbench_revision = canonical.revision;
    target.intent = ClientViewIntent::Panel {
        terminal_context_id: cache.terminal_context_id().unwrap().clone(),
        tab_id: tab.tab_id().clone(),
        panel_id: tab.panel(0).unwrap().panel_id().clone(),
    };
    assert_eq!(
        prepare_control(&native, &target, Some(&canonical), None).unwrap(),
        target
    );
}

mod split_ratios;

mod background_read;
mod divider;
mod keyboard;

use super::*;
use crate::workbench_client::mailbox::{
    CommandMailbox, EventDeadline, MutationResult, QueuedCommand, UiReply, WorkerEvent,
};
use crate::workbench_client::ui::DisplayPhase;
use crate::workbench_client::ui::tests::context_selection::make_desired_ready;
use crate::workbench_client::ui::tests::move_membership::ready_ui_with_snapshot;
use crate::workbench_state::{ControllerMode, FrameIdentity, RefreshDecision};
use orchbee_workbench_protocol::{SessionId, TerminalId, WorkbenchSnapshotOutcome};

fn first() -> crate::machine_agents::Target {
    let mut target = crate::machine_agents::test_support::row(12).target;
    let snapshot = crate::machine_agents::test_support::fixture();
    target.tab = snapshot.terminal_contexts[0].tabs[0].tab_id.clone();
    let orchbee_workbench_protocol::PanelNode::Panel { panel_id, .. } =
        &snapshot.terminal_contexts[0].tabs[0].root
    else {
        panic!("panel");
    };
    target.panel = panel_id.clone();
    target
}

fn install_matching_projection(ui: &mut WorkbenchClientUi, target: &crate::machine_agents::Target) {
    use orchbee_client_server_protocol::HostTerminalId;
    use orchbee_terminal_protocol::HostEpoch;
    crate::workbench_client::ui::tests::viewport::install_projection(ui, None, 0);
    let projection = ui.projections.last_mut().unwrap();
    projection.value = crate::terminal_projection::TerminalProjection::new(
        *projection.value.attachment(),
        HostEpoch::parse(target.physical.host_epoch.as_str()).unwrap(),
    )
    .with_host_terminal(HostTerminalId::parse(target.physical.host_terminal_id.as_str()).unwrap());
    projection.effect_stream = crate::workbench_client::terminal_effects::Stream::new(
        *projection.value.attachment(),
        *projection.value.epoch(),
    );
}

fn settle_switch(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &crate::local_client::LocalOperationClock,
) {
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Switch(marker) = lease.record() else {
        panic!("one exact member switch");
    };
    let target = marker.target().clone();
    let permit = mailbox
        .authorize_switch_snapshot(&lease, &target, clock)
        .unwrap()
        .unwrap();
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: crate::machine_agents::test_support::fixture(),
                },
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("switch settlement");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("old viewer cleanup precedes installation");
    };
    let permit = mailbox
        .authorize_switch_cleanup(&lease, cleanup, cleanup.old(), clock)
        .unwrap()
        .expect("the exact switch still owns cleanup");
    assert!(mailbox.answer_switch_cleanup(permit, true).unwrap());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Switch(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("membership settlement");
    };
    let RefreshDecision::AttachRecovered(installed) = settlement.decision() else {
        panic!("exact attachment");
    };
    assert_eq!(
        installed.expected_host, target.expected_host,
        "the Host fence survives cleanup"
    );
    assert_eq!(installed.binding(), target.binding());
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.is_closed().is_none());
    assert!(!ui.retirement_requested());
}

#[test]
fn direct_agent_guards_identity_and_busy_before_current_then_selects_exact_child() {
    let (mut ui, mailbox, clock, _) =
        ready_ui_with_snapshot(crate::machine_agents::test_support::fixture());
    let original = first();
    let destination = crate::machine_agents::test_support::target();
    install_matching_projection(&mut ui, &original);
    assert_eq!(ui.navigate_agent(&original), Admission::Current);
    for invalid in 0..3 {
        let mut target = destination.clone();
        match invalid {
            0 => target.context.session_id = SessionId::parse(&format!("{:032x}", 999)).unwrap(),
            1 => target.terminal.terminal_id = TerminalId::parse(&format!("{:032x}", 999)).unwrap(),
            _ => target.member = original.member.clone(),
        }
        assert_eq!(ui.navigate_agent(&target), Admission::Unavailable);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
    assert_eq!(ui.navigate_agent(&destination), Admission::Admitted);
    assert_eq!(ui.navigate_agent(&original), Admission::Busy);
    assert_eq!(ui.navigate_agent(&destination), Admission::Busy);
    assert!(!ui.forward(b"cannot reach the previous terminal"));
    settle_switch(&mut ui, &mailbox, &clock);
    let view = ui.view();
    assert_eq!(view.phase(), DisplayPhase::Switching);
    assert_eq!(
        view.focused_member().unwrap().presentation,
        destination.member
    );
    assert_eq!(view.focus_panel_id(), Some(&destination.panel));
    make_desired_ready(&mut ui, 2);
    install_matching_projection(&mut ui, &destination);
    assert_eq!(ui.navigate_agent(&destination), Admission::Current);
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(ui.navigate_agent(&original), Admission::Admitted);
    settle_switch(&mut ui, &mailbox, &clock);
    make_desired_ready(&mut ui, 3);
    install_matching_projection(&mut ui, &original);
    assert_eq!(ui.navigate_agent(&original), Admission::Current);
}

#[test]
fn direct_cross_context_agent_installs_exact_child_without_intermediate_default_member() {
    use orchbee_workbench_protocol::{WorkbenchTerminalContextId, WorkbenchWorkspaceId, Workspace};
    let mut snapshot = crate::machine_agents::test_support::fixture();
    let mut workspace = snapshot.terminal_contexts[0].clone();
    snapshot.terminal_contexts[0].tabs.truncate(1);
    workspace.tabs.remove(0);
    workspace.terminal_context_id =
        WorkbenchTerminalContextId::parse(&format!("{:032x}", 40)).unwrap();
    let workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 41)).unwrap();
    snapshot.navigation.workspaces.push(Workspace {
        project_id: None,
        label: "other".into(),
        primary_directory_id: None,
        workspace_id: workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    });
    snapshot.navigation.workspace_order = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    snapshot.terminal_contexts.push(workspace.clone());
    let (mut ui, mailbox, clock, _) = ready_ui_with_snapshot(snapshot.clone());
    let mut target = crate::machine_agents::test_support::target();
    target.context.location = crate::workbench_projection::ClientContextLocation::Workspace {
        workspace_id,
        terminal_context_id: workspace.terminal_context_id,
    };
    assert_eq!(ui.navigate_agent(&target), Admission::Admitted);
    assert_eq!(ui.view().active_context_position(), Some(1));
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one context snapshot");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_refresh_result_before(
            marker.generation(),
            WorkbenchSnapshotOutcome::Available { snapshot },
            marker.context_deadlines().unwrap().forward_cutoff(),
            &clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Refresh(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap() else {
        panic!("context settlement");
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("cleanup before context commit");
    };
    assert_eq!(ui.view().active_context_position(), Some(1));
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    let UiReply::Membership(settlement) = mailbox.take_deposited_reply(&wait).unwrap().unwrap()
    else {
        panic!("exact context commit");
    };
    let RefreshDecision::AttachRecovered(attach) = settlement.decision() else {
        panic!("attach chosen child");
    };
    assert_eq!(attach.expected_host.as_deref(), Some(&target.physical));
    assert_eq!(
        attach.binding().unwrap().terminal_id(),
        &target.terminal.terminal_id
    );
    assert_eq!(ui.view().active_context_position(), Some(2));
    assert_eq!(
        ui.view().focused_member().unwrap().presentation,
        target.member
    );
    assert!(ui.view().pane_projection(&target.panel).is_none());
    assert!(!ui.forward(b"no stale input route"));
    mailbox.finish_lease(lease).unwrap();
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(2)));
    assert!(state.publish_ready_t(&key, FrameIdentity::new(2), ControllerMode::ReadOnly, None));
    install_matching_projection(&mut ui, &target);
    assert_eq!(ui.navigate_agent(&target), Admission::Current);
}

#[test]
fn current_agent_with_same_canonical_binding_but_different_host_requires_fenced_switch() {
    use orchbee_agent_observation_protocol::{AgentHostEpoch, AgentHostTerminalId};
    for mismatch in 0..3 {
        let (mut ui, mailbox, _, _) =
            ready_ui_with_snapshot(crate::machine_agents::test_support::fixture());
        let mut target = first();
        if mismatch != 2 {
            install_matching_projection(&mut ui, &target);
        }
        match mismatch {
            0 => {
                target.physical.host_epoch =
                    AgentHostEpoch::parse(&format!("{:032x}", 999)).unwrap()
            }
            1 => {
                target.physical.host_terminal_id =
                    AgentHostTerminalId::parse(&format!("{:032x}", 999)).unwrap()
            }
            _ => {}
        }
        assert_eq!(ui.navigate_agent(&target), Admission::Admitted);
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Switch(marker) = lease.record() else {
            panic!("physical mismatch must reattach");
        };
        assert_eq!(
            marker.target().expected_host.as_deref(),
            Some(&target.physical)
        );
        assert!(!ui.forward(b"old Host is not input authority"));
    }
}

#[test]
fn same_tab_agent_focus_requires_the_live_peer_host_before_local_focus() {
    use crate::workbench_client::ui::tests::publication_hint::{attach_split_peer, ready_split_ui};
    use orchbee_agent_observation_protocol::{AgentHostEpoch, AgentHostTerminalId};
    use orchbee_client_server_protocol::HostTerminalId;
    for mismatch in [false, true] {
        let (mut ui, mailbox, clock, _) = ready_split_ui();
        attach_split_peer(&mut ui, &mailbox, &clock);
        let state = ui.state.as_ref().unwrap();
        let tab = &state.cache().tabs()[0];
        let panel = tab.panel(1).unwrap();
        let member = state.selected_presentation(panel);
        let binding = member.binding().unwrap();
        let mut target = crate::machine_agents::test_support::target();
        target.context = crate::workbench_projection::ClientContextTarget {
            session_id: state.cache().session_id().clone(),
            location: state.cache().location().clone(),
        };
        target.tab = tab.tab_id().clone();
        target.panel = panel.panel_id().clone();
        target.member = member.presentation_id().clone();
        target.terminal = orchbee_workbench_protocol::PresentationBinding {
            worker_id: binding.worker_id().clone(),
            terminal_id: binding.terminal_id().clone(),
        };
        let pane = state.pane_by_panel(&target.panel).unwrap().pane_ref();
        let projection = ui.projections.iter_mut().find(|p| p.key == pane).unwrap();
        target.physical.host_epoch =
            AgentHostEpoch::parse(&projection.value.epoch().to_text()).unwrap();
        let terminal = format!("{:032x}", 9000);
        target.physical.host_terminal_id = AgentHostTerminalId::parse(&terminal).unwrap();
        projection.value = crate::terminal_projection::TerminalProjection::new(
            *projection.value.attachment(),
            *projection.value.epoch(),
        )
        .with_host_terminal(HostTerminalId::parse(&terminal).unwrap());
        if mismatch {
            target.physical.host_terminal_id =
                AgentHostTerminalId::parse(&format!("{:032x}", 9001)).unwrap();
        }
        let previous = ui
            .state
            .as_ref()
            .unwrap()
            .desired()
            .unwrap()
            .anchor()
            .clone();
        assert_eq!(ui.navigate_agent(&target), Admission::Admitted);
        if mismatch {
            let lease = mailbox.try_recv().unwrap().unwrap();
            let QueuedCommand::Switch(marker) = lease.record() else {
                panic!("mismatched peer Host must reattach");
            };
            assert_eq!(
                marker.target().expected_host.as_deref(),
                Some(&target.physical)
            );
            assert_ne!(
                ui.state.as_ref().unwrap().desired().unwrap().anchor(),
                &previous
            );
        } else {
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "the exact live peer only moves focus"
            );
            assert_eq!(
                ui.state.as_ref().unwrap().desired().unwrap().anchor(),
                &previous
            );
            assert_eq!(ui.view().focus_panel_id(), Some(&target.panel));
        }
    }
}

#[test]
fn direct_host_replacement_discards_old_pixels_using_shared_viewport_geometry() {
    use crate::client_view::ClientView;
    use orchbee_agent_observation_protocol::AgentHostTerminalId;
    use ratatui::layout::Rect;
    for mismatch in [false, true] {
        let (mut ui, _, _, _) =
            ready_ui_with_snapshot(crate::machine_agents::test_support::fixture());
        let mut target = first();
        install_matching_projection(&mut ui, &target);
        let client = ClientView::new(120);
        let areas = crate::tui_layout::project_shell_areas(
            Rect::new(0, 0, 120, 32),
            client.sidebar_surface(),
        );
        let area = crate::tui_layout::arranged_pane_area(&areas, &ui.view()).unwrap();
        let projection = ui.projections.last_mut().unwrap();
        let mut frame = crate::workbench_client::ui::tests::viewport::frame(
            &projection.value,
            0,
            false,
            area.height,
        );
        frame.geometry.columns = area.width;
        frame.geometry.rows = area.height;
        projection.value.install_frame_for_test(frame);
        ui.after_draw();
        ui.projections.last_mut().unwrap().status =
            crate::workbench_client::ui::ClientProjectionStatus::WaitingForFullFrame;
        assert!(
            ui.view()
                .retained_pane_frame(&target.panel, area.width, area.height)
                .is_some()
        );
        if mismatch {
            target.physical.host_terminal_id =
                AgentHostTerminalId::parse(&format!("{:032x}", 999)).unwrap();
        }
        assert_eq!(
            ui.navigate_agent(&target),
            if mismatch {
                Admission::Admitted
            } else {
                Admission::Current
            }
        );
        assert_eq!(
            ui.view()
                .retained_pane_frame(&target.panel, area.width, area.height)
                .is_none(),
            mismatch
        );
    }
}

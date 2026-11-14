use super::super::{tests::move_membership::ready_ui, tests::viewport::install_projection};
use super::*;
use crate::workbench_client::agent_read_lane::AttentionRound;
use crate::{
    client_view::ClientView, localization::Locale, tui_hit_map::HitMapBuilder,
    workbench_client::DrawReceipt,
};
use orchbee_agent_observation_protocol::{AgentChangesCursor, AgentSessionSummary};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

fn enable_shared_notifications(
    ui: &mut WorkbenchClientUi,
    settings: &orchbee_local_runtime::NotificationSettings,
) {
    ui.apply_user_preferences(&orchbee_workbench_protocol::UserSettingsValues {
        agent_notification_mode: orchbee_workbench_protocol::AgentReminderMode::Hidden,
        sound_enabled: settings.sound_enabled,
        ..orchbee_workbench_protocol::UserSettingsValues::default()
    });
    ui.configure_agent_notifications(1, settings);
}

#[test]
fn focused_completion_first_paint_has_no_unread_flash_and_seen_still_waits_for_the_draw() {
    use orchbee_workbench_protocol::{AgentCompletion, PanelAgentState, WorkbenchRevision};
    let (mut ui, pane, mut state) = physical_fixture();
    ui.observe_focus(true);
    state.sequence = WorkbenchRevision::new(2);
    state.state = PanelAgentState::Idle;
    state.completion = AgentCompletion::AwaitingInput;
    publish_physical(&mut ui, &state);
    assert!(!physical_seen(&ui, &state));
    {
        let mut view = ui.view();
        view.set_attention_visible(true);
        assert!(view.will_see_agent(&state.binding, state.sequence));
        assert_eq!(view.panel_unseen_count(pane.pane().panel()), 0);
    }
    assert!(
        !physical_seen(&ui, &state),
        "visual suppression must not pre-acknowledge a failed draw"
    );
    ui.after_close_draw(draw(&ui, 120, 40, &ClientView::new(120)));
    assert!(physical_seen(&ui, &state));
}

#[test]
fn shared_never_remains_disabled_after_legacy_notification_reload() {
    use orchbee_workbench_protocol::{
        AgentReminderMode, PanelAgentState, UserSettingsValues, WorkbenchRevision,
    };
    let settings = orchbee_local_runtime::NotificationSettings::from_profile(
        "[ui.toast]\nagent_delivery='orchbee'\ndelay_seconds=0\n[ui.sound]\nenabled=true",
    )
    .unwrap();
    for enabled in [false, true] {
        let (mut ui, _, mut state) = physical_fixture();
        ui.apply_user_preferences(&UserSettingsValues {
            agent_notification_mode: if enabled {
                AgentReminderMode::Hidden
            } else {
                AgentReminderMode::Never
            },
            sound_enabled: true,
            ..UserSettingsValues::default()
        });
        ui.configure_agent_notifications(2, &settings);
        ui.observe_focus(false);
        state.sequence = WorkbenchRevision::new(2);
        state.state = PanelAgentState::Working;
        publish_physical(&mut ui, &state);
        state.sequence = WorkbenchRevision::new(3);
        state.state = PanelAgentState::Idle;
        publish_physical(&mut ui, &state);
        ui.after_close_draw(draw(&ui, 8, 2, &ClientView::new(8)));
        assert_eq!(
            ui.take_agent_notification(Locale::EnUs, true).is_some(),
            enabled
        );
    }
}

fn fixture() -> (WorkbenchClientUi, PaneRef) {
    let (mut ui, _, _, _) = ready_ui();
    let pane = install_projection(&mut ui, Some((9, false, 24)), 0);
    let presentation = ui.view().attention_presentation(&pane).unwrap();
    let row: AgentSessionSummary = serde_json::from_value(json!({
        "executionLocation":{"executionLocationId":"local"}, "lifecycle":"live",
        "observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":1,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,
        "presentationAttachments":[{"generation":"g1","navigable":true,"presentationId":presentation,"visibility":"visible"}],
        "providerId":"test","sessionId":"agent-1","status":{"kind":"succeeded"}
    })).unwrap();
    ui.attention.publish_for_test(&AttentionRound {
        items: vec![row],
        changes: vec![],
        prior_revision: None,
        revision: 1,
        cursor: AgentChangesCursor::parse(&format!("v1.{}.{}", "A".repeat(59), "A".repeat(43)))
            .unwrap(),
        reset: true,
    });
    (ui, pane)
}

fn draw(ui: &WorkbenchClientUi, width: u16, height: u16, shell: &ClientView) -> DrawReceipt {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                shell,
                Locale::EnUs,
                Some(ui.view()),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    receipt
}

fn physical_fixture() -> (
    WorkbenchClientUi,
    PaneRef,
    orchbee_workbench_protocol::TerminalAgentStateSnapshot,
) {
    let (mut ui, _, _, _) = ready_ui();
    let pane = install_projection(&mut ui, Some((9, false, 24)), 0);
    let presentation = ui.view().attention_presentation(&pane).unwrap();
    let projection = &mut ui.projections[0].value;
    let frame = projection.take_frame().unwrap();
    let state:orchbee_workbench_protocol::TerminalAgentStateSnapshot=serde_json::from_value(json!({
        "binding":{"presentationId":presentation,"generation":"11111111111111111111111111111111",
        "hostEpoch":projection.epoch().to_text(),"hostTerminalId":"22222222222222222222222222222222"},
        "sequence":"1","agent":"pi","state":"idle"
    })).unwrap();
    *projection = crate::terminal_projection::TerminalProjection::new(
        *projection.attachment(),
        *projection.epoch(),
    )
    .with_host_terminal(
        orchbee_client_server_protocol::HostTerminalId::parse(
            state.binding.host_terminal_id.as_str(),
        )
        .unwrap(),
    );
    projection.install_frame_for_test(frame);
    ui.attention.publish_physical_for_test(
        &crate::workbench_client::agent_attention::PhysicalRound {
            report_revision: orchbee_workbench_protocol::WorkbenchRevision::new(0),
            states: vec![],
        },
    );
    publish_physical(&mut ui, &state);
    (ui, pane, state)
}

pub(in crate::workbench_client) fn focused_physical_agent_fixture() -> (
    WorkbenchClientUi,
    orchbee_workbench_protocol::TerminalAgentStateSnapshot,
    crate::local_client::LocalOperationClock,
) {
    let (ui, _, state) = physical_fixture();
    let clock = ui.clock.clone();
    (ui, state, clock)
}
fn publish_physical(
    ui: &mut WorkbenchClientUi,
    state: &orchbee_workbench_protocol::TerminalAgentStateSnapshot,
) {
    ui.attention.publish_physical_for_test(
        &crate::workbench_client::agent_attention::PhysicalRound {
            report_revision: state.sequence,
            states: vec![state.clone()],
        },
    );
}
fn physical_seen(
    ui: &WorkbenchClientUi,
    state: &orchbee_workbench_protocol::TerminalAgentStateSnapshot,
) -> bool {
    ui.read_terminal_agent_attention(std::slice::from_ref(&state.binding))
        .unwrap()
        .entries[0]
        .state
        .as_ref()
        .unwrap()
        .seen
}

#[test]
fn hidden_physical_transition_emits_after_draw_but_visible_transition_is_seen() {
    use orchbee_local_runtime::{NotificationDelivery as Delivery, NotificationSettings};
    use orchbee_workbench_protocol::{NotificationSound, PanelAgentState, WorkbenchRevision};

    let settings =
        NotificationSettings::from_profile("[ui.toast]\nagent_delivery='orchbee'\ndelay_seconds=0")
            .unwrap();
    let shell = ClientView::new(120);

    let (mut background, _, mut state) = physical_fixture();
    enable_shared_notifications(&mut background, &settings);
    background.observe_focus(false);
    state.binding.presentation_id = orchbee_agent_observation_protocol::AgentPresentationId::parse(
        background
            .view()
            .tab(1)
            .unwrap()
            .panel(0)
            .unwrap()
            .presentation_id()
            .as_str(),
    )
    .unwrap();
    state.sequence = WorkbenchRevision::new(2);
    state.state = PanelAgentState::Working;
    publish_physical(&mut background, &state);
    state.sequence = WorkbenchRevision::new(3);
    state.state = PanelAgentState::Idle;
    publish_physical(&mut background, &state);
    assert!(
        background
            .take_agent_notification(Locale::EnUs, true)
            .is_none(),
        "even zero delay waits for a successful post-transition draw"
    );
    background.after_close_draw(draw(&background, 120, 40, &shell));
    let (delivery, message, _) = background
        .take_agent_notification(Locale::EnUs, true)
        .unwrap();
    assert_eq!(delivery, Delivery::Tui);
    assert_eq!(message.title, "pi finished");
    assert_eq!(message.sound, NotificationSound::None);
    assert!(
        message
            .body
            .as_deref()
            .is_some_and(|body| body.contains("Tab"))
    );

    let (mut visible, _, mut state) = physical_fixture();
    enable_shared_notifications(&mut visible, &settings);
    visible.observe_focus(true);
    state.sequence = WorkbenchRevision::new(2);
    state.state = PanelAgentState::Working;
    publish_physical(&mut visible, &state);
    state.sequence = WorkbenchRevision::new(3);
    state.state = PanelAgentState::Idle;
    publish_physical(&mut visible, &state);
    visible.after_close_draw(draw(&visible, 120, 40, &shell));
    assert!(physical_seen(&visible, &state));
    assert!(
        visible
            .take_agent_notification(Locale::EnUs, true)
            .is_none()
    );
}

#[test]
fn unfocused_exact_visible_paint_suppresses_only_that_notification_version() {
    use orchbee_local_runtime::NotificationSettings;
    use orchbee_workbench_protocol::{NotificationSound, PanelAgentState, WorkbenchRevision};

    let settings = NotificationSettings::from_profile(
        "[ui.toast]\nagent_delivery='orchbee'\ndelay_seconds=0\n[ui.sound]\nenabled=true",
    )
    .unwrap();
    let shell = ClientView::new(120);
    let (mut ui, _, mut state) = physical_fixture();
    enable_shared_notifications(&mut ui, &settings);
    ui.observe_focus(false);

    state.sequence = WorkbenchRevision::new(2);
    state.state = PanelAgentState::Working;
    publish_physical(&mut ui, &state);
    state.sequence = WorkbenchRevision::new(3);
    state.state = PanelAgentState::Idle;
    publish_physical(&mut ui, &state);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(
        !physical_seen(&ui, &state),
        "an unfocused paint must preserve the attention badge"
    );
    assert!(
        ui.take_agent_notification(Locale::EnUs, true).is_none(),
        "the exact visible version must suppress both popup and sound"
    );
    publish_physical(&mut ui, &state);
    assert!(
        ui.take_agent_notification(Locale::EnUs, true).is_none(),
        "repeating the same unseen sequence must not undo paint suppression"
    );

    state.sequence = WorkbenchRevision::new(4);
    state.state = PanelAgentState::Working;
    publish_physical(&mut ui, &state);
    state.sequence = WorkbenchRevision::new(5);
    state.state = PanelAgentState::Idle;
    publish_physical(&mut ui, &state);
    ui.after_close_draw(draw(&ui, 8, 2, &shell));
    assert!(!physical_seen(&ui, &state));
    let (_, message, _) = ui.take_agent_notification(Locale::EnUs, true).unwrap();
    assert_eq!(message.sound, NotificationSound::Done);
}

#[test]
fn shared_hidden_request_uses_localized_context_and_per_agent_sound_mute_is_hard() {
    use orchbee_local_runtime::{NotificationDelivery as Delivery, NotificationSettings};
    use orchbee_workbench_protocol::{NotificationSound, PanelAgentState, WorkbenchRevision};

    for (override_line, expected) in [("", true), ("[ui.sound.agents]\npi='off'", false)] {
        let settings = NotificationSettings::from_profile(&format!(
            "[ui.toast]\nagent_delivery='off'\ndelay_seconds=0\n[ui.sound]\nenabled=true\n{override_line}"
        ))
        .unwrap();
        let (mut ui, _, mut state) = physical_fixture();
        enable_shared_notifications(&mut ui, &settings);
        ui.observe_focus(false);
        state.binding.presentation_id =
            orchbee_agent_observation_protocol::AgentPresentationId::parse(
                ui.view()
                    .tab(1)
                    .unwrap()
                    .panel(0)
                    .unwrap()
                    .presentation_id()
                    .as_str(),
            )
            .unwrap();
        state.sequence = WorkbenchRevision::new(2);
        state.state = PanelAgentState::Idle;
        publish_physical(&mut ui, &state);
        state.sequence = WorkbenchRevision::new(3);
        state.state = PanelAgentState::Blocked;
        publish_physical(&mut ui, &state);
        let shell = ClientView::new(120);
        ui.after_close_draw(draw(&ui, 120, 40, &shell));
        let notification = ui.take_agent_notification(Locale::ZhCn, true);
        assert!(notification.is_some());
        if let Some((delivery, message, target)) = notification {
            assert_eq!(delivery, Delivery::Tui);
            // The card opens the Panel hosting the observed presentation, in a background Tab.
            let tab = ui.view().tab(1).unwrap();
            let target = target.expect("the observed presentation has a canonical placement");
            assert_eq!(&target.tab, tab.tab_id());
            assert_eq!(&target.panel, tab.panel(0).unwrap().panel_id());
            assert_eq!(target.physical, state.binding);
            assert_eq!(message.title, "pi 需要处理");
            assert_eq!(
                message.sound,
                if expected {
                    NotificationSound::Request
                } else {
                    NotificationSound::None
                }
            );
            assert!(
                message
                    .body
                    .as_deref()
                    .is_some_and(|body| body.contains("标签页"))
            );
        }
    }
}

#[test]
fn machine_view_reads_exact_physical_attention_without_consuming_it() {
    let (mut ui, _, mut state) = physical_fixture();
    let bindings = [state.binding.clone()];
    let seen = |ui: &WorkbenchClientUi| {
        ui.view()
            .machine_agent_attention(&bindings)
            .unwrap()
            .entries[0]
            .state
            .as_ref()
            .unwrap()
            .seen
    };
    assert!(!seen(&ui));
    assert!(!seen(&ui), "reading a view cannot acknowledge it");
    let shell = ClientView::new(120);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(seen(&ui));
    state.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    publish_physical(&mut ui, &state);
    assert!(!seen(&ui));
    ui.observe_focus(false);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(!seen(&ui));
    ui.observe_focus(true);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(seen(&ui));
    let mut other = state.binding.clone();
    other.generation =
        orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&"9".repeat(32))
            .unwrap();
    assert!(
        ui.view().machine_agent_attention(&[other]).unwrap().entries[0]
            .state
            .is_none()
    );
}

#[test]
fn overlay_sidebar_cannot_acknowledge_covered_physical_terminals() {
    for managed in [false, true] {
        let (mut ui, _, mut state) = physical_fixture();
        state.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
        publish_physical(&mut ui, &state);
        let mut shell = ClientView::new(120);
        shell.machines.available = managed;
        assert!(shell.overlay().is_none());
        assert!(!physical_seen(&ui, &state));
        ui.after_close_draw(draw(&ui, 80, 40, &shell));
        assert!(
            !physical_seen(&ui, &state),
            "a narrow host overlays the sidebar on the terminal"
        );
        ui.after_close_draw(draw(&ui, 120, 12, &shell));
        assert!(
            physical_seen(&ui, &state),
            "a short host with room docks the sidebar beside the terminal"
        );
    }
}

fn focus_command(
    ui: &WorkbenchClientUi,
    target: orchbee_agent_observation_protocol::AgentNameTarget,
    presentation: &str,
) -> orchbee_workbench_protocol::ClientViewControl {
    let (_, view) = ui.control_observation(false).unwrap();
    let selection = view.selection.unwrap();
    serde_json::from_value(json!({"operationId":"1".repeat(32),"clientId":"2".repeat(32),
        "expectedViewRevision":"1","minimumWorkbenchRevision":view.workbench_revision,
        "intent":{"kind":"agent-focus","terminalContextId":selection.terminal_context_id,
            "tabId":selection.tab_id,"panelId":selection.panel_id,"presentationId":presentation,"target":target}})).unwrap()
}

#[test]
fn agent_focus_cancels_only_after_validating_its_source_and_does_not_discard_history() {
    use crate::terminal_viewport::ViewportStep;
    use crate::workbench_client::mailbox::QueuedCommand;
    let (mut ui, _, agent) = physical_fixture();
    let command = focus_command(
        &ui,
        orchbee_agent_observation_protocol::AgentNameTarget::Terminal {
            binding: agent.binding.clone(),
        },
        agent.binding.presentation_id.as_str(),
    );
    let mut stale = command.clone();
    let orchbee_workbench_protocol::ClientViewIntent::AgentFocus { target, .. } = &mut stale.intent
    else {
        panic!()
    };
    *target =
        serde_json::from_value(json!({"kind":"session","sessionId":"missing-agent"})).unwrap();
    assert!(ui.request_rename_draft().is_some());
    ui.board.open_picker(None, false);
    assert_eq!(
        ui.dismiss_focus_modes(&stale),
        Err(orchbee_workbench_protocol::ClientViewControlReason::ProjectionUnavailable)
    );
    assert!(ui.view().rename_draft().is_some());
    assert!(ui.view().agent_board().is_some());
    assert_eq!(ui.dismiss_focus_modes(&command), Ok(true));
    assert!(ui.view().rename_draft().is_none());
    assert!(ui.view().agent_board().is_none());

    ui.request_close_confirmation();
    assert!(ui.view().close_confirmation().is_some());
    assert_eq!(ui.dismiss_focus_modes(&command), Ok(true));
    assert!(ui.view().close_confirmation().is_none());
    assert!(
        ui.request_setup_draft(
            crate::workbench_state::workspace_setup::SetupIntent::TabGroupName(Box::new(
                orchbee_workbench_protocol::TabConfiguration::CreateGroup {
                    tab_id: ui.state.as_ref().unwrap().cache().tabs()[0]
                        .tab_id()
                        .clone(),
                    label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse("Group")
                        .unwrap(),
                    color: orchbee_workbench_protocol::TabGroupColor::Blue,
                }
            ))
        )
        .is_some()
    );
    assert_eq!(ui.dismiss_focus_modes(&command), Ok(true));
    assert!(ui.view().setup_draft().is_none());

    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
    ui.enter_history();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
    assert!(ui.control_history());
    assert_eq!(ui.dismiss_focus_modes(&command), Ok(true));
    let mut steps = vec![];
    while let Some(lease) = mailbox.try_recv().unwrap() {
        let QueuedCommand::Viewport(tagged) = lease.record() else {
            panic!("unexpected command: {:?}", lease.record())
        };
        steps.push(*tagged.value());
        mailbox.finish_lease(lease).unwrap();
    }
    assert_eq!(steps, [ViewportStep::Recover]);
    assert!(
        ui.control_history(),
        "core is retained until recovery and a new live frame"
    );
    assert!(ui.projections[0].viewport.as_ref().unwrap().recovering());
    assert!(!physical_seen(&ui, &agent));
    let key = ui.projections[0].key.clone();
    let before = ui.projections[0].viewport.as_mut().unwrap().frame_applied();
    assert!(!before.live, "an earlier frame cannot complete recovery");
    ui.apply_viewport_outcome(
        &mailbox,
        key.clone(),
        crate::workbench_client::mailbox::ViewportOutcome::Recovered,
    );
    assert!(ui.control_history());
    let after = ui.projections[0].viewport.as_mut().unwrap().frame_applied();
    ui.settle_viewport_effect(&mailbox, &key, after);
    assert!(!ui.control_history());
}

#[test]
fn explicit_agent_focus_acknowledges_an_already_focused_agent_despite_outer_focus_loss() {
    use crate::workbench_client::ui::control_steps::{ControlStep, StepProgress};
    use orchbee_agent_observation_protocol::AgentNameTarget;
    let (mut ui, pane) = fixture();
    let command = focus_command(
        &ui,
        AgentNameTarget::Session {
            session_id: serde_json::from_value(json!("agent-1")).unwrap(),
        },
        &ui.view().attention_presentation(&pane).unwrap(),
    );
    ui.observe_focus(false);
    assert!(matches!(
        ui.control_step(&command, &mut StepProgress::default()),
        ControlStep::Observed
    ));
    let shell = ClientView::new(120);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert_eq!(
        ui.view().panel_unseen_count(pane.pane().panel()),
        1,
        "ordinary unfocused paint never acknowledges"
    );
    ui.finish_focus(&command).unwrap();
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 0);

    let (mut ui, _, mut state) = physical_fixture();
    let command = focus_command(
        &ui,
        AgentNameTarget::Terminal {
            binding: state.binding.clone(),
        },
        state.binding.presentation_id.as_str(),
    );
    ui.observe_focus(false);
    assert!(matches!(
        ui.control_step(&command, &mut StepProgress::default()),
        ControlStep::Observed
    ));
    assert!(!physical_seen(&ui, &state));
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(!physical_seen(&ui, &state));
    ui.finish_focus(&command).unwrap();
    assert!(physical_seen(&ui, &state));
    state.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    publish_physical(&mut ui, &state);
    assert!(
        !physical_seen(&ui, &state),
        "explicit focus cannot pre-acknowledge future changes"
    );
}

#[test]
fn agent_focus_refuses_replacement_and_requires_the_matching_live_frame() {
    use orchbee_agent_observation_protocol::{AgentHostTerminalId, AgentNameTarget};
    use orchbee_workbench_protocol::ClientViewControlReason as Reason;
    let (mut ui, _, mut state) = physical_fixture();
    let command = focus_command(
        &ui,
        AgentNameTarget::Terminal {
            binding: state.binding.clone(),
        },
        state.binding.presentation_id.as_str(),
    );
    state.binding.host_terminal_id = AgentHostTerminalId::parse(&"3".repeat(32)).unwrap();
    state.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    publish_physical(&mut ui, &state);
    assert_eq!(ui.finish_focus(&command), Err(Reason::TargetUnavailable));
    let replacement = focus_command(
        &ui,
        AgentNameTarget::Terminal {
            binding: state.binding.clone(),
        },
        state.binding.presentation_id.as_str(),
    );
    assert!(ui.agent_focus_source(&replacement).is_ok());
    assert_eq!(
        ui.finish_focus(&replacement),
        Err(Reason::TargetUnavailable),
        "old frame belongs to another terminal in the same Host"
    );
    assert!(!physical_seen(&ui, &state));
    ui.projections[0].status = ClientProjectionStatus::Empty;
    assert_eq!(
        ui.finish_focus(&replacement),
        Err(Reason::ProjectionUnavailable)
    );
    assert!(!physical_seen(&ui, &state));
}

#[test]
fn ordinary_panel_focus_marks_only_the_focused_panel_current_version() {
    use crate::workbench_client::agent_attention::PhysicalRound;
    use crate::workbench_client::ui::tests::publication_hint::ready_split_ui;
    use orchbee_workbench_protocol::{TerminalAgentStateSnapshot, WorkbenchRevision};
    let (mut ui, _, _, _) = ready_split_ui();
    let view = ui.view();
    let tab = view.tab(0).unwrap();
    let mut states: Vec<TerminalAgentStateSnapshot> = (0..tab.panel_count()).map(|index| {
        serde_json::from_value(json!({"binding":{"presentationId":tab.panel(index).unwrap().presentation_id(),
            "generation":"1".repeat(32),"hostEpoch":"2".repeat(32),"hostTerminalId":format!("{:032x}",index+1)},
            "sequence":"1","agent":"pi","state":"idle"})).unwrap()
    }).collect();
    let mut outside = states[0].clone();
    outside.binding.presentation_id = serde_json::from_value(json!("9".repeat(32))).unwrap();
    states.push(outside.clone());
    ui.attention.publish_physical_for_test(&PhysicalRound {
        report_revision: WorkbenchRevision::new(0),
        states: vec![],
    });
    ui.attention.publish_physical_for_test(&PhysicalRound {
        report_revision: WorkbenchRevision::new(1),
        states: states.clone(),
    });
    ui.observe_focus(false);
    let (_, view) = ui.control_observation(false).unwrap();
    let selection = view.selection.unwrap();
    let command = serde_json::from_value(json!({
        "operationId":"1".repeat(32), "clientId":"2".repeat(32), "expectedViewRevision":"1",
        "minimumWorkbenchRevision":view.workbench_revision,
        "intent":{"kind":"panel", "terminalContextId":selection.terminal_context_id,
            "tabId":selection.tab_id, "panelId":selection.panel_id}
    }))
    .unwrap();
    ui.finish_focus(&command).unwrap();
    let focused = ui.view().focus_panel_id().unwrap().clone();
    let tab = ui.view().tab(0).unwrap();
    let focused_presentation = (0..tab.panel_count())
        .filter_map(|index| tab.panel(index))
        .find(|panel| panel.panel_id() == &focused)
        .unwrap()
        .presentation_id()
        .clone();
    for state in &states {
        assert_eq!(
            physical_seen(&ui, state),
            state.binding.presentation_id.as_str() == focused_presentation.as_str()
        );
    }
    assert!(!physical_seen(&ui, &outside));
    for state in &mut states {
        state.sequence = WorkbenchRevision::new(2);
    }
    ui.attention.publish_physical_for_test(&PhysicalRound {
        report_revision: WorkbenchRevision::new(2),
        states: states.clone(),
    });
    assert!(states.iter().all(|state| !physical_seen(&ui, state)));
}

#[test]
fn physical_paint_requires_exact_terminal_and_current_visible_frame() {
    let (mut ui, pane, mut state) = physical_fixture();
    let shell = ClientView::new(120);
    assert!(!physical_seen(&ui, &state), "reading does not acknowledge");
    ui.board.open_picker(None, false);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    ui.board.close();
    assert!(!physical_seen(&ui, &state));
    let old = draw(&ui, 120, 40, &shell);
    state.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    publish_physical(&mut ui, &state);
    ui.after_close_draw(old);
    assert!(!physical_seen(&ui, &state));
    ui.observe_focus(false);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(!physical_seen(&ui, &state));
    ui.observe_focus(true);
    ui.enter_history();
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(!physical_seen(&ui, &state));
    // Reinstall a live projection without the attach identity: epoch alone is insufficient.
    install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(!physical_seen(&ui, &state));
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
}

#[test]
fn physical_terminal_replacement_under_same_epoch_cannot_consume_old_frame() {
    let (mut ui, _, mut state) = physical_fixture();
    let shell = ClientView::new(120);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(physical_seen(&ui, &state));
    state.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    state.binding.host_terminal_id =
        orchbee_agent_observation_protocol::AgentHostTerminalId::parse(&"3".repeat(32)).unwrap();
    state.binding.generation =
        orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&"4".repeat(32))
            .unwrap();
    publish_physical(&mut ui, &state);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert!(!physical_seen(&ui, &state));
}

#[test]
fn attention_render_receipts_only_acknowledge_visible_live_panes_after_success() {
    let (mut ui, pane) = fixture();
    let shell = ClientView::new(120);
    let count = |ui: &WorkbenchClientUi| ui.view().panel_unseen_count(pane.pane().panel());
    let receipt = draw(&ui, 120, 40, &shell);
    assert_eq!(count(&ui), 1, "rendering alone never acknowledges");
    ui.observe_focus(false);
    ui.after_close_draw(receipt);
    assert_eq!(count(&ui), 1);
    ui.observe_focus(true);
    assert!(ui.attention.tick(), "focus gain requests a fresh paint");
    assert!(
        !ui.attention.tick(),
        "one focus event cannot sustain a redraw loop"
    );
    ui.after_close_draw(draw(&ui, 8, 2, &shell));
    assert_eq!(count(&ui), 1, "too small has no visible pane content");
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert_eq!(count(&ui), 0);
}

#[test]
fn attention_hidden_history_overlay_and_changed_pane_reject_receipts() {
    let (mut ui, pane) = fixture();
    let shell = ClientView::new(120);
    ui.board.open_picker(None, false);
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
    ui.board.close();
    let receipt = draw(&ui, 120, 40, &shell);
    ui.projections[0].status = ClientProjectionStatus::Empty;
    ui.after_close_draw(receipt);
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
    ui.projections[0].status = ClientProjectionStatus::FrameAvailable;
    ui.enter_history();
    assert!(ui.view().pane_viewport(pane.pane().panel()).is_some());
    ui.after_close_draw(draw(&ui, 120, 40, &shell));
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
}

#[test]
fn attention_query_preserves_unseen_state_and_distinguishes_missing_sessions() {
    use orchbee_agent_observation_protocol::AgentSessionId;
    let (mut ui, pane) = fixture();
    let sessions = [
        AgentSessionId::parse("missing").unwrap(),
        AgentSessionId::parse("agent-1").unwrap(),
    ];
    ui.board.open_picker(None, false);
    let snapshot = ui.read_agent_attention(&sessions).unwrap();
    assert!(snapshot.answers(&sessions));
    assert_eq!(snapshot.outer_focused, None);
    assert_eq!(snapshot.entries[0].state, None);
    assert!(!snapshot.entries[1].state.as_ref().unwrap().seen);
    assert_eq!(ui.view().panel_unseen_count(pane.pane().panel()), 1);
    ui.attention.close();
    assert!(ui.read_agent_attention(&sessions).is_none());
}

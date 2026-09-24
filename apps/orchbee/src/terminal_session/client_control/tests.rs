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

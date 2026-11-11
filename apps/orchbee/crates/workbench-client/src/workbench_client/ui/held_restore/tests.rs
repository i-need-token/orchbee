use super::*;
use crate::client_view::{ClientAction, PanelDirection};
use crate::workbench_client::ui::tests::publication_hint::{ready_ui_over, split_snapshot};
use crate::workbench_state::ControllerMode;
use orchbee_workbench_protocol::{
    PanelNode, PresentationLaunchFailure, Snapshot, WorkbenchDirectoryPath,
};

/// `split_snapshot` with its second leaf held by a restore that found its saved directory gone.
fn held_peer_snapshot() -> Snapshot {
    let mut snapshot = split_snapshot(1);
    let PanelNode::Split { children, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
        unreachable!("the split fixture");
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        unreachable!("the split fixture's second leaf");
    };
    primary_presentation.binding = None;
    primary_presentation.launch_failure =
        Some(Box::new(PresentationLaunchFailure::directory_unavailable(
            WorkbenchDirectoryPath::parse("C:/saved/a").unwrap(),
            false,
        )));
    snapshot
}

/// Focus reaches a held Panel from its live split sibling by panel identity (a pointer press or
/// `panel focus`) and by direction (`Ctrl+B` and an arrow key). Either way the held Panel is
/// selected without attaching: the Client stays connected, the sibling keeps its viewer, the
/// placeholder shows with keyboard focus and takes the keys, and focus returns to the sibling.
#[test]
fn a_held_panel_focused_from_its_live_sibling_keeps_the_client_and_shows_its_placeholder() {
    let routes: [fn(&WorkbenchPanelId) -> ClientAction; 2] = [
        |held| ClientAction::FocusPanel(held.clone()),
        |_| ClientAction::MovePanelFocus(PanelDirection::Right),
    ];
    for route in routes {
        let (mut ui, mailbox, _clock, _elapsed) =
            ready_ui_over(&held_peer_snapshot(), ControllerMode::ReadOnly);
        let state = ui.state.as_ref().unwrap();
        let live = state.focus_panel_id().unwrap().clone();
        let live_ref = state.focus_pane_ref().unwrap();
        let held = state.cache().tabs()[0].panel(1).unwrap().panel_id().clone();

        ui.apply_client_action(route(&held));

        assert_eq!(mailbox.is_closed(), None, "the Client stays connected");
        assert!(ui.retirement_failure.is_none());
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.focus_panel_id(), Some(&held));
        assert_eq!(
            state.phase_kind(),
            ClientPhaseKind::SelectedWithoutAttachment
        );
        assert!(
            state.pane_attached(&live_ref),
            "the sibling keeps its viewer"
        );
        assert!(state.invariant_holds());
        let placeholder = |ui: &WorkbenchClientUi| {
            let panel = ui.state.as_ref().unwrap().cache().tabs()[0]
                .panel(1)
                .unwrap();
            ui.view().held_restore(panel).and_then(|held| held.focused)
        };
        let before = placeholder(&ui).expect("the focused placeholder shows");
        assert_eq!(ui.held_restore_key(b"\t"), Some(true));
        assert_ne!(
            placeholder(&ui),
            Some(before),
            "the placeholder takes the keys"
        );

        ui.apply_client_action(ClientAction::FocusPanel(live.clone()));
        assert_eq!(mailbox.is_closed(), None);
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.focus_panel_id(), Some(&live));
        assert_eq!(state.focus_pane_ref(), Some(live_ref));
        assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    }
}

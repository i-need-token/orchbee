use super::super::test_support::{controlling_ui, install_projection};
use super::super::*;
use super::normal_exit_hold::{exited, poll};
use crate::action_menu::MenuAction;
use crate::localization::CloseImpact;

const fn impact(running: usize, ended: usize) -> CloseImpact {
    CloseImpact { running, ended }
}

fn tab_impact(ui: &WorkbenchClientUi) -> CloseImpact {
    ui.view().close_confirmation().unwrap().impact()
}

/// The selected Tab's attached terminal is the only one whose settlement this Client observes;
/// the background Tab stays running until the Server closes it.
#[test]
fn a_close_names_only_settled_terminals_as_ended_and_every_unobserved_one_as_running() {
    let (mut ui, mailbox, clock, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((0, false, 24)), 0);
    let ids: Vec<_> = (0..2)
        .map(|index| ui.view().tab(index).unwrap().tab_id().clone())
        .collect();

    ui.apply_client_action(ClientAction::RequestCloseTab);
    assert_eq!(tab_impact(&ui), impact(1, 0));
    // A settlement arriving under an open prompt changes the next draw's copy, never its target.
    let token = ui.view().close_confirmation().unwrap().token();
    let record = exited(&ui, &key, 1);
    poll(&mut ui, &mailbox, &clock, &key, record);
    let confirmation = ui.view().close_confirmation().unwrap();
    assert_eq!(confirmation.token(), token);
    assert_eq!(confirmation.impact(), impact(0, 1));
    ui.cancel_close();

    // The Tab's only Panel counts the same settled terminal.
    ui.apply_client_action(ClientAction::ClosePanel);
    assert_eq!(
        ui.view().panel_close_confirmation().unwrap().impact(),
        impact(0, 1)
    );
    ui.cancel_panel_close();

    for (action, target, expected) in [
        // Closing the other Tab of the background Tab closes only the exited one.
        (MenuAction::CloseOtherTabs, &ids[1], impact(0, 1)),
        // Closing to the right of the exited Tab closes only the unobserved one.
        (MenuAction::CloseTrailingTabs, &ids[0], impact(1, 0)),
    ] {
        ui.apply_client_action(ClientAction::TabMenu {
            action: action.clone(),
            tab_id: Some(target.clone()),
        });
        let confirmation = ui.view().close_confirmation().unwrap();
        assert_eq!(confirmation.set_count(), Some(1), "{action:?}");
        assert_eq!(confirmation.impact(), expected, "{action:?}");
        ui.cancel_close();
    }

    ui.state
        .as_mut()
        .unwrap()
        .mark_tabs(&ids[0], super::super::super::state::TabSelectionChange::All)
        .unwrap();
    ui.apply_client_action(ClientAction::TabMenu {
        action: MenuAction::CloseSelectedTabs,
        tab_id: Some(ids[0].clone()),
    });
    assert_eq!(tab_impact(&ui), impact(1, 1));
}

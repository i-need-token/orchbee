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
        .mark_tabs(&ids[0], crate::workbench_state::TabSelectionChange::All)
        .unwrap();
    ui.apply_client_action(ClientAction::TabMenu {
        action: MenuAction::CloseSelectedTabs,
        tab_id: Some(ids[0].clone()),
    });
    assert_eq!(tab_impact(&ui), impact(1, 1));
}

/// Every close prompt names an automatic Tab exactly as the Tab strip does, not by its ordinal.
#[test]
fn close_prompts_name_an_automatic_tab_as_the_tab_strip_does() {
    use crate::localization::Locale;
    use orchbee_workbench_protocol::{PanelNode, PresentationLaunchProgram};
    let mut source = super::membership::snapshot(2, 1);
    for tab in &mut source.terminal_contexts[0].tabs {
        tab.automatic_label = true;
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut tab.root
        else {
            unreachable!()
        };
        primary_presentation.launch_program =
            Some(PresentationLaunchProgram::parse("pwsh.exe").unwrap());
    }
    let (mut ui, _, _, _) = super::move_membership::ready_ui_with_snapshot(source);
    install_projection(&mut ui, Some((0, false, 24)), 0);
    let ids = ui.view().selected_tab_ids();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert_eq!(
            ui.view().tab(0).unwrap().display_label(locale),
            "PowerShell"
        );
    }

    let _ = ui.request_panel_close_confirmation();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let view = ui.view();
        let named = view.panel_close_confirmation().unwrap().tab_label(locale);
        assert_eq!(named, view.tab(0).unwrap().display_label(locale));
    }
    ui.state.as_mut().unwrap().cancel_panel_close_confirmation();

    let _ = ui.request_close_confirmation();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let view = ui.view();
        let named = view.close_confirmation().unwrap().target_label(locale);
        assert_eq!(named, view.tab(0).unwrap().display_label(locale));
    }
    ui.state.as_mut().unwrap().cancel_close_confirmation();

    ui.request_close_set(&ids).unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let view = ui.view();
        let labels = view
            .close_confirmation()
            .unwrap()
            .set_labels(locale)
            .unwrap();
        assert_eq!(labels.len(), ids.len());
        assert!(labels.iter().all(|(_, label)| label == "PowerShell"));
    }
}

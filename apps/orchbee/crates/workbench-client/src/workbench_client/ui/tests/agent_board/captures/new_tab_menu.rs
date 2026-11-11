//! The Tab strip's new-Tab menu lists Agents after its shells and starts one as the picker's
//! new-Tab place does, from raw terminal bytes through the production dispatcher.

use super::*;
use crate::action_menu::{MenuAction, MenuTarget};
use crate::workbench_client::mailbox::QueuedCommand;

/// Opens the new-Tab menu as a right-click on `[+]` asks for it, then paints it once.
fn open_menu(ui: &mut WorkbenchClientUi, route: &TerminalRoute) -> Vec<MenuAction> {
    let create = ui
        .view()
        .tab_create_target()
        .expect("the Tab strip paints [+]");
    assert!(ui.open_action_menu(MenuTarget::TabProfiles(create), None, false));
    render_board_frame(ui, 120, 36, crate::localization::Locale::EnUs, &route.theme).settle(ui);
    menu_actions(ui)
}

fn menu_actions(ui: &WorkbenchClientUi) -> Vec<MenuAction> {
    ui.view()
        .action_menu()
        .expect("the new-Tab menu is open")
        .actions
        .to_vec()
}

/// Moves the menu's candidate onto `wanted` with Down arrows.
fn select(ui: &mut WorkbenchClientUi, route: &mut TerminalRoute, wanted: &MenuAction) {
    let actions = menu_actions(ui);
    let target = actions
        .iter()
        .position(|action| action == wanted)
        .expect("the row is listed");
    let selected = ui.view().action_menu().unwrap().selected;
    for _ in selected..target {
        route.send(ui, b"\x1b[B");
    }
    let menu = ui.view();
    let menu = menu.action_menu().unwrap();
    assert_eq!(menu.actions.get(menu.selected), Some(wanted));
}

/// The Agent rows follow the shells: the found providers, then "More Agents…".
fn agent_rows(actions: &[MenuAction]) -> Vec<MenuAction> {
    actions
        .iter()
        .filter(|action| !matches!(action, MenuAction::NewTabWithProfile(_)))
        .cloned()
        .collect()
}

fn queued_agent_start(ui: &WorkbenchClientUi) -> Option<&'static str> {
    let mut lease = ui
        .mailbox
        .as_ref()
        .unwrap()
        .try_recv()
        .unwrap()
        .expect("a Tab creation is queued");
    let QueuedCommand::Create(marker) = lease.record_mut() else {
        panic!("the queued command creates a Tab");
    };
    marker.operation().agent_start()
}

/// A terminal's Enter is a bare CR, the canonical `Ctrl+M`: on an Agent row it asks for a new Tab
/// whose shell runs that Agent, closes the menu, opens no picker and types into no Panel.
#[test]
fn terminal_enter_on_an_agent_row_starts_it_in_a_new_tab() {
    let (mut ui, mut route) = TerminalRoute::ready(&["codex", "gemini"]);
    let actions = open_menu(&mut ui, &route);
    assert_eq!(
        agent_rows(&actions),
        [
            MenuAction::StartAgentInTab("codex"),
            MenuAction::StartAgentInTab("gemini"),
            MenuAction::MoreAgents,
        ]
    );
    select(&mut ui, &mut route, &MenuAction::StartAgentInTab("gemini"));
    route.send(&mut ui, b"\r");

    assert!(ui.view().action_menu().is_none(), "Enter closes the menu");
    assert!(!ui.board.is_open(), "no picker opens");
    assert!(route.forwarded.is_empty(), "no key reached the Panel");
    assert_eq!(
        ui.board.start_operation_panel_t(),
        None,
        "nothing is typed into the focused Panel"
    );
    assert_eq!(queued_agent_start(&ui), Some("gemini"));
}

/// "More Agents…" opens the picker alone with a new Tab as its place, although the focused idle
/// Panel would be its automatic place.
#[test]
fn terminal_enter_on_more_agents_opens_the_picker_at_a_new_tab() {
    let (mut ui, mut route) = TerminalRoute::ready(&["codex"]);
    open_menu(&mut ui, &route);
    select(&mut ui, &mut route, &MenuAction::MoreAgents);
    route.send(&mut ui, b"\r");

    assert!(ui.view().action_menu().is_none(), "Enter closes the menu");
    assert_eq!(
        picker_place(&ui),
        Some(crate::agent_board::start::StartPlace::NewTab)
    );
    assert!(route.forwarded.is_empty(), "no key reached the Panel");
}

/// The rows are fixed when the menu opens: a lookup result that arrives while it is open neither
/// inserts a row nor moves the candidate, and Enter starts the provider it rests on.
#[test]
fn a_late_lookup_result_leaves_the_open_menu_unchanged() {
    let (mut ui, mut route) = TerminalRoute::ready(&[]);
    ui.board.retire_start_catalog();
    ui.board.begin_start_lookup_t(&["codex"]);
    let opened = open_menu(&mut ui, &route);
    assert_eq!(
        agent_rows(&opened),
        [MenuAction::StartAgentInTab("codex"), MenuAction::MoreAgents],
        "the first lookup's findings so far, and always More Agents…"
    );
    select(&mut ui, &mut route, &MenuAction::StartAgentInTab("codex"));

    // Amp sorts before Codex, so the picker would now list it first.
    assert!(ui.board.find_start_provider_t("amp"));
    assert_eq!(ui.board.menu_agents(), ["amp", "codex"]);
    route.send(&mut ui, b"");
    assert_eq!(menu_actions(&ui), opened, "the open menu keeps its rows");

    route.send(&mut ui, b"\r");
    assert_eq!(queued_agent_start(&ui), Some("codex"));
}

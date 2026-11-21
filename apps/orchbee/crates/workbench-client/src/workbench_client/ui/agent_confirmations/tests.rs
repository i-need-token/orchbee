//! What this Client says and offers about answered confirmations, from the Server's rows alone.
use super::*;
use crate::action_menu::MenuRowNote;
use orchbee_workbench_protocol::{
    AgentConfirmationKind, AgentConfirmationPress, PanelNode, Snapshot, TerminalId, WorkerId,
};

fn primary(source: &Snapshot, tab: usize) -> (TerminalPresentationId, PresentationBinding) {
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &source.terminal_contexts[0].tabs[tab].root
    else {
        unreachable!("a one-Panel Tab");
    };
    (
        primary_presentation.presentation_id.clone(),
        primary_presentation.binding.clone().unwrap(),
    )
}

fn view(choice: AgentConfirmationChoice, offer: bool, serial: u32) -> AgentConfirmationView {
    AgentConfirmationView {
        agent: "claude".to_owned(),
        choice,
        offer,
        wait: None,
        pressed: (serial > 0).then(|| AgentConfirmationPress {
            serial,
            kind: AgentConfirmationKind::Command,
            text: format!("command {serial}"),
        }),
        stopped: false,
    }
}

fn row(
    (presentation_id, binding): &(TerminalPresentationId, PresentationBinding),
    view: AgentConfirmationView,
) -> PresentationConfirmationRow {
    PresentationConfirmationRow {
        presentation_id: presentation_id.clone(),
        binding: Some(binding.clone()),
        confirmation: Some(view),
    }
}

/// Two one-Panel Tabs, the first active, and the terminal each Tab shows.
fn two_tabs() -> (
    WorkbenchClientUi,
    [(TerminalPresentationId, PresentationBinding); 2],
) {
    let source = super::super::tests::membership::snapshot(2, 1);
    let tabs = [primary(&source, 0), primary(&source, 1)];
    let (mut ui, _, _, _) = super::super::tests::ready::ready_ui_with_snapshot(source);
    super::super::test_support::install_projection(&mut ui, Some((0, false, 24)), 0);
    (ui, tabs)
}

/// "Allowed" tells the person Orchbee just pressed a key for them, so it must never be said for
/// a press they cannot see, a press from before this Client looked, or a press said already.
#[test]
fn an_answer_is_said_once_when_it_is_news_for_a_terminal_in_view() {
    use AgentConfirmationChoice::Run;
    let (mut ui, tabs) = two_tabs();
    let said = |ui: &WorkbenchClientUi| {
        let view = ui.view();
        let notice = view.notice()?;
        let text = match view.notice_context() {
            Some(NoticeContext::Confirmation(text)) => text.clone(),
            _ => String::new(),
        };
        Some((notice, text))
    };
    let allowed = |serial: u32| {
        Some((
            WorkbenchClientNotice::ConfirmationAllowed(AgentConfirmationKind::Command),
            format!("command {serial}"),
        ))
    };

    // Answers given before the first read are not news, in view or not.
    ui.receive_agent_confirmations(vec![
        row(&tabs[0], view(Run, false, 3)),
        row(&tabs[1], view(Run, false, 7)),
    ]);
    assert_eq!(said(&ui), None);

    ui.receive_agent_confirmations(vec![row(&tabs[0], view(Run, false, 4))]);
    assert_eq!(said(&ui), allowed(4));
    // The same row again, and an answer in a Tab the person is not looking at, say nothing new.
    ui.receive_agent_confirmations(vec![
        row(&tabs[0], view(Run, false, 4)),
        row(&tabs[1], view(Run, false, 8)),
    ]);
    assert_eq!(said(&ui), allowed(4));
    ui.receive_agent_confirmations(vec![row(&tabs[0], view(Run, false, 5))]);
    assert_eq!(said(&ui), allowed(5));

    // A row read for another terminal than the one the Presentation shows names no Agent here.
    let other = PresentationBinding {
        terminal_id: TerminalId::parse(&format!("{:032x}", 9001)).unwrap(),
        worker_id: WorkerId::parse(&format!("{:032x}", 9101)).unwrap(),
    };
    ui.receive_agent_confirmations(vec![row(&(tabs[0].0.clone(), other), view(Run, false, 9))]);
    assert_eq!(said(&ui), allowed(5));
    assert!(ui.agent_confirmation(&tabs[0].0).is_none());

    // The stop is said for any Tab, once.
    let mut stopped = view(Run, false, 8);
    stopped.stopped = true;
    ui.receive_agent_confirmations(vec![row(&tabs[1], stopped.clone())]);
    assert_eq!(
        said(&ui),
        Some((WorkbenchClientNotice::ConfirmationStopped, String::new()))
    );
    ui.command_notice.set(None);
    stopped.wait = Some(AgentConfirmationWait::Repeating);
    ui.receive_agent_confirmations(vec![row(&tabs[1], stopped)]);
    assert_eq!(said(&ui), None);
}

/// The key's menu answers for the Agent in view, lists what the Server says is open, marks the
/// choice in force, and refuses when the Server names no Agent there.
#[test]
fn the_answers_follow_the_servers_row_for_the_focused_agent() {
    use ConfirmationAnswer as A;
    let key = MenuTarget::AgentConfirmations {
        presentation: None,
        offer: false,
    };
    let (mut ui, tabs) = two_tabs();
    let answers = |ui: &WorkbenchClientUi| {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        (
            menu.actions.to_vec(),
            menu.actions
                .iter()
                .zip(menu.notes)
                .filter(|(_, note)| **note == Some(MenuRowNote::Applied))
                .map(|(action, _)| action.clone())
                .collect::<Vec<_>>(),
        )
    };

    // An Agent in another Tab is not the key's subject.
    ui.receive_agent_confirmations(vec![row(
        &tabs[1],
        view(AgentConfirmationChoice::Off, true, 0),
    )]);
    assert_eq!(ui.confirmation_offer(), None);
    assert!(!ui.open_action_menu(key.clone(), None, false));

    ui.receive_agent_confirmations(vec![row(
        &tabs[0],
        view(AgentConfirmationChoice::Off, true, 0),
    )]);
    assert_eq!(ui.confirmation_offer(), Some("claude"));
    assert!(ui.open_action_menu(key.clone(), None, false));
    assert_eq!(
        answers(&ui),
        (
            [A::Run, A::AgentType, A::No]
                .map(MenuAction::AgentConfirmation)
                .to_vec(),
            Vec::new()
        )
    );
    ui.cancel_action_menu();

    ui.receive_agent_confirmations(vec![row(
        &tabs[0],
        view(AgentConfirmationChoice::Run, false, 0),
    )]);
    assert_eq!(ui.confirmation_offer(), None);
    assert!(ui.open_action_menu(key, None, false));
    assert_eq!(
        answers(&ui),
        (
            [A::Off, A::Run, A::AgentType]
                .map(MenuAction::AgentConfirmation)
                .to_vec(),
            vec![MenuAction::AgentConfirmation(A::Run)]
        )
    );
    // The Agent's exit ends its row, and with it the open answers.
    ui.receive_agent_confirmations(vec![PresentationConfirmationRow {
        presentation_id: tabs[0].0.clone(),
        binding: Some(tabs[0].1.clone()),
        confirmation: None,
    }]);
    ui.drive_action_menu();
    assert!(ui.view().action_menu().is_none());
}

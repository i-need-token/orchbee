//! A close of one Tab, of a set of Tabs or of a Panel asks the Server first from every route;
//! `cli_windows_idle_close` witnesses the Server's answers on a real PowerShell. A lone close stays
//! unreported for its delay.
use super::super::test_support::{controlling_ui, install_projection};
use super::super::*;
use super::normal_exit_hold::{exited, poll};
use crate::workbench_state::{CloseOperationKind, PanelIntent};
use std::sync::atomic::Ordering;

/// The next queued command that is not an ordinary Poll.
fn next(mailbox: &CommandMailbox) -> crate::workbench_client::mailbox::CommandLease {
    loop {
        let lease = mailbox.try_recv().unwrap().expect("queued command");
        if !matches!(lease.record(), QueuedCommand::Poll(_)) {
            return lease;
        }
        mailbox.finish_lease(lease).unwrap();
    }
}

/// A Controlling Client showing one terminal; `idle_close` says whether a close of one Tab or Panel
/// asks the Server first.
fn shown_ui(
    idle_close: bool,
) -> (
    WorkbenchClientUi,
    std::sync::Arc<CommandMailbox>,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
) {
    let (mut ui, mailbox, _, elapsed) = controlling_ui();
    ui.idle_close = idle_close;
    install_projection(&mut ui, Some((0, false, 24)), 0);
    (ui, mailbox, elapsed)
}

fn tab_ids(ui: &WorkbenchClientUi) -> Vec<orchbee_workbench_protocol::WorkbenchTabId> {
    let state = ui.state.as_ref().unwrap();
    state
        .cache()
        .tabs()
        .iter()
        .map(|tab| tab.tab_id().clone())
        .collect()
}

/// Whether the next queued command is the conditional close of one Tab.
fn asks_the_server(mailbox: &CommandMailbox) -> bool {
    matches!(
        next(mailbox).record(),
        QueuedCommand::Close(marker)
            if matches!(marker.operation().kind(), CloseOperationKind::CloseIfIdle { .. })
    )
}

/// The key, the [x] and the Tab menus close one Tab the same way, and so does the Panel key: the
/// confirmation stays hidden while the Server decides.
#[test]
fn one_tab_or_panel_asks_the_server_first() {
    let (mut ui, mailbox, _) = shown_ui(true);
    ui.apply_client_action(ClientAction::RequestCloseTab);
    assert!(ui.view().close_confirmation().is_none());
    assert!(asks_the_server(&mailbox));

    // The [x] and the Tab menus open a set; a set of one is the same close as the key.
    let (mut ui, mailbox, _) = shown_ui(true);
    let tabs = tab_ids(&ui);
    assert!(ui.request_close_set(&tabs[..1]).is_some());
    assert!(ui.view().close_confirmation().is_none());
    assert!(asks_the_server(&mailbox));

    let (mut ui, mailbox, _) = shown_ui(true);
    ui.apply_client_action(ClientAction::ClosePanel);
    assert!(ui.view().panel_close_confirmation().is_none());
    assert!(matches!(
        next(&mailbox).record(),
        QueuedCommand::Panel(marker) if *marker.operation().intent() == PanelIntent::CloseIfIdle
    ));
}

/// A set of several Tabs asks with one conditional Start of the whole set: its confirmation stays
/// hidden while the Server decides, presents when a member runs, and goes when the Server closes
/// the set unasked.
#[test]
fn a_set_asks_the_server_first_with_one_conditional_start() {
    use crate::workbench_client::mailbox::MutationResult;
    use crate::workbench_state::CloseResult;
    use orchbee_workbench_protocol::{
        TabSetCloseAction, TabSetCloseCondition, TabSetCloseMember, TabSetCloseOutcome,
        TabSetCloseStatus, WorkbenchLabel,
    };
    for asks in [true, false] {
        let (mut ui, mailbox, _) = shown_ui(true);
        let clock = ui.clock.clone();
        let tabs = tab_ids(&ui);
        assert!(ui.request_close_set(&tabs).is_some());
        assert!(ui.view().close_confirmation().is_none());
        let lease = next(&mailbox);
        let QueuedCommand::Close(marker) = lease.record() else {
            panic!("the set Start")
        };
        let operation = marker.operation().clone();
        let CloseOperationKind::TabSet(TabSetCloseAction::Start {
            terminal_context_id,
            condition: TabSetCloseCondition::IfIdle,
            tab_ids,
            ..
        }) = operation.kind().clone()
        else {
            panic!("one conditional Start of the whole set")
        };
        assert_eq!(tab_ids, tabs);
        assert_eq!(
            mailbox.mark_close_in_flight(&operation).unwrap(),
            MutationResult::Applied
        );
        let answer = if asks {
            TabSetCloseOutcome::ConfirmationRequired {}
        } else {
            TabSetCloseOutcome::Available {
                terminal_context_id,
                condition: TabSetCloseCondition::IfIdle,
                status: TabSetCloseStatus::Pending {},
                members: tab_ids
                    .into_iter()
                    .map(|tab_id| TabSetCloseMember {
                        tab_id,
                        label: WorkbenchLabel::parse("member").unwrap(),
                        close: None,
                        absent: false,
                    })
                    .collect(),
            }
        };
        let wait = mailbox
            .publish_close_result_before(
                operation.clone(),
                CloseResult::TabSet(answer),
                operation.deadline(),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
        mailbox.finish_lease(lease).unwrap();
        assert_eq!(ui.view().close_confirmation().is_some(), asks);
        assert!(tabs.iter().all(|tab| ui.view().tab_leaving(tab) != asks));
    }
}

/// While a close stays unreported, its own terminal ending is no news; the UI wakes when its delay
/// ends, and from then on the status line speaks again.
#[test]
fn an_unreported_close_withholds_the_exit_cue_and_wakes_at_the_end_of_its_delay() {
    use crate::client_view::ClientView;
    use crate::localization::Locale;
    use crate::tui_hit_map::HitMapBuilder;
    use crate::tui_theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};
    // The status line alone: the Panel's own title keeps saying how its terminal ended.
    let status_line = |ui: &WorkbenchClientUi| {
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &ClientView::new(120),
                    Locale::EnUs,
                    Some(ui.view()),
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..120)
            .map(|x| buffer[(x, 39)].symbol())
            .collect::<String>()
    };
    let (mut ui, mailbox, elapsed) = shown_ui(false);
    let key = ui.projections[0].key.clone();
    let record = exited(&ui, &key, 1);
    let clock = ui.clock.clone();
    assert!(next_is_empty(&mailbox));
    poll(&mut ui, &mailbox, &clock, &key, record);
    let panel = key.pane().panel().clone();
    assert!(ui.view().terminal_end(&panel).is_some());
    let told = status_line(&ui);
    assert!(told.contains("exited"), "{told}");

    // A confirmed close, retained from its submission.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    let token = ui.view().close_confirmation().unwrap().token();
    ui.state
        .as_mut()
        .unwrap()
        .acknowledge_close_draw(Some(token), None);
    let _ = ui.confirm_close();
    assert!(matches!(next(&mailbox).record(), QueuedCommand::Close(_)));
    assert!(ui.view().close_quiet_on_selected());
    let quiet = status_line(&ui);
    assert!(!quiet.contains("exited"), "{quiet}");
    assert!(
        ui.next_poll_delay()
            .is_some_and(|delay| delay <= std::time::Duration::from_secs(1))
    );

    elapsed.fetch_add(1000, Ordering::SeqCst);
    ui.after_draw();
    assert!(ui.tick());
    assert!(!ui.view().close_quiet_on_selected());
    assert!(
        ui.state
            .as_ref()
            .unwrap()
            .close_pending_deadline()
            .is_none()
    );
    let slow = status_line(&ui);
    assert!(slow.contains("exited"), "{slow}");
}

fn next_is_empty(mailbox: &CommandMailbox) -> bool {
    loop {
        let Some(lease) = mailbox.try_recv().unwrap() else {
            return true;
        };
        if !matches!(lease.record(), QueuedCommand::Poll(_)) {
            mailbox.finish_lease(lease).unwrap();
            return false;
        }
        mailbox.finish_lease(lease).unwrap();
    }
}

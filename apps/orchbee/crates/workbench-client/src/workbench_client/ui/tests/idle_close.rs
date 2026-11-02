//! A Tab or Panel with no program running closes without a confirmation, from every route. The two
//! facts are scripted here in the shapes a real Host produces; `cli_windows_idle_close` witnesses
//! them on a real PowerShell.
use super::super::test_support::{controlling_ui, frame, install_projection};
use super::super::*;
use super::normal_exit_hold::{exited, poll};
use crate::agent_start::CloseSample;
use crate::terminal_projection::ShellReport;
use orchbee_terminal_protocol::{TerminalCommandObservation, TerminalObservedCommand};
use std::num::NonZeroU64;
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

/// Takes the one queued read of a terminal and answers it, or leaves it unanswered with `None`.
fn settle(ui: &mut WorkbenchClientUi, mailbox: &CommandMailbox, sample: Option<CloseSample>) {
    let lease = next(mailbox);
    let QueuedCommand::CloseProbe(probe) = lease.record() else {
        panic!("terminal read: {:?}", lease.record())
    };
    if let Some(sample) = sample {
        mailbox
            .publish_notice_before(
                WorkerEvent::CloseProbeOutcome {
                    probe: probe.serial,
                    binding: probe.terminal.binding.clone(),
                    sample,
                    report: ShellReport::Silent,
                },
                ui.clock.now() + crate::workbench_state::OPERATION_TOTAL,
                &ui.clock,
            )
            .unwrap();
        ui.tick();
    }
    mailbox.finish_lease(lease).unwrap();
}

/// Installs a frame carrying the shell's report, as managed PowerShell produces it at its prompt
/// (`ready`) and between its run markers (`running`).
fn report(ui: &mut WorkbenchClientUi, ready: bool, running: bool) {
    let one = NonZeroU64::MIN;
    let mut next = frame(&ui.projections[0].value, 0, false, 24);
    next.command_observation = Some(TerminalCommandObservation {
        output_epoch: one,
        ready,
        command: running.then_some(TerminalObservedCommand {
            id: [1; 16],
            completed: false,
            exit_code: 0,
            start_row: 0,
            start_column: 0,
            end_row: 0,
            end_column: 0,
            generation: one,
            output_epoch: one,
            started_at: None,
        }),
    });
    ui.projections[0].value.install_frame_for_test(next);
}

fn armed_ui() -> (
    WorkbenchClientUi,
    std::sync::Arc<CommandMailbox>,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
) {
    let (mut ui, mailbox, _, elapsed) = controlling_ui();
    ui.idle_close = super::super::idle_close::IdleClose::new(true);
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

#[test]
fn a_silent_shell_is_judged_by_its_sample_alone() {
    let (mut ui, mailbox, elapsed) = armed_ui();

    // A program holding the console presents the hidden confirmation once its sample arrives.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    assert!(ui.view().close_confirmation().is_none());
    settle(&mut ui, &mailbox, Some(CloseSample::Program));
    assert!(ui.view().close_confirmation().is_some());
    ui.cancel_close();

    // So does a sample that cannot answer.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    settle(&mut ui, &mailbox, Some(CloseSample::Unknown));
    assert!(ui.view().close_confirmation().is_some());
    ui.cancel_close();

    // And no sample within the budget.
    ui.apply_client_action(ClientAction::ClosePanel);
    settle(&mut ui, &mailbox, None);
    assert!(ui.view().panel_close_confirmation().is_none());
    elapsed.fetch_add(1001, Ordering::SeqCst);
    ui.tick();
    assert!(ui.view().panel_close_confirmation().is_some());
    ui.cancel_panel_close();

    // Only the shell alone closes the Tab without presenting anything.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    settle(&mut ui, &mailbox, Some(CloseSample::ShellOnly));
    assert!(ui.view().close_confirmation().is_none());
    assert!(matches!(next(&mailbox).record(), QueuedCommand::Close(_)));
}

#[test]
fn a_shell_at_its_prompt_closes_when_its_sample_is_refused_or_late() {
    let (mut ui, mailbox, _) = armed_ui();
    report(&mut ui, true, false);
    ui.apply_client_action(ClientAction::RequestCloseTab);
    settle(&mut ui, &mailbox, Some(CloseSample::Unknown));
    assert!(ui.view().close_confirmation().is_none());
    assert!(matches!(next(&mailbox).record(), QueuedCommand::Close(_)));

    let (mut ui, mailbox, elapsed) = armed_ui();
    report(&mut ui, true, false);
    ui.apply_client_action(ClientAction::RequestCloseTab);
    settle(&mut ui, &mailbox, None);
    elapsed.fetch_add(1001, Ordering::SeqCst);
    ui.tick();
    assert!(ui.view().close_confirmation().is_none());
    assert!(matches!(next(&mailbox).record(), QueuedCommand::Close(_)));

    // A program sharing the console of a ready shell still asks.
    let (mut ui, mailbox, _) = armed_ui();
    report(&mut ui, true, false);
    ui.apply_client_action(ClientAction::RequestCloseTab);
    settle(&mut ui, &mailbox, Some(CloseSample::Program));
    assert!(ui.view().close_confirmation().is_some());
}

/// A cmdlet such as `Start-Sleep` starts no process, so its sample alone reads as an idle shell.
#[test]
fn a_command_the_shell_reports_running_asks_whatever_the_sample_says() {
    let (mut ui, mailbox, _) = armed_ui();
    report(&mut ui, false, true);
    ui.apply_client_action(ClientAction::RequestCloseTab);
    assert!(ui.view().close_confirmation().is_some());
    ui.cancel_close();

    // A command that starts while its sample is on the way.
    report(&mut ui, true, false);
    ui.apply_client_action(ClientAction::RequestCloseTab);
    report(&mut ui, false, true);
    settle(&mut ui, &mailbox, Some(CloseSample::ShellOnly));
    assert!(ui.view().close_confirmation().is_some());
}

/// The [x] and the Tab menus open a set; a set of one is the same close as the key.
#[test]
fn the_button_route_of_one_tab_closes_unasked_and_a_larger_set_always_asks() {
    let (mut ui, mailbox, _) = armed_ui();
    let tabs = tab_ids(&ui);
    assert!(ui.request_close_set(&tabs[..1]).is_some());
    assert!(ui.view().close_confirmation().is_none());
    settle(&mut ui, &mailbox, Some(CloseSample::ShellOnly));
    assert!(ui.view().close_confirmation().is_none());
    assert!(matches!(next(&mailbox).record(), QueuedCommand::Close(_)));

    let (mut ui, mailbox, _) = armed_ui();
    let tabs = tab_ids(&ui);
    assert!(ui.request_close_set(&tabs).is_some());
    assert!(ui.view().close_confirmation().is_some());
    assert!(next_is_empty(&mailbox));
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
    let (mut ui, mailbox, elapsed) = armed_ui();
    let key = ui.projections[0].key.clone();
    let record = exited(&ui, &key, 1);
    let clock = ui.clock.clone();
    assert!(next_is_empty(&mailbox));
    poll(&mut ui, &mailbox, &clock, &key, record);
    let panel = key.pane().panel().clone();
    assert!(ui.view().terminal_end(&panel).is_some());
    let told = status_line(&ui);
    assert!(told.contains("exited"), "{told}");

    // Runs the close the way an unasked route does, from the confirmation it arms.
    ui.apply_client_action(ClientAction::RequestCloseTab);
    let token = ui.view().close_confirmation().unwrap().token();
    assert!(ui.state.as_mut().unwrap().admit_unconfirmed_close(token));
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

/// A Tab this Client shows no pane of is read through a temporary Viewer, and judged by the report
/// that Viewer brings back.
#[test]
fn a_background_tab_is_decided_from_the_facts_of_its_read() {
    let (mut ui, mailbox, _) = armed_ui();
    let tabs = tab_ids(&ui);
    assert!(ui.request_close_set(&tabs[1..]).is_some());
    assert!(ui.view().close_confirmation().is_none());
    let lease = next(&mailbox);
    let QueuedCommand::CloseProbe(probe) = lease.record() else {
        panic!("terminal read: {:?}", lease.record())
    };
    assert!(probe.terminal.pane.is_none());
    mailbox
        .publish_notice_before(
            WorkerEvent::CloseProbeOutcome {
                probe: probe.serial,
                binding: probe.terminal.binding.clone(),
                sample: CloseSample::ShellOnly,
                report: ShellReport::Running,
            },
            ui.clock.now() + crate::workbench_state::OPERATION_TOTAL,
            &ui.clock,
        )
        .unwrap();
    ui.tick();
    mailbox.finish_lease(lease).unwrap();
    assert!(ui.view().close_confirmation().is_some());
}

/// The shapes real frames take without the reader's markers: under the system ConHost only OSC 133
/// reaches the Host, and it marks the row the prompt ends on. The terminal also leaves the
/// continuation class on rows that no prompt holds, the cursor's own among them.
#[test]
fn the_marks_on_the_screen_rows_report_where_the_shell_is() {
    let (mut ui, _, _) = armed_ui();
    let mut screen = |cursor: u16, marked: &[u16], continuation: &[u16], alternate: bool| {
        let mut next = frame(&ui.projections[0].value, 0, alternate, 24);
        next.cursor.row = cursor;
        for row in &mut next.rows {
            row.prompt_class = u8::from(marked.contains(&row.index));
            row.flags = if continuation.contains(&row.index) {
                2
            } else {
                0
            };
        }
        ui.projections[0].value.install_frame_for_test(next);
        ui.projections[0].value.shell_report()
    };
    // No mark at all: `cmd`, or a screen a clear erased while PSReadLine redrew its prompt.
    assert_eq!(screen(3, &[], &[], false), ShellReport::Silent);
    assert_eq!(screen(1, &[1], &[], false), ShellReport::AtPrompt);
    // Input longer than the row stays at the prompt, on rows that carry no mark.
    assert_eq!(screen(3, &[1], &[2, 3], false), ShellReport::AtPrompt);
    // A cmdlet or a program below the prompt it left.
    assert_eq!(screen(3, &[1], &[], false), ShellReport::Running);
    assert_eq!(screen(3, &[1], &[2], false), ShellReport::Running);
    // A full-screen view says nothing about the shell beneath it.
    assert_eq!(screen(3, &[1], &[], true), ShellReport::Silent);

    let mut stained = |cursor: u16, marked: &[u16], continuation: &[u16]| {
        let mut next = frame(&ui.projections[0].value, 0, false, 24);
        next.cursor.row = cursor;
        for row in &mut next.rows {
            row.prompt_class = if marked.contains(&row.index) { 1 } else { 2 };
            row.flags = if continuation.contains(&row.index) {
                2
            } else {
                0
            };
        }
        ui.projections[0].value.install_frame_for_test(next);
        ui.projections[0].value.shell_report()
    };
    // The cursor on a stained blank row below the prompt a cmdlet left.
    assert_eq!(stained(5, &[3], &[]), ShellReport::Running);
    // Wrapped input keeps its stained rows at the prompt.
    assert_eq!(stained(5, &[3], &[4, 5]), ShellReport::AtPrompt);
    // A clear erased the mark and left the stain.
    assert_eq!(stained(5, &[], &[]), ShellReport::Silent);
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

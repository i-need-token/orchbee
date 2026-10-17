use super::*;
use crate::{
    client_view::ClientView, localization::Locale, terminal_session::TerminalSessionView,
    tui_pointer::HitMapBuilder, tui_theme::Theme,
};
use orchbee_terminal_protocol::{
    HistoryAnchorStatus, HistoryChanged, TerminalTextFrame, TerminalTextStyle,
};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use std::num::NonZeroU64;

fn fixture() -> (NativeUi, PaneRef, Arc<CommandMailbox>) {
    let (mut ui, _, _) = super::super::agent_attention::tests::focused_physical_agent_fixture();
    let key = ui.projections[0].key.clone();
    let mut frame = ui.projections[0].value.frame().unwrap().clone();
    let cell = orchbee_terminal_protocol::TerminalTextCell {
        text: " ".into(),
        width: 0,
        style: TerminalTextStyle {
            foreground: 0x1000000,
            background: 0x1000000,
            underline_color: 0x2000000,
            flags: 0,
            underline: 0,
        },
    };
    for row in &mut frame.rows {
        row.cells = vec![cell.clone(); 80];
    }
    frame.rows[0].cells[0].text = ">".into();
    ui.projections[0].value.install_frame_for_test(frame);
    let mailbox = ui.mailbox.as_ref().unwrap().clone();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
    (ui, key, mailbox)
}

fn install_tail(ui: &mut NativeUi, key: &PaneRef, mailbox: &Arc<CommandMailbox>) {
    assert!(ui.drive_terminal_tails(mailbox));
    let lease = mailbox.try_recv().unwrap().expect("tail entry");
    assert!(matches!(
        lease.record(),
        QueuedCommand::Viewport(tagged) if tagged.key() == key && tagged.value() == &ViewportStep::EnterTail
    ));
    mailbox.finish_lease(lease).unwrap();
    let frame = ui.projections[0].value.frame().unwrap().clone();
    let core = ui.projections[0].viewport.as_mut().unwrap();
    core.enter_outcome(true);
    core.facts(HistoryChanged {
        attachment_id: frame.route.attachment_id,
        sequence: NonZeroU64::MIN,
        revision: 1,
        generation: frame.route.generation,
        anchor_status: HistoryAnchorStatus::Valid,
        total_rows: frame.scrollback_rows + u64::from(frame.geometry.rows),
    });
    serve_tail_window(ui, key, mailbox);
}

fn serve_tail_window(ui: &mut NativeUi, key: &PaneRef, mailbox: &Arc<CommandMailbox>) {
    let frame = ui.projections[0].value.frame().unwrap().clone();
    let target = Target::capture(&frame).unwrap();
    let core = ui.projections[0].viewport.as_mut().unwrap();
    let step = if let Some(step) = core.peek_command() {
        core.command_admitted();
        step
    } else {
        let lease = mailbox.try_recv().unwrap().expect("admitted tail read");
        let QueuedCommand::Viewport(tagged) = lease.record() else {
            panic!("viewport read");
        };
        assert_eq!(tagged.key(), key);
        let step = *tagged.value();
        mailbox.finish_lease(lease).unwrap();
        step
    };
    let ViewportStep::Read {
        request_sequence,
        revision,
        ..
    } = step
    else {
        panic!("read");
    };
    assert_eq!(revision, frame.route.revision);
    let mut rows = frame.rows.clone();
    for (index, row) in rows.iter_mut().enumerate() {
        if index < usize::try_from(target.rows_before).unwrap() {
            row.cells[0].text = "h".into();
        } else {
            row.cells = frame.rows[index - usize::try_from(target.rows_before).unwrap()]
                .cells
                .clone();
        }
    }
    let window = ViewportWindow {
        route: orchbee_terminal_protocol::TerminalRowWindowRoute {
            attachment_id: frame.route.attachment_id,
            host_epoch: frame.route.host_epoch,
            sequence: request_sequence,
            revision: frame.route.revision,
            generation: frame.route.generation,
            total_rows: frame.scrollback_rows + u64::from(frame.geometry.rows),
            start_row: frame.scrollback_rows - u64::from(target.rows_before),
            columns: 80,
            row_count: 24,
            detail: orchbee_terminal_protocol::RowWindowDetail::Render,
        },
        foreground: frame.foreground,
        background: frame.background,
        graphics_present: false,
        rows,
    };
    core.window(window, ui.clock.now());
    assert!(
        ui.view().pane_tail_window(key.pane().panel()).is_none(),
        "unacknowledged rows cannot replace live input coordinates"
    );
    let core = ui.projections[0].viewport.as_mut().unwrap();
    assert!(matches!(
        core.peek_command(),
        Some(ViewportStep::Ack { .. })
    ));
    core.command_admitted();
    core.ack_outcome(request_sequence, true);
}

fn paint(ui: &NativeUi, locale: Locale, theme: &Theme) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &ClientView::new(120),
                locale,
                Some(TerminalSessionView::Native(ui.view())),
                theme,
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn in_place_text_and_color_updates_replace_a_cropped_frame_without_manual_refresh() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            let (mut ui, key, mailbox) = fixture();
            let mut frame = ui.projections[0].value.frame().unwrap().clone();
            frame.rows[0].cells[0].text = "2".into();
            frame.rows[0].cells[0].style.foreground = 0xff0000;
            ui.projections[0]
                .value
                .install_frame_for_test(frame.clone());
            install_tail(&mut ui, &key, &mailbox);
            assert!(
                paint(&ui, locale, &theme)
                    .content
                    .iter()
                    .any(|cell| { cell.symbol() == "2" && cell.fg == Color::Rgb(255, 0, 0) })
            );

            frame.route.revision = 2;
            frame.route.sequence = NonZeroU64::new(2).unwrap();
            frame.rows[0].cells[0].text = "3".into();
            frame.rows[0].cells[0].style.foreground = 0x00ff00;
            ui.projections[0]
                .value
                .install_frame_for_test(frame.clone());
            ui.projections[0]
                .viewport
                .as_mut()
                .unwrap()
                .facts(HistoryChanged {
                    attachment_id: frame.route.attachment_id,
                    sequence: NonZeroU64::new(2).unwrap(),
                    revision: frame.route.revision,
                    generation: frame.route.generation,
                    anchor_status: HistoryAnchorStatus::Valid,
                    total_rows: frame.scrollback_rows + u64::from(frame.geometry.rows),
                });
            assert!(ui.view().pane_tail_window(key.pane().panel()).is_none());
            assert!(ui.drive_terminal_tails(&mailbox));
            // The old window cannot paint while a current crop is being acquired.
            let pending = paint(&ui, locale, &theme);
            assert!(
                pending
                    .content
                    .iter()
                    .any(|cell| { cell.symbol() == "3" && cell.fg == Color::Rgb(0, 255, 0) })
            );
            assert!(
                !pending
                    .content
                    .iter()
                    .any(|cell| { cell.symbol() == "2" && cell.fg == Color::Rgb(255, 0, 0) })
            );

            serve_tail_window(&mut ui, &key, &mailbox);
            assert!(ui.view().pane_tail_window(key.pane().panel()).is_some());
            assert!(
                paint(&ui, locale, &theme)
                    .content
                    .iter()
                    .any(|cell| { cell.symbol() == "3" && cell.fg == Color::Rgb(0, 255, 0) })
            );
        }
    }
}

#[test]
fn a_smaller_live_view_uses_cursor_follow_instead_of_automatic_history_trimming() {
    let (mut ui, _, mailbox) = fixture();
    ui.preferences.trim_trailing_blank_rows_for_agents = true;
    let mut frame = ui.projections[0].value.frame().unwrap().clone();
    frame.geometry.columns = 104;
    frame.geometry.rows = 36;
    ui.projections[0].value.install_frame_for_test(frame);
    assert!(!ui.drive_terminal_tails(&mailbox));
    assert!(ui.projections[0].viewport.is_none());
    assert!(ui.projections[0].tail.is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn shared_tail_requires_a_live_agent_and_preserves_presented_rows() {
    let (mut ui, _, mailbox) = fixture();
    let frame = ui.projections[0].value.frame().unwrap();
    assert_eq!(Target::capture(frame).unwrap().rows_before, 9);
    let refuse = |mut frame: TerminalTextFrame, change: fn(&mut TerminalTextFrame)| {
        change(&mut frame);
        assert!(Target::capture(&frame).is_none());
    };
    for change in [
        (|f: &mut TerminalTextFrame| f.alternate_screen = true) as fn(&mut TerminalTextFrame),
        |f| f.graphics_present = Some(true),
        |f| f.scrollback_rows = 0,
        |f| f.rows.last_mut().unwrap().prompt_class = 1,
        |f| f.rows.last_mut().unwrap().flags = 1,
        |f| f.rows.last_mut().unwrap().cells[0].style.background = 0,
        |f| f.rows.last_mut().unwrap().cells[0].style.flags = 1,
        |f| f.rows.last_mut().unwrap().cells[0].style.underline = 1,
        |f| f.rows.last_mut().unwrap().cells[0].text = "visible".into(),
        |f| f.cursor.row = 23,
    ] {
        refuse(frame.clone(), change);
    }
    ui.preferences.trim_trailing_blank_rows_for_agents = false;
    assert!(!ui.drive_terminal_tails(&mailbox));
    ui.preferences.trim_trailing_blank_rows_for_agents = true;
    ui.attention.close();
    assert!(!ui.drive_terminal_tails(&mailbox));
    assert!(ui.projections[0].viewport.is_none());
}

#[test]
fn tail_waits_for_exact_ack_and_user_scrolling_takes_over_without_a_second_lease() {
    let (mut ui, key, mailbox) = fixture();
    install_tail(&mut ui, &key, &mailbox);
    assert_eq!(ui.view().pane_tail_window(key.pane().panel()).unwrap().1, 9);
    assert!(ui.view().pane_viewport(key.pane().panel()).is_none());
    assert!(!ui.focus_viewport_non_live());
    assert!(!ui.history_key(&crate::client_view::KeyEvent::press(
        crate::client_view::KeyCode::Char('x'),
        crate::client_view::Modifiers::NONE
    )));
    assert!(ui.history_wheel(&key, true));
    assert!(ui.projections[0].tail.is_none());
    assert!(ui.focus_viewport_non_live());
}

#[test]
fn frame_change_and_disabled_preference_never_show_stale_rows() {
    let (mut ui, key, mailbox) = fixture();
    install_tail(&mut ui, &key, &mailbox);
    let mut frame = ui.projections[0].value.frame().unwrap().clone();
    frame.route.revision = 2;
    frame.route.sequence = NonZeroU64::new(2).unwrap();
    ui.projections[0].value.install_frame_for_test(frame);
    assert!(ui.view().pane_tail_window(key.pane().panel()).is_none());
    ui.preferences.trim_trailing_blank_rows_for_agents = false;
    ui.drive_terminal_tails(&mailbox);
    assert!(ui.projections[0].viewport.as_ref().unwrap().recovering());
    assert!(ui.projections[0].tail_attempted.is_none());
}

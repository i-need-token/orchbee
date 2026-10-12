use super::*;
use orchbee_terminal_protocol::{
    HistoryAnchorStatus, HistoryChanged, TerminalTextFrame, TerminalTextStyle,
};
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
    let ViewportStep::Read {
        request_sequence, ..
    } = core.peek_command().unwrap()
    else {
        panic!("read");
    };
    core.command_admitted();
    let mut rows = frame.rows.clone();
    for row in &mut rows {
        row.cells[0].text = "h".into();
    }
    let window = ViewportWindow {
        route: orchbee_terminal_protocol::TerminalRowWindowRoute {
            attachment_id: frame.route.attachment_id,
            host_epoch: frame.route.host_epoch,
            sequence: request_sequence,
            revision: 1,
            generation: frame.route.generation,
            total_rows: 33,
            start_row: 0,
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

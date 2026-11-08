use super::*;
use crate::{
    client_view::{ClientAction, ClientView},
    localization::Locale,
    tui_hit_map::HitMapBuilder,
};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn an_empty_workspace_publishes_its_exact_position_even_while_a_status_notice_is_visible() {
    let mut snapshot = super::super::tests::context_selection::snapshot(true);
    snapshot.navigation.workspace_order = snapshot
        .navigation
        .workspaces
        .iter()
        .rev()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    let (mut ui, _, _, snapshot) = super::super::tests::quick_workspace::empty_ui(snapshot);
    assert_eq!(ui.view().phase(), DisplayPhase::Empty);
    let saved = Position {
        location: ui.state.as_ref().unwrap().cache().location().clone(),
        member: None,
    };
    ui.report_position_fallback(Some(&saved));
    assert_eq!(
        ui.view().notice(),
        Some(super::super::WorkbenchClientNotice::PositionChangedEmpty)
    );
    let shell = ClientView::new(120);
    for _ in 0..3 {
        let receipt = draw(&ui, &shell, 120, 40);
        let proof = receipt
            .position()
            .expect("displayed empty position survives status overlay");
        assert!(proof.position.member.is_none());
        assert!(ui.view().accepts_position_draw(proof));
        let (restored, member, fallback) =
            crate::startup_preference::resolve(&snapshot, &proof.position).unwrap();
        assert_eq!(restored.location(), &proof.position.location);
        assert!(restored.tabs().is_empty());
        assert!(member.is_none());
        assert!(!fallback);
    }
    assert!(draw(&ui, &shell, 2, 2).position().is_none());
}

fn draw(
    ui: &WorkbenchClientUi,
    shell: &ClientView,
    width: u16,
    height: u16,
) -> crate::workbench_client::DrawReceipt {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = Default::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                shell,
                Locale::EnUs,
                Some(ui.view()),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut HitMapBuilder::new(),
            )
        })
        .unwrap();
    receipt
}

#[test]
fn startup_preference_render_proof_rejects_covered_small_and_stale_selection() {
    let (mut ui, _, _, _) = super::super::tests::viewport::controlling_ui();
    super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    let mut shell = ClientView::new(120);
    let receipt = draw(&ui, &shell, 120, 40);
    let proof = receipt
        .position()
        .expect("uncovered focused live pane")
        .clone();
    assert!(ui.view().accepts_position_draw(&proof));
    assert!(draw(&ui, &shell, 2, 2).position().is_none());
    use crate::client_view::{ClientActionDisposition, KeyCode, KeyEvent, Modifiers, PrefixInput};
    for (character, modifiers) in [('b', Modifiers::CTRL), ('?', Modifiers::NONE)] {
        let transition = shell.decide(PrefixInput::Key(KeyEvent::press(
            KeyCode::Char(character),
            modifiers,
        )));
        shell.commit(transition, ClientActionDisposition::KeepNavigator);
    }
    assert!(draw(&ui, &shell, 120, 40).position().is_none());
    ui.apply_client_action(ClientAction::SelectNextTab);
    assert!(!ui.view().accepts_position_draw(&proof));
    assert!(
        draw(&ui, &ClientView::new(120), 120, 40)
            .position()
            .is_none()
    );
}

#[test]
fn a_startup_record_notice_stays_readable_through_startup_until_a_client_action() {
    use orchbee_local_runtime::{RecordProblem, VersionedRecord};
    use std::sync::atomic::Ordering;
    let (mut ui, _mailbox, _clock, elapsed) = super::super::tests::viewport::controlling_ui();
    super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.report_position_notice(false);
    ui.report_record_notice(VersionedRecord::Machines, RecordProblem::Newer);
    // Startup goes on without the person: time passes and automatic transient notices arrive.
    assert!(!ui.expire_status_notices());
    elapsed.store(60_000, Ordering::SeqCst);
    assert!(!ui.expire_status_notices());
    ui.state
        .as_mut()
        .unwrap()
        .set_notice(ClientNotice::HistoryRecovering);
    assert_eq!(
        ui.view().notice(),
        Some(super::super::WorkbenchClientNotice::RecordUnusable(
            VersionedRecord::Machines,
            RecordProblem::Newer
        ))
    );
    let shell = ClientView::new(120);
    for (locale, width) in [
        (Locale::EnUs, 120),
        (Locale::ZhCn, 120),
        (Locale::EnUs, 300),
    ] {
        let screen = screen_text(&ui, &shell, locale, width, 36);
        let whole = locale.record_message(VersionedRecord::Machines, RecordProblem::Newer, None);
        let shorter = locale.record_status(VersionedRecord::Machines, RecordProblem::Newer);
        assert!(
            screen.contains(&whole) || screen.contains(&shorter),
            "{locale:?} at {width} columns shows the record message:\n{screen}"
        );
    }
    ui.apply_client_action(ClientAction::DismissPrefix);
    assert!(ui.view().notice().is_none());
}

fn screen_text(
    ui: &WorkbenchClientUi,
    shell: &ClientView,
    locale: Locale,
    width: u16,
    height: u16,
) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                shell,
                locale,
                Some(ui.view()),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..height {
        text.push('\n');
        let mut x = 0;
        while x < width {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            // A wide glyph owns its following cell; that placeholder blank is not text.
            x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
        }
    }
    text
}

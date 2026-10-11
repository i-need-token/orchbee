use super::*;
use crate::{
    client_view::{ClientAction, ClientView},
    localization::Locale,
    terminal_session::TerminalSessionView,
    tui_pointer::HitMapBuilder,
};
use ratatui::{Terminal, backend::TestBackend};

fn draw(
    ui: &NativeUi,
    shell: &ClientView,
    width: u16,
    height: u16,
) -> crate::terminal_session::NativeDrawReceipt {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = Default::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                shell,
                Locale::EnUs,
                Some(TerminalSessionView::Native(ui.view())),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut HitMapBuilder::new(),
            )
        })
        .unwrap();
    receipt
}

#[test]
fn startup_preference_render_proof_rejects_covered_small_and_stale_selection() {
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui();
    super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    let mut shell = ClientView::new(120);
    let receipt = draw(&ui, &shell, 120, 40);
    let proof = receipt
        .position()
        .expect("uncovered focused live pane")
        .clone();
    assert!(ui.view().accepts_position_draw(&proof));
    assert!(draw(&ui, &shell, 2, 2).position().is_none());
    use crate::client_view::{KeyCode, KeyEvent, Modifiers, PrefixInput, SessionActionDisposition};
    for (character, modifiers) in [('b', Modifiers::CTRL), ('?', Modifiers::NONE)] {
        let transition = shell.decide(PrefixInput::Key(KeyEvent::press(
            KeyCode::Char(character),
            modifiers,
        )));
        shell.commit(transition, SessionActionDisposition::KeepNavigator);
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

use super::*;
use crate::terminal_session::WorkspaceCloseToken;

#[derive(Default)]
struct SetupPort {
    decisions: Vec<ModalControl>,
}
impl TerminalInputPort for SetupPort {
    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("modal controls cannot forward terminal input");
    }
    fn choose_setup_project(&mut self, _: crate::terminal_session::NativeSetupToken, index: usize) {
        self.decisions.push(ModalControl::Choice(index));
    }
    fn submit_setup_draft(
        &mut self,
        _: crate::terminal_session::NativeSetupToken,
    ) -> Option<crate::client_view::SessionActionDisposition> {
        self.decisions.push(ModalControl::Submit);
        None
    }
    fn cancel_setup_draft(&mut self, _: crate::terminal_session::NativeSetupToken) {
        self.decisions.push(ModalControl::Cancel);
    }
}

#[test]
fn setup_choice_and_form_actions_resolve_the_painted_cell_on_matching_release() {
    let area = Rect::new(10, 5, 8, 1);
    for control in [
        ModalControl::Choice(17),
        ModalControl::Submit,
        ModalControl::Cancel,
    ] {
        for (x, y, admitted) in [
            (10, 5, true),
            (17, 5, true),
            (9, 5, false),
            (18, 5, false),
            (10, 4, false),
            (10, 6, false),
        ] {
            let mut pointer = PointerState::default();
            let mut port = SetupPort::default();
            let mut hits = HitMapBuilder::new();
            hits.start_modal();
            hits.record(
                HitLayer::Overlay,
                area,
                HitTarget::Setup {
                    token: crate::terminal_session::NativeSetupToken::for_test(7),
                    control,
                },
            );
            pointer.set_capture(true);
            pointer.publish(hits.finish());
            assert!(!pointer.modal_event(
                &mut port,
                event(MouseAction::Press, MouseButton::Left, x, y)
            ));
            assert!(port.decisions.is_empty());
            // An unrelated button release cannot commit or steal the left press.
            assert!(!pointer.modal_event(
                &mut port,
                event(MouseAction::Release, MouseButton::Right, x, y)
            ));
            assert!(port.decisions.is_empty());
            assert_eq!(
                pointer.modal_event(
                    &mut port,
                    event(MouseAction::Release, MouseButton::Left, x, y)
                ),
                admitted
            );
            assert_eq!(
                port.decisions,
                if admitted { vec![control] } else { vec![] }
            );
            assert!(!pointer.modal_event(
                &mut port,
                event(MouseAction::Release, MouseButton::Left, x, y)
            ));
        }
    }
}

#[derive(Default)]
struct ClosePort {
    decisions: Vec<(WorkspaceCloseToken, bool)>,
}
impl TerminalInputPort for ClosePort {
    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("close gestures cannot forward terminal input");
    }
    fn click_workspace_close(&mut self, token: WorkspaceCloseToken, confirm: bool) {
        self.decisions.push((token, confirm));
    }
}

fn map(token: WorkspaceCloseToken, confirm: bool, area: Rect) -> FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record(
        HitLayer::Overlay,
        area,
        HitTarget::WorkspaceClose { token, confirm },
    );
    hits.finish()
}

#[test]
fn close_controls_require_matching_release_within_every_painted_edge() {
    let token = WorkspaceCloseToken::for_test(1);
    let area = Rect::new(10, 5, 8, 2);
    for confirm in [false, true] {
        for (x, y, admitted) in [
            (10, 5, true),
            (17, 5, true),
            (10, 6, true),
            (17, 6, true),
            (9, 5, false),
            (18, 5, false),
            (10, 4, false),
            (10, 7, false),
        ] {
            let mut pointer = PointerState::default();
            let mut port = ClosePort::default();
            pointer.set_capture(true);
            pointer.publish(map(token, confirm, area));
            let press = event(MouseAction::Press, MouseButton::Left, x, y);
            let release = event(MouseAction::Release, MouseButton::Left, x, y);
            assert_eq!(
                pointer.confirmation_event(&mut port, press),
                admitted.then_some(false)
            );
            assert!(port.decisions.is_empty());
            assert_eq!(
                pointer.confirmation_event(&mut port, release),
                admitted.then_some(true)
            );
            assert_eq!(
                port.decisions,
                if admitted {
                    vec![(token, confirm)]
                } else {
                    vec![]
                }
            );
            assert_eq!(pointer.confirmation_event(&mut port, release), None);
        }
    }
}

#[test]
fn close_press_never_survives_changed_token_geometry_modifiers_focus_or_capture() {
    let token = WorkspaceCloseToken::for_test(1);
    let area = Rect::new(10, 5, 8, 1);
    for change in 0..9 {
        let mut pointer = PointerState::default();
        let mut port = ClosePort::default();
        pointer.set_capture(true);
        pointer.publish(map(token, true, area));
        assert_eq!(
            pointer.confirmation_event(
                &mut port,
                event(MouseAction::Press, MouseButton::Left, 12, 5)
            ),
            Some(false)
        );
        let mut release = event(MouseAction::Release, MouseButton::Left, 12, 5);
        match change {
            0 => pointer.publish(map(WorkspaceCloseToken::for_test(2), true, area)),
            1 => pointer.publish(map(token, false, area)),
            2 => {
                pointer.publish(map(token, true, Rect::new(11, 5, 8, 1)));
                pointer.publish(map(token, true, area));
            }
            3 => {
                pointer.invalidate_map();
                pointer.publish(map(token, true, area));
            }
            4 => {
                pointer.set_capture(false);
                pointer.set_capture(true);
            }
            5 => release.column = area.right(),
            6 => release.mods = Modifiers::CTRL,
            7 => pointer.cancel_modal_press(),
            8 => {
                pointer.publish(FrameHitMap::default());
                pointer.publish(map(token, true, area));
            }
            _ => unreachable!(),
        }
        assert_eq!(
            pointer.confirmation_event(&mut port, release),
            Some(true),
            "change {change}"
        );
        assert!(port.decisions.is_empty(), "change {change}");
    }
}

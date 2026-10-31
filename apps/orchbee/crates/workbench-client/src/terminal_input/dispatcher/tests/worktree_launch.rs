use super::*;
use crate::workbench_state::worktree_launch::{WorktreeEdit, WorktreeStage, WorktreeToken};

struct Port {
    active: bool,
    serial: u64,
    stage: WorktreeStage,
    text: String,
    forwarded: Vec<u8>,
    submits: usize,
    selection: i32,
    complete: bool,
    fault: bool,
    shown: Option<WorktreeToken>,
}
impl Port {
    fn new() -> Self {
        Self {
            active: true,
            serial: 1,
            stage: WorktreeStage::Branch,
            text: String::new(),
            forwarded: Vec::new(),
            submits: 0,
            selection: 0,
            complete: true,
            fault: false,
            shown: None,
        }
    }
    fn token(&self) -> WorktreeToken {
        WorktreeToken::for_test(self.serial)
    }
    fn paint(&mut self) {
        self.shown = Some(self.token());
    }
}
impl Port {
    fn edit_worktree(&mut self, token: WorktreeToken, edit: WorktreeEdit<'_>) -> bool {
        assert_eq!(token, self.token());
        if matches!(
            self.stage,
            WorktreeStage::Waiting | WorktreeStage::Preparing
        ) {
            return true;
        }
        match edit {
            WorktreeEdit::Completeness(value) => {
                self.complete = value;
                if !value {
                    self.shown = None;
                }
                return true;
            }
            WorktreeEdit::Append(text) | WorktreeEdit::Paste(text) => self.text.push_str(text),
            WorktreeEdit::NextField => {
                self.stage = WorktreeStage::Base;
                self.text.clear();
            }
            WorktreeEdit::Next => self.selection += 1,
            WorktreeEdit::Previous => self.selection -= 1,
            WorktreeEdit::Fault => {
                self.fault = true;
                return true;
            }
            _ => {}
        }
        self.serial += 1;
        true
    }

    fn submit_worktree(&mut self, token: WorktreeToken) {
        assert_eq!(token, self.token());
        if self.complete && !self.fault && self.shown == Some(token) {
            self.submits += 1;
            self.stage = WorktreeStage::Waiting;
            self.shown = None;
        }
    }

    fn dismiss_worktree(&mut self, token: WorktreeToken) {
        assert_eq!(token, self.token());
        self.active = false;
    }

    fn choose_worktree(&mut self, token: WorktreeToken, index: usize) {
        if token != self.token()
            || self.shown != Some(token)
            || index >= 8
            || index == 3
            || self.stage != WorktreeStage::Checkout
        {
            return;
        }
        self.selection = i32::try_from(index).unwrap();
        self.submit_worktree(token);
    }
}

impl TerminalInputPort for Port {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{SurfaceAction, WorktreeAction};
        match action {
            SurfaceAction::Worktree(WorktreeAction::Submit(token)) => {
                self.submit_worktree(token);
                None
            }
            SurfaceAction::Worktree(WorktreeAction::Dismiss(token)) => {
                self.dismiss_worktree(token);
                None
            }
            SurfaceAction::Worktree(WorktreeAction::Choose { token, index }) => {
                self.choose_worktree(token, index);
                None
            }
            _ => None,
        }
    }

    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::SurfaceInput;
        match input {
            SurfaceInput::Worktree { token, edit } => self.edit_worktree(token, edit),
            _ => false,
        }
    }

    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        crate::tui_composition::CaptureClaims::default().with_worktree(self.active)
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        if owner != crate::tui_composition::WorkbenchCaptureOwner::Worktree || !self.active {
            return None;
        }
        Some(crate::workbench_client::CaptureContext::Worktree(
            crate::workbench_client::WorktreeCaptureContext {
                token: self.token(),
                readiness: crate::workbench_client::CaptureReadiness::Interactive,
                stage: self.stage,
                remaining_bytes: 1024usize.saturating_sub(self.text.len()),
            },
        ))
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        let action = command.intent();
        match action {
            ClientAction::RequestWorktreeCreate => self.stage = WorktreeStage::Preparing,
            ClientAction::RequestWorktreeOpen => self.stage = WorktreeStage::Checkout,
            _ => return None,
        }
        self.active = true;
        Some(ClientActionDisposition::DismissNavigator)
    }
    fn scroll_list(
        &mut self,
        target: &crate::tui_hit_map::ListScrollTarget,
        up: bool,
        lines: usize,
    ) -> Option<crate::tui_hit_map::ListScrollTarget> {
        let &crate::tui_hit_map::ListScrollTarget::Worktree(token) = target else {
            return None;
        };
        if token == self.token() && self.shown == Some(token) {
            self.selection += if up { -(lines as i32) } else { lines as i32 };
            self.serial += 1;
            self.shown = None;
            Some(crate::tui_hit_map::ListScrollTarget::Worktree(self.token()))
        } else {
            None
        }
    }
    fn observe_terminal(&mut self, observation: crate::terminal_input::TerminalObservation) {
        if observation == crate::terminal_input::TerminalObservation::DisplayInvalidated {
            self.shown = None;
        }
    }
}
#[test]
fn worktree_prefix_arms_paste_and_captures_same_packet_tail() {
    for key in *b"WO" {
        let mut port = Port::new();
        port.active = false;
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        let mut modes = Vec::new();
        dispatcher
            .dispatch(
                &mut view,
                &[2, key, b'x'],
                &mut port,
                Instant::now(),
                |_, _| Ok::<bool, io::Error>(false),
                &mut |mode| {
                    modes.push(mode);
                    Ok(())
                },
            )
            .unwrap();
        assert!(modes.contains(&ModeRequest::BracketedPaste(true)));
        assert!(port.forwarded.is_empty());
        assert!(port.active);
        assert!(port.text.is_empty());
        assert_eq!(
            port.stage,
            if key == b'W' {
                WorktreeStage::Preparing
            } else {
                WorktreeStage::Checkout
            }
        );
    }
}
#[test]
fn unicode_paste_uses_byte_budget_and_confirm_tail_stays_captured() {
    let mut port = Port::new();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let text = format!("{}a", "猫".repeat(341));
    let bytes = format!("\x1b[200~{text}\x1b[201~");
    for byte in bytes.as_bytes() {
        dispatch_all(&mut dispatcher, &mut view, &mut port, &[*byte]);
    }
    port.paint();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\rcommand");
    assert_eq!(port.text, text);
    assert_eq!(port.submits, 1);
    assert!(!port.fault);
    assert!(port.forwarded.is_empty());
    let mut port = Port::new();
    let mut dispatcher = InputDispatcher::default();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[200~{text}z\x1b[201~\r").as_bytes(),
    );
    assert!(port.fault);
    assert_eq!(port.submits, 0);
    assert!(port.forwarded.is_empty());
}

#[test]
fn removal_confirmations_ignore_text_paste_and_modified_enter_and_capture_confirm_tail() {
    for stage in [WorktreeStage::Remove, WorktreeStage::ResumeRemove] {
        let mut port = Port::new();
        port.stage = stage;
        port.paint();
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"y \t\x1b[A\x1b[13;2u\x1b[200~\rforce\x1b[201~",
        );
        assert_eq!(port.submits, 0);
        assert!(port.text.is_empty());
        assert_eq!(port.stage, stage);
        assert_eq!(port.serial, 1);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r\rcommand");
        assert_eq!(port.submits, 1);
        assert!(port.text.is_empty());
        assert!(port.forwarded.is_empty());
    }
}

fn painted_choice(dispatcher: &mut InputDispatcher, port: &mut Port, index: usize) {
    use crate::tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ModalControl,
    };
    dispatcher.pointer.set_capture(true);
    dispatcher
        .synchronize(&mut ClientView::new(100), port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record(
        HitLayer::OverlaySurface,
        ratatui::layout::Rect::new(2, 2, 30, 9),
        HitTarget::ListScroll(crate::tui_hit_map::ListScrollTarget::Worktree(port.token())),
    );
    hits.record_modal_control(
        HitLayer::Overlay,
        ratatui::layout::Rect::new(5, 5, 15, 2),
        HitTarget::Worktree {
            token: port.token(),
            control: ModalControl::Choice(index),
        },
        InteractionAvailability::Enabled,
    );
    dispatcher.publish_hit_map(hits.finish());
    port.paint();
}

#[test]
fn open_checkout_click_submits_painted_index_once_without_a_second_form() {
    for row in [6, 7] {
        let mut port = Port::new();
        port.stage = WorktreeStage::Checkout;
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        painted_choice(&mut dispatcher, &mut port, 6);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;6;{row}M\x1b[<0;6;{row}mtail\r").as_bytes(),
        );
        assert_eq!(port.selection, 6);
        assert_eq!(port.submits, 1);
        assert_eq!(port.stage, WorktreeStage::Waiting);
        assert!(port.text.is_empty());
        assert!(port.forwarded.is_empty());
    }
}

#[test]
fn open_picker_stale_and_bare_choices_cannot_submit_and_wheel_requires_repaint() {
    for reason in 0..4 {
        let mut port = Port::new();
        port.stage = WorktreeStage::Checkout;
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        painted_choice(
            &mut dispatcher,
            &mut port,
            if reason == 1 {
                3
            } else if reason == 2 {
                80
            } else {
                0
            },
        );
        match reason {
            0 => {
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[B");
            }
            3 => dispatcher.invalidate_hit_map(&mut port),
            _ => {}
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;6;6M\x1b[<0;6;6mtail",
        );
        assert_eq!(port.submits, 0);
        assert!(port.forwarded.is_empty());
    }
    let mut port = Port::new();
    port.stage = WorktreeStage::Checkout;
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    painted_choice(&mut dispatcher, &mut port, 0);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<65;6;6M\r");
    assert_eq!(port.selection, 3);
    assert_eq!(port.submits, 0);
    assert_eq!(dispatcher.epoch, Some(ModalEpoch::Worktree(port.token())));
    port.paint();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r");
    assert_eq!(port.submits, 1);
}

#[test]
fn open_picker_mouse_framing_never_revokes_the_draw_it_needs_to_activate() {
    let report = b"\x1b[<0;6;6M\x1b[<0;6;6m";
    for split in 0..=report.len() {
        let mut port = Port::new();
        port.stage = WorktreeStage::Checkout;
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        painted_choice(&mut dispatcher, &mut port, 2);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &report[..split]);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &report[split..]);
        assert_eq!(port.submits, 1, "split {split}");
        assert_eq!(port.selection, 2);
        assert!(port.forwarded.is_empty());
    }
}
#[test]
fn list_arrows_are_owned_and_escape_releases_only_the_next_packet() {
    let mut port = Port::new();
    port.stage = WorktreeStage::Checkout;
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[B\x1b[B\x1b[A");
    assert_eq!(port.selection, 1);
    assert!(port.text.is_empty());
    let now = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b",
            &mut port,
            now,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    let later = now + Duration::from_millis(60);
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            later,
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    assert!(!port.active);
    assert!(port.forwarded.is_empty());
    dispatcher
        .dispatch(
            &mut view,
            b"next",
            &mut port,
            later + Duration::from_millis(1),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(port.forwarded, b"next");
}

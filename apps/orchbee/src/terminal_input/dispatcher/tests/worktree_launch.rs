use super::*;
use crate::terminal_session::{WorktreeEdit, WorktreeInputContext, WorktreeStage, WorktreeToken};

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
        }
    }
    fn token(&self) -> WorktreeToken {
        WorktreeToken::for_test(self.serial)
    }
}
impl TerminalInputPort for Port {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        match action {
            ClientAction::RequestWorktreeCreate => self.stage = WorktreeStage::Branch,
            ClientAction::RequestWorktreeOpen => self.stage = WorktreeStage::Checkout,
            _ => return None,
        }
        self.active = true;
        Some(SessionActionDisposition::DismissNavigator)
    }
    fn worktree_input_context(&mut self) -> Option<WorktreeInputContext> {
        self.active.then_some(WorktreeInputContext {
            token: self.token(),
            stage: self.stage,
            remaining_bytes: 1024usize.saturating_sub(self.text.len()),
        })
    }
    fn edit_worktree(&mut self, token: WorktreeToken, edit: WorktreeEdit<'_>) -> bool {
        assert_eq!(token, self.token());
        if self.stage == WorktreeStage::Waiting {
            return true;
        }
        match edit {
            WorktreeEdit::Completeness(value) => {
                self.complete = value;
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
        if self.complete && !self.fault {
            self.submits += 1;
            self.stage = WorktreeStage::Waiting;
        }
    }
    fn dismiss_worktree(&mut self, token: WorktreeToken) {
        assert_eq!(token, self.token());
        self.active = false;
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
                |_, _| Ok::<(), io::Error>(()),
                &mut |mode| {
                    modes.push(mode);
                    Ok(())
                },
            )
            .unwrap();
        assert!(modes.contains(&ModeRequest::BracketedPaste(true)));
        assert!(port.forwarded.is_empty());
        assert!(port.active);
        assert_eq!(port.text, if key == b'W' { "x" } else { "" });
    }
}
#[test]
fn unicode_paste_uses_byte_budget_and_confirm_tail_stays_captured() {
    let mut port = Port::new();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let text = format!("{}a", "猫".repeat(341));
    let bytes = format!("\x1b[200~{text}\x1b[201~\rcommand");
    for byte in bytes.as_bytes() {
        dispatch_all(&mut dispatcher, &mut view, &mut port, &[*byte]);
    }
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
            |_, _| Ok::<(), io::Error>(()),
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
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(port.forwarded, b"next");
}

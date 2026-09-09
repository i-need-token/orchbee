use super::*;
use crate::terminal_session::NativeSetupInputContext;
use std::time::Duration;

struct SetupPort {
    active: bool,
    token: u64,
    stage: NativeSetupStage,
    text: String,
    shown: bool,
    complete: bool,
    submissions: Vec<NativeSetupStage>,
    forwarded: Vec<u8>,
    selections: i32,
    fault: bool,
}

impl SetupPort {
    fn new(stage: NativeSetupStage) -> Self {
        Self {
            active: true,
            token: 1,
            stage,
            text: String::new(),
            shown: false,
            complete: true,
            submissions: Vec::new(),
            forwarded: Vec::new(),
            selections: 0,
            fault: false,
        }
    }

    fn token(&self) -> NativeSetupToken {
        NativeSetupToken::for_test(self.token)
    }

    fn next_step(&mut self, stage: NativeSetupStage) {
        self.token += 1;
        self.stage = stage;
        self.text.clear();
        self.shown = false;
        self.complete = true;
        self.fault = false;
    }
}

impl TerminalInputPort for SetupPort {
    fn forward(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }

    fn setup_input_context(&mut self) -> Option<NativeSetupInputContext> {
        if !self.active {
            return None;
        }
        Some(NativeSetupInputContext::for_test(
            self.token(),
            self.stage,
            512_usize.saturating_sub(self.text.chars().count()),
            4096_usize.saturating_sub(self.text.len()),
        ))
    }

    fn edit_setup_draft(&mut self, token: NativeSetupToken, edit: NativeSetupEdit<'_>) -> bool {
        assert_eq!(token, self.token());
        match edit {
            NativeSetupEdit::Append(text) => self.text.push_str(text),
            NativeSetupEdit::Backspace => {
                let _removed = self.text.pop();
                self.fault = false;
            }
            NativeSetupEdit::Clear => {
                self.text.clear();
                self.fault = false;
            }
            NativeSetupEdit::Fault => self.fault = true,
            NativeSetupEdit::Completeness(complete) => self.complete = complete,
            NativeSetupEdit::SelectPreviousProject => {
                self.selections -= 1;
                self.shown = false;
                self.token += 1;
            }
            NativeSetupEdit::SelectNextProject => {
                self.selections += 1;
                self.shown = false;
                self.token += 1;
            }
        }
        true
    }

    fn submit_setup_draft(&mut self, token: NativeSetupToken) -> Option<SessionActionDisposition> {
        assert_eq!(token, self.token());
        if self.shown && self.complete && !self.fault {
            self.submissions.push(self.stage);
            self.next_step(match self.stage {
                NativeSetupStage::ChooseProject => NativeSetupStage::ProjectLabel,
                NativeSetupStage::ProjectLabel => NativeSetupStage::WorkspaceLabel,
                NativeSetupStage::WorkspaceLabel | NativeSetupStage::DirectoryPath => {
                    NativeSetupStage::DirectoryPath
                }
            });
        }
        Some(SessionActionDisposition::KeepNavigator)
    }
}

fn dispatch(dispatcher: &mut InputDispatcher, port: &mut SetupPort, bytes: &[u8], now: Instant) {
    dispatcher
        .dispatch(
            &mut ClientView::new(120),
            bytes,
            port,
            now,
            |_, _| Ok::<_, io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn picker_owns_brackets_and_packet_tail_cannot_edit_or_submit_the_next_step() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::ChooseProject);
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"[ignored]]\r", now);
    assert_eq!(port.selections, 1);
    assert_eq!(port.token, 4);
    assert_eq!(dispatcher.epoch, Some(ModalEpoch::Setup(port.token())));
    assert!(port.text.is_empty());
    assert!(port.submissions.is_empty());
    port.shown = true;
    dispatch(&mut dispatcher, &mut port, b"\rproject\rworkspace\r", now);
    assert_eq!(port.submissions, [NativeSetupStage::ChooseProject]);
    assert!(port.text.is_empty());
    assert!(port.forwarded.is_empty());

    let later = now + Duration::from_millis(60);
    dispatch(&mut dispatcher, &mut port, "项目\r".as_bytes(), later);
    assert_eq!(port.text, "项目");
    assert_eq!(port.submissions.len(), 1);
    port.shown = true;
    dispatch(&mut dispatcher, &mut port, b"\rworkspace\r", later);
    assert_eq!(
        port.submissions,
        [
            NativeSetupStage::ChooseProject,
            NativeSetupStage::ProjectLabel
        ]
    );
    assert!(port.text.is_empty());
    assert!(port.forwarded.is_empty());
}

#[test]
fn replacement_token_drains_a_partial_paste_without_retargeting_or_forwarding_it() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::DirectoryPath);
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\x1b[200~/old/\xe9", now);
    let old = port.token();
    port.next_step(NativeSetupStage::WorkspaceLabel);
    dispatcher.apply_setup_event(&mut port, old, ModalInputEvent::Append("stale".to_owned()));
    let later = now + Duration::from_secs(1);
    dispatch(&mut dispatcher, &mut port, b"\x9b\xaa\x1b[201~\r", later);
    assert!(port.text.is_empty());
    assert!(port.submissions.is_empty());
    assert!(port.forwarded.is_empty());
    dispatch(
        &mut dispatcher,
        &mut port,
        b"fresh",
        later + Duration::from_millis(60),
    );
    assert_eq!(port.text, "fresh");
}

#[test]
fn directory_paste_uses_remaining_bytes_and_does_not_append_an_overflow_prefix() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::DirectoryPath);
    port.text = format!("/{}", "x".repeat(4092));
    let original = port.text.clone();
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\x1b[200~\xf0\x9f", now);
    dispatch(&mut dispatcher, &mut port, b"\x98\x80\x1b[201~", now);
    assert_eq!(port.text, original);
    assert!(port.fault);
    assert!(port.complete);
    assert!(port.forwarded.is_empty());
}

#[test]
fn an_asynchronous_step_reacquires_bracketed_paste_after_a_quiet_handoff() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceLabel);
    let mut dispatcher = InputDispatcher::default();
    let mut modes = Vec::new();
    dispatcher
        .synchronize(&mut ClientView::new(120), &mut port, now, &mut |enabled| {
            modes.push(enabled);
            Ok::<_, io::Error>(())
        })
        .unwrap();
    for _ in 0..3 {
        dispatcher
            .synchronize(&mut ClientView::new(120), &mut port, now, &mut |enabled| {
                modes.push(enabled);
                Ok::<_, io::Error>(())
            })
            .unwrap();
    }
    assert_eq!(modes, [true]);
    port.active = false;
    dispatcher
        .synchronize(&mut ClientView::new(120), &mut port, now, &mut |enabled| {
            modes.push(enabled);
            Ok::<_, io::Error>(())
        })
        .unwrap();
    let later = now + Duration::from_millis(60);
    dispatcher
        .synchronize(
            &mut ClientView::new(120),
            &mut port,
            later,
            &mut |enabled| {
                modes.push(enabled);
                Ok::<_, io::Error>(())
            },
        )
        .unwrap();
    port.next_step(NativeSetupStage::WorkspaceLabel);
    port.active = true;
    dispatcher
        .synchronize(
            &mut ClientView::new(120),
            &mut port,
            later,
            &mut |enabled| {
                modes.push(enabled);
                Ok::<_, io::Error>(())
            },
        )
        .unwrap();
    assert_eq!(modes, [true, false, true]);
    assert_eq!(dispatcher.epoch, Some(ModalEpoch::Setup(port.token())));
}

use super::*;
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
    choice_count: usize,
    fault: bool,
    focus: Vec<bool>,
    edits: Vec<&'static str>,
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
            choice_count: 2,
            fault: false,
            focus: Vec::new(),
            edits: Vec::new(),
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

#[test]
fn project_delete_decision_accepts_only_drawn_plain_enter_and_never_editor_or_paste_input() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::ProjectDeleteConfirm);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(
        &mut dispatcher,
        &mut port,
        b" yY\x1b[13;5u\x1b[13;2u\x1b[200~\r\x1b[201~",
        now,
    );
    assert!(port.active);
    assert!(port.submissions.is_empty());
    assert!(port.text.is_empty());
    assert!(!port.fault);
    assert!(port.forwarded.is_empty());
    dispatch(&mut dispatcher, &mut port, b"\rTAIL", now);
    assert_eq!(port.submissions, [NativeSetupStage::ProjectDeleteConfirm]);
    assert!(!port.active);
    assert!(port.forwarded.is_empty());

    let mut port = SetupPort::new(NativeSetupStage::ProjectDeleteConfirm);
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\r", now);
    assert!(
        port.submissions.is_empty(),
        "undrawn question cannot confirm"
    );
    assert!(port.active);
}

impl TerminalInputPort for SetupPort {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        crate::tui_composition::NativeCaptureClaims::default().with_setup(self.active)
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<crate::terminal_session::NativeCaptureContext> {
        if owner != crate::tui_composition::NativeCaptureOwner::Setup || !self.active {
            return None;
        }
        Some(crate::terminal_session::NativeCaptureContext::Setup(
            crate::terminal_session::SetupCaptureContext {
                token: self.token(),
                phase: crate::tui_presentation::PresentationPhase::Admitted,
                readiness: crate::terminal_session::CaptureReadiness::Interactive,
                stage: self.stage,
                remaining_scalars: 512_usize.saturating_sub(self.text.chars().count()),
                remaining_bytes: 4096_usize.saturating_sub(self.text.len()),
            },
        ))
    }

    fn choose_setup_project(&mut self, token: NativeSetupToken, index: usize) {
        if token != self.token() || !self.shown || !self.complete || index >= self.choice_count {
            return;
        }
        self.selections = i32::try_from(index).unwrap();
        self.submit_setup_draft(token);
    }

    fn scroll_setup_projects(
        &mut self,
        token: NativeSetupToken,
        up: bool,
        lines: usize,
    ) -> Option<NativeSetupToken> {
        if token == self.token() && self.shown {
            let lines = i32::try_from(lines).unwrap();
            self.selections = if up {
                (self.selections - lines).max(0)
            } else {
                (self.selections + lines).min(self.choice_count as i32 - 1)
            };
            self.token += 1;
            self.shown = false;
            Some(self.token())
        } else {
            None
        }
    }

    fn invalidate_setup_display(&mut self) {
        self.shown = false;
    }
    fn observe_focus(&mut self, focused: bool) {
        self.focus.push(focused);
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.stage = match action {
            ClientAction::RequestProjectRename => NativeSetupStage::ProjectRename,
            ClientAction::RequestWorkspaceRename => NativeSetupStage::WorkspaceRename,
            _ => return None,
        };
        self.active = true;
        self.token += 1;
        self.shown = false;
        Some(SessionActionDisposition::KeepNavigator)
    }

    fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        assert_eq!(token, self.token());
        self.active = false;
    }

    fn forward(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }

    fn edit_setup_draft(&mut self, token: NativeSetupToken, edit: NativeSetupEdit<'_>) -> bool {
        assert_eq!(token, self.token());
        match edit {
            NativeSetupEdit::NextField => {
                if !matches!(
                    self.stage,
                    NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory
                ) {
                    return false;
                }
                self.stage = if self.stage == NativeSetupStage::QuickName {
                    NativeSetupStage::QuickDirectory
                } else {
                    NativeSetupStage::QuickName
                };
                self.token += 1;
                self.shown = false;
            }
            NativeSetupEdit::Append(text) => self.text.push_str(text),
            NativeSetupEdit::Paste(text) => {
                self.edits.push("paste");
                self.text.push_str(text);
            }
            NativeSetupEdit::Backspace => {
                let _removed = self.text.pop();
                self.fault = false;
            }
            NativeSetupEdit::Clear => {
                self.text.clear();
                self.fault = false;
            }
            NativeSetupEdit::Left => self.edits.push("left"),
            NativeSetupEdit::Right => self.edits.push("right"),
            NativeSetupEdit::Home => self.edits.push("home"),
            NativeSetupEdit::End => self.edits.push("end"),
            NativeSetupEdit::Delete => self.edits.push("delete"),
            NativeSetupEdit::WordBackspace => self.edits.push("word_backspace"),
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
            NativeSetupEdit::SelectDirectory(_) => self.edits.push("select_directory"),
            NativeSetupEdit::ScrollProjects { .. } => {
                unreachable!("scrolling has its own port method")
            }
        }
        true
    }

    fn submit_setup_draft(&mut self, token: NativeSetupToken) -> Option<SessionActionDisposition> {
        assert_eq!(token, self.token());
        if self.shown && self.complete && !self.fault {
            self.submissions.push(self.stage);
            match self.stage {
                NativeSetupStage::ChooseWorkspaceProject
                    if self.selections == i32::try_from(self.choice_count - 1).unwrap() =>
                {
                    self.next_step(NativeSetupStage::WorkspaceProjectName);
                }
                NativeSetupStage::QuickProject => self.next_step(NativeSetupStage::QuickName),
                _ => self.active = false,
            }
        }
        Some(SessionActionDisposition::KeepNavigator)
    }
}

fn painted_setup_control(
    dispatcher: &mut InputDispatcher,
    port: &mut SetupPort,
    control: crate::tui_pointer::ModalControl,
) {
    use crate::tui_pointer::{HitLayer, HitMapBuilder, HitTarget, InteractionAvailability};
    dispatcher.pointer.set_capture(true);
    dispatcher
        .synchronize(&mut ClientView::new(120), port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    let token = port.token();
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record(
        HitLayer::OverlaySurface,
        ratatui::layout::Rect::new(2, 2, 30, 8),
        HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Setup(token)),
    );
    hits.record_modal_control(
        HitLayer::Overlay,
        ratatui::layout::Rect::new(5, 5, 15, 1),
        HitTarget::Setup { token, control },
        InteractionAvailability::Enabled,
    );
    dispatcher.publish_hit_map(hits.finish());
}

#[test]
fn organization_click_submits_existing_or_no_project_and_new_only_opens_one_name_field() {
    use crate::tui_pointer::ModalControl;
    for index in 0..4 {
        let now = Instant::now();
        let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
        port.choice_count = 4;
        port.shown = true;
        let mut dispatcher = InputDispatcher::default();
        painted_setup_control(&mut dispatcher, &mut port, ModalControl::Choice(index));
        dispatch(
            &mut dispatcher,
            &mut port,
            b"\x1b[<0;6;6M\x1b[<0;6;6mterminal\r",
            now,
        );
        assert_eq!(port.submissions, [NativeSetupStage::ChooseWorkspaceProject]);
        assert!(port.forwarded.is_empty());
        assert!(port.text.is_empty());
        if index == 3 {
            assert!(port.active);
            assert_eq!(port.stage, NativeSetupStage::WorkspaceProjectName);
            port.shown = true;
            dispatch(
                &mut dispatcher,
                &mut port,
                "项目".as_bytes(),
                now + Duration::from_millis(1),
            );
            painted_setup_control(&mut dispatcher, &mut port, ModalControl::Submit);
            dispatch(
                &mut dispatcher,
                &mut port,
                b"\x1b[<0;6;6M\x1b[<0;6;6mtail\r",
                now + Duration::from_millis(2),
            );
            assert_eq!(
                port.submissions,
                [
                    NativeSetupStage::ChooseWorkspaceProject,
                    NativeSetupStage::WorkspaceProjectName
                ]
            );
            assert_eq!(port.text, "项目");
            assert!(port.forwarded.is_empty());
        }
        assert!(!port.active);
        dispatch(
            &mut dispatcher,
            &mut port,
            b"next",
            now + Duration::from_millis(5),
        );
        assert_eq!(port.forwarded, b"next");
    }
}

#[test]
fn organization_press_waits_for_release_and_intervening_input_or_resize_cancels_it() {
    use crate::tui_pointer::ModalControl;
    for change in 0..5 {
        let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
        port.choice_count = 4;
        port.shown = true;
        let mut dispatcher = InputDispatcher::default();
        painted_setup_control(&mut dispatcher, &mut port, ModalControl::Choice(2));
        let now = Instant::now();
        dispatch(&mut dispatcher, &mut port, b"\x1b[<0;6;6M", now);
        assert!(port.submissions.is_empty());
        assert_eq!(port.selections, 0);
        match change {
            0 => {
                dispatcher.invalidate_hit_map(&mut port);
                painted_setup_control(&mut dispatcher, &mut port, ModalControl::Choice(2));
            }
            1 => dispatch(&mut dispatcher, &mut port, b"\x1b[O\x1b[I", now),
            2 => dispatch(&mut dispatcher, &mut port, b"x", now),
            3 => dispatch(&mut dispatcher, &mut port, b"\x1b[B", now),
            4 => {
                dispatcher.pointer.set_capture(false);
                dispatcher.pointer.set_capture(true);
            }
            _ => unreachable!(),
        }
        dispatch(&mut dispatcher, &mut port, b"\x1b[<0;6;6mTAIL", now);
        assert!(port.submissions.is_empty(), "change {change}");
        assert!(port.forwarded.is_empty(), "change {change}");
    }
}

#[test]
fn project_arrows_and_wheel_replace_selection_without_arming_an_unpainted_submit() {
    use crate::tui_pointer::ModalControl;
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    painted_setup_control(&mut dispatcher, &mut port, ModalControl::Choice(0));
    dispatch(&mut dispatcher, &mut port, b"\x1b[B\x1b[B\x1b[A\r", now);
    assert_eq!(port.selections, 1);
    assert!(port.submissions.is_empty());
    assert!(!port.shown);
    // The old drawn row cannot be retargeted to the current selection.
    dispatch(
        &mut dispatcher,
        &mut port,
        b"\x1b[<0;6;6M\x1b[<0;6;6m",
        now + Duration::from_millis(1),
    );
    assert!(port.submissions.is_empty());
    port.shown = true;
    painted_setup_control(&mut dispatcher, &mut port, ModalControl::Choice(0));
    dispatch(
        &mut dispatcher,
        &mut port,
        b"\x1b[<64;6;6M\r",
        now + Duration::from_millis(2),
    );
    assert_eq!(port.selections, 0);
    assert!(port.submissions.is_empty());
    assert!(!port.shown);
    port.shown = true;
    dispatch(
        &mut dispatcher,
        &mut port,
        b"\rterminal",
        now + Duration::from_millis(3),
    );
    assert_eq!(port.submissions, [NativeSetupStage::ChooseWorkspaceProject]);
    assert!(port.forwarded.is_empty());
}

#[test]
fn project_mouse_reports_respect_split_frames_modifiers_bounds_and_invalidated_geometry() {
    use crate::tui_pointer::ModalControl;
    for rejected in [
        b"\x1b[<4;6;6M\x1b[<0;6;6m".as_slice(),
        b"\x1b[<0;21;6M\x1b[<0;21;6m",
        b"\x1b[<0;6;7M\x1b[<0;6;7m",
        b"\x1b[<32;6;6M\x1b[<0;6;6m",
    ] {
        let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
        port.shown = true;
        let mut dispatcher = InputDispatcher::default();
        painted_setup_control(&mut dispatcher, &mut port, ModalControl::Choice(0));
        dispatch(&mut dispatcher, &mut port, rejected, Instant::now());
        assert!(port.submissions.is_empty());
        assert!(port.forwarded.is_empty());
    }
    let report = b"\x1b[<0;6;6M\x1b[<0;6;6m";
    for split in 0..=report.len() {
        let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
        port.shown = true;
        let mut dispatcher = InputDispatcher::default();
        painted_setup_control(&mut dispatcher, &mut port, ModalControl::Choice(0));
        let now = Instant::now();
        dispatch(&mut dispatcher, &mut port, &report[..split], now);
        dispatch(
            &mut dispatcher,
            &mut port,
            &report[split..],
            now + Duration::from_millis(1),
        );
        assert_eq!(port.submissions.len(), 1, "split {split}");
    }
    for reason in 0..3 {
        let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
        port.shown = true;
        let mut dispatcher = InputDispatcher::default();
        painted_setup_control(
            &mut dispatcher,
            &mut port,
            ModalControl::Choice(if reason == 2 { 99 } else { 0 }),
        );
        match reason {
            0 => dispatcher.invalidate_hit_map(&mut port),
            1 => dispatcher.pointer.set_capture(false),
            _ => {}
        }
        dispatch(&mut dispatcher, &mut port, report, Instant::now());
        assert!(port.submissions.is_empty());
        assert!(port.forwarded.is_empty());
    }
}

#[test]
fn visible_setup_cancel_owns_its_packet_and_a_split_release() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceProjectName);
    let mut dispatcher = InputDispatcher::default();
    painted_setup_control(
        &mut dispatcher,
        &mut port,
        crate::tui_pointer::ModalControl::Cancel,
    );
    dispatch(&mut dispatcher, &mut port, b"\x1b[<0;6;6M\x1b[<0;", now);
    assert!(port.active);
    dispatch(
        &mut dispatcher,
        &mut port,
        b"6;6mtail\x1b[",
        now + Duration::from_millis(1),
    );
    assert!(!port.active);
    assert!(port.forwarded.is_empty());
    dispatch(
        &mut dispatcher,
        &mut port,
        b"Anext",
        now + Duration::from_millis(2),
    );
    assert!(port.forwarded.is_empty());
    dispatch(
        &mut dispatcher,
        &mut port,
        b"next",
        now + Duration::from_millis(3),
    );
    assert_eq!(port.forwarded, b"next");
}

#[test]
fn quick_workspace_tab_changes_capacity_and_requires_a_fresh_draw_before_submit() {
    let mut port = SetupPort::new(NativeSetupStage::QuickName);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    let now = Instant::now();
    dispatch(&mut dispatcher, &mut port, b"\t\r", now);
    assert_eq!(port.stage, NativeSetupStage::QuickDirectory);
    assert!(port.submissions.is_empty());
    assert!(!port.shown);
    port.shown = true;
    let path = format!("C:/{}x", "猫".repeat(1364));
    let input = format!("\x1b[200~{path}\x1b[201~\r");
    dispatch(&mut dispatcher, &mut port, input.as_bytes(), now);
    assert_eq!(port.text, path);
    assert!(!port.fault);
    assert_eq!(port.submissions, [NativeSetupStage::QuickDirectory]);
    assert!(port.forwarded.is_empty());
}

#[test]
fn quick_workspace_tab_remains_inert_in_the_project_name_prompt() {
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceProjectName);
    port.shown = true;
    let token = port.token();
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\t", Instant::now());
    assert_eq!(port.token(), token);
    assert_eq!(port.stage, NativeSetupStage::WorkspaceProjectName);
    assert!(port.forwarded.is_empty());
}

#[test]
fn location_rename_prefill_requires_first_draw_and_submits_exact_unicode_paste_once() {
    for (stage, key) in [
        (NativeSetupStage::WorkspaceRename, b'r'),
        (NativeSetupStage::ProjectRename, b'R'),
    ] {
        let now = Instant::now();
        let mut port = SetupPort::new(stage);
        port.text = "原名".to_owned();
        port.active = false;
        let mut dispatcher = InputDispatcher::default();
        dispatch(&mut dispatcher, &mut port, &[2, key, b'\r'], now);
        assert_eq!(port.text, "原名");
        assert!(port.submissions.is_empty());
        assert!(port.forwarded.is_empty());
        port.shown = true;
        dispatch(
            &mut dispatcher,
            &mut port,
            "\x15\x1b[200~新名😀".as_bytes(),
            now,
        );
        assert!(port.text.is_empty());
        dispatch(
            &mut dispatcher,
            &mut port,
            b"\x1b[201~\r\x02rignored\r",
            now,
        );
        assert_eq!(port.text, "新名😀");
        assert_eq!(port.submissions, [stage]);
        assert!(port.forwarded.is_empty());
        assert!(!port.active);
    }
}

#[test]
fn location_rename_preserves_prefilled_label_on_submit_and_cancels_without_submission() {
    for stage in [
        NativeSetupStage::ProjectRename,
        NativeSetupStage::WorkspaceRename,
    ] {
        let now = Instant::now();
        let mut port = SetupPort::new(stage);
        port.text = "原名".to_owned();
        port.shown = true;
        let mut dispatcher = InputDispatcher::default();
        dispatch(&mut dispatcher, &mut port, b"\r", now);
        assert_eq!(port.text, "原名");
        assert_eq!(port.submissions, [stage]);

        let mut port = SetupPort::new(stage);
        port.text = "原名".to_owned();
        let mut dispatcher = InputDispatcher::default();
        dispatch(&mut dispatcher, &mut port, b"\x1b", now);
        // A real idle poll expires the Escape before the independent Enter.
        dispatcher
            .synchronize(
                &mut ClientView::new(120),
                &mut port,
                now + Duration::from_millis(50),
                &mut |_| Ok::<(), io::Error>(()),
            )
            .unwrap();
        dispatch(
            &mut dispatcher,
            &mut port,
            b"\r",
            now + Duration::from_millis(50),
        );
        assert!(port.submissions.is_empty());
        assert!(!port.active);
        // The `\r` is an independent key in a new packet after the modal
        // released at the boundary: it reaches the pane instead of being
        // swallowed by a time window.
        assert_eq!(port.forwarded.as_slice(), b"\r");
    }
}

#[test]
fn location_rename_paste_retains_the_512_scalar_limit_instead_of_the_directory_byte_limit() {
    let now = Instant::now();
    for stage in [
        NativeSetupStage::ProjectRename,
        NativeSetupStage::WorkspaceRename,
    ] {
        let mut port = SetupPort::new(stage);
        port.text = "界".repeat(511);
        let mut dispatcher = InputDispatcher::default();
        dispatch(
            &mut dispatcher,
            &mut port,
            "\x1b[200~😀\x1b[201~".as_bytes(),
            now,
        );
        let exact = format!("{}😀", "界".repeat(511));
        assert_eq!(port.text, exact);
        assert!(!port.fault);
        dispatch(&mut dispatcher, &mut port, b"\x1b[200~x\x1b[201~", now);
        assert_eq!(port.text, exact);
        assert!(port.fault);
        assert!(port.forwarded.is_empty());
    }
}

#[test]
fn the_packet_after_a_completed_decision_forwards_in_order() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\r", now);
    assert_eq!(port.submissions, [NativeSetupStage::WorkspaceRename]);
    assert!(!port.active);
    dispatch(
        &mut dispatcher,
        &mut port,
        b"ab",
        now + Duration::from_millis(10),
    );
    assert_eq!(port.forwarded, b"ab");
}

#[test]
fn the_decision_packet_tail_stays_owned() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\rxy", now);
    assert_eq!(port.submissions, [NativeSetupStage::WorkspaceRename]);
    assert!(port.forwarded.is_empty());
}

#[test]
fn a_csi_tail_started_in_the_released_packet_keeps_frame_ownership() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\r\x1b", now);
    dispatch(
        &mut dispatcher,
        &mut port,
        b"[D",
        now + Duration::from_millis(10),
    );
    dispatch(
        &mut dispatcher,
        &mut port,
        b"z",
        now + Duration::from_millis(20),
    );
    assert_eq!(port.forwarded, b"z");
}

#[test]
fn an_unterminated_paste_tail_keeps_frame_ownership_across_reads() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\r\x1b[200~ab", now);
    dispatch(
        &mut dispatcher,
        &mut port,
        b"cd\x1b[201~",
        now + Duration::from_millis(10),
    );
    dispatch(
        &mut dispatcher,
        &mut port,
        b"z",
        now + Duration::from_millis(20),
    );
    assert_eq!(port.forwarded, b"z");
}

#[test]
fn a_utf8_tail_started_in_the_released_packet_keeps_continuations_owned() {
    for gap in [10, 60] {
        let now = Instant::now();
        let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
        port.shown = true;
        let mut dispatcher = InputDispatcher::default();
        dispatch(&mut dispatcher, &mut port, b"\r\xe4", now);
        dispatch(
            &mut dispatcher,
            &mut port,
            b"\xb8\xad",
            now + Duration::from_millis(gap),
        );
        dispatch(
            &mut dispatcher,
            &mut port,
            b"z",
            now + Duration::from_millis(gap + 10),
        );
        assert_eq!(port.forwarded, b"z", "gap {gap}ms");
    }
}

#[test]
fn an_unexpired_escape_resolver_stays_owned_but_the_next_packet_is_free() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\r\x1b", now);
    dispatch(
        &mut dispatcher,
        &mut port,
        b"x",
        now + Duration::from_millis(10),
    );
    dispatch(
        &mut dispatcher,
        &mut port,
        b"y",
        now + Duration::from_millis(20),
    );
    assert_eq!(port.forwarded, b"y");
}

#[test]
fn ready_mouse_fragments_do_not_cancel_or_type_into_a_form_after_a_busy_frame() {
    let now = Instant::now();
    let report = b"\x1b[<0;66;8M";
    for split in 1..report.len() {
        let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
        let mut dispatcher = InputDispatcher::default();
        dispatch(&mut dispatcher, &mut port, &report[..split], now);
        dispatch(
            &mut dispatcher,
            &mut port,
            &report[split..],
            now + Duration::from_millis(60),
        );
        dispatch(
            &mut dispatcher,
            &mut port,
            b"probe",
            now + Duration::from_millis(120),
        );
        assert!(port.active, "split {split}");
        assert!(!port.fault, "split {split}");
        assert_eq!(port.text, "probe", "split {split}");
        assert!(port.forwarded.is_empty(), "split {split}");
        assert!(port.submissions.is_empty(), "split {split}");
    }
}

#[test]
fn an_expired_escape_releases_before_the_next_packet() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\r\x1b", now);
    dispatcher
        .synchronize(
            &mut ClientView::new(120),
            &mut port,
            now + Duration::from_millis(60),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    dispatch(
        &mut dispatcher,
        &mut port,
        b"x",
        now + Duration::from_millis(60),
    );
    assert_eq!(port.forwarded, b"x");
}

#[test]
fn a_c1_sequence_tail_started_in_the_released_packet_keeps_ownership() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceRename);
    port.shown = true;
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\r\x9b", now);
    dispatch(
        &mut dispatcher,
        &mut port,
        b"D",
        now + Duration::from_millis(10),
    );
    dispatch(
        &mut dispatcher,
        &mut port,
        b"z",
        now + Duration::from_millis(20),
    );
    assert_eq!(port.forwarded, b"z");
}

fn dispatch(dispatcher: &mut InputDispatcher, port: &mut SetupPort, bytes: &[u8], now: Instant) {
    dispatcher
        .dispatch(
            &mut ClientView::new(120),
            bytes,
            port,
            now,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn picker_owns_brackets_and_packet_tail_cannot_edit_or_submit_the_next_step() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"[ignored]]\r", now);
    assert_eq!(port.selections, 1);
    assert_eq!(port.token, 4);
    assert_eq!(dispatcher.epoch, Some(ModalEpoch::Setup(port.token())));
    assert!(port.text.is_empty());
    assert!(port.submissions.is_empty());
    port.shown = true;
    dispatch(&mut dispatcher, &mut port, b"\rproject\rworkspace\r", now);
    assert_eq!(port.submissions, [NativeSetupStage::ChooseWorkspaceProject]);
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
            NativeSetupStage::ChooseWorkspaceProject,
            NativeSetupStage::WorkspaceProjectName
        ]
    );
    assert_eq!(port.text, "项目");
    assert!(!port.active);
    assert!(port.forwarded.is_empty());
}

#[test]
fn replacement_token_drains_a_partial_paste_without_retargeting_or_forwarding_it() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::DirectoryPath);
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\x1b[200~/old/\xe9", now);
    let old = port.token();
    port.next_step(NativeSetupStage::WorkspaceProjectName);
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
fn an_asynchronous_step_reacquires_bracketed_paste_after_the_packet_boundary() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceProjectName);
    let mut dispatcher = InputDispatcher::default();
    let mut modes = Vec::new();
    dispatcher
        .synchronize(&mut ClientView::new(120), &mut port, now, &mut |request| {
            modes.push(request);
            Ok::<_, io::Error>(())
        })
        .unwrap();
    for _ in 0..3 {
        dispatcher
            .synchronize(&mut ClientView::new(120), &mut port, now, &mut |request| {
                modes.push(request);
                Ok::<_, io::Error>(())
            })
            .unwrap();
    }
    assert_eq!(modes, [ModeRequest::BracketedPaste(true)]);
    port.active = false;
    dispatcher
        .synchronize(&mut ClientView::new(120), &mut port, now, &mut |request| {
            modes.push(request);
            Ok::<_, io::Error>(())
        })
        .unwrap();
    // The lease ends at the next read packet, not after a quiet window.
    let later = now + Duration::from_millis(60);
    dispatch(&mut dispatcher, &mut port, b"", later);
    dispatcher
        .synchronize(
            &mut ClientView::new(120),
            &mut port,
            later,
            &mut |request| {
                modes.push(request);
                Ok::<_, io::Error>(())
            },
        )
        .unwrap();
    port.next_step(NativeSetupStage::WorkspaceProjectName);
    port.active = true;
    dispatcher
        .synchronize(
            &mut ClientView::new(120),
            &mut port,
            later,
            &mut |request| {
                modes.push(request);
                Ok::<_, io::Error>(())
            },
        )
        .unwrap();
    assert_eq!(
        modes,
        [
            ModeRequest::BracketedPaste(true),
            ModeRequest::BracketedPaste(false),
            ModeRequest::BracketedPaste(true)
        ]
    );
    assert_eq!(dispatcher.epoch, Some(ModalEpoch::Setup(port.token())));
}

#[test]
fn navigation_delete_and_word_keys_reach_the_draft_as_semantic_edits() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceProjectName);
    let mut dispatcher = InputDispatcher::default();
    dispatch(
        &mut dispatcher,
        &mut port,
        b"\x1b[D\x1b[C\x1b[H\x1b[F\x1b[3~\x1b[1~\x1b[4~\x9bD\x9b3~\x1bOD\x1bOH\x8fF\x01\x05\x17",
        now,
    );
    assert_eq!(
        port.edits,
        [
            "left",
            "right",
            "home",
            "end",
            "delete",
            "home",
            "end",
            "left",
            "delete",
            "left",
            "home",
            "end",
            "home",
            "end",
            "word_backspace"
        ]
    );
    assert!(port.text.is_empty());
    assert!(port.active);
    assert!(!port.fault);
    assert!(port.forwarded.is_empty());
}

#[test]
fn modified_and_unbound_sequences_reach_no_draft_and_do_not_cancel_or_leak() {
    let now = Instant::now();
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceProjectName);
    let mut dispatcher = InputDispatcher::default();
    dispatch(
        &mut dispatcher,
        &mut port,
        b"\x1b[1;5D\x1b[3;5~\x1b[A\x1b[B\x1b[2~\x1b[5~\x1b[6~\x1bOA\x1bOB\x1b[Z\x1b[201~",
        now,
    );
    assert!(port.edits.is_empty());
    assert!(port.text.is_empty());
    assert!(port.active);
    assert!(!port.fault);
    assert!(port.forwarded.is_empty());
    // An ordinary key after the consumed sequences still edits the draft.
    dispatch(&mut dispatcher, &mut port, b"x", now);
    assert_eq!(port.text, "x");
}

#[test]
fn a_paste_is_atomic_to_the_draft_and_cannot_escalate_picker_or_commands() {
    let now = Instant::now();
    // A bracket inside a paste payload never selects a project.
    let mut port = SetupPort::new(NativeSetupStage::ChooseWorkspaceProject);
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut port, b"\x1b[200~[\x1b[201~", now);
    assert_eq!(port.edits, ["paste"]);
    assert_eq!(port.selections, 0);
    assert_eq!(port.token, 1);
    assert!(port.submissions.is_empty());
    assert!(port.forwarded.is_empty());

    // Prefix, selection brackets, and newline bytes stay inside the payload.
    let mut port = SetupPort::new(NativeSetupStage::WorkspaceProjectName);
    let mut dispatcher = InputDispatcher::default();
    dispatch(
        &mut dispatcher,
        &mut port,
        b"\x1b[200~\x02x\n[ignored]\x1b[201~",
        now,
    );
    assert_eq!(port.edits, ["paste"]);
    assert_eq!(port.text, "\x02x\n[ignored]");
    assert!(port.submissions.is_empty());
    assert!(port.forwarded.is_empty());
    // Typed text afterwards is still a separate Append, not part of the paste.
    dispatch(&mut dispatcher, &mut port, b"y", now);
    assert_eq!(port.text, "\x02x\n[ignored]y");
}

#[test]
fn paste_framing_cannot_turn_c1_payload_controls_into_setup_commands() {
    let now = Instant::now();
    let bytes = b"\x9b200~\xc2\x9b201~\r\x9b201~";
    for split in 0..=bytes.len() {
        let mut port = SetupPort::new(NativeSetupStage::WorkspaceProjectName);
        let mut dispatcher = InputDispatcher::default();
        dispatch(&mut dispatcher, &mut port, &bytes[..split], now);
        dispatch(&mut dispatcher, &mut port, &bytes[split..], now);
        assert_eq!(port.text, "\u{9b}201~\r", "split {split}");
        assert_eq!(port.edits, ["paste"]);
        assert!(port.submissions.is_empty());
        assert!(port.forwarded.is_empty());
        assert!(port.active);
        assert!(!port.fault);
        dispatch(&mut dispatcher, &mut port, b"y", now);
        assert_eq!(port.text, "\u{9b}201~\ry");
    }
}

#[test]
fn focused_host_metadata_reaches_the_client_during_setup_editing() {
    let mut port = SetupPort::new(NativeSetupStage::ProjectRename);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"a\x1b[Ob\x1b[I",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(port.focus, [false, true]);
    assert_eq!(port.text, "ab");
    assert!(port.forwarded.is_empty());
    assert!(port.active);
}

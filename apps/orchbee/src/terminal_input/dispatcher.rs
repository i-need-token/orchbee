use std::time::Instant;

use crate::client_view::{
    ClientAction, ClientKey, ClientView, KeyCode, KeyKind, Modifiers, PREFIX_CHORD, PrefixInput,
    SessionActionDisposition, ShellCommand,
};
use crate::tui_pointer::{FrameHitMap, PointerOutcome, PointerState};

use super::{
    DispatchOutcome, ModeRequest, NativeRenameDraftToken, NativeRenameEdit, NativeSetupEdit,
    NativeSetupStage, NativeSetupToken, TerminalInputPort,
    decoder::{DecodedItem, DecodedKind, LegacyDecoder},
    forward_or_report,
    modal::{ModalCapacity, ModalDecoder, ModalInputEvent},
};

#[derive(Default)]
pub(crate) struct InputDispatcher {
    decoder: ModalDecoder,
    keys: LegacyDecoder,
    epoch: Option<ModalEpoch>,
    /// Decoded bytes of the in-flight packet that still owe an ordered
    /// forward; always drained before `dispatch`/`synchronize` returns.
    pending: Vec<u8>,
    /// An open confirmation captured the current packet whole, so every
    /// later item of the same read stays swallowed.
    confirm_packet: bool,
    /// Pointer ownership: capture flag, last-drawn hit map, gesture
    /// lifecycle, and the bounded wheel stream.
    pointer: PointerState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModalEpoch {
    Rename(NativeRenameDraftToken),
    Setup(NativeSetupToken),
}

#[cfg(test)]
mod tests;

/// Adapts one decoded item to the legacy confirmation-port vocabulary. Only
/// legal single keys convert; every other complete item (modified keys,
/// navigation keys, paste runs, non-decision sequences) maps to `Other` so an
/// open confirmation swallows it without a decision — never a tail leak.
fn client_key(item: &DecodedItem) -> ClientKey {
    match item.kind {
        DecodedKind::Key(event) if event.kind != KeyKind::Release => {
            let (code, mods) = (event.code, event.mods);
            if mods == Modifiers::NONE {
                match code {
                    KeyCode::Char(c) => ClientKey::Plain(c),
                    KeyCode::Esc => ClientKey::Escape,
                    _ => ClientKey::Other,
                }
            } else if code == KeyCode::Char('b') && mods == Modifiers::CTRL {
                ClientKey::ControlB
            } else {
                ClientKey::Other
            }
        }
        _ => ClientKey::Other,
    }
}

/// The single C0 byte a literal-prefix forward emits for the chord.
fn literal_chord_byte(item: &DecodedItem) -> Option<u8> {
    match item.kind {
        DecodedKind::Key(event) => match event.normalized() {
            (KeyCode::Char(c @ 'a'..='z'), mods) if mods == Modifiers::CTRL => Some(c as u8 - 0x60),
            (KeyCode::Char(' '), mods) if mods == Modifiers::CTRL => Some(0x00),
            _ => None,
        },
        _ => None,
    }
}

impl InputDispatcher {
    /// The shell settles the armed flag after the terminal mode write
    /// succeeded, so client state never claims a capture the host did not
    /// actually take.
    pub(crate) fn set_pointer_capture(&mut self, view: &mut ClientView, enabled: bool) {
        self.pointer.set_capture(enabled);
        view.settle_pointer_released(!enabled);
    }

    /// The renderer's finished frame map becomes the resolution authority.
    pub(crate) fn publish_hit_map(&mut self, map: FrameHitMap) {
        self.pointer.publish(map);
    }

    /// A layout change or pending redraw drops the recorded regions:
    /// until the next publish, pointer events drop instead of misrouting.
    pub(crate) fn invalidate_hit_map(&mut self) {
        self.pointer.invalidate_map();
    }

    #[cfg(test)]
    pub(crate) const fn pointer_capture_enabled(&self) -> bool {
        self.pointer.capture_enabled()
    }

    pub(crate) fn synchronize<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        let owned_input = self.decoder.owns_input();
        // An idle read is a packet boundary too: a `Released` guard left
        // over from an earlier packet ends here so the lease is returned
        // even when no further bytes ever arrive. A release produced by
        // this call's own reconcile still guards the current packet tail.
        self.decoder.end_released(now);
        let mut changed = self.reconcile(view, port, now);
        let events = self.decoder.tick(now);
        changed |= !events.is_empty();
        self.apply_events(view, port, events);
        self.confirm_packet = false;
        for item in self.keys.tick(now) {
            changed = true;
            self.handle_item(view, port, now, item, &mut |_, _| Ok(()), mode)?;
        }
        self.flush_pending(view, port);
        changed |= self.pointer.tick(view, port, now);
        changed |= self.reconcile(view, port, now);
        if !self.decoder.owns_input() {
            mode(ModeRequest::BracketedPaste(false))?;
        } else if !owned_input {
            // A later wizard step can arrive after the previous decoder released its lease.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        Ok(changed)
    }

    pub(crate) fn dispatch<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        input: &[u8],
        port: &mut Port,
        now: Instant,
        mut after_layout: impl FnMut(&mut ClientView, &mut Port) -> Result<(), Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        self.synchronize(view, port, now, mode)?;
        // A new read packet begins: the modal's packet-tail guard ends at
        // this boundary, while an in-flight frame or an open ESC
        // disambiguation keeps ownership under its own rules.
        let events = self.decoder.begin_packet(now);
        self.apply_events(view, port, events);
        for byte in input.iter().copied() {
            self.reconcile(view, port, now);
            let remaining = match self.epoch {
                Some(ModalEpoch::Rename(_)) => port
                    .rename_input_context()
                    .map_or(0, |context| context.remaining_scalar_capacity()),
                Some(ModalEpoch::Setup(_)) => port.setup_input_context().map_or(0, |context| {
                    if context.stage() == NativeSetupStage::DirectoryPath {
                        context.remaining_bytes()
                    } else {
                        context.remaining_scalars()
                    }
                }),
                None => 0,
            };
            let (captured, events) = self.decoder.push(byte, now, remaining);
            if captured || !events.is_empty() {
                self.flush_pending(view, port);
                self.apply_events(view, port, events);
                self.reconcile(view, port, now);
                if captured {
                    continue;
                }
            }
            // A frame opened inside a captured packet carries `owned`
            // provenance: it stays captured even when it completes in a
            // later read, while genuinely later input decodes normally.
            for item in self.keys.push(byte, now, self.confirm_packet) {
                if let DispatchOutcome::Exit =
                    self.handle_item(view, port, now, item, &mut after_layout, mode)?
                {
                    self.flush_pending(view, port);
                    return Ok(DispatchOutcome::Exit);
                }
            }
        }
        self.flush_pending(view, port);
        // Queued pointer gestures and the wheel stream advance at the same
        // packet boundary the modal leases do.
        let _changed = self.pointer.tick(view, port, now);
        Ok(DispatchOutcome::Continue)
    }

    /// Routes one decoded item through the ownership order: an open
    /// confirmation packet consumes the whole rest of the chunk, an armed
    /// prefix resolves the item against the binding registry, and anything
    /// else joins `self.pending` so its source bytes forward in order. Only
    /// the literal `Ctrl+B` event is intercepted in the ordinary layer.
    fn handle_item<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        item: DecodedItem,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<(), Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        if item.owned {
            // The item completes a frame that began inside a captured
            // packet: consumed by that capture, never forwarded or decided.
            self.flush_pending(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        if self.confirm_packet || port.capture_confirmation_key(client_key(&item)) {
            self.flush_pending(view, port);
            self.confirm_packet = true;
            return Ok(DispatchOutcome::Continue);
        }
        if view.prefix_active() {
            self.flush_pending(view, port);
            match item.kind {
                DecodedKind::Key(event) if event.kind != KeyKind::Release => {}
                DecodedKind::Key(_) => {
                    // A release under an armed prefix is consumed without a
                    // decision; the prefix stays armed for the next press.
                    return Ok(DispatchOutcome::Continue);
                }
                DecodedKind::Paste { .. } => {
                    // Paste is exclusive-capture data, never a binding key;
                    // the pane owns it in original order and the prefix
                    // stays armed.
                    forward_or_report(view, &item.raw, port);
                    return Ok(DispatchOutcome::Continue);
                }
                DecodedKind::Mouse(_) => {}
                DecodedKind::Ignored => {}
                DecodedKind::Nondecision => {}
            }
            return self.decide(view, port, now, item, after_layout, mode);
        }
        match item.kind {
            DecodedKind::Key(event)
                if event.kind != KeyKind::Release && PREFIX_CHORD.matches(event) =>
            {
                self.flush_pending(view, port);
                return self.decide(view, port, now, item, after_layout, mode);
            }
            DecodedKind::Key(event) if event.kind == KeyKind::Release => {
                // Releases are consumed, never dispatched or forwarded.
            }
            DecodedKind::Mouse(event) => {
                // Pointer reports are semantic input, never raw-forwarded:
                // pending bytes flush first so pane-bound reports preserve
                // packet order, then the pointer state resolves the event
                // against the last drawn frame's hit map. A handled event
                // syncs the viewport like a keypress; a region-changing one
                // also stales the map — the gesture that owns the pointer
                // survives either way.
                self.flush_pending(view, port);
                match self.pointer.event(view, port, now, event) {
                    PointerOutcome::Ignored => {}
                    outcome => {
                        after_layout(view, port)?;
                        if outcome == PointerOutcome::RegionsChanged {
                            self.pointer.stale_map();
                        }
                    }
                }
            }
            DecodedKind::Ignored => {
                // Mouse-shaped bytes that failed validation or never
                // completed: pointer protocol consumed whole — never keys,
                // never pending, never forwarded raw to the pane.
            }
            _ => self.pending.extend_from_slice(&item.raw),
        }
        Ok(DispatchOutcome::Continue)
    }

    /// Applies one `ClientView` decision plus the action shell effects that
    /// used to be inlined in the byte loop.
    fn decide<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        item: DecodedItem,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<(), Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        let input = match item.kind {
            DecodedKind::Key(event) if event.kind != KeyKind::Release => PrefixInput::Key(event),
            _ => PrefixInput::NonKey,
        };
        let transition = view.decide(input);
        let action = transition.action();
        let layout_changed = transition.layout_changed();
        let shell = transition.shell();
        let requests_modal = matches!(
            action,
            Some(
                ClientAction::RequestRenameTab
                    | ClientAction::RequestWorkspaceCreate
                    | ClientAction::RequestWorkspaceDirectory
                    | ClientAction::RequestWorkspaceRename
                    | ClientAction::RequestProjectRename
            )
        );
        if requests_modal {
            // The terminal lease must exist before the state can capture any draft authority.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        let applied = action
            .clone()
            .and_then(|action| port.apply_client_action(action));
        view.commit(
            transition,
            applied.unwrap_or(SessionActionDisposition::KeepNavigator),
        );
        if action == Some(ClientAction::TogglePointerCapture) {
            // The client-local release entry: the mode write must succeed
            // before the settled flag mirrors it, exactly like the modal
            // leases above.
            let next = !self.pointer.capture_enabled();
            mode(ModeRequest::PointerCapture(next))?;
            self.pointer.set_capture(next);
            view.settle_pointer_released(!next);
        }
        if applied.is_none()
            && let Some(action) = action.as_ref().and_then(ClientAction::backend_action)
        {
            view.report_backend_unavailable(action);
        }
        self.reconcile(view, port, now);
        if requests_modal && self.epoch.is_none() {
            mode(ModeRequest::BracketedPaste(false))?;
        }
        if layout_changed || applied == Some(SessionActionDisposition::LayoutChanged) {
            after_layout(view, port)?;
            // Keyboard-driven layout changes stale the map the same way a
            // pointer navigation does.
            self.pointer.invalidate_map();
        }
        match shell {
            ShellCommand::Exit => return Ok(DispatchOutcome::Exit),
            ShellCommand::ForwardTerminal => {
                // The literal chord forwards its canonical C0 encoding, so a
                // future kitty-encoded Ctrl+B still emits the byte a pane
                // without keyboard negotiation understands.
                let byte = literal_chord_byte(&item).unwrap_or(0x02);
                forward_or_report(view, std::slice::from_ref(&byte), port);
            }
            ShellCommand::None => {}
        }
        Ok(DispatchOutcome::Continue)
    }

    fn flush_pending(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
    ) {
        if self.pending.is_empty() {
            return;
        }
        let bytes = std::mem::take(&mut self.pending);
        forward_or_report(view, &bytes, port);
    }

    fn reconcile(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        now: Instant,
    ) -> bool {
        let setup = port.setup_input_context();
        let current = setup
            .as_ref()
            .map(|context| ModalEpoch::Setup(context.token()))
            .or_else(|| {
                port.rename_input_context()
                    .map(|context| ModalEpoch::Rename(context.token()))
            });
        if current == self.epoch {
            return false;
        }
        if self.epoch.take().is_some() {
            let events = self.decoder.cancel(now);
            self.apply_events(view, port, events);
        }
        let capacity =
            if setup.is_some_and(|context| context.stage() == NativeSetupStage::DirectoryPath) {
                ModalCapacity::Bytes4096
            } else {
                ModalCapacity::Scalars512
            };
        if let Some(token) = current
            && self.decoder.start_with_capacity(capacity)
        {
            self.epoch = Some(token);
        }
        true
    }

    fn apply_events(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        events: Vec<ModalInputEvent>,
    ) {
        for event in events {
            if event == ModalInputEvent::InputFailure {
                view.report_terminal_input_unavailable();
                continue;
            }
            let Some(epoch) = self.epoch else {
                continue;
            };
            let token = match epoch {
                ModalEpoch::Rename(token) => token,
                ModalEpoch::Setup(token) => {
                    self.apply_setup_event(port, token, event);
                    continue;
                }
            };
            let edit = match event {
                ModalInputEvent::Append(ref text) => NativeRenameEdit::Append(text),
                ModalInputEvent::Paste(ref text) => NativeRenameEdit::Paste(text),
                ModalInputEvent::Backspace => NativeRenameEdit::Backspace,
                ModalInputEvent::Clear => NativeRenameEdit::Clear,
                ModalInputEvent::Left => NativeRenameEdit::Left,
                ModalInputEvent::Right => NativeRenameEdit::Right,
                ModalInputEvent::Home => NativeRenameEdit::Home,
                ModalInputEvent::End => NativeRenameEdit::End,
                ModalInputEvent::Delete => NativeRenameEdit::Delete,
                ModalInputEvent::WordBackspace => NativeRenameEdit::WordBackspace,
                ModalInputEvent::Fault => NativeRenameEdit::Fault,
                ModalInputEvent::Completeness(complete) => NativeRenameEdit::Completeness(complete),
                ModalInputEvent::Submit => {
                    let _disposition = port.submit_rename_draft(token);
                    continue;
                }
                ModalInputEvent::Cancel => {
                    port.cancel_rename_draft(token);
                    continue;
                }
                ModalInputEvent::InputFailure => continue,
            };
            let _accepted = port.edit_rename_draft(token, edit);
        }
    }

    fn apply_setup_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        token: NativeSetupToken,
        event: ModalInputEvent,
    ) {
        let Some(context) = port
            .setup_input_context()
            .filter(|context| context.token() == token)
        else {
            return;
        };
        let edit = match event {
            ModalInputEvent::Append(ref text)
                if context.stage() == NativeSetupStage::ChooseProject =>
            {
                match text.as_str() {
                    "[" => NativeSetupEdit::SelectPreviousProject,
                    "]" => NativeSetupEdit::SelectNextProject,
                    _ => return,
                }
            }
            ModalInputEvent::Append(ref text) => NativeSetupEdit::Append(text),
            ModalInputEvent::Paste(ref text) => NativeSetupEdit::Paste(text),
            ModalInputEvent::Backspace => NativeSetupEdit::Backspace,
            ModalInputEvent::Clear => NativeSetupEdit::Clear,
            ModalInputEvent::Left => NativeSetupEdit::Left,
            ModalInputEvent::Right => NativeSetupEdit::Right,
            ModalInputEvent::Home => NativeSetupEdit::Home,
            ModalInputEvent::End => NativeSetupEdit::End,
            ModalInputEvent::Delete => NativeSetupEdit::Delete,
            ModalInputEvent::WordBackspace => NativeSetupEdit::WordBackspace,
            ModalInputEvent::Fault => NativeSetupEdit::Fault,
            ModalInputEvent::Completeness(complete) => NativeSetupEdit::Completeness(complete),
            ModalInputEvent::Submit => {
                let _disposition = port.submit_setup_draft(token);
                return;
            }
            ModalInputEvent::Cancel => {
                port.cancel_setup_draft(token);
                return;
            }
            ModalInputEvent::InputFailure => return,
        };
        let selecting = matches!(
            edit,
            NativeSetupEdit::SelectPreviousProject | NativeSetupEdit::SelectNextProject
        );
        if port.edit_setup_draft(token, edit)
            && selecting
            && let Some(context) = port.setup_input_context()
            && context.stage() == NativeSetupStage::ChooseProject
        {
            // Selection changes draw authority, while this decoder still owns the same picker.
            self.epoch = Some(ModalEpoch::Setup(context.token()));
        }
    }
}

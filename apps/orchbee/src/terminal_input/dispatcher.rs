use std::time::Instant;

use super::{
    CONTROL_B, ClientAction, ClientView, DispatchOutcome, NativeRenameDraftToken, NativeRenameEdit,
    NativeSetupEdit, NativeSetupStage, NativeSetupToken, SessionActionDisposition, ShellCommand,
    TerminalInputPort, forward_or_report,
    modal::{ModalCapacity, ModalDecoder, ModalInputEvent},
    normalize_byte,
};

#[derive(Default)]
pub(crate) struct InputDispatcher {
    decoder: ModalDecoder,
    epoch: Option<ModalEpoch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModalEpoch {
    Rename(NativeRenameDraftToken),
    Setup(NativeSetupToken),
}

#[cfg(test)]
mod tests;

impl InputDispatcher {
    pub(crate) fn synchronize<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        mode: &mut impl FnMut(bool) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        let owned_input = self.decoder.owns_input();
        let mut changed = self.reconcile(view, port, now);
        let events = self.decoder.tick(now);
        changed |= !events.is_empty();
        self.apply_events(view, port, events);
        changed |= self.reconcile(view, port, now);
        if !self.decoder.owns_input() {
            mode(false)?;
        } else if !owned_input {
            // A later wizard step can arrive after the previous decoder released its lease.
            mode(true)?;
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
        mode: &mut impl FnMut(bool) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        self.synchronize(view, port, now, mode)?;
        let mut ordinary_start = 0;
        let mut modal_packet = false;
        for (offset, byte) in input.iter().copied().enumerate() {
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
                if ordinary_start < offset {
                    forward_or_report(view, &input[ordinary_start..offset], port);
                }
                self.apply_events(view, port, events);
                self.reconcile(view, port, now);
                ordinary_start = offset;
                if captured {
                    ordinary_start += 1;
                    continue;
                }
            }
            if port.capture_confirmation_key(normalize_byte(byte)) || modal_packet {
                if ordinary_start < offset {
                    forward_or_report(view, &input[ordinary_start..offset], port);
                }
                modal_packet = true;
                ordinary_start = offset + 1;
                continue;
            }
            if !view.prefix_active() && byte != CONTROL_B {
                continue;
            }
            if ordinary_start < offset {
                forward_or_report(view, &input[ordinary_start..offset], port);
            }
            let transition = view.decide(normalize_byte(byte));
            let action = transition.action();
            let requests_modal = matches!(
                action,
                Some(
                    ClientAction::RequestRenameTab
                        | ClientAction::RequestWorkspaceCreate
                        | ClientAction::RequestWorkspaceDirectory
                )
            );
            if requests_modal {
                // The terminal lease must exist before the state can capture any draft authority.
                mode(true)?;
            }
            let applied = action.and_then(|action| port.apply_client_action(action));
            view.commit(
                transition,
                applied.unwrap_or(SessionActionDisposition::KeepNavigator),
            );
            if applied.is_none()
                && let Some(action) = action.and_then(ClientAction::backend_action)
            {
                view.report_backend_unavailable(action);
            }
            self.reconcile(view, port, now);
            if requests_modal && self.epoch.is_none() {
                mode(false)?;
            }
            if transition.layout_changed() {
                after_layout(view, port)?;
            }
            match transition.shell() {
                ShellCommand::Exit => return Ok(DispatchOutcome::Exit),
                ShellCommand::ForwardTerminal => {
                    forward_or_report(view, std::slice::from_ref(&byte), port);
                }
                ShellCommand::None => {}
            }
            ordinary_start = offset + 1;
        }
        if ordinary_start < input.len() {
            forward_or_report(view, &input[ordinary_start..], port);
        }
        Ok(DispatchOutcome::Continue)
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
                ModalInputEvent::Backspace => NativeRenameEdit::Backspace,
                ModalInputEvent::Clear => NativeRenameEdit::Clear,
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
            ModalInputEvent::Backspace => NativeSetupEdit::Backspace,
            ModalInputEvent::Clear => NativeSetupEdit::Clear,
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

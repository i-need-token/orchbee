use std::time::Instant;

use super::{
    CONTROL_B, ClientAction, ClientView, DispatchOutcome, NativeRenameDraftToken, NativeRenameEdit,
    SessionActionDisposition, ShellCommand, TerminalInputPort, forward_or_report, normalize_byte,
    rename::{RenameDecoder, RenameInputEvent},
};

#[derive(Default)]
pub(crate) struct InputDispatcher {
    rename: RenameDecoder,
    epoch: Option<NativeRenameDraftToken>,
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
        let mut changed = self.reconcile(view, port, now);
        let events = self.rename.tick(now);
        changed |= !events.is_empty();
        self.apply_events(view, port, events);
        changed |= self.reconcile(view, port, now);
        if !self.rename.owns_input() {
            mode(false)?;
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
            let remaining = port
                .rename_input_context()
                .map_or(0, |context| context.remaining_scalar_capacity());
            let (captured, events) = self.rename.push(byte, now, remaining);
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
            if action == Some(ClientAction::RequestRenameTab) {
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
            if action == Some(ClientAction::RequestRenameTab) && self.epoch.is_none() {
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
        let current = port.rename_input_context().map(|context| context.token());
        if current == self.epoch {
            return false;
        }
        if self.epoch.take().is_some() {
            let events = self.rename.cancel(now);
            self.apply_events(view, port, events);
        }
        if let Some(token) = current
            && self.rename.start()
        {
            self.epoch = Some(token);
        }
        true
    }

    fn apply_events(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        events: Vec<RenameInputEvent>,
    ) {
        for event in events {
            if event == RenameInputEvent::InputFailure {
                view.report_terminal_input_unavailable();
                continue;
            }
            let Some(token) = self.epoch else {
                continue;
            };
            let edit = match event {
                RenameInputEvent::Append(ref text) => NativeRenameEdit::Append(text),
                RenameInputEvent::Backspace => NativeRenameEdit::Backspace,
                RenameInputEvent::Clear => NativeRenameEdit::Clear,
                RenameInputEvent::Fault => NativeRenameEdit::Fault,
                RenameInputEvent::Completeness(complete) => {
                    NativeRenameEdit::Completeness(complete)
                }
                RenameInputEvent::Submit => {
                    let _disposition = port.submit_rename_draft(token);
                    continue;
                }
                RenameInputEvent::Cancel => {
                    port.cancel_rename_draft(token);
                    continue;
                }
                RenameInputEvent::InputFailure => continue,
            };
            let _accepted = port.edit_rename_draft(token, edit);
        }
    }
}

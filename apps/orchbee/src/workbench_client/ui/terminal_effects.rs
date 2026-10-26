//! Pane effect intake and post-flush dispatch through one lifecycle-owned FIFO.
use super::{NativeProjectionStatus, NativeUi};
use crate::workbench_client::{
    state::ClientNotice,
    terminal_effects::{Kind, Outcome, Payload, Scope},
};
use std::io;

impl NativeUi {
    pub(in crate::workbench_client) fn copy_picker_identifier(&mut self, text: String) -> bool {
        if text.len() != 32 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return false;
        }
        let Some(key) = self.state.as_ref().and_then(|state| state.focus_pane_ref()) else {
            return false;
        };
        self.effects.enqueue_identifier(key, text)
    }

    pub(super) fn apply_program_effect(&mut self, index: usize, record: &[u8]) -> Result<bool, ()> {
        let projection = &mut self.projections[index];
        let Some(payload) = projection.effect_stream.receive(record).map_err(|_| ())? else {
            return Ok(false);
        };
        let bell = matches!(payload, Payload::Bell);
        if !self.effects.enqueue_program(
            projection.key.clone(),
            *projection.value.attachment(),
            *projection.value.epoch(),
            payload,
        ) && let Some(state) = self.state.as_mut()
        {
            state.set_notice(if bell {
                ClientNotice::BellFailed
            } else {
                ClientNotice::CopyFailed
            });
        }
        Ok(true)
    }

    pub(super) fn reconcile_effects(&mut self) {
        let projections = &self.projections;
        let state = self.state.as_ref();
        let copy_mode = &self.copy_mode;
        self.effects.reconcile(|key, scope| {
            let Some(state) = state else {
                return false;
            };
            if scope == Scope::PickerIdentifier {
                return state.focus_pane_ref().as_ref() == Some(key)
                    && !state.close_suppresses_attachment(key.pane().anchor());
            }
            if scope == Scope::Diagnostic {
                return state.focus_pane_ref().as_ref() == Some(key)
                    && state
                        .view()
                        .selected_provenance()
                        .is_some_and(|provenance| {
                            matches!(
                                provenance,
                                crate::workbench_client::state::CreateTerminalProvenance::Failed(_)
                            )
                        })
                    && !state.close_suppresses_attachment(key.pane().anchor());
            }
            if !state.pane_attached(key)
                || state
                    .desired()
                    .is_none_or(|desired| desired.anchor() != key.pane().anchor())
                || state.close_suppresses_attachment(key.pane().anchor())
            {
                return false;
            }
            projections.iter().any(|projection| {
                if &projection.key != key {
                    return false;
                }
                match scope {
                    Scope::Diagnostic | Scope::PickerIdentifier => false,
                    Scope::Program { attachment, epoch } => {
                        *projection.value.attachment() == attachment
                            && *projection.value.epoch() == epoch
                    }
                    Scope::Selection {
                        generation,
                        export,
                        episode,
                    } => {
                        let current_projection = state.ready().is_some()
                            && state.panel_viewer_live(key.pane().panel())
                            && projection.status == NativeProjectionStatus::FrameAvailable
                            && projection
                                .value
                                .frame()
                                .is_some_and(|frame| frame.route.generation == generation);
                        if !current_projection {
                            return false;
                        }
                        match (export, episode) {
                            (None, None) => projection.selection.as_ref().is_some_and(|driver| {
                                matches!(
                                    driver.selection(),
                                    orchbee_terminal_protocol::SelectionChangedState::Active { .. }
                                )
                            }),
                            (Some(export), Some(episode)) => {
                                copy_mode.effect_current(key, generation, export, episode)
                            }
                            _ => false,
                        }
                    }
                }
            })
        });
        self.discard_cancelled_terminal_effect();
    }

    pub(super) fn discard_cancelled_terminal_effect(&mut self) {
        if self
            .terminal_effect
            .as_ref()
            .is_some_and(|job| job.cancelled())
        {
            self.terminal_effect.take();
        }
    }

    pub(super) fn dispatch_effect_after_draw(&mut self) {
        self.reconcile_effects();
        if self.retirement_failure.is_none()
            && let Some(job) = self.effects.dispatch()
        {
            match job.kind() {
                Kind::Clipboard => self.clipboard.submit(job),
                Kind::Bell => self.terminal_effect = Some(job),
            }
        }
    }

    pub(in crate::workbench_client) fn deliver_terminal_effect(
        &mut self,
        emit_bell: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        self.reconcile_effects();
        let Some(job) = self.terminal_effect.take() else {
            return Ok(());
        };
        if !job.begin() {
            return Ok(());
        }
        let result = emit_bell();
        job.settle(if result.is_ok() {
            Outcome::Delivered
        } else {
            Outcome::Unknown
        });
        result
    }

    pub(super) fn settle_effect_delivery(&mut self) -> bool {
        self.reconcile_effects();
        let Some(completion) = self.effects.take_completion() else {
            return false;
        };
        if let Scope::Selection {
            generation,
            export: Some(export),
            episode: Some(episode),
        } = completion.scope
        {
            let _ = self.copy_effect_outcome(
                &completion.key,
                generation,
                export,
                episode,
                completion.outcome,
            );
            return true;
        }
        let kind = completion.kind;
        let outcome = completion.outcome;
        let notice = match (kind, outcome) {
            (_, Outcome::Delivered | Outcome::Cancelled) => None,
            (_, Outcome::Unsupported) => Some(ClientNotice::Unsupported(
                crate::tui_capabilities::Capability::Clipboard,
            )),
            (Kind::Clipboard, Outcome::Failed) => Some(ClientNotice::CopyFailed),
            (Kind::Clipboard, Outcome::Unknown) => Some(ClientNotice::CopyUnknown),
            (Kind::Bell, Outcome::Failed) => Some(ClientNotice::BellFailed),
            (Kind::Bell, Outcome::Unknown) => Some(ClientNotice::BellUnknown),
        };
        if let Some(notice) = notice
            && let Some(state) = self.state.as_mut()
        {
            state.set_notice(notice);
        }
        true
    }
}

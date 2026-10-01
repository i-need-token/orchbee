//! Typed popup lifecycle adapters share the actor without impersonating a plugin.
use super::*;
use orchbee_workbench_protocol::{
    TerminalEditorAction, TerminalEditorFailure, TerminalEditorOutcome, TerminalEditorRequest,
};

#[derive(Clone, Copy)]
pub(super) enum PopupKind {
    Plugin,
    Editor,
}
pub(super) enum PopupOutcome {
    Opened(String),
    State(bool),
    Closing,
    Closed,
    Refused(Reason),
}
impl PopupOutcome {
    pub(super) fn plugin(outcome: PluginOutcome) -> Result<Self, Reason> {
        match outcome {
            PluginOutcome::PopupOpened { title, .. } => Ok(Self::Opened(title)),
            PluginOutcome::PopupState { settled, .. } => Ok(Self::State(settled)),
            PluginOutcome::PopupClosing { .. } => Ok(Self::Closing),
            PluginOutcome::PopupClosed { .. } => Ok(Self::Closed),
            PluginOutcome::Failed { .. } => Ok(Self::Refused(Reason::TargetUnavailable)),
            _ => Err(Reason::Interrupted),
        }
    }
    fn editor(outcome: TerminalEditorOutcome) -> Result<Self, Reason> {
        match outcome {
            TerminalEditorOutcome::Opened { title, .. } => Ok(Self::Opened(title)),
            TerminalEditorOutcome::State { settled, .. } => Ok(Self::State(settled)),
            TerminalEditorOutcome::Closing { .. } => Ok(Self::Closing),
            TerminalEditorOutcome::Closed { .. } => Ok(Self::Closed),
            TerminalEditorOutcome::Failed {
                reason: TerminalEditorFailure::PhysicalUnknown,
            }
            | TerminalEditorOutcome::Unknown { .. } => Err(Reason::Interrupted),
            TerminalEditorOutcome::Failed { reason } => Ok(Self::Refused(match reason {
                TerminalEditorFailure::StaleSource => Reason::StaleView,
                TerminalEditorFailure::Busy => Reason::Busy,
                _ => Reason::TargetUnavailable,
            })),
        }
    }
}
impl PopupSession {
    pub(super) fn editor(
        &self,
        client: &mut LocalClient,
        action: TerminalEditorAction,
    ) -> Result<PopupOutcome, Reason> {
        let deadline = self.clock.now()
            + if matches!(action, TerminalEditorAction::Open { .. }) {
                Duration::from_secs(20)
            } else {
                Duration::from_secs(2)
            };
        client
            .terminal_editor(
                TerminalEditorRequest {
                    protocol_version: ProtocolVersion,
                    site_id: self.site.clone(),
                    request_id: request_id()?,
                    action,
                },
                deadline,
                Some(self.wake.as_local_io()),
            )
            .map_err(|_| Reason::ProjectionUnavailable)
            .and_then(|result| PopupOutcome::editor(result.outcome))
    }
    pub(super) fn observe_popup(
        &self,
        client: &mut LocalClient,
        kind: PopupKind,
        close: bool,
    ) -> Result<PopupOutcome, Reason> {
        match kind {
            PopupKind::Plugin => self
                .plugin(
                    client,
                    if close {
                        PluginAction::PopupClose {
                            popup_id: self.id.clone(),
                        }
                    } else {
                        PluginAction::PopupRead {
                            popup_id: self.id.clone(),
                        }
                    },
                )
                .and_then(PopupOutcome::plugin),
            PopupKind::Editor => self.editor(
                client,
                if close {
                    TerminalEditorAction::Close {
                        popup_id: self.id.clone(),
                    }
                } else {
                    TerminalEditorAction::Read {
                        popup_id: self.id.clone(),
                    }
                },
            ),
        }
    }
}

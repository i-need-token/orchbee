//! Typed popup lifecycle adapters share the actor without impersonating a plugin.
use super::{
    AsLocalIo, Duration, LocalClient, PluginAction, PluginOutcome, PopupRunner, ProtocolVersion,
    Reason, request_id,
};

#[derive(Clone, Copy)]
pub(super) enum PopupKind {
    Configured,
    Plugin,
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
            PluginOutcome::PopupUnknown { .. }
            | PluginOutcome::PanelObserved { .. }
            | PluginOutcome::PanelFound { .. }
            | PluginOutcome::Started { .. }
            | PluginOutcome::Logs { .. }
            | PluginOutcome::Cancellation { .. }
            | PluginOutcome::Unknown { .. } => Err(Reason::Interrupted),
        }
    }
}
impl PopupRunner {
    pub(super) fn configured(
        &self,
        client: &mut LocalClient,
        action: orchbee_workbench_protocol::ConfiguredCommandAction,
    ) -> Result<PopupOutcome, Reason> {
        use orchbee_workbench_protocol::{
            ConfiguredCommandFailure as F, ConfiguredCommandOutcome as O, ConfiguredCommandRequest,
        };
        let deadline = self.clock.now()
            + if matches!(
                action,
                orchbee_workbench_protocol::ConfiguredCommandAction::PopupOpen { .. }
            ) {
                Duration::from_secs(20)
            } else {
                Duration::from_secs(2)
            };
        let reply = client
            .configured_command(
                ConfiguredCommandRequest {
                    protocol_version: ProtocolVersion,
                    session_id: self.session.clone(),
                    request_id: request_id()?,
                    action,
                },
                deadline,
                Some(self.wake.as_local_io()),
            )
            .map_err(|_| Reason::ProjectionUnavailable)?;
        match reply.outcome {
            O::PopupOpened { title, .. } => Ok(PopupOutcome::Opened(title)),
            O::PopupState { settled, .. } => Ok(PopupOutcome::State(settled)),
            O::PopupClosing { .. } => Ok(PopupOutcome::Closing),
            O::PopupClosed { .. } => Ok(PopupOutcome::Closed),
            O::PopupUnknown { .. }
            | O::Failed {
                reason: F::PhysicalUnknown,
            } => Err(Reason::Interrupted),
            O::Failed { reason } => Ok(PopupOutcome::Refused(match reason {
                F::StaleSource => Reason::StaleView,
                F::Busy => Reason::Busy,
                F::Denied
                | F::InvalidIntent
                | F::SessionMismatch
                | F::Unavailable
                | F::Conflict
                | F::NotFound
                | F::PhysicalUnknown => Reason::TargetUnavailable,
            })),
            // This phase consumes its requested plugin/popup receipt; other receipt kinds cannot confirm that operation.
            _ => Err(Reason::Interrupted),
        }
    }
    pub(super) fn observe_popup(
        &self,
        client: &mut LocalClient,
        kind: PopupKind,
        close: bool,
    ) -> Result<PopupOutcome, Reason> {
        match kind {
            PopupKind::Configured => self.configured(
                client,
                if close {
                    orchbee_workbench_protocol::ConfiguredCommandAction::PopupClose {
                        popup_id: self.id.clone(),
                    }
                } else {
                    orchbee_workbench_protocol::ConfiguredCommandAction::PopupRead {
                        popup_id: self.id.clone(),
                    }
                },
            ),
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
        }
    }
}

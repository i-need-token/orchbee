//! Configured popup launch uses the ordinary independent owner, with no plugin admission.
use super::{
    Failure, LocalTerminalService, Outcome, Phase, PopupKind, PopupOwner, ProtocolVersion,
    WorkbenchRequestId, client, lease_expiry, placement,
};
use orchbee_workbench_protocol::{
    ConfiguredCommandAction as Action, ConfiguredCommandFailure as CommandFailure,
    ConfiguredCommandOutcome as CommandOutcome, ConfiguredCommandPopupSpec,
    ConfiguredCommandRequest, ConfiguredCommandResult, WorkbenchTerminalGeometry,
};
impl From<Failure> for CommandFailure {
    fn from(value: Failure) -> Self {
        match value {
            Failure::InvalidIntent => Self::InvalidIntent,
            Failure::SessionMismatch => Self::SessionMismatch,
            Failure::Unavailable | Failure::EditorUnavailable => Self::Unavailable,
            Failure::PhysicalUnknown => Self::PhysicalUnknown,
            Failure::Busy => Self::Busy,
            Failure::Conflict => Self::Conflict,
            Failure::NotFound => Self::NotFound,
            Failure::StaleSource => Self::StaleSource,
        }
    }
}
impl From<CommandFailure> for Failure {
    fn from(value: CommandFailure) -> Self {
        match value {
            CommandFailure::Denied | CommandFailure::Unavailable => Self::Unavailable,
            CommandFailure::InvalidIntent => Self::InvalidIntent,
            CommandFailure::SessionMismatch => Self::SessionMismatch,
            CommandFailure::PhysicalUnknown => Self::PhysicalUnknown,
            CommandFailure::Busy => Self::Busy,
            CommandFailure::Conflict => Self::Conflict,
            CommandFailure::NotFound => Self::NotFound,
            CommandFailure::StaleSource => Self::StaleSource,
        }
    }
}
impl From<Outcome> for CommandOutcome {
    fn from(value: Outcome) -> Self {
        match value {
            Outcome::PopupOpened { popup_id, title } => Self::PopupOpened { popup_id, title },
            Outcome::PopupState { popup_id, settled } => Self::PopupState { popup_id, settled },
            Outcome::PopupClosing { popup_id } => Self::PopupClosing { popup_id },
            Outcome::PopupClosed { popup_id } => Self::PopupClosed { popup_id },
            Outcome::PopupUnknown { popup_id } => Self::PopupUnknown { popup_id },
            Outcome::Failed { reason } => Self::Failed {
                reason: reason.into(),
            },
        }
    }
}
impl LocalTerminalService {
    pub(crate) fn execute_configured_popup(
        &self,
        owner: impl Into<PopupOwner>,
        request: ConfiguredCommandRequest,
    ) -> ConfiguredCommandResult {
        let owner = owner.into();
        let outcome = (|| {
            request.validate().map_err(|_| Failure::InvalidIntent)?;
            if self.workbench_session_matches(&request.session_id) != Some(true) {
                return Err(Failure::SessionMismatch);
            }
            match &request.action {
                Action::PopupOpen {
                    popup_id,
                    spec,
                    geometry,
                } => self.open_configured_popup(owner, popup_id, spec, *geometry),
                Action::PopupRead { popup_id } => {
                    self.popup_read(owner, popup_id, &request.session_id, PopupKind::Configured)
                }
                Action::PopupClose { popup_id } => {
                    self.popup_close(owner, popup_id, PopupKind::Configured)
                }
                Action::List { .. } | Action::Invoke { .. } => Err(Failure::InvalidIntent),
            }
        })()
        .unwrap_or_else(|reason| {
            if let Action::PopupOpen { popup_id, .. } = &request.action
                && let Ok(mut owners) = self.popups.0.lock()
                && let Some(popup) = owners.get_mut(&owner)
                && popup.id == *popup_id
                && popup.kind == PopupKind::Configured
                && (popup.phase == Phase::Unknown
                    || popup.phase == Phase::Opening && popup.authorization.is_some())
            {
                popup.phase = Phase::Unknown;
                return Outcome::PopupUnknown {
                    popup_id: popup_id.clone(),
                };
            }
            Outcome::Failed { reason }
        });
        ConfiguredCommandResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome: outcome.into(),
        }
    }
    fn open_configured_popup(
        &self,
        owner: PopupOwner,
        id: &WorkbenchRequestId,
        spec: &ConfiguredCommandPopupSpec,
        geometry: WorkbenchTerminalGeometry,
    ) -> Result<Outcome, Failure> {
        self.reserve_popup(owner, id, PopupKind::Configured)?;
        let reference =
            client::RequestId::parse(id.as_str()).map_err(|_| Failure::InvalidIntent)?;
        let prepared = (|| {
            let revision = self
                .coordinator
                .lock()
                .map_err(|_| Failure::Unavailable)?
                .configured_command_revision()?;
            let frozen = self.prepare_configured_launch(
                &spec.command_id,
                Some(&spec.source),
                &spec.context,
                revision,
                None,
            )?;
            if frozen.command.action != orchbee_local_runtime::CustomCommandAction::Popup {
                return Err(Failure::InvalidIntent);
            }
            let entry = super::super::configured_commands::descriptor(
                &orchbee_local_runtime::CustomCommandEntry {
                    id: spec.command_id.clone(),
                    definition: frozen.command.clone(),
                },
            );
            if entry.width != spec.width || entry.height != spec.height {
                return Err(Failure::StaleSource);
            }
            let title = frozen
                .command
                .description
                .as_deref()
                .unwrap_or("Command")
                .to_owned();
            let selection = super::super::configured_commands::launch(
                &frozen.command.command,
                &frozen.environment,
            )?;
            let launch = client::LaunchSelection::new(
                selection.executable().into(),
                selection.arguments().to_vec(),
            )
            .map_err(|_| Failure::InvalidIntent)?;
            let geometry = client::Geometry::new(
                geometry.columns(),
                geometry.rows(),
                geometry.cell_pixel_width(),
                geometry.cell_pixel_height(),
            )
            .map_err(|_| Failure::InvalidIntent)?;
            let cwd = client::WorkingDirectory::parse(frozen.directory.as_str())
                .map_err(|_| Failure::InvalidIntent)?;
            let request = client::TerminalCreateRequest::new(
                reference.clone(),
                None,
                Some(cwd),
                client::StartMode::Immediate { geometry },
                Some(launch),
            );
            let capture = self.launch_capture();
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            let handle = workers.request_handle();
            let (prepared, authorization) = self
                .coordinator
                .lock()
                .map_err(|_| Failure::Unavailable)?
                .prepare_configured_popup(
                    request,
                    &spec.source,
                    frozen.snapshot.revision,
                    placement(handle.as_ref()),
                    lease_expiry(),
                    &capture,
                )?;
            self.popups
                .0
                .lock()
                .map_err(|_| Failure::Unavailable)?
                .get_mut(&owner)
                .ok_or(Failure::Unavailable)?
                .authorization = authorization;
            Ok((prepared, title, handle))
        })();
        let (prepared, title, handle) = match prepared {
            Ok(value) => value,
            Err(reason) => {
                let mut owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
                if owners
                    .get(&owner)
                    .is_some_and(|popup| popup.authorization.is_none())
                {
                    owners.remove(&owner);
                } else if let Some(popup) = owners.get_mut(&owner) {
                    popup.phase = Phase::Unknown;
                }
                return Err(reason);
            }
        };
        let result = self.complete_popup_dispatch(prepared, handle.as_ref())?;
        let workers = self.workers.lock().map_err(|_| Failure::PhysicalUnknown)?;
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| Failure::PhysicalUnknown)?;
        let target = placement(workers.request_handle().as_ref())
            .and_then(|worker| coordinator.viewer_target(&reference, worker));
        let mut owners = self.popups.0.lock().map_err(|_| Failure::PhysicalUnknown)?;
        let popup = owners.get_mut(&owner).ok_or(Failure::PhysicalUnknown)?;
        match result {
            client::ServerMessage::TerminalCreateResult(result)
                if matches!(
                    result.outcome(),
                    client::CreateOutcome::Accepted {
                        state: client::CreateAcceptedState::Started { .. }
                    }
                ) && target.is_some() =>
            {
                popup.target = target;
                popup.phase = Phase::Active;
                Ok(Outcome::PopupOpened {
                    popup_id: id.clone(),
                    title,
                })
            }
            _ if popup.authorization.is_none()
                || popup
                    .authorization
                    .is_some_and(|auth| coordinator.popup_definitively_uncreated(auth)) =>
            {
                if let Some(auth) = popup.authorization {
                    coordinator.release_independent_popup(&reference, auth);
                }
                owners.remove(&owner);
                Err(Failure::Unavailable)
            }
            // This exchange accepts only the result family selected by its submitted request.
            _ => {
                popup.phase = Phase::Unknown;
                Ok(Outcome::PopupUnknown {
                    popup_id: id.clone(),
                })
            }
        }
    }
}

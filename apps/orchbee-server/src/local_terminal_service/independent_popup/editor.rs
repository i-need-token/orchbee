//! Editor opening shares popup ownership and cleanup, without plugin command admission.
use super::{
    BindingViewerTarget, Failure, Instant, LocalTerminalService, NonZeroU64, Outcome, Phase,
    PopupKind, PreparedOperation, ProtocolVersion, SETTLEMENT_WINDOW, WorkbenchRequestId,
    WorkerOperationRequest, WorkerOperationResult, client, hex, lease_expiry, node, placement,
};
use orchbee_workbench_protocol::{
    TerminalEditorAction as Action, TerminalEditorFailure as EditorFailure,
    TerminalEditorOutcome as EditorOutcome, TerminalEditorRequest, TerminalEditorResult,
};

impl From<Failure> for EditorFailure {
    fn from(value: Failure) -> Self {
        match value {
            Failure::InvalidIntent => Self::InvalidIntent,
            Failure::SessionMismatch => Self::SessionMismatch,
            Failure::Unavailable => Self::Unavailable,
            Failure::PhysicalUnknown => Self::PhysicalUnknown,
            Failure::Busy => Self::Busy,
            Failure::Conflict => Self::Conflict,
            Failure::NotFound => Self::NotFound,
            Failure::StaleSource => Self::StaleSource,
            Failure::EditorUnavailable => Self::EditorUnavailable,
        }
    }
}
impl From<EditorFailure> for Failure {
    fn from(value: EditorFailure) -> Self {
        match value {
            EditorFailure::InvalidIntent => Self::InvalidIntent,
            EditorFailure::SessionMismatch => Self::SessionMismatch,
            EditorFailure::PhysicalUnknown => Self::PhysicalUnknown,
            EditorFailure::Busy => Self::Busy,
            EditorFailure::Conflict => Self::Conflict,
            EditorFailure::NotFound => Self::NotFound,
            EditorFailure::StaleSource => Self::StaleSource,
            EditorFailure::EditorUnavailable => Self::EditorUnavailable,
            EditorFailure::Denied | EditorFailure::Unavailable => Self::Unavailable,
        }
    }
}
impl From<Outcome> for EditorOutcome {
    fn from(value: Outcome) -> Self {
        match value {
            Outcome::PopupOpened { popup_id, title } => Self::Opened { popup_id, title },
            Outcome::PopupState { popup_id, settled } => Self::State { popup_id, settled },
            Outcome::PopupClosing { popup_id } => Self::Closing { popup_id },
            Outcome::PopupClosed { popup_id } => Self::Closed { popup_id },
            Outcome::PopupUnknown { popup_id } => Self::Unknown { popup_id },
            Outcome::Failed { reason } => Self::Failed {
                reason: reason.into(),
            },
        }
    }
}

impl LocalTerminalService {
    pub(in crate::local_terminal_service) fn execute_editor_popup(
        &self,
        owner: NonZeroU64,
        request: TerminalEditorRequest,
        source: Option<(node::TerminalEditorSource, BindingViewerTarget)>,
    ) -> TerminalEditorResult {
        let result = (|| {
            request.validate().map_err(|_| Failure::InvalidIntent)?;
            if self.workbench_session_matches(&request.session_id) != Some(true) {
                return Err(Failure::SessionMismatch);
            }
            match &request.action {
                Action::Open {
                    popup_id,
                    spec,
                    geometry,
                    ..
                } => {
                    let (source, target) = source.ok_or(Failure::StaleSource)?;
                    self.open_editor_popup(owner, popup_id, spec, source, target, *geometry)
                }
                Action::Read { popup_id } => {
                    self.popup_read(owner, popup_id, &request.session_id, PopupKind::Editor)
                }
                Action::Close { popup_id } => self.popup_close(owner, popup_id, PopupKind::Editor),
            }
        })();
        let outcome = result.unwrap_or_else(|reason| {
            if let Action::Open { popup_id, .. } = &request.action
                && let Ok(mut owners) = self.popups.0.lock()
                && let Some(popup) = owners.get_mut(&owner)
                && popup.id == *popup_id
                && popup.kind == PopupKind::Editor
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
        TerminalEditorResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome: outcome.into(),
        }
    }

    fn open_editor_popup(
        &self,
        owner: NonZeroU64,
        id: &WorkbenchRequestId,
        spec: &orchbee_workbench_protocol::ScrollbackEditorSpec,
        source: node::TerminalEditorSource,
        source_target: BindingViewerTarget,
        geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry,
    ) -> Result<Outcome, Failure> {
        self.reserve_popup(owner, id, PopupKind::Editor)?;
        let prepared = (|| {
            let handle = self
                .workers
                .lock()
                .map_err(|_| Failure::Unavailable)?
                .request_handle()
                .ok_or(Failure::Unavailable)?;
            if !self
                .coordinator
                .lock()
                .map_err(|_| Failure::Unavailable)?
                .editor_source_is_current(spec, source_target, placement(Some(&handle)))
            {
                return Err(Failure::StaleSource);
            }
            let query = node::TerminalEditorCommandRequest {
                request_id: self.next_viewer_operation().ok_or(Failure::Unavailable)?,
                worker_id: node::WorkerId::from_bytes(handle.worker_id().as_bytes())
                    .map_err(|_| Failure::Unavailable)?,
                worker_generation: node::WorkerGeneration::new(handle.generation()),
            };
            let selection = match handle.request(WorkerOperationRequest::EditorCommand(query)) {
                Ok(WorkerOperationResult::EditorCommand(result)) if result.request == query => {
                    match result.outcome {
                        node::TerminalEditorCommandOutcome::Resolved { selection, .. } => selection,
                        node::TerminalEditorCommandOutcome::Refused(..) => {
                            return Err(Failure::EditorUnavailable);
                        }
                    }
                }
                _ => return Err(Failure::Unavailable),
            };
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            let current = workers.request_handle();
            let (prepared, title, authorization) = self
                .coordinator
                .lock()
                .map_err(|_| Failure::Unavailable)?
                .prepare_editor_popup(
                    crate::session_coordinator::EditorPopupLaunch {
                        id,
                        spec,
                        source,
                        target: source_target,
                        selection: &selection,
                    },
                    placement(current.as_ref()),
                    lease_expiry(),
                )?;
            let mut owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
            owners
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
                }
                return Err(reason);
            }
        };
        let result = self.complete_popup_dispatch(prepared, Some(&handle))?;
        let reference =
            client::RequestId::parse(id.as_str()).map_err(|_| Failure::InvalidIntent)?;
        {
            let workers = self.workers.lock().map_err(|_| Failure::PhysicalUnknown)?;
            let mut coordinator = self
                .coordinator
                .lock()
                .map_err(|_| Failure::PhysicalUnknown)?;
            let mut owners = self.popups.0.lock().map_err(|_| Failure::PhysicalUnknown)?;
            let popup = owners.get_mut(&owner).ok_or(Failure::PhysicalUnknown)?;
            if !matches!(result, client::ServerMessage::TerminalCreateResult(ref result) if matches!(result.outcome(),client::CreateOutcome::Accepted {state:client::CreateAcceptedState::AwaitingStart}))
            {
                if popup.authorization.is_none()
                    || popup
                        .authorization
                        .is_some_and(|auth| coordinator.popup_definitively_uncreated(auth))
                {
                    if let Some(auth) = popup.authorization {
                        coordinator.release_independent_popup(&reference, auth);
                    }
                    owners.remove(&owner);
                    return Err(Failure::Unavailable);
                }
                popup.phase = Phase::Unknown;
                return Ok(Outcome::PopupUnknown {
                    popup_id: id.clone(),
                });
            }
            let target = placement(workers.request_handle().as_ref())
                .and_then(|worker| coordinator.viewer_target(&reference, worker))
                .ok_or(Failure::PhysicalUnknown)?;
            popup.target = Some(target);
        }
        let prepared = (|| {
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            let worker = placement(workers.request_handle().as_ref());
            let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
            if !coordinator.editor_source_is_current(spec, source_target, worker) {
                return Err(Failure::StaleSource);
            }
            let start_id = coordinator
                .allocate_identity()
                .and_then(|bytes| client::RequestId::parse(&hex(bytes)).ok())
                .ok_or(Failure::Unavailable)?;
            let geometry = client::Geometry::new(
                geometry.columns(),
                geometry.rows(),
                geometry.cell_pixel_width(),
                geometry.cell_pixel_height(),
            )
            .map_err(|_| Failure::InvalidIntent)?;
            let request = client::TerminalStartRequest::new(start_id, reference.clone(), geometry)
                .map_err(|_| Failure::InvalidIntent)?;
            // Start renews binding authority. Own its exact fence before any possible handoff.
            let mut owners = self.popups.0.lock().map_err(|_| Failure::Unavailable)?;
            let popup = owners
                .get_mut(&owner)
                .filter(|popup| {
                    popup.id == *id
                        && popup.kind == PopupKind::Editor
                        && popup.phase == Phase::Opening
                })
                .ok_or(Failure::Unavailable)?;
            let prepared = coordinator.prepare(
                client::ClientMessage::TerminalStart(request),
                worker,
                lease_expiry(),
            );
            if let PreparedOperation::Dispatch(pending) = &prepared {
                let node::CapabilityNodeMessage::AuthorizedTerminalStart(start) = pending.command()
                else {
                    unreachable!("prepared Start contains its exact authorization")
                };
                popup.authorization = Some(start.authorization());
            }
            Ok(prepared)
        })();
        let prepared = match prepared {
            Ok(PreparedOperation::Dispatch(pending)) => PreparedOperation::Dispatch(pending),
            Ok(PreparedOperation::Ready(_)) | Err(_) => {
                self.queue_editor_cleanup(owner, id, Phase::Dispose)?;
                return Err(Failure::Unavailable);
            }
        };
        let result = self.complete_popup_dispatch(prepared, Some(&handle))?;
        let mut owners = self.popups.0.lock().map_err(|_| Failure::PhysicalUnknown)?;
        let popup = owners.get_mut(&owner).ok_or(Failure::PhysicalUnknown)?;
        match result {
            client::ServerMessage::TerminalStartResult(result)
                if matches!(result.outcome(), client::StartOutcome::Accepted { .. }) =>
            {
                popup.phase = Phase::Active;
                Ok(Outcome::PopupOpened {
                    popup_id: id.clone(),
                    title,
                })
            }
            client::ServerMessage::TerminalStartResult(result)
                if !matches!(result.outcome(), client::StartOutcome::OutcomeUnknown) =>
            {
                drop(owners);
                self.queue_editor_cleanup(owner, id, Phase::Queued)?;
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

    fn queue_editor_cleanup(
        &self,
        owner: NonZeroU64,
        id: &WorkbenchRequestId,
        phase: Phase,
    ) -> Result<(), Failure> {
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| Failure::PhysicalUnknown)?;
        let mut owners = self.popups.0.lock().map_err(|_| Failure::PhysicalUnknown)?;
        let popup = owners
            .get_mut(&owner)
            .filter(|popup| popup.id == *id && popup.kind == PopupKind::Editor)
            .ok_or(Failure::PhysicalUnknown)?;
        if !popup
            .authorization
            .is_some_and(|auth| coordinator.fence_independent_popup(&popup.reference, auth))
        {
            return Err(Failure::PhysicalUnknown);
        }
        popup.phase = phase;
        popup.deadline = Instant::now().checked_add(SETTLEMENT_WINDOW);
        popup.next = Instant::now();
        drop(owners);
        drop(coordinator);
        self.close_maintenance.notify();
        Ok(())
    }
}

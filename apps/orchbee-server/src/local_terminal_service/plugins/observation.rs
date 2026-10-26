//! One serial temporary viewer; an uncertain Detach retains ownership and blocks replacement.
use super::{
    Failure, LocalTerminalService, Outcome, PluginRequest, WorkerOperationRequest,
    WorkerOperationResult, node,
};
use crate::{local_terminal_service::placement, site_coordinator::ProcessObservationTarget};
use orchbee_workbench_protocol::PluginPaneInfo;
use std::sync::atomic::AtomicBool;

impl LocalTerminalService {
    pub(in crate::local_terminal_service) fn observe_plugin_pane(
        &self,
        request: &PluginRequest,
        pane: &PluginPaneInfo,
    ) -> Result<Outcome, Failure> {
        let target = {
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
            let Some(Outcome::PaneFound { pane: current, .. }) =
                coordinator.plugin_pane(&pane.panel_id)
            else {
                return Err(Failure::NotFound);
            };
            if *current != *pane {
                return Err(Failure::Conflict);
            }
            let prepared = coordinator
                .prepare_binding_viewer(
                    &orchbee_client_server_protocol::SiteId::parse(request.site_id.as_str())
                        .map_err(|_| Failure::SiteMismatch)?,
                    &orchbee_client_server_protocol::WorkerId::parse(
                        pane.binding.worker_id.as_str(),
                    )
                    .map_err(|_| Failure::InvalidIntent)?,
                    &orchbee_client_server_protocol::TerminalId::parse(
                        pane.binding.terminal_id.as_str(),
                    )
                    .map_err(|_| Failure::InvalidIntent)?,
                    placement(workers.request_handle().as_ref()),
                )
                .map_err(|_| Failure::Unavailable)?;
            ProcessObservationTarget {
                site_id: request.site_id.clone(),
                binding: pane.binding.clone(),
                target: prepared.target,
            }
        };
        let settled = self.observe_plugin_runtime(&target, None)?;
        let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
        if placement(workers.request_handle().as_ref()) != Some(target.target.worker) {
            return Err(Failure::WorkerChanged);
        }
        let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
        match coordinator.plugin_pane(&pane.panel_id) {
            Some(Outcome::PaneFound {
                revision,
                pane: current,
            }) if *current == *pane => {
                let fresh = coordinator
                    .prepare_binding_viewer(
                        &orchbee_client_server_protocol::SiteId::parse(request.site_id.as_str())
                            .map_err(|_| Failure::SiteMismatch)?,
                        &orchbee_client_server_protocol::WorkerId::parse(
                            pane.binding.worker_id.as_str(),
                        )
                        .map_err(|_| Failure::InvalidIntent)?,
                        &orchbee_client_server_protocol::TerminalId::parse(
                            pane.binding.terminal_id.as_str(),
                        )
                        .map_err(|_| Failure::InvalidIntent)?,
                        placement(workers.request_handle().as_ref()),
                    )
                    .map_err(|_| Failure::Unavailable)?;
                if fresh.target != target.target {
                    return Err(Failure::Conflict);
                }
                Ok(Outcome::PaneObserved {
                    revision,
                    pane: current,
                    settled,
                })
            }
            _ => Err(Failure::Conflict),
        }
    }
    pub(in crate::local_terminal_service) fn observe_plugin_runtime(
        &self,
        target: &ProcessObservationTarget,
        reference: Option<&orchbee_client_server_protocol::RequestId>,
    ) -> Result<bool, Failure> {
        let mut pending = self.observer.claim(self).map_err(|cause| match cause {
            super::super::observation::Failure::Busy => Failure::Busy,
            super::super::observation::Failure::PhysicalUnknown => Failure::PhysicalUnknown,
        })?;
        let mut sampled = None;
        let observe =
            |viewer: &mut crate::local_terminal_service::viewer_connection::ViewerAttachment<
                '_,
            >,
             template: node::TerminalViewerRequest| {
                let Some(id) = self.next_viewer_operation() else {
                    return;
                };
                let dispatch = node::TerminalViewerRequest {
                    request_id: id,
                    action: node::TerminalViewerAction::ReadLifecycle {
                        target: target.target.identity,
                    },
                    ..template
                };
                if let Ok(WorkerOperationResult::Viewer(result)) = viewer
                    .handle
                    .request(WorkerOperationRequest::Viewer(dispatch))
                    && result.request_id == id
                    && result.attachment_id == template.attachment_id
                    && let node::TerminalViewerOutcome::LifecycleObserved {
                        target: actual,
                        settled,
                        ..
                    } = result.outcome
                    && actual == target.target.identity
                    && self.viewer_worker_is_current(&viewer.handle)
                {
                    sampled = Some(settled);
                }
            };
        let stop = AtomicBool::new(false);
        *pending = if let Some(reference) = reference {
            self.with_reference_observation_viewer(target, reference, &stop, observe)
        } else {
            self.with_observation_viewer(target, &stop, observe)
        };
        // No settlement claim can escape while the observer still owns uncertain cleanup.
        if pending.is_some() {
            return Err(Failure::PhysicalUnknown);
        }
        let settled = sampled.ok_or(Failure::Unavailable)?;
        Ok(settled)
    }
}

use orchbee_capability_node_protocol::{
    TerminalId as CapabilityTerminalId, WorkerId as CapabilityWorkerId,
};
use orchbee_client_server_protocol::{
    SiteId as ClientSiteId, TerminalId as ClientTerminalId, WorkerId as ClientWorkerId,
};
use orchbee_server::WorkbenchBindingExposure;
use orchbee_workbench_protocol::{
    PresentationBinding, SiteId as WorkbenchSiteId, TerminalId as WorkbenchTerminalId,
    WorkerId as WorkbenchWorkerId,
};

use super::SiteCoordinator;
use crate::terminal_coordinator::{
    BindingViewerTarget, BindingViewerTargetError, SendOperationIdentities,
    ViewerOperationIdentities, WorkerPlacement,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BindingViewerPreparationError {
    ServerBusy,
    TerminalRuntimeUnavailable,
    TerminalUnavailable,
    WorkbenchUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedBindingViewer {
    pub(crate) target: BindingViewerTarget,
    pub(crate) identities: ViewerOperationIdentities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedSendTarget {
    pub(crate) target: BindingViewerTarget,
    pub(crate) identities: SendOperationIdentities,
}

impl SiteCoordinator {
    /// Recheck Site exposure and the exact physical viewer target without allocating a viewer.
    pub(crate) fn binding_viewer_is_current(
        &self,
        target: BindingViewerTarget,
        current_worker: Option<WorkerPlacement>,
    ) -> bool {
        let Some(workbench) = self.workbench.as_ref() else {
            return false;
        };
        let terminal_id = WorkbenchTerminalId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(target.terminal_id.as_bytes())
        ));
        let worker_id = WorkbenchWorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(target.worker.worker_id.as_bytes())
        ));
        let (Ok(terminal_id), Ok(worker_id)) = (terminal_id, worker_id) else {
            return false;
        };
        let binding = PresentationBinding {
            terminal_id,
            worker_id,
        };
        workbench.terminal_binding_exposure(workbench.site_id(), &binding)
            == WorkbenchBindingExposure::Exposed
            && !self.binding_has_close(&binding)
            && self
                .terminals
                .binding_viewer_target(target.terminal_id, target.worker.worker_id, current_worker)
                .is_ok_and(|current| current == target)
    }

    pub(crate) fn prepare_binding_viewer(
        &mut self,
        site_id: &ClientSiteId,
        worker_id: &ClientWorkerId,
        terminal_id: &ClientTerminalId,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<PreparedBindingViewer, BindingViewerPreparationError> {
        let Some(workbench) = self.workbench.as_ref() else {
            return Err(BindingViewerPreparationError::WorkbenchUnavailable);
        };
        let workbench_site = WorkbenchSiteId::parse(site_id.as_str())
            .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?;
        let workbench_binding = PresentationBinding {
            terminal_id: WorkbenchTerminalId::parse(terminal_id.as_str())
                .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?,
            worker_id: WorkbenchWorkerId::parse(worker_id.as_str())
                .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?,
        };
        match workbench.terminal_binding_exposure(&workbench_site, &workbench_binding) {
            WorkbenchBindingExposure::Unavailable => {
                return Err(BindingViewerPreparationError::WorkbenchUnavailable);
            }
            WorkbenchBindingExposure::NotExposed => {
                return Err(BindingViewerPreparationError::TerminalUnavailable);
            }
            WorkbenchBindingExposure::Exposed => {}
        }
        let terminal_id = CapabilityTerminalId::from_bytes(
            identity_bytes(terminal_id.as_str())
                .ok_or(BindingViewerPreparationError::TerminalUnavailable)?,
        )
        .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?;
        let worker_id = CapabilityWorkerId::from_bytes(
            identity_bytes(worker_id.as_str())
                .ok_or(BindingViewerPreparationError::TerminalUnavailable)?,
        )
        .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?;
        let target = self
            .terminals
            .binding_viewer_target(terminal_id, worker_id, current_worker)
            .map_err(|error| match error {
                BindingViewerTargetError::TerminalUnavailable => {
                    BindingViewerPreparationError::TerminalUnavailable
                }
                BindingViewerTargetError::TerminalRuntimeUnavailable => {
                    BindingViewerPreparationError::TerminalRuntimeUnavailable
                }
            })?;
        if self.binding_has_close(&workbench_binding) {
            return Err(BindingViewerPreparationError::TerminalUnavailable);
        }
        let identities = self
            .terminals
            .allocate_viewer_identities()
            .ok_or(BindingViewerPreparationError::ServerBusy)?;
        Ok(PreparedBindingViewer { target, identities })
    }

    pub(crate) fn prepare_send_target(
        &mut self,
        site_id: &ClientSiteId,
        worker_id: &ClientWorkerId,
        terminal_id: &ClientTerminalId,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<PreparedSendTarget, BindingViewerPreparationError> {
        let Some(workbench) = self.workbench.as_ref() else {
            return Err(BindingViewerPreparationError::WorkbenchUnavailable);
        };
        let workbench_site = WorkbenchSiteId::parse(site_id.as_str())
            .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?;
        let workbench_binding = PresentationBinding {
            terminal_id: WorkbenchTerminalId::parse(terminal_id.as_str())
                .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?,
            worker_id: WorkbenchWorkerId::parse(worker_id.as_str())
                .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?,
        };
        match workbench.terminal_binding_exposure(&workbench_site, &workbench_binding) {
            WorkbenchBindingExposure::Unavailable => {
                return Err(BindingViewerPreparationError::WorkbenchUnavailable);
            }
            WorkbenchBindingExposure::NotExposed => {
                return Err(BindingViewerPreparationError::TerminalUnavailable);
            }
            WorkbenchBindingExposure::Exposed => {}
        }
        let terminal_id = CapabilityTerminalId::from_bytes(
            identity_bytes(terminal_id.as_str())
                .ok_or(BindingViewerPreparationError::TerminalUnavailable)?,
        )
        .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?;
        let worker_id = CapabilityWorkerId::from_bytes(
            identity_bytes(worker_id.as_str())
                .ok_or(BindingViewerPreparationError::TerminalUnavailable)?,
        )
        .map_err(|_| BindingViewerPreparationError::TerminalUnavailable)?;
        let target = self
            .terminals
            .binding_send_target(terminal_id, worker_id, current_worker)
            .map_err(|error| match error {
                BindingViewerTargetError::TerminalUnavailable => {
                    BindingViewerPreparationError::TerminalUnavailable
                }
                BindingViewerTargetError::TerminalRuntimeUnavailable => {
                    BindingViewerPreparationError::TerminalRuntimeUnavailable
                }
            })?;
        if self.binding_has_close(&workbench_binding) {
            return Err(BindingViewerPreparationError::TerminalUnavailable);
        }
        let identities = self
            .terminals
            .allocate_send_identities()
            .ok_or(BindingViewerPreparationError::ServerBusy)?;
        Ok(PreparedSendTarget { target, identities })
    }
}

fn identity_bytes(value: &str) -> Option<[u8; 16]> {
    u128::from_str_radix(value, 16).ok().map(u128::to_be_bytes)
}

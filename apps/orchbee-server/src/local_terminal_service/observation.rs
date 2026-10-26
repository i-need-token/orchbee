//! Shared serial read-only observation; uncertain cleanup retains the Session's existing owner.
use super::{
    LocalTerminalService,
    viewer_connection::{PendingCleanup, process_sampling::screen},
};
use crate::session_coordinator::ProcessObservationTarget;
use std::sync::{Mutex, MutexGuard, atomic::AtomicBool};

#[derive(Default)]
pub(super) struct Observer(Mutex<Option<PendingCleanup>>);
pub(super) enum Failure {
    Busy,
    PhysicalUnknown,
}
impl Observer {
    pub(super) fn claim(
        &self,
        service: &LocalTerminalService,
    ) -> Result<MutexGuard<'_, Option<PendingCleanup>>, Failure> {
        let mut pending = self.0.try_lock().map_err(|_| Failure::Busy)?;
        if let Some(cleanup) = pending.as_ref() {
            if !cleanup.settle(service) {
                return Err(Failure::PhysicalUnknown);
            }
            *pending = None;
        }
        Ok(pending)
    }
}
impl LocalTerminalService {
    pub(in crate::local_terminal_service) fn configured_command_capture(
        &self,
        target: &ProcessObservationTarget,
        selection: Option<&orchbee_workbench_protocol::ConfiguredCommandSelection>,
    ) -> Result<
        super::configured_commands::Captured,
        orchbee_workbench_protocol::ConfiguredCommandFailure,
    > {
        use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
        let mut pending = self.observer.claim(self).map_err(|cause| match cause {
            Failure::Busy => F::Busy,
            Failure::PhysicalUnknown => F::PhysicalUnknown,
        })?;
        let stop = AtomicBool::new(false);
        let mut capture = Err(F::Unavailable);
        *pending = self.with_observation_viewer(target, &stop, |viewer, template| {
            capture = screen::configured_capture(viewer, template, &stop, selection);
        });
        if pending.is_some() {
            return Err(F::PhysicalUnknown);
        }
        capture
    }
    /// Only exact known physical Dispose discharges uncertain temporary-viewer ownership.
    pub(in crate::local_terminal_service) fn settle_observation_disposed(
        &self,
        target: crate::terminal_coordinator::BindingViewerTarget,
    ) {
        if let Ok(mut pending) = self.observer.0.lock()
            && pending
                .as_ref()
                .is_some_and(|cleanup| cleanup.matches_target(target))
        {
            *pending = None;
        }
    }
}

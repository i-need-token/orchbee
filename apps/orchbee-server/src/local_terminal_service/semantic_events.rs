//! Read-only event sources reuse canonical admission and the retained temporary-viewer owner.
use super::{LocalTerminalService, placement, viewer_connection::process_sampling::screen};
use crate::session_coordinator::ProcessObservationTarget;
use orchbee_workbench_protocol::{
    ClientViewportObservation, SemanticEventSource, SemanticEventsFailure as Failure,
    SemanticOutputSource, SemanticPane, SessionId, WorkbenchRevision,
};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct OutputRead {
    pub source: SemanticOutputSource,
    pub lines: u16,
    pub strip_ansi: bool,
    pub viewport: Option<ClientViewportObservation>,
}
pub(crate) struct OutputSample {
    pub pane: SemanticPane,
    pub workbench_revision: WorkbenchRevision,
    pub content_generation: WorkbenchRevision,
    pub content_revision: WorkbenchRevision,
    pub lines: Vec<String>,
}

impl LocalTerminalService {
    pub(in crate::local_terminal_service) fn observe_terminal_projection(
        &self,
        handle: &crate::worker_obcn_carrier::WorkerRequestHandle,
        target: crate::terminal_coordinator::BindingViewerTarget,
        frame: Option<(u64, u64)>,
        lifecycle: Option<(u64, bool)>,
        started: std::time::Instant,
    ) {
        let Ok(workers) = self.workers.lock() else {
            return;
        };
        if !handle.is_active()
            || placement(workers.request_handle().as_ref()) != Some(target.worker)
            || placement(Some(handle)) != Some(target.worker)
        {
            return;
        }
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return;
        };
        coordinator.observe_terminal_projection(
            target,
            frame,
            lifecycle,
            started,
            std::time::Instant::now(),
        );
    }
    pub(crate) fn capture_event_output(
        &self,
        target: &ProcessObservationTarget,
        pane: SemanticPane,
        revision: WorkbenchRevision,
        read: &OutputRead,
        stop: &AtomicBool,
    ) -> Result<OutputSample, Failure> {
        let source = SemanticEventSource {
            panel_id: pane.panel_id.clone(),
            presentation_id: Some(pane.presentation_id.clone()),
        };
        let before = self.event_source(&target.session_id, &source)?;
        if before.2 != *target || before.1 < revision {
            return Err(Failure::SourceChanged);
        }
        let mut pending = self.observer.claim(self).map_err(|cause| match cause {
            super::observation::Failure::Busy => Failure::Busy,
            super::observation::Failure::PhysicalUnknown => Failure::PhysicalUnknown,
        })?;
        let mut captured = None;
        *pending = self.with_observation_viewer(target, stop, |viewer, template| {
            captured = Some(screen::event_capture(viewer, template, stop, read));
        });
        if pending.is_some() {
            return Err(Failure::PhysicalUnknown);
        }
        if stop.load(Ordering::Acquire) {
            return Err(Failure::Unavailable);
        }
        let (content_generation, content_revision, lines) =
            captured.ok_or(Failure::SourceUnavailable)??;
        let after = self.event_source(&target.session_id, &source)?;
        if after.2 != *target || after.0.presentation_id != pane.presentation_id {
            return Err(Failure::SourceChanged);
        }
        Ok(OutputSample {
            pane: after.0,
            workbench_revision: after.1,
            content_generation,
            content_revision,
            lines,
        })
    }
    pub(crate) fn event_journal(&self) -> Option<orchbee_server::SessionEventJournal> {
        self.coordinator.lock().ok()?.event_journal()
    }
    pub(crate) fn maintain_events(&self) -> Result<(), Failure> {
        self.coordinator
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .maintain_events()
    }
    pub(crate) fn event_source(
        &self,
        session: &SessionId,
        source: &SemanticEventSource,
    ) -> Result<(SemanticPane, WorkbenchRevision, ProcessObservationTarget), Failure> {
        let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
        let handle = workers.request_handle();
        self.coordinator
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .event_source(
                session,
                source,
                placement(handle.as_ref().filter(|h| h.is_active())),
            )
    }
}

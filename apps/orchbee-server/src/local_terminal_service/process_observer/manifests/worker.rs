//! The observer polls completion; the worker retains every download and cache owner until settled.
use super::*;
use std::{
    sync::{Arc, atomic::Ordering},
    thread::{self, JoinHandle},
};
#[derive(Default)]
pub(in super::super) struct Worker(Option<Running>);
struct Running {
    work: Work,
    wake: Arc<Wake>,
    handle: Option<JoinHandle<UpdateReport>>,
}
impl Worker {
    pub(in super::super) fn active(&self) -> bool {
        self.0.is_some()
    }
    pub(in super::super) fn serve(
        &mut self,
        service: Option<&LocalTerminalService>,
        wake: &Arc<Wake>,
        detector: &mut ScreenDetector,
    ) {
        if self
            .0
            .as_ref()
            .is_some_and(|running| running.handle.as_ref().is_some_and(|h| h.is_finished()))
            && let Some(mut running) = self.0.take()
        {
            let report = running.handle.take().and_then(|h| h.join().ok()).ok_or(());
            let loaded = report
                .as_ref()
                .ok()
                .filter(|r| {
                    r.failure != Some(UpdateFailure::Cancelled)
                        && (r.failure.is_none()
                            || r.agents
                                .iter()
                                .any(|row| row.result != updating::Disposition::Failed))
                })
                .filter(|_| !wake.stop.load(Ordering::Acquire))
                .map(|_| ScreenDetector::load(running.work.root.as_deref()));
            wake.manifests
                .complete_update(running.work.id.clone(), report, loaded, |next| {
                    install(service, detector, next)
                });
        }
        if self.0.is_some() || wake.stop.load(Ordering::Acquire) {
            return;
        }
        let Some(work) = wake.manifests.take() else {
            return;
        };
        if work.kind == Kind::Reload {
            let next = ScreenDetector::load(work.root.as_deref());
            let maintenance = updating::maintenance(work.root.as_deref(), &work.source);
            wake.manifests
                .complete_reload(work.id, next, Some(maintenance), |next| {
                    install(service, detector, next)
                });
            return;
        }
        let (Some(root), Ok(source)) = (work.root.clone(), work.source.clone()) else {
            wake.manifests
                .refuse_work(work.id, Failure::UpdateUnavailable);
            return;
        };
        let owner = Arc::clone(wake);
        match thread::Builder::new()
            .name("orchbee-agent-manifest-update".into())
            .spawn(move || {
                let report = updating::run(&root, &source, &owner.stop);
                owner.changed.notify_all();
                report
            }) {
            Ok(handle) => {
                self.0 = Some(Running {
                    work,
                    wake: Arc::clone(wake),
                    handle: Some(handle),
                })
            }
            Err(_) => wake.manifests.refuse_work(work.id, Failure::Unavailable),
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        // Exceptional observer unwind still owns the worker; never detach a cache writer.
        if let Some(handle) = self.handle.take() {
            self.wake.stop.store(true, Ordering::Release);
            self.wake.manifests.cancel();
            let report = handle.join().map_err(|_| ());
            self.wake
                .manifests
                .complete_update(self.work.id.clone(), report, None, |_| {
                    Err(Failure::Cancelled)
                });
        }
    }
}

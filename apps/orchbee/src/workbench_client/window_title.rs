//! Owns one independently cancellable listener connection and its latest handoff.
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{ProtocolVersion, SiteId, WorkbenchRequestId};
use orchbee_workbench_protocol::{
    WindowTitleAction, WindowTitleContext, WindowTitleOutcome, WindowTitleRequest,
    WindowTitleSnapshot,
};
use std::time::Duration;
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};
#[derive(Default)]
struct Handoff {
    stopped: bool,
    focused: bool,
    context: WindowTitleContext,
    snapshot: Option<WindowTitleSnapshot>,
    last: Option<WindowTitleSnapshot>,
}
impl Handoff {
    fn receive(&mut self, snapshot: Option<WindowTitleSnapshot>) {
        if snapshot.is_none() || self.last != snapshot {
            self.snapshot = snapshot.clone();
            self.last = snapshot;
        }
    }
}
type Shared = Arc<(Mutex<Handoff>, Condvar)>;
pub(super) struct Subscription {
    shared: Shared,
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl Subscription {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        site: SiteId,
    ) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (reader, wake) = LocalStream::pair()?;
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-window-title".into())
            .spawn(move || run(state, origin, clock, site, reader))?;
        Ok(Self {
            shared,
            wake,
            task: Some(task),
        })
    }
    pub(super) fn poll(
        &self,
        focused: bool,
        context: WindowTitleContext,
    ) -> Option<WindowTitleSnapshot> {
        let Ok(mut state) = self.shared.0.lock() else {
            return None;
        };
        if focused && !state.focused {
            state.last = None;
        }
        state.focused = focused;
        state.context = context;
        state.snapshot.take()
    }
    pub(super) fn close(&mut self) {
        if self.task.is_none() {
            return;
        }
        if let Ok(mut state) = self.shared.0.lock() {
            state.stopped = true;
        }
        self.shared.1.notify_all();
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.close();
    }
}
fn run(
    shared: Shared,
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    site: SiteId,
    wake: LocalStream,
) {
    let mut client = None;
    let mut serial = 0_u64;
    loop {
        let (focused, context) = {
            let Ok(state) = shared.0.lock() else {
                return;
            };
            if state.stopped {
                return;
            }
            (state.focused, state.context.clone())
        };
        if client.is_none() {
            client = origin
                .reconnect_before_cancellable(
                    clock.now() + Duration::from_secs(2),
                    clock.clone(),
                    Some(wake.as_local_io()),
                )
                .ok();
        }
        let Some(next) = serial.checked_add(1) else {
            return;
        };
        serial = next;
        let result = client.as_mut().map(|client| {
            client.window_title(
                WindowTitleRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{serial:032x}"))
                        .expect("nonzero serial"),
                    site_id: site.clone(),
                    action: WindowTitleAction::Listen { focused, context },
                },
                clock.now() + Duration::from_secs(2),
                Some(wake.as_local_io()),
            )
        });
        let snapshot = match result {
            Some(Ok(reply)) => match reply.outcome {
                WindowTitleOutcome::Listening { snapshot } => snapshot,
                WindowTitleOutcome::Published { .. } | WindowTitleOutcome::Failed { .. } => None,
            },
            Some(Err(_)) => {
                client = None;
                None
            }
            None => None,
        };
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        state.receive(snapshot);
        if let Ok((next, _)) = shared.1.wait_timeout(state, Duration::from_millis(100))
            && next.stopped
        {
            return;
        }
    }
}
#[cfg(test)]
mod tests;

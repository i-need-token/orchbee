//! Owns a cancellable title subscription on the shared Client connection.
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{ProtocolVersion, SessionId, WorkbenchRequestId};
use orchbee_workbench_protocol::{
    WindowTitleAction, WindowTitleContext, WindowTitleOutcome, WindowTitleRequest,
    WindowTitleSnapshot,
};
#[cfg(test)]
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
        session: SessionId,
    ) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (reader, wake) = LocalStream::pair()?;
        reader.set_nonblocking(true)?;
        wake.set_nonblocking(true)?;
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-window-title".into())
            .spawn(move || run(state, origin, clock, session, reader))?;
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
        let changed = state.focused != focused || state.context != context;
        state.focused = focused;
        state.context = context;
        let snapshot = state.snapshot.take();
        drop(state);
        if changed {
            let _notified = crate::local_client::subscription::notify(&self.wake);
        }
        snapshot
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
    session: SessionId,
    wake: LocalStream,
) {
    let _clock = clock;
    let mut active = None;
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
        if active
            .as_ref()
            .is_none_or(|(previous_focus, previous_context, _)| {
                *previous_focus != focused || previous_context != &context
            })
        {
            let Some(next) = serial.checked_add(1) else {
                return;
            };
            serial = next;
            let request =
                orchbee_workbench_protocol::WorkbenchRequest::WindowTitle(WindowTitleRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{serial:032x}"))
                        .expect("nonzero serial"),
                    session_id: session.clone(),
                    action: WindowTitleAction::Listen {
                        focused,
                        context: context.clone(),
                    },
                });
            let Ok(subscription) =
                origin.subscribe(&request, orchbee_workbench_protocol::StreamMode::State)
            else {
                return;
            };
            active = Some((focused, context, subscription));
        }
        let Some((_, _, subscription)) = &active else {
            return;
        };
        let reply = match crate::local_client::subscription::next_workbench(subscription, &wake) {
            Ok(Some(orchbee_workbench_protocol::WorkbenchResponse::WindowTitle(reply)))
                if reply.request_id.as_str() == format!("{serial:032x}")
                    && reply.session_id == session =>
            {
                reply
            }
            Ok(None) => continue,
            _ => return,
        };
        let snapshot = match reply.outcome {
            WindowTitleOutcome::Listening { snapshot } => snapshot,
            _ => None,
        };
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        state.receive(snapshot);
    }
}

#[cfg(test)]
mod tests;

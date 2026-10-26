//! Owns an event subscription on the shared Client connection and its UI handoff.
#[cfg(test)]
use super::Duration;
use super::Notification;
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{
    NotificationAction, NotificationOutcome, NotificationRequest, ProtocolVersion, SessionId,
    WorkbenchRequestId,
};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};
#[derive(Default)]
struct Handoff {
    stopped: bool,
    accepting: bool,
    message: Option<Notification>,
}
type Shared = Arc<(Mutex<Handoff>, Condvar)>;
pub(crate) struct Subscription {
    shared: Shared,
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl Subscription {
    pub(crate) fn spawn(
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
            .name("orchbee-notifications".into())
            .spawn(move || run(state, origin, clock, session, reader))?;
        Ok(Self {
            shared,
            wake,
            task: Some(task),
        })
    }
    pub(crate) fn poll(&self, accepting: bool) -> Option<Notification> {
        let Ok(mut state) = self.shared.0.lock() else {
            return None;
        };
        let changed = state.accepting != accepting || state.message.is_some();
        state.accepting = accepting;
        // A surface that appeared since the last wire poll wins; never replace it.
        let message = state.message.take();
        drop(state);
        if changed {
            let _notified = crate::local_client::subscription::notify(&self.wake);
        }
        if accepting { message } else { None }
    }
    pub(crate) fn close(&mut self) {
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
        let accepting = {
            let Ok(state) = shared.0.lock() else {
                return;
            };
            if state.stopped {
                return;
            }
            state.accepting && state.message.is_none()
        };
        if active
            .as_ref()
            .is_none_or(|(previous, _)| *previous != accepting)
        {
            let Some(next) = serial.checked_add(1) else {
                return;
            };
            serial = next;
            let request =
                orchbee_workbench_protocol::WorkbenchRequest::Notification(NotificationRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{serial:032x}"))
                        .expect("nonzero serial"),
                    session_id: session.clone(),
                    action: NotificationAction::Listen { accepting },
                });
            let Ok(subscription) =
                origin.subscribe(&request, orchbee_workbench_protocol::StreamMode::Events)
            else {
                return;
            };
            active = Some((accepting, subscription));
        }
        let Some((_, subscription)) = &active else {
            return;
        };
        let reply = match crate::local_client::subscription::next_workbench(subscription, &wake) {
            Ok(Some(orchbee_workbench_protocol::WorkbenchResponse::Notification(reply)))
                if reply.request_id.as_str() == format!("{serial:032x}")
                    && reply.session_id == session =>
            {
                reply
            }
            Ok(None) => continue,
            _ => return,
        };
        let message = match reply.outcome {
            NotificationOutcome::Listening { notification } => notification,
            _ => None,
        };
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        if state.message.is_none() {
            state.message = message;
        }
    }
}

#[cfg(test)]
mod tests;

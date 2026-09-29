//! Owns one independently cancellable listener connection and its latest handoff.
use super::*;
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{
    NotificationAction, NotificationOutcome, NotificationRequest, ProtocolVersion, SiteId,
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
        site: SiteId,
    ) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (reader, wake) = LocalStream::pair()?;
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-notifications".into())
            .spawn(move || run(state, origin, clock, site, reader))?;
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
        state.accepting = accepting;
        // A surface that appeared since the last wire poll wins; never replace it.
        let message = state.message.take();
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
    site: SiteId,
    wake: LocalStream,
) {
    let mut client = None;
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
            client.notification(
                NotificationRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{serial:032x}"))
                        .expect("nonzero serial"),
                    site_id: site.clone(),
                    action: NotificationAction::Listen { accepting },
                },
                clock.now() + Duration::from_secs(2),
                Some(wake.as_local_io()),
            )
        });
        let message = match result {
            Some(Ok(reply)) => match reply.outcome {
                NotificationOutcome::Listening { notification } => notification,
                _ => None,
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
        if state.message.is_none() {
            state.message = message;
        }
        if let Ok((next, _)) = shared.1.wait_timeout(state, Duration::from_millis(100))
            && next.stopped
        {
            return;
        }
    }
}
#[cfg(test)]
mod tests;

//! One bounded Settings exchange; reconnect prepares a new explicit job, never a replay.
use crate::{
    local_client::{LocalClientOrigin, LocalOperationClock},
    native_settings::Token,
};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{NativeSettingsRequest, NativeSettingsResult};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
pub(super) struct Job {
    pub(super) token: Token,
    pub(super) request: NativeSettingsRequest,
}
pub(super) struct Reply {
    pub(super) token: Token,
    pub(super) result: Result<NativeSettingsResult, bool>,
}
#[derive(Default)]
struct Handoff {
    stopped: bool,
    busy: bool,
    job: Option<Job>,
    reply: Option<Reply>,
}
type Shared = Arc<(Mutex<Handoff>, Condvar)>;
pub(super) struct Lane {
    shared: Shared,
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl Lane {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
    ) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (reader, wake) = LocalStream::pair()?;
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-client-settings".into())
            .spawn(move || run(state, origin, clock, reader))?;
        Ok(Self {
            shared,
            wake,
            task: Some(task),
        })
    }
    pub(super) fn submit(&self, job: Job) -> bool {
        let Ok(mut state) = self.shared.0.lock() else {
            return false;
        };
        if state.stopped || state.busy {
            return false;
        }
        state.busy = true;
        state.job = Some(job);
        self.shared.1.notify_all();
        true
    }
    pub(super) fn take(&self) -> Option<Reply> {
        let mut state = self.shared.0.lock().ok()?;
        let reply = state.reply.take()?;
        state.busy = false;
        Some(reply)
    }
    pub(super) fn close(&mut self) {
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
impl Drop for Lane {
    fn drop(&mut self) {
        self.close();
    }
}
fn run(shared: Shared, origin: LocalClientOrigin, clock: LocalOperationClock, wake: LocalStream) {
    let mut client = None;
    loop {
        let job = {
            let Ok(mut state) = shared.0.lock() else {
                return;
            };
            while state.job.is_none() && !state.stopped {
                let Ok(next) = shared.1.wait(state) else {
                    return;
                };
                state = next;
            }
            if state.stopped {
                return;
            }
            let Some(job) = state.job.take() else {
                return;
            };
            job
        };
        let deadline = clock.now() + Duration::from_secs(2);
        if client.is_none() {
            client = origin
                .reconnect_before_cancellable(deadline, clock.clone(), Some(wake.as_local_io()))
                .ok();
        }
        let result = match client.as_mut() {
            Some(client) => client
                .settings(job.request, deadline, Some(wake.as_local_io()))
                .map_err(|error| error.submitted),
            None => Err(false),
        };
        if result.is_err() {
            client = None;
        }
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        state.reply = Some(Reply {
            token: job.token,
            result,
        });
    }
}

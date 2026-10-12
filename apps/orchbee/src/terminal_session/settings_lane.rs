//! One ordered settings lane: passive reads and explicit writes share a transport sequence.
use crate::{
    local_client::{LocalClientOrigin, LocalOperationClock},
    native_settings::Token,
};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{
    NativeSettingsOperation, NativeSettingsOutcome, NativeSettingsRequest, NativeSettingsResult,
    ProtocolVersion, SiteId, UserPreferencesSnapshot, WorkbenchRequestId,
};
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
    observation: Option<UserPreferencesSnapshot>,
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
        site: SiteId,
    ) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (reader, wake) = LocalStream::pair()?;
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-client-settings".into())
            .spawn(move || run(state, origin, clock, site, reader))?;
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
        self.shared.1.notify_all();
        Some(reply)
    }
    pub(super) fn take_observation(&self) -> Option<UserPreferencesSnapshot> {
        self.shared.0.lock().ok()?.observation.take()
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
fn run(
    shared: Shared,
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    site: SiteId,
    wake: LocalStream,
) {
    let mut client = None;
    let mut serial = 0_u64;
    let mut next_poll = clock.now();
    loop {
        let job = {
            let Ok(mut state) = shared.0.lock() else {
                return;
            };
            loop {
                if state.stopped {
                    return;
                }
                if let Some(job) = state.job.take() {
                    break Some(job);
                }
                let now = clock.now();
                if !state.busy && now >= next_poll {
                    break None;
                }
                let wait = if state.busy {
                    Duration::from_secs(1)
                } else {
                    next_poll
                        .saturating_duration_since(now)
                        .min(Duration::from_secs(1))
                };
                let Ok((next, _)) = shared.1.wait_timeout(state, wait) else {
                    return;
                };
                state = next;
            }
        };
        let (token, request) = match job {
            Some(job) => (Some(job.token), job.request),
            None => {
                let Some(next) = serial.checked_add(1) else {
                    return;
                };
                serial = next;
                (
                    None,
                    NativeSettingsRequest {
                        protocol_version: ProtocolVersion,
                        request_id: WorkbenchRequestId::parse(&format!("ffff{serial:028x}"))
                            .expect("bounded read serial"),
                        site_id: site.clone(),
                        operation: NativeSettingsOperation::Open {},
                    },
                )
            }
        };
        let deadline = clock.now() + Duration::from_secs(2);
        if client.is_none() {
            client = origin
                .reconnect_before_cancellable(deadline, clock.clone(), Some(wake.as_local_io()))
                .ok();
        }
        let result = match client.as_mut() {
            Some(client) => client
                .settings(request, deadline, Some(wake.as_local_io()))
                .map_err(|error| error.submitted),
            None => Err(false),
        };
        if result.is_err() {
            client = None;
        }
        next_poll = clock.now() + Duration::from_secs(1);
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        if let Ok(result) = &result {
            match &result.outcome {
                NativeSettingsOutcome::Opened { snapshot }
                | NativeSettingsOutcome::Saved { snapshot, .. } => {
                    state.observation = Some(snapshot.clone())
                }
                _ => {}
            }
        }
        if let Some(token) = token {
            state.reply = Some(Reply { token, result });
        }
    }
}

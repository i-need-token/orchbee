//! One ordered settings lane: passive reads and explicit writes share a transport sequence.
use crate::{
    local_client::{LocalClientOrigin, LocalOperationClock},
    native_settings::Token,
};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, SettingsOperation, SettingsOutcome, SettingsRequest,
    SettingsResult, UserPreferencesSnapshot, WorkbenchRequestId,
};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
pub(super) struct Job {
    pub(super) token: Token,
    pub(super) request: SettingsRequest,
}
pub(super) struct Reply {
    pub(super) token: Token,
    pub(super) result: Result<SettingsResult, bool>,
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
    changed: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl Lane {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session: SessionId,
    ) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (reader, wake) = LocalStream::pair()?;
        let (changes, changed) = LocalStream::pair()?;
        changes.set_nonblocking(true)?;
        changed.set_nonblocking(true)?;
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-client-settings".into())
            .spawn(move || run(state, origin, clock, session, reader, changes))?;
        Ok(Self {
            shared,
            wake,
            changed,
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
        drop(state);
        crate::local_client::subscription::notify(&self.changed).is_ok()
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
        let _ = self.changed.shutdown(std::net::Shutdown::Both);
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
    session: SessionId,
    wake: LocalStream,
    changes: LocalStream,
) {
    let request_id = WorkbenchRequestId::parse("ffff0000000000000000000000000001")
        .expect("nonzero read identity");
    let Ok(subscription) = origin.subscribe(
        &orchbee_workbench_protocol::WorkbenchRequest::Settings(SettingsRequest {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            session_id: session.clone(),
            operation: SettingsOperation::Open {},
        }),
        orchbee_workbench_protocol::StreamMode::State,
    ) else {
        return;
    };
    loop {
        let job = {
            let Ok(mut state) = shared.0.lock() else {
                return;
            };
            if state.stopped {
                return;
            }
            state.job.take()
        };
        let (token, result) = if let Some(job) = job {
            let deadline = clock.now() + Duration::from_secs(2);
            let result = match origin.lane_before_cancellable(
                deadline,
                clock.clone(),
                Some(wake.as_local_io()),
            ) {
                Ok(mut client) => client
                    .settings(job.request, deadline, Some(wake.as_local_io()))
                    .map_err(|error| error.submitted),
                Err(_) => Err(false),
            };
            (Some(job.token), result)
        } else {
            match crate::local_client::subscription::next_workbench(&subscription, &changes) {
                Ok(Some(orchbee_workbench_protocol::WorkbenchResponse::Settings(reply)))
                    if reply.request_id == request_id && reply.session_id == session =>
                {
                    (None, Ok(reply))
                }
                Ok(None) => continue,
                _ => return,
            }
        };
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        if let Ok(result) = &result {
            match &result.outcome {
                SettingsOutcome::Opened { snapshot } | SettingsOutcome::Saved { snapshot, .. } => {
                    state.observation = Some(snapshot.clone())
                }
                SettingsOutcome::OutcomeUnknown { .. } | SettingsOutcome::Refused { .. } => {}
            }
        }
        if let Some(token) = token {
            state.reply = Some(Reply { token, result });
        }
    }
}

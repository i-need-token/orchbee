//! Settings observes pushed snapshots and submits each explicit write exactly once.
use crate::{
    local_client::{
        LocalClientOrigin, LocalOperationClock,
        multiplex::{StreamEvent, Subscription},
        subscription::Request,
    },
    native_settings::Token,
};
use orchbee_workbench_protocol::{
    SettingsOperation, SettingsOutcome, SettingsRequest, SettingsResult,
    ProtocolVersion, SessionId, StreamMode, UserPreferencesSnapshot, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, read_workbench_response,
};
use std::{io, sync::Mutex, time::Duration};

pub(super) struct Job {
    pub(super) token: Token,
    pub(super) request: SettingsRequest,
}
pub(super) struct Reply {
    pub(super) token: Token,
    pub(super) result: Result<SettingsResult, bool>,
}
struct Pending {
    token: Token,
    id: WorkbenchRequestId,
    request: Request,
}
struct State {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    session: SessionId,
    subscription: Subscription,
    pending: Option<Pending>,
    reply: Option<Reply>,
    observation: Option<UserPreferencesSnapshot>,
}
pub(super) struct Lane {
    state: Mutex<Option<State>>,
}
impl Lane {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session: SessionId,
    ) -> io::Result<Self> {
        let subscription = observe(&origin, &session)?;
        Ok(Self {
            state: Mutex::new(Some(State {
                origin,
                clock,
                session,
                subscription,
                pending: None,
                reply: None,
                observation: None,
            })),
        })
    }
    pub(super) fn submit(&self, job: Job) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let Some(state) = state.as_mut() else {
            return false;
        };
        state.poll();
        if state.pending.is_some() || state.reply.is_some() {
            return false;
        }
        let id = job.request.request_id.clone();
        match state.origin.request(
            &WorkbenchRequest::Settings(job.request),
            state.clock.now() + Duration::from_secs(2),
            state.clock.clone(),
        ) {
            Ok(request) => {
                state.pending = Some(Pending {
                    token: job.token,
                    id,
                    request,
                })
            }
            Err(_) => {
                state.reply = Some(Reply {
                    token: job.token,
                    result: Err(false),
                })
            }
        }
        true
    }
    pub(super) fn take(&self) -> Option<Reply> {
        let mut state = self.state.lock().ok()?;
        let state = state.as_mut()?;
        state.poll();
        state.reply.take()
    }
    pub(super) fn take_observation(&self) -> Option<UserPreferencesSnapshot> {
        let mut state = self.state.lock().ok()?;
        let state = state.as_mut()?;
        state.poll();
        state.observation.take()
    }
    pub(super) fn close(&mut self) {
        self.state
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        self.close();
    }
}
fn observe(origin: &LocalClientOrigin, session: &SessionId) -> io::Result<Subscription> {
    origin.subscribe(
        &WorkbenchRequest::Settings(SettingsRequest {
            protocol_version: ProtocolVersion,
            request_id: observation_id(),
            session_id: session.clone(),
            operation: SettingsOperation::Open {},
        }),
        StreamMode::State,
    )
}
fn observation_id() -> WorkbenchRequestId {
    WorkbenchRequestId::parse("ffff0000000000000000000000000001").expect("nonzero read identity")
}
impl State {
    fn poll(&mut self) {
        loop {
            match self.subscription.poll() {
                Ok(Some(StreamEvent::Record(bytes))) => {
                    if let Ok(WorkbenchResponse::Settings(reply)) =
                        read_workbench_response(&mut io::Cursor::new(bytes))
                        && reply.request_id == observation_id()
                        && reply.session_id == self.session
                    {
                        self.observe(&reply);
                    }
                }
                Ok(Some(StreamEvent::Gap | StreamEvent::Disconnected)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        let Some(pending) = &self.pending else {
            return;
        };
        let Some(result) = pending.request.poll() else {
            return;
        };
        let result = match result {
            Ok(WorkbenchResponse::Settings(reply))
                if reply.request_id == pending.id && reply.session_id == self.session =>
            {
                Ok(reply)
            }
            Ok(_) => Err(true),
            Err(submitted) => Err(submitted),
        };
        let pending = self.pending.take().expect("pending request");
        if let Ok(reply) = &result {
            self.observe(reply);
        }
        self.reply = Some(Reply {
            token: pending.token,
            result,
        });
        // Preference source tokens are opaque, not ordered revisions. A fresh capture after the
        // write prevents an older queued observation from replacing its acknowledged snapshot.
        self.subscription.cancel();
        if let Ok(subscription) = observe(&self.origin, &self.session) {
            self.subscription = subscription;
        }
    }
    fn observe(&mut self, reply: &SettingsResult) {
        if let SettingsOutcome::Opened { snapshot }
        | SettingsOutcome::Saved { snapshot, .. } = &reply.outcome
        {
            self.observation = Some(snapshot.clone());
        }
    }
}

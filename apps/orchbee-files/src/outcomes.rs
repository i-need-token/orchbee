//! The request identities of the mutations served beside the Git engine, kept per Workspace as
//! [`Requests`] keeps them. A mutation runs under its identity once: a repeat while it runs waits
//! for its reply, and a repeat after it answers the reply it recorded. File edits and Local History
//! each keep one [`Outcomes`], and Git mutations keep their identities in the Git engine, so
//! `request.outcome` asks each service in turn and the engine for an identity none of them knows;
//! a Client names each mutation with a fresh identity, so no two of them hold the same one.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
    time::Instant,
};

use orchbee_files_protocol::{
    ErrorCode, FilesError, FilesReply, MAXIMUM_REPLY_BYTES, MutationReply, MutationState,
    RequestId, RequestOutcomeResult, WorkspaceId, encode_reply,
};

use crate::{
    calls::Call,
    requests::{Begin, Outcome, Requests},
};

/// The identities of one service's mutations in every Workspace. The lock is never held while a
/// mutation runs or a call is answered; a panic ends the process, so a poisoned lock is never
/// observed.
pub(crate) struct Outcomes {
    spaces: Mutex<HashMap<WorkspaceId, Space>>,
}

struct Space {
    requests: Requests<MutationReply>,
    /// The calls that repeated a running identity.
    waiting: HashMap<RequestId, Vec<Call>>,
}

impl Outcomes {
    pub(crate) fn new() -> Self {
        Self {
            spaces: Mutex::new(HashMap::new()),
        }
    }

    /// Runs `mutation` under identity `request` of `workspace` and answers `call` with the reply
    /// it returns, unless the identity ran before: a completed run's reply answers `call` at once,
    /// and a run still going answers it when it ends. The reply is recorded whether or not its
    /// Client still waits.
    pub(crate) fn run(
        &self,
        workspace: WorkspaceId,
        request: RequestId,
        call: Call,
        mutation: impl FnOnce(&Call) -> MutationReply,
    ) {
        let begun = {
            let mut spaces = self.lock();
            let space = spaces.entry(workspace).or_insert_with(|| Space {
                requests: Requests::new(),
                waiting: HashMap::new(),
            });
            match space.requests.begin(Instant::now(), request) {
                Begin::Completed(reply) => Some(reply),
                Begin::Running => {
                    space.waiting.entry(request).or_default().push(call);
                    return;
                }
                Begin::Run => None,
            }
        };
        if let Some(reply) = begun {
            return answer(call, reply);
        }
        let reply = mutation(&call);
        let waiting = {
            let mut spaces = self.lock();
            let now = Instant::now();
            let space = spaces.get_mut(&workspace);
            let waiting = space.map(|space| {
                space.requests.complete(now, request, reply.clone());
                space.waiting.remove(&request).unwrap_or_default()
            });
            spaces.retain(|_, space| !space.requests.idle(now));
            waiting.unwrap_or_default()
        };
        for waiter in waiting {
            answer(waiter, reply.clone());
        }
        answer(call, reply);
    }

    /// `request.outcome` of identity `request` in `workspace`, unless no mutation here knows it.
    pub(crate) fn outcome(
        &self,
        workspace: WorkspaceId,
        request: RequestId,
    ) -> Option<RequestOutcomeResult> {
        let mut spaces = self.lock();
        let state = match spaces
            .get_mut(&workspace)?
            .requests
            .outcome(Instant::now(), request)
        {
            Outcome::Completed(reply) => MutationState::Completed(reply),
            Outcome::Running => MutationState::Running,
            Outcome::Unknown => return None,
        };
        Some(RequestOutcomeResult { state })
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<WorkspaceId, Space>> {
        self.spaces.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Answers `call` with `reply`, or closes it as cancelled when its Client cancelled it meanwhile.
fn answer(call: Call, reply: MutationReply) {
    if call.cancelled() {
        // Dropping a cancelled call closes it as cancelled.
        return drop(call);
    }
    finish(call, &FilesReply::from(reply));
}

/// Ends `call` with `reply`, or as failed when its record would pass the Result bound.
pub(crate) fn finish(call: Call, reply: &FilesReply) {
    if encode_reply(reply).len() <= MAXIMUM_REPLY_BYTES {
        call.reply(reply);
    } else {
        call.reply(&FilesReply::Error(FilesError::new(ErrorCode::Failed)));
    }
}

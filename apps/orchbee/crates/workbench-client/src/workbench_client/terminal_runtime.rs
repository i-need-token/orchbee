//! The Session's terminal-runtime state over one UI-owned `workbench.terminal-runtime` State
//! stream on the shared connection, without a bridge thread: the Server sends the state as it
//! stands first, then one frame per revision. Only a Session whose handshake answer lists
//! `terminal-runtime` is asked. While no stream answers, the state is serving, so each terminal
//! keeps its own recovery as it would without the read; an ended or refused stream opens again
//! after a delay.
use crate::local_client::{
    LocalClientOrigin, LocalOperationClock,
    multiplex::{StreamEvent, Subscription as Stream},
};
use orchbee_connection_handshake::TERMINAL_RUNTIME;
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, StreamMode, TerminalRuntimeOutcome, TerminalRuntimeRequest,
    TerminalRuntimeState, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    read_workbench_response,
};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

/// An ended stream opens again after this long; so does a check of a handshake that did not list
/// the capability, which a reconnect to another Server may change.
const REOPEN_DELAY: Duration = Duration::from_secs(2);

/// The open stream, the request it answers, and the revision it last adopted.
struct Open {
    request: TerminalRuntimeRequest,
    stream: Stream,
    revision: Option<WorkbenchRevision>,
}

pub(super) struct Lane {
    origin: Option<LocalClientOrigin>,
    /// The Session the stream follows, once one is known.
    session: Option<SessionId>,
    open: Option<Open>,
    reopen_at: Option<Instant>,
    state: TerminalRuntimeState,
    /// A later serving revision arrived on the open stream. The revision advances only when the
    /// state changes, so the Worker was replaced even when the frames between were never seen.
    served_again: bool,
    /// The shared connection lost its Server since the last call. It reconnects by itself, maybe
    /// to another Server that holds none of this Client's viewers or registration.
    carrier_lost: bool,
}

impl Default for Lane {
    fn default() -> Self {
        Self {
            origin: None,
            session: None,
            open: None,
            reopen_at: None,
            state: TerminalRuntimeState::Serving,
            served_again: false,
            carrier_lost: false,
        }
    }
}

impl Lane {
    /// Follows the state on `origin` from the next poll; nothing the old origin sent stays.
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.origin = Some(origin);
    }

    /// Follows `session`'s state, opening the stream when it is closed and due, and returns the
    /// state as of every frame received so far.
    pub(super) fn poll(
        &mut self,
        session: Option<&SessionId>,
        clock: &LocalOperationClock,
    ) -> TerminalRuntimeState {
        let (Some(origin), Some(session)) = (&self.origin, session) else {
            return self.state;
        };
        let now = clock.now();
        if self.session.as_ref() != Some(session) {
            self.session = Some(session.clone());
            self.open = None;
            self.state = TerminalRuntimeState::Serving;
            self.reopen_at = Some(now);
        }
        if self.open.is_none() && self.reopen_at.is_some_and(|at| now >= at) {
            self.open = origin
                .session_capabilities()
                .is_some_and(|capabilities| capabilities.contains(TERMINAL_RUNTIME))
                .then(|| open(origin, session))
                .flatten();
            self.reopen_at = self.open.is_none().then(|| now + REOPEN_DELAY);
        }
        self.read(now);
        self.state
    }

    /// Whether a serving revision followed an earlier one since the last call.
    pub(super) fn take_served_again(&mut self) -> bool {
        std::mem::take(&mut self.served_again)
    }

    /// Whether the shared connection lost its Server since the last call.
    pub(super) fn take_carrier_lost(&mut self) -> bool {
        std::mem::take(&mut self.carrier_lost)
    }

    /// Stands in for the stream's frames in a harness without a Server.
    #[cfg(test)]
    pub(super) fn observe(&mut self, state: TerminalRuntimeState) {
        self.state = state;
    }

    /// Closes the stream and forgets the origin; the composition's connection ended.
    pub(super) fn close(&mut self) {
        self.open = None;
        self.session = None;
        self.reopen_at = None;
        self.origin = None;
        self.state = TerminalRuntimeState::Serving;
    }

    /// Drains the stream. A record that does not answer it, or a refusal, ends the stream, which
    /// counts as serving and opens again after [`REOPEN_DELAY`].
    fn read(&mut self, now: Instant) {
        while let Some(open) = &mut self.open {
            let ended = match open.stream.poll() {
                Ok(Some(StreamEvent::Record(bytes))) => match runtime(&open.request, &bytes) {
                    Some((revision, state)) => {
                        if open.revision.is_none_or(|adopted| revision > adopted) {
                            self.served_again |=
                                open.revision.is_some() && state == TerminalRuntimeState::Serving;
                            open.revision = Some(revision);
                            self.state = state;
                        }
                        false
                    }
                    None => true,
                },
                Ok(Some(StreamEvent::Gap)) => false,
                Ok(Some(StreamEvent::Disconnected)) => {
                    self.carrier_lost = true;
                    false
                }
                Ok(None) => break,
                Err(_) => true,
            };
            if ended {
                self.open = None;
                self.state = TerminalRuntimeState::Serving;
                self.reopen_at = Some(now + REOPEN_DELAY);
            }
        }
    }
}

impl Drop for Lane {
    fn drop(&mut self) {
        self.close();
    }
}

fn open(origin: &LocalClientOrigin, session: &SessionId) -> Option<Open> {
    let request = TerminalRuntimeRequest {
        protocol_version: ProtocolVersion,
        request_id: crate::machine_observation::request_id().ok()?,
        session_id: session.clone(),
    };
    let stream = origin
        .subscribe(
            &WorkbenchRequest::TerminalRuntime(request.clone()),
            StreamMode::State,
        )
        .ok()?;
    Some(Open {
        request,
        stream,
        revision: None,
    })
}

/// The revision and state one record carries, or `None` for a refusal or a record that does not
/// answer `request`.
fn runtime(
    request: &TerminalRuntimeRequest,
    bytes: &[u8],
) -> Option<(WorkbenchRevision, TerminalRuntimeState)> {
    let Ok(WorkbenchResponse::TerminalRuntime(result)) =
        read_workbench_response(&mut Cursor::new(bytes))
    else {
        return None;
    };
    if !result.answers(request) {
        return None;
    }
    match result.outcome {
        TerminalRuntimeOutcome::Current { runtime } => Some((runtime.revision, runtime.state)),
        TerminalRuntimeOutcome::Failed { .. } => None,
    }
}

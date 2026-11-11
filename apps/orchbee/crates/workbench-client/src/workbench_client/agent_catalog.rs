//! The Session's Agent catalog over one UI-owned `workbench.agent-catalog` State stream on the
//! shared connection, without a bridge thread: the Server sends the catalog as it stands first,
//! then one frame whenever it changes. The lane keeps no catalog. A stream that ends or cannot
//! open opens again after a delay, and nothing is polled: the only other request is the refresh
//! an opened picker asks for, whose lookup reaches the stream like any other change.
use crate::agent_board::AgentCatalogEvent;
use crate::local_client::{
    LocalClientOrigin, LocalOperationClock,
    multiplex::{StreamEvent, Subscription as Stream},
    subscription::Request,
};
use orchbee_workbench_protocol::{
    AgentCatalogAction, AgentCatalogFailure, AgentCatalogOutcome, AgentCatalogRequest,
    ProtocolVersion, SessionId, StreamMode, WorkbenchRequest, WorkbenchResponse,
    read_workbench_response,
};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

/// An ended stream opens again after this long, so a Server that refused it for capacity or
/// restarted publishes again without the Client asking.
const REOPEN_DELAY: Duration = Duration::from_secs(5);
/// A refresh answers at once with the catalog as it stands; its lookup reaches the stream.
const REFRESH_DEADLINE: Duration = Duration::from_secs(2);

/// The open stream and the request it answers.
struct Open {
    request: AgentCatalogRequest,
    stream: Stream,
}

#[derive(Default)]
pub(super) struct Lane {
    origin: Option<LocalClientOrigin>,
    /// The Session the stream follows, once one is known.
    session: Option<SessionId>,
    /// A catalog of another origin or Session may be held: the next poll reports a reset.
    reset: bool,
    open: Option<Open>,
    reopen_at: Option<Instant>,
    /// The one refresh in flight; its answer is not read, since the stream carries the change.
    refresh: Option<Request>,
}

impl Lane {
    /// Follows the catalog on `origin` from the next poll; nothing the old origin sent stays.
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        let followed = self.session.is_some();
        self.close();
        self.reset |= followed;
        self.origin = Some(origin);
    }

    /// Follows `session`'s catalog, opening the stream when it is closed and due, and returns
    /// every report since the last call, in arrival order. A catalog of another origin or
    /// Session that may still be held is first reported as a reset.
    pub(super) fn poll(
        &mut self,
        session: Option<&SessionId>,
        clock: &LocalOperationClock,
    ) -> Vec<AgentCatalogEvent> {
        let mut events = Vec::new();
        if self.origin.is_none() {
            return events;
        }
        if self
            .refresh
            .as_ref()
            .is_some_and(|refresh| refresh.poll().is_some())
        {
            self.refresh = None;
        }
        let Some(session) = session else {
            return events;
        };
        let now = clock.now();
        if self.session.as_ref() != Some(session) {
            self.reset |= self.session.is_some();
            self.session = Some(session.clone());
            self.open = None;
            self.refresh = None;
            self.reopen_at = Some(now);
        }
        if std::mem::take(&mut self.reset) {
            events.push(AgentCatalogEvent::Reset);
        }
        if self.open.is_none() && self.reopen_at.is_some_and(|at| now >= at) {
            self.open = self
                .origin
                .as_ref()
                .and_then(|origin| open(origin, session));
            if self.open.is_none() {
                events.push(AgentCatalogEvent::Refused);
            }
            self.reopen_at = self.open.is_none().then(|| now + REOPEN_DELAY);
        }
        self.read(now, &mut events);
        events
    }

    /// Asks the Server to look for installed Agents again, unless a refresh is in flight or no
    /// stream follows a Session.
    pub(super) fn refresh(&mut self, clock: &LocalOperationClock) {
        let (Some(origin), Some(session)) = (&self.origin, &self.session) else {
            return;
        };
        if self.refresh.is_some() {
            return;
        }
        let Ok(request_id) = crate::machine_observation::request_id() else {
            return;
        };
        let request = WorkbenchRequest::AgentCatalog(AgentCatalogRequest {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: session.clone(),
            action: AgentCatalogAction::Refresh {},
        });
        self.refresh = origin
            .request(&request, clock.now() + REFRESH_DEADLINE, clock.clone())
            .ok();
    }

    /// Closes the stream and forgets the origin; the composition's connection ended.
    pub(super) fn close(&mut self) {
        self.open = None;
        self.refresh = None;
        self.session = None;
        self.reset = false;
        self.reopen_at = None;
        self.origin = None;
    }

    /// Drains the stream. A record that does not answer it ends the stream, which reports a
    /// refusal and opens again after [`REOPEN_DELAY`] like any ended stream.
    fn read(&mut self, now: Instant, events: &mut Vec<AgentCatalogEvent>) {
        while let Some(Open { request, stream }) = &self.open {
            let ended = match stream.poll() {
                Ok(Some(StreamEvent::Record(bytes))) => match catalog_event(request, &bytes) {
                    Some(event) => {
                        events.push(event);
                        false
                    }
                    None => true,
                },
                Ok(Some(StreamEvent::Gap | StreamEvent::Disconnected)) => false,
                Ok(None) => break,
                Err(_) => true,
            };
            if ended {
                self.open = None;
                self.reopen_at = Some(now + REOPEN_DELAY);
                events.push(AgentCatalogEvent::Refused);
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
    let request = AgentCatalogRequest {
        protocol_version: ProtocolVersion,
        request_id: crate::machine_observation::request_id().ok()?,
        session_id: session.clone(),
        action: AgentCatalogAction::Current {},
    };
    let stream = origin
        .subscribe(
            &WorkbenchRequest::AgentCatalog(request.clone()),
            StreamMode::State,
        )
        .ok()?;
    Some(Open { request, stream })
}

/// The report one record carries, or `None` for a record that does not answer `request`.
fn catalog_event(request: &AgentCatalogRequest, bytes: &[u8]) -> Option<AgentCatalogEvent> {
    let Ok(WorkbenchResponse::AgentCatalog(result)) =
        read_workbench_response(&mut Cursor::new(bytes))
    else {
        return None;
    };
    if !result.answers(request) {
        return None;
    }
    Some(match result.outcome {
        AgentCatalogOutcome::Current { catalog } => AgentCatalogEvent::Catalog(catalog),
        AgentCatalogOutcome::Failed {
            reason: AgentCatalogFailure::Unavailable,
        } => AgentCatalogEvent::Unavailable,
        AgentCatalogOutcome::Failed {
            reason: AgentCatalogFailure::Denied,
        } => AgentCatalogEvent::Refused,
    })
}

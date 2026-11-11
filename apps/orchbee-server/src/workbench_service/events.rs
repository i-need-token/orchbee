//! Bounded subscriptions share one physical observation slot on the authenticated connection.
use super::WorkbenchClientConnection;
use orchbee_server::{SessionEventJournal, SessionEventPage};
use orchbee_workbench_protocol::{
    MAXIMUM_EVENTS_BYTES, SemanticEventCursor, SemanticEventFilter, SemanticEventsAction as Action,
    SemanticEventsFailure as Failure, SemanticEventsOutcome as Outcome, SemanticEventsRequest,
    SemanticEventsResult, WorkbenchRequest, WorkbenchRequestId, serialize_workbench_request,
};
use std::time::{Duration, Instant};
mod capture;
mod probes;

pub(super) struct Subscription {
    id: WorkbenchRequestId,
    filters: Vec<SemanticEventFilter>,
    journal_filters: Vec<SemanticEventFilter>,
    definition_cursor: Option<SemanticEventCursor>,
    cursor: SemanticEventCursor,
    journal: SessionEventJournal,
    probes: Vec<probes::Probe>,
    next_probe: usize,
    probe_sequence: u64,
    capture: capture::Capture,
    pending_failure: Option<Failure>,
    changed: std::sync::Arc<orchbee_server::SessionChangeSignal>,
}

impl WorkbenchClientConnection<'_> {
    pub(super) fn semantic_events(&self, request: SemanticEventsRequest) -> SemanticEventsResult {
        let outcome = self
            .event_outcome(&request)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        SemanticEventsResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
    fn event_outcome(&self, request: &SemanticEventsRequest) -> Result<Outcome, Failure> {
        if !self.service.policy.read {
            return Err(Failure::Denied);
        }
        serialize_workbench_request(&WorkbenchRequest::Events(request.clone()))
            .map_err(|_| Failure::InvalidIntent)?;
        if self
            .service
            .terminals
            .workbench_session_matches(&request.session_id)
            != Some(true)
        {
            return Err(Failure::SessionMismatch);
        }
        let mut owned = self.events.try_lock().map_err(|_| Failure::Busy)?;
        match &request.action {
            Action::Subscribe {
                subscription_id,
                filters,
                after,
            } => {
                if let Some(subscription) = owned.get(subscription_id) {
                    if &subscription.filters != filters || &subscription.definition_cursor != after
                    {
                        return Err(Failure::Conflict);
                    }
                    return Ok(Outcome::Opened {
                        subscription_id: subscription.id.clone(),
                        cursor: subscription.cursor.clone(),
                        existing: true,
                    });
                }
                let journal = self
                    .service
                    .terminals
                    .event_journal()
                    .ok_or(Failure::Unavailable)?;
                if owned.len() >= orchbee_workbench_protocol::MAXIMUM_MULTIPLEX_STREAMS {
                    return Err(Failure::Busy);
                }
                let changed = std::sync::Arc::new(orchbee_server::SessionChangeSignal::default());
                journal.watch(&changed);
                self.service.terminals.maintain_events()?;
                let cursor = match after {
                    Some(cursor) => {
                        journal.validate_cursor(cursor)?;
                        cursor.clone()
                    }
                    None => journal.head()?,
                };
                let probes = probes::prepare(self, &request.session_id, filters)?;
                let journal_filters = probes::journal_filters(filters, &probes);
                let subscription = Subscription {
                    id: subscription_id.clone(),
                    filters: filters.clone(),
                    journal_filters,
                    definition_cursor: after.clone(),
                    cursor: cursor.clone(),
                    journal,
                    probes,
                    next_probe: 0,
                    probe_sequence: 0,
                    capture: capture::Capture::new(std::sync::Arc::clone(&self.event_capture)),
                    pending_failure: None,
                    changed,
                };
                owned.insert(subscription_id.clone(), subscription);
                Ok(Outcome::Opened {
                    subscription_id: subscription_id.clone(),
                    cursor,
                    existing: false,
                })
            }
            Action::Close { subscription_id } => {
                if owned.remove(subscription_id).is_none() {
                    return Err(Failure::NotFound);
                }
                // Dropping the actor consumes Required I/O, cancels, and joins before Closed escapes.
                Ok(Outcome::Closed {
                    subscription_id: subscription_id.clone(),
                })
            }
            Action::Read {
                subscription_id,
                limit,
            } => {
                let subscription = owned.get_mut(subscription_id).ok_or(Failure::NotFound)?;
                let page = subscription.read(self, request, usize::from(*limit), false)?;
                Ok(Outcome::Events {
                    subscription_id: subscription.id.clone(),
                    cursor: page.cursor,
                    events: page.events,
                    gap: page.gap,
                    more: page.more,
                })
            }
            Action::Wait {
                subscription_id,
                timeout_milliseconds,
            } => {
                let subscription = owned.get_mut(subscription_id).ok_or(Failure::NotFound)?;
                let deadline =
                    Instant::now() + Duration::from_millis(u64::from(*timeout_milliseconds));
                loop {
                    let observed = subscription.changed.wake_revision();
                    // The admitted baseline is read even for a zero-duration wait.
                    let mut page = subscription.read(self, request, 1, true)?;
                    if let Some(gap) = page.gap {
                        return Ok(Outcome::Gap {
                            subscription_id: subscription.id.clone(),
                            cursor: page.cursor,
                            gap,
                        });
                    }
                    if let Some(event) = page.events.pop() {
                        return Ok(Outcome::Matched {
                            subscription_id: subscription.id.clone(),
                            cursor: page.cursor,
                            event: Box::new(event),
                        });
                    }
                    let now = Instant::now();
                    if now >= deadline {
                        if subscription
                            .probes
                            .iter()
                            .any(|probe| !probe.has_baseline())
                        {
                            return Err(Failure::Busy);
                        }
                        return Ok(Outcome::TimedOut {
                            subscription_id: subscription.id.clone(),
                            cursor: page.cursor,
                        });
                    }
                    // Filesystem, process and Host probes have no notification source yet.
                    // Journal-only waits sleep until publication or their actual deadline.
                    let remaining = deadline.saturating_duration_since(now);
                    let wait = if subscription.probes.is_empty() {
                        remaining
                    } else {
                        remaining.min(Duration::from_millis(20))
                    };
                    subscription.changed.wait(observed, Some(wait));
                }
            }
        }
    }
}
impl Subscription {
    fn read(
        &mut self,
        session: &WorkbenchClientConnection<'_>,
        request: &SemanticEventsRequest,
        limit: usize,
        gap_only: bool,
    ) -> Result<SessionEventPage, Failure> {
        let result = self.read_page(session, request, limit, gap_only);
        if result.is_err() || self.pending_failure.is_some() {
            // Source resolution, initial probes and journal reads can all fail before sample()
            // consumes its completed job. No failure may retain the sibling capture permit.
            self.capture.cancel();
        }
        result
    }
    fn read_page(
        &mut self,
        session: &WorkbenchClientConnection<'_>,
        request: &SemanticEventsRequest,
        limit: usize,
        gap_only: bool,
    ) -> Result<SessionEventPage, Failure> {
        if let Some(reason) = self.pending_failure {
            return Err(reason);
        }
        // Emit an already-matching Agent state captured by Subscribe before future changes.
        for probe in &mut self.probes {
            let sequence = self
                .probe_sequence
                .checked_add(1)
                .ok_or(Failure::RevisionExhausted)?;
            if let Some(event) = probe.initial(session, &self.id, sequence)? {
                self.probe_sequence = sequence;
                return Ok(SessionEventPage {
                    cursor: self.cursor.clone(),
                    events: vec![event],
                    gap: None,
                    more: true,
                });
            }
        }
        session.service.terminals.maintain_events()?;
        let mut page = self
            .journal
            .read(&self.cursor, &self.journal_filters, limit)?;
        if gap_only && let Some(gap) = &page.gap {
            // A gap receipt must not consume a retained match that was not delivered.
            page.cursor.sequence = gap.through;
            page.events.clear();
            page.more = true;
            self.cursor = page.cursor.clone();
            return Ok(page);
        }
        let bytes = page
            .events
            .iter()
            .try_fold(0usize, |count, event| {
                serde_json::to_vec(event).map(|encoded| count.saturating_add(encoded.len()))
            })
            .map_err(|_| Failure::Unavailable)?;
        for event in &page.events {
            for probe in &mut self.probes {
                probe.journal_observed(event);
            }
        }
        if !page.more
            && page.events.len() < limit
            && !self.probes.is_empty()
            && bytes <= MAXIMUM_EVENTS_BYTES - 4096 - probes::MAXIMUM_PROBE_BYTES
        {
            let index = self.next_probe;
            let sequence = self
                .probe_sequence
                .checked_add(1)
                .ok_or(Failure::RevisionExhausted)?;
            let observed = self.probes[index].observe(
                session,
                &request.session_id,
                &self.id,
                sequence,
                index,
                &mut self.capture,
            );
            self.next_probe = (index + 1) % self.probes.len();
            let observed = match observed {
                Ok(event) => event,
                Err(Failure::Busy) => None,
                Err(reason) if !page.events.is_empty() || page.gap.is_some() => {
                    self.pending_failure = Some(reason);
                    page.more = true;
                    None
                }
                Err(reason) => return Err(reason),
            };
            if let Some(event) = observed {
                page.events.push(event);
                self.probe_sequence = sequence;
            }
        }
        self.cursor = page.cursor.clone();
        Ok(page)
    }
}

// Linux and macOS withhold semantic events at admission (`withheld_off_windows`).
#[cfg(all(test, windows))]
mod tests;

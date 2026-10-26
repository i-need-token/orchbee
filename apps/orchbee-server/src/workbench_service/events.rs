//! One immutable subscription and one bounded observation actor belong to the authenticated connection.
use super::WorkbenchClientConnection;
use orchbee_server::{SiteEventJournal, SiteEventPage};
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
    journal: SiteEventJournal,
    probes: Vec<probes::Probe>,
    next_probe: usize,
    probe_sequence: u64,
    capture: capture::Capture,
    pending_failure: Option<Failure>,
}

impl WorkbenchClientConnection<'_> {
    pub(super) fn semantic_events(&self, request: SemanticEventsRequest) -> SemanticEventsResult {
        let outcome = self
            .event_outcome(&request)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        SemanticEventsResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            site_id: request.site_id,
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
            .workbench_site_matches(&request.site_id)
            != Some(true)
        {
            return Err(Failure::SiteMismatch);
        }
        let mut owned = self.events.try_lock().map_err(|_| Failure::Busy)?;
        match &request.action {
            Action::Subscribe {
                subscription_id,
                filters,
                after,
            } => {
                if let Some(subscription) = owned.as_ref() {
                    if &subscription.id != subscription_id
                        || &subscription.filters != filters
                        || &subscription.definition_cursor != after
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
                self.service.terminals.maintain_events()?;
                let cursor = match after {
                    Some(cursor) => {
                        journal.validate_cursor(cursor)?;
                        cursor.clone()
                    }
                    None => journal.head()?,
                };
                let probes = probes::prepare(self, &request.site_id, filters)?;
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
                    capture: capture::Capture::default(),
                    pending_failure: None,
                };
                *owned = Some(subscription);
                Ok(Outcome::Opened {
                    subscription_id: subscription_id.clone(),
                    cursor,
                    existing: false,
                })
            }
            Action::Close { subscription_id } => {
                if !owned.as_ref().is_some_and(|s| &s.id == subscription_id) {
                    return Err(Failure::NotFound);
                }
                // Dropping the actor consumes Required I/O, cancels, and joins before Closed escapes.
                *owned = None;
                Ok(Outcome::Closed {
                    subscription_id: subscription_id.clone(),
                })
            }
            Action::Read {
                subscription_id,
                limit,
            } => {
                let subscription = owned
                    .as_mut()
                    .filter(|s| &s.id == subscription_id)
                    .ok_or(Failure::NotFound)?;
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
                let subscription = owned
                    .as_mut()
                    .filter(|s| &s.id == subscription_id)
                    .ok_or(Failure::NotFound)?;
                let deadline =
                    Instant::now() + Duration::from_millis(u64::from(*timeout_milliseconds));
                loop {
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
                    std::thread::sleep(
                        deadline
                            .saturating_duration_since(now)
                            .min(Duration::from_millis(20)),
                    );
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
    ) -> Result<SiteEventPage, Failure> {
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
                return Ok(SiteEventPage {
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
                &request.site_id,
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

#[cfg(test)]
mod tests;

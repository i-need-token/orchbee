//! `agent wait`: one held Client Session polls the canonical session
//! document under a single monotonic total deadline until a requested
//! condition matches, the revision fence refuses, the session answer is
//! terminal, or the budget lands. The whole loop runs inside the same
//! armed interruption owner as every Agent read: the wake descriptor joins
//! each exchange and each private backoff sleep, the session is dropped
//! before the single `finish` arbitration, and nothing projects to a
//! `Failure` before that arbitration.
//!
//! Windows also observes contiguous canonical change pages, preserving a
//! transient match separately from the latest detail. Linux retains sampled
//! polling. `--after-revision` is a global fence, not a turn identity.

use orchbee_local_ipc::LocalIo as BorrowedFd;
use std::time::{Duration, Instant};

use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentObservationFreshness, AgentReadRequest, AgentReadResult,
    AgentSafeInteger, AgentSemanticStatus, AgentSessionDetail, AgentSessionId, AgentWireError,
    ObapRequestId,
};
use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use serde_json::json;

use super::{
    AGENT_TOTAL_BUDGET, AgentAdmissionError, AgentCancelled, AgentQueryError,
    agent_protocol_invalid, agent_read_details, agent_session_details, agent_session_rejection,
    agent_session_until, agent_unavailable_details, classify_agent_read, classify_connect,
    is_deadline_elapsed, obap_request_id,
};
use crate::cli::results::{AgentWaitDoc, Projection, agent_source};
use crate::cli::source::{ArmedSignals, Finish, SignalWatch};
use crate::cli::wait_deadline::WaitDeadline;
use crate::cli::{Failure, FailureKind};
use crate::invocation::AgentWaitCondition;
use crate::local_client::agent::{AgentExchangeError, AgentExchangeFailure, AgentReadOperation};
use crate::local_client::{LocalClient, exchange_wake_fault, exchange_wake_ready};

/// The private first poll interval — no public flag exposes it.
const WAIT_POLL_INITIAL: Duration = Duration::from_millis(50);
/// Consecutive quiet polls double the interval up to this ceiling.
const WAIT_POLL_MAXIMUM: Duration = Duration::from_millis(500);

#[cfg(windows)]
use super::{AgentJournal, JournalError};

/// `agent wait`: the armed span owns the whole wait — one admitted Client
/// Session held across every poll, one absolute deadline covering
/// admission, all Gets, and all backoff. The decisive Get's request
/// identity is the emitted document's only honest source.
pub(crate) fn agent_wait(
    target: &crate::invocation::AgentTarget,
    until: &[AgentWaitCondition],
    after_revision: Option<AgentSafeInteger>,
    timeout_ms: Option<u32>,
    selected: Option<&orchbee_workbench_protocol::WorkbenchClientId>,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    #[cfg(not(windows))]
    if selected.is_some() {
        return Err(AgentQueryError::Admission(
            Failure::new(FailureKind::ServiceUnavailable).details(
                crate::cli::stage_reason_details("platform", "unsupported-platform"),
            ),
        ));
    }
    let start = Instant::now();
    let total = WaitDeadline::new(start, timeout_ms).map_err(AgentQueryError::Admission)?;
    let armed = ArmedSignals::arm_interrupts()
        .map_err(|_| AgentQueryError::Admission(Failure::new(FailureKind::InternalFailure)))?;
    let attempt = agent_wait_finish(
        total,
        start,
        AGENT_TOTAL_BUDGET,
        target,
        until,
        after_revision,
        timeout_ms,
        selected,
        armed,
        agent_session_until,
    )?;
    classify_wait_attempt(attempt)
}

/// The armed span's finish half, separated exactly like
/// `agent_armed_finish` so a recording watch can prove the held session
/// is already dropped when the arbitration read runs: `agent_wait_span`
/// owns the session and drops it before returning; only then does
/// `armed.finish()` read the signal descriptor. `Clear` hands the raw
/// attempt back; `Cancelled` answers the typed 130 family and publishes
/// no source.
#[expect(clippy::too_many_arguments)]
fn agent_wait_finish(
    total: WaitDeadline,
    start: Instant,
    read_budget: Duration,
    target: &crate::invocation::AgentTarget,
    until: &[AgentWaitCondition],
    after_revision: Option<AgentSafeInteger>,
    timeout_ms: Option<u32>,
    selected: Option<&orchbee_workbench_protocol::WorkbenchClientId>,
    mut armed: ArmedSignals,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> Result<WaitAttempt, AgentQueryError> {
    let attempt = {
        agent_wait_span(
            total,
            start,
            read_budget,
            target,
            until,
            after_revision,
            timeout_ms,
            selected,
            &mut armed,
            acquire,
        )
    };
    match armed.finish() {
        Err(_) => Err(match attempt {
            WaitAttempt::Admission(_) => {
                AgentQueryError::Admission(Failure::new(FailureKind::InternalFailure))
            }
            _ => AgentQueryError::Read(Failure::new(FailureKind::InternalFailure)),
        }),
        Ok(Finish::Cancelled { finalization, .. }) => {
            Err(AgentQueryError::Cancelled(Box::new(AgentCancelled {
                // A cancelled wait carries no source: only a trustworthy
                // semantic answer may publish the correlated request
                // identity.
                failure: Failure::new(FailureKind::Cancelled)
                    .details(agent_read_details(AgentReadOperation::Wait)),
                finalization,
            })))
        }
        Ok(Finish::Clear) => Ok(attempt),
    }
}

/// The armed wait span's retained raw outcome — the same arbitration
/// discipline as `AgentAttempt`: nothing projects to a `Failure` before
/// the signal owner's single arbitration, so a provisional wake marker
/// survives intact to that point.
enum WaitAttempt {
    /// The span never reached an admitted session — generic failure family.
    Admission(AgentAdmissionError),
    /// The wake descriptor reported readiness during a backoff or an
    /// exchange surfaced a provisional wake marker — the owner's
    /// arbitration decides; `Clear` classifies it as `internal-failure`.
    Wake,
    /// A settled outcome: the decisive Get's identity plus the matched
    /// document, or an already-classified wait failure.
    Done(Result<(ObapRequestId, AgentWaitDoc), Failure>),
    #[cfg(windows)]
    TerminalDone(Result<super::terminal_agent_wait::Matched, Failure>),
}

/// Facts the last completed observation carried, for honest timeout
/// details — absent entirely when zero observations completed, never
/// invented.
struct WaitObservation {
    status: &'static str,
    freshness: AgentObservationFreshness,
    revision: AgentSafeInteger,
}

/// Why a wait ended at a deadline — two distinct honest reasons, never
/// conflated into one generic timeout.
#[derive(Clone, Copy)]
enum WaitDeadlineReason {
    /// `now >= total`: admission, every read, and every backoff together
    /// spent the whole shared budget.
    Total,
    /// The per-read bound `min(now + read_budget, total)` fired while
    /// `total` still had room. The held session is already retired by the
    /// exchange and is never re-admitted; the details record the budget
    /// actually admitted so the early stop is explicit — it does NOT mean
    /// the requested conditions were absent for the whole wait.
    Read { budget_ms: u64 },
}

/// The armed wait span body: admission under the borrowed wake descriptor,
/// then per-poll `settle → Get → evaluate → backoff` — the session is
/// dropped before the caller's finish arbitration. Each Get mints its own
/// request identity and runs under `min(read_budget, remaining total)`;
/// a session-retiring or ambiguous exchange is a definite typed outcome,
/// never a re-admission.
#[expect(clippy::too_many_arguments)]
fn agent_wait_span(
    total: WaitDeadline,
    start: Instant,
    read_budget: Duration,
    target: &crate::invocation::AgentTarget,
    until: &[AgentWaitCondition],
    after_revision: Option<AgentSafeInteger>,
    timeout_ms: Option<u32>,
    selected: Option<&orchbee_workbench_protocol::WorkbenchClientId>,
    signals: &mut ArmedSignals,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> WaitAttempt {
    let admission = match total.step(Instant::now(), read_budget) {
        Ok(deadline) => deadline,
        Err(failure) => return WaitAttempt::Admission(AgentAdmissionError::Preamble(failure)),
    };
    let mut client = match acquire(admission, signals.poll_fd()) {
        Err(error) => return WaitAttempt::Admission(error),
        Ok(client) => client,
    };
    let resolve_deadline = match total.step(Instant::now(), read_budget) {
        Ok(deadline) => deadline,
        Err(failure) => return WaitAttempt::Done(Err(failure)),
    };
    let snapshot = if selected.is_some() {
        match super::agent_target::read_snapshot(&mut client, resolve_deadline, signals.poll_fd()) {
            Ok(snapshot) => Some(snapshot),
            Err(failure) => return WaitAttempt::Done(Err(failure)),
        }
    } else {
        None
    };
    #[cfg(windows)]
    let session_id = match super::agent_target::resolve_named(
        &mut client,
        target,
        snapshot.as_ref(),
        resolve_deadline,
        signals.poll_fd(),
    ) {
        Ok(resolved) => match resolved.target {
            orchbee_agent_observation_protocol::AgentNameTarget::Session { session_id } => {
                session_id
            }
            orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding } => {
                return WaitAttempt::TerminalDone(
                    super::terminal_agent_wait::admit(until).and_then(|()| {
                        super::terminal_agent_wait::observe(
                            &mut client,
                            super::terminal_agent_wait::Options {
                                selected,
                                target,
                                binding: &binding,
                                until,
                                after_revision,
                                timeout_ms,
                                start,
                                total,
                            },
                            None,
                            signals,
                        )
                    }),
                );
            }
        },
        Err(failure) => return WaitAttempt::Done(Err(failure)),
    };
    #[cfg(not(windows))]
    let session_id = match super::agent_target::resolve_on_session(
        &mut client,
        target,
        snapshot.as_ref(),
        resolve_deadline,
        signals.poll_fd(),
    ) {
        Ok(resolved) => resolved.session_id,
        Err(failure) => return WaitAttempt::Done(Err(failure)),
    };
    let session_id = &session_id;
    let mut observations: u64 = 0;
    let mut last: Option<WaitObservation> = None;
    let mut backoff = WAIT_POLL_INITIAL;
    let mut first_evaluation = true;
    #[cfg(windows)]
    let mut journal = AgentJournal::default();
    #[cfg(windows)]
    let historical_until = until.to_vec();

    let outcome = loop {
        // Post-step total settlement before any new step: an expired
        // shared budget is always the wait's own timeout.
        if total.expired(Instant::now()) {
            break wait_timeout(
                WaitDeadlineReason::Total,
                timeout_ms,
                start,
                observations,
                last.as_ref(),
            );
        }
        #[cfg(windows)]
        let historical = {
            let now = Instant::now();
            let deadline = match total.step(now, read_budget) {
                Ok(deadline) => deadline,
                Err(failure) => break WaitAttempt::Done(Err(failure)),
            };
            let result = journal.read(&mut client, deadline, signals.poll_fd());
            if total.expired(Instant::now()) {
                break wait_timeout(
                    WaitDeadlineReason::Total,
                    timeout_ms,
                    start,
                    observations,
                    last.as_ref(),
                );
            }
            match result {
                Ok((source, changes)) => changes.into_iter().find_map(|change| {
                    if change.session.session_id != *session_id
                        || after_revision
                            .is_some_and(|fence| change.revision.value() <= fence.value())
                    {
                        return None;
                    }
                    let conditions: Vec<_> = historical_until
                        .iter()
                        .copied()
                        .filter(|condition| {
                            status_matches(
                                *condition,
                                change.session.observation.freshness,
                                &change.session.status,
                            )
                        })
                        .collect();
                    (!conditions.is_empty()).then_some((source.clone(), change, conditions))
                }),
                Err(JournalError::Failure(failure)) => {
                    break WaitAttempt::Done(Err(failure));
                }
                Err(JournalError::Exchange(error)) => {
                    if wait_wake_marker(&error) {
                        break WaitAttempt::Wake;
                    }
                    if wait_deadline_elapsed(&error) {
                        break wait_timeout(
                            WaitDeadlineReason::Read {
                                budget_ms: deadline.duration_since(now).as_millis() as u64,
                            },
                            timeout_ms,
                            start,
                            observations,
                            last.as_ref(),
                        );
                    }
                    break WaitAttempt::Done(Err(classify_agent_read(
                        error,
                        AgentReadOperation::Wait,
                    )));
                }
            }
        };
        let request_id = match obap_request_id() {
            Ok(request_id) => request_id,
            Err(failure) => break WaitAttempt::Done(Err(failure)),
        };
        let now = Instant::now();
        let read_deadline = match total.step(now, read_budget) {
            Ok(deadline) => deadline,
            Err(failure) => break WaitAttempt::Done(Err(failure)),
        };
        // The budget this poll's Get was actually admitted with: the
        // read-deadline reason records it verbatim.
        #[cfg(windows)]
        let attention = if let (Some(selected), Some(snapshot)) = (selected, snapshot.as_ref()) {
            let client_start = Instant::now();
            let client_deadline = read_deadline.min(client_start + Duration::from_secs(2));
            match crate::cli::agent_view::windows::read(
                &mut client,
                snapshot,
                selected,
                session_id,
                client_deadline,
                signals,
            ) {
                Ok(read) => Some(read),
                Err(failure) => {
                    break client_wait_failure(
                        failure,
                        total,
                        timeout_ms,
                        start,
                        observations,
                        last.as_ref(),
                        client_deadline
                            .saturating_duration_since(client_start)
                            .as_millis() as u64,
                    );
                }
            }
        } else {
            None
        };
        let read_budget_ms = read_deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .max(1) as u64;
        let result = client.submit_agent_read_before_cancellable(
            AgentReadRequest::Get {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
            },
            read_deadline,
            signals.poll_fd(),
        );
        // Post-step total settlement before the outcome may match or
        // classify — never a late success.
        let settled = total.expired(Instant::now());
        match result {
            Ok(AgentReadResult::Get {
                request_id: answered,
                result,
            }) => {
                observations += 1;
                if let AgentGetOutcome::Found {
                    revision,
                    session: ref detail,
                } = result.outcome
                {
                    last = Some(WaitObservation {
                        status: status_kind(&detail.status),
                        freshness: detail.snapshot.observation.freshness,
                        revision,
                    });
                }
                if settled {
                    break wait_timeout(
                        WaitDeadlineReason::Total,
                        timeout_ms,
                        start,
                        observations,
                        last.as_ref(),
                    );
                }
                match result.outcome {
                    AgentGetOutcome::Found { revision, session } => {
                        #[cfg(windows)]
                        if journal
                            .observed_through()
                            .is_some_and(|head| head.value() > revision.value())
                        {
                            break WaitAttempt::Done(Err(agent_protocol_invalid(
                                AgentReadOperation::Wait,
                            )));
                        }
                        if session.snapshot.session_id != *session_id {
                            break WaitAttempt::Done(Err(agent_protocol_invalid(
                                AgentReadOperation::Wait,
                            )));
                        }
                        #[cfg(windows)]
                        let (session, client_view) = {
                            let canonical = super::AgentObservationRead {
                                request_id: answered.clone(),
                                revision,
                                session,
                            };
                            let joined = match (attention, snapshot.as_ref()) {
                                (Some(attention), Some(snapshot)) => match attention.corroborate(
                                    &mut client,
                                    snapshot,
                                    &canonical,
                                    read_deadline,
                                    signals,
                                ) {
                                    Ok(view) => Some(Box::new(view)),
                                    Err(failure) => {
                                        break client_wait_failure(
                                            failure,
                                            total,
                                            timeout_ms,
                                            start,
                                            observations,
                                            last.as_ref(),
                                            read_budget_ms,
                                        );
                                    }
                                },
                                _ => None,
                            };
                            (canonical.session, joined)
                        };
                        // The canonical fence refuses only against the
                        // FIRST returned revision — it can never fire
                        // later (revision is monotonic); if it ever did,
                        // the same arm still answers honestly.
                        if first_evaluation {
                            first_evaluation = false;
                            if let Some(fence) = after_revision
                                && fence.value() > revision.value()
                            {
                                break WaitAttempt::Done(Err(Failure::new(FailureKind::Rejected)
                                    .details(revision_ahead_details(session_id, fence, revision))
                                    .source(agent_source(&answered))));
                            }
                        }
                        let eligible =
                            after_revision.is_none_or(|fence| revision.value() > fence.value());
                        #[cfg(windows)]
                        if let Some((source, change, matched)) = historical {
                            if revision.value() < change.revision.value() {
                                break WaitAttempt::Done(Err(agent_protocol_invalid(
                                    AgentReadOperation::Wait,
                                )));
                            }
                            break WaitAttempt::Done(Ok((
                                source,
                                AgentWaitDoc {
                                    matched,
                                    revision,
                                    session,
                                    matched_change: Some(change),
                                    client: client_view,
                                    until: until.to_vec(),
                                    after_revision,
                                    timeout_ms,
                                    observations,
                                    elapsed_ms: start.elapsed().as_millis() as u64,
                                },
                            )));
                        }
                        if eligible {
                            // Requested order is preserved: the canonical
                            // conditions.filter keeps the caller's order,
                            // never an invented canonical sort.
                            let matched: Vec<AgentWaitCondition> = until
                                .iter()
                                .copied()
                                .filter(|condition| condition_matches(*condition, &session))
                                .collect();
                            if !matched.is_empty() {
                                break WaitAttempt::Done(Ok((
                                    answered,
                                    AgentWaitDoc {
                                        matched,
                                        revision,
                                        session,
                                        #[cfg(windows)]
                                        matched_change: None,
                                        #[cfg(windows)]
                                        client: client_view,
                                        until: until.to_vec(),
                                        after_revision,
                                        timeout_ms,
                                        observations,
                                        elapsed_ms: start.elapsed().as_millis() as u64,
                                    },
                                )));
                            }
                        }
                        #[cfg(windows)]
                        if !journal.has_more()
                            && session.snapshot.lifecycle
                                == orchbee_agent_observation_protocol::AgentLifecycle::Ended
                        {
                            break WaitAttempt::Done(Err(Failure::new(FailureKind::Rejected)
                                .details(super::agent_rejection_details(
                                    AgentReadOperation::Wait,
                                    "session-ended",
                                ))
                                .source(agent_source(&answered))));
                        }
                    }
                    AgentGetOutcome::Rejected { reason } => {
                        break WaitAttempt::Done(Err(agent_session_rejection(
                            AgentReadOperation::Wait,
                            reason,
                            session_id,
                        )
                        .source(agent_source(&answered))));
                    }
                    AgentGetOutcome::Unavailable { .. } => {
                        break WaitAttempt::Done(Err(Failure::new(
                            FailureKind::ServiceUnavailable,
                        )
                        .details(agent_unavailable_details(AgentReadOperation::Wait))
                        .source(agent_source(&answered))));
                    }
                }
            }
            // A correlated non-Get answer is a protocol contradiction the
            // wire layer normally retires first; keep the honest arm.
            Ok(_) => {
                if settled {
                    break wait_timeout(
                        WaitDeadlineReason::Total,
                        timeout_ms,
                        start,
                        observations,
                        last.as_ref(),
                    );
                }
                break WaitAttempt::Done(Err(agent_protocol_invalid(AgentReadOperation::Wait)));
            }
            Err(error) => {
                if settled {
                    break wait_timeout(
                        WaitDeadlineReason::Total,
                        timeout_ms,
                        start,
                        observations,
                        last.as_ref(),
                    );
                }
                if wait_wake_marker(&error) {
                    break WaitAttempt::Wake;
                }
                if wait_deadline_elapsed(&error) {
                    // The per-read bound fired with total budget still
                    // open — the exchange already retired the session and
                    // the wait never reconnects.
                    break wait_timeout(
                        WaitDeadlineReason::Read {
                            budget_ms: read_budget_ms,
                        },
                        timeout_ms,
                        start,
                        observations,
                        last.as_ref(),
                    );
                }
                break WaitAttempt::Done(Err(classify_agent_read(error, AgentReadOperation::Wait)));
            }
        }
        // Bounded interruptible backoff: the wake descriptor joins the
        // sleep so a captured signal ends the wait promptly instead of at
        // the next poll boundary.
        #[cfg(windows)]
        if journal.has_more() {
            continue;
        }
        let remaining = total.backoff(Instant::now(), backoff);
        if !remaining.is_zero() {
            if wait_backoff(signals.poll_fd(), backoff.min(remaining)) {
                break WaitAttempt::Wake;
            }
            backoff = (backoff.saturating_mul(2)).min(WAIT_POLL_MAXIMUM);
        }
    };
    // The held Client Session is fully dropped before the owner arbitrates.
    drop(client);
    outcome
}

#[cfg(windows)]
fn client_wait_failure(
    mut failure: Failure,
    total: WaitDeadline,
    timeout_ms: Option<u32>,
    start: Instant,
    observations: u64,
    last: Option<&WaitObservation>,
    read_budget_ms: u64,
) -> WaitAttempt {
    if failure.kind == FailureKind::Timeout || total.expired(Instant::now()) {
        return wait_timeout(
            if total.expired(Instant::now()) {
                WaitDeadlineReason::Total
            } else {
                WaitDeadlineReason::Read {
                    budget_ms: read_budget_ms,
                }
            },
            timeout_ms,
            start,
            observations,
            last,
        );
    }
    if failure.failure_schema.is_none() {
        failure.failure_schema = Some(crate::cli::schema::FAILURE_ID);
    }
    WaitAttempt::Done(Err(failure))
}

/// Classify a cleared wait attempt the existing way: admission outcomes
/// keep the generic failure family, an unread wake marker is an internal
/// contradiction, and the settled outcome carries its own document or
/// classified failure.
fn classify_wait_attempt(
    attempt: WaitAttempt,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    match attempt {
        WaitAttempt::Admission(AgentAdmissionError::Preamble(failure)) => {
            Err(AgentQueryError::Admission(failure))
        }
        WaitAttempt::Admission(AgentAdmissionError::Connect(error)) => {
            Err(AgentQueryError::Admission(classify_connect(error)))
        }
        WaitAttempt::Wake => Err(AgentQueryError::Read(Failure::new(
            FailureKind::InternalFailure,
        ))),
        WaitAttempt::Done(result) => result
            .map(|(request_id, doc)| (agent_source(&request_id), Projection::AgentWait(doc)))
            .map_err(AgentQueryError::Read),
        #[cfg(windows)]
        WaitAttempt::TerminalDone(result) => result
            .map(|doc| {
                (
                    doc.read.source.clone(),
                    Projection::AgentTerminalWait(Box::new(doc)),
                )
            })
            .map_err(AgentQueryError::Read),
    }
}

/// The canonical match predicate over ONE returned session detail:
/// freshness conditions match the observation's freshness directly;
/// every status condition requires `fresh` freshness and a status kind
/// equal to the condition's canonical name.
pub(crate) fn condition_matches(
    condition: AgentWaitCondition,
    session: &AgentSessionDetail,
) -> bool {
    status_matches(
        condition,
        session.snapshot.observation.freshness,
        &session.status,
    )
}

pub(crate) fn status_matches(
    condition: AgentWaitCondition,
    freshness: AgentObservationFreshness,
    status: &AgentSemanticStatus,
) -> bool {
    match condition {
        AgentWaitCondition::Blocked => {
            status_matches(AgentWaitCondition::NeedsAttention, freshness, status)
        }
        AgentWaitCondition::Done => {
            freshness == AgentObservationFreshness::Fresh
                && matches!(
                    status,
                    AgentSemanticStatus::Succeeded { .. }
                        | AgentSemanticStatus::Failed { .. }
                        | AgentSemanticStatus::Canceled { .. }
                        | AgentSemanticStatus::Interrupted { .. }
                )
        }
        AgentWaitCondition::Fresh => freshness == AgentObservationFreshness::Fresh,
        AgentWaitCondition::Stale => freshness == AgentObservationFreshness::Stale,
        AgentWaitCondition::Disconnected => freshness == AgentObservationFreshness::Disconnected,
        _ => {
            freshness == AgentObservationFreshness::Fresh
                && status_kind(status) == condition.as_str()
        }
    }
}

/// The canonical kebab-case status-kind name a semantic status carries —
/// the same names the wait conditions and emitted documents use.
fn status_kind(status: &AgentSemanticStatus) -> &'static str {
    match status {
        AgentSemanticStatus::NeedsAttention { .. } => "needs-attention",
        AgentSemanticStatus::Failed { .. } => "failed",
        AgentSemanticStatus::Succeeded { .. } => "succeeded",
        AgentSemanticStatus::Canceled { .. } => "canceled",
        AgentSemanticStatus::Interrupted { .. } => "interrupted",
        AgentSemanticStatus::Working { .. } => "working",
        AgentSemanticStatus::Idle { .. } => "idle",
        AgentSemanticStatus::Unknown { .. } => "unknown",
        AgentSemanticStatus::Ended { .. } => "ended",
    }
}

/// The canonical freshness name for observed-fact fields.
fn freshness_kind(freshness: AgentObservationFreshness) -> &'static str {
    match freshness {
        AgentObservationFreshness::Fresh => "fresh",
        AgentObservationFreshness::Stale => "stale",
        AgentObservationFreshness::Disconnected => "disconnected",
    }
}

/// The io carrier inside a failed exchange, when one exists — the shared
/// wake-marker and deadline probes inspect the same error the single-read
/// classifier weighs.
fn wait_io_error(error: &AgentExchangeError) -> Option<&std::io::Error> {
    match error {
        AgentExchangeError::RequestNotSubmitted(error) => Some(error),
        AgentExchangeError::Failed { reason, .. } => match reason {
            AgentExchangeFailure::Write(error) | AgentExchangeFailure::ResponseDeadline(error) => {
                Some(error)
            }
            AgentExchangeFailure::Response(AgentWireError::Io(error)) => Some(error),
            AgentExchangeFailure::Response(_)
            | AgentExchangeFailure::UnexpectedResponse
            | AgentExchangeFailure::CorrelationMismatch { .. } => None,
        },
        AgentExchangeError::SessionUnavailable | AgentExchangeError::RequestEncoding(_) => None,
    }
}

/// A provisional wake marker surviving inside the exchange error: the
/// loop stops and the owner's arbitration decides — never classified
/// here.
fn wait_wake_marker(error: &AgentExchangeError) -> bool {
    wait_io_error(error)
        .is_some_and(|error| exchange_wake_ready(error) || exchange_wake_fault(error))
}

/// A deadline-elapsed exchange error: the per-read bound fired before the
/// shared total — the caller decides which of the two deadlines reports.
fn wait_deadline_elapsed(error: &AgentExchangeError) -> bool {
    wait_io_error(error).is_some_and(is_deadline_elapsed)
}

/// The interruptible private backoff: poll the borrowed wake descriptor
/// for up to `duration`; `true` means it reported readiness or an
/// unrecoverable poll fault, and the owner's arbitration must run before
/// any further step.
pub(crate) fn wait_backoff(wake: Option<BorrowedFd<'_>>, duration: Duration) -> bool {
    let Some(fd) = wake else {
        if !duration.is_zero() {
            std::thread::sleep(duration);
        }
        return false;
    };
    if duration.is_zero() {
        return false;
    }
    let mut descriptors = [PollFd::new(fd, PollFlags::POLLIN)];
    let rounded_milliseconds = duration.as_millis().saturating_add(u128::from(
        !duration.subsec_nanos().is_multiple_of(1_000_000),
    ));
    let timeout = PollTimeout::try_from(rounded_milliseconds).unwrap_or(PollTimeout::MAX);
    match poll(&mut descriptors, timeout) {
        Ok(0) => false,
        Ok(_) => descriptors[0]
            .revents()
            .unwrap_or_else(PollFlags::empty)
            .intersects(
                PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR | PollFlags::POLLNVAL,
            ),
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => false,
        // A descriptor-level poll fault is an internal condition: stop so
        // the owner's arbitration can land it on `internal-failure`.
        Err(_) => true,
    }
}

/// The `revision-ahead` refusal details: the incoherent fence is a
/// rejection, never a wait — `{stage:"agent-read", operation:"wait",
/// sessionId, afterRevision, revision}`.
fn revision_ahead_details(
    session_id: &AgentSessionId,
    after_revision: AgentSafeInteger,
    revision: AgentSafeInteger,
) -> serde_json::Map<String, serde_json::Value> {
    let mut details = agent_session_details(AgentReadOperation::Wait, session_id.as_str());
    details.insert("afterRevision".to_owned(), json!(after_revision.value()));
    details.insert("revision".to_owned(), json!(revision.value()));
    details
}

/// The wait timeout arm: `{stage:"agent-read", operation:"wait",
/// timeoutMs, elapsedMs, reason, completedObservations}` plus the honest
/// observed facts — `readBudgetMs` only under `read-deadline`, `last*`
/// only when at least one observation completed.
fn wait_timeout(
    deadline: WaitDeadlineReason,
    timeout_ms: Option<u32>,
    start: Instant,
    observations: u64,
    last: Option<&WaitObservation>,
) -> WaitAttempt {
    let mut details = agent_read_details(AgentReadOperation::Wait);
    details.insert("timeoutMs".to_owned(), json!(timeout_ms));
    details.insert(
        "elapsedMs".to_owned(),
        json!(start.elapsed().as_millis() as u64),
    );
    let reason = match deadline {
        WaitDeadlineReason::Total => "wait-deadline",
        WaitDeadlineReason::Read { .. } => "read-deadline",
    };
    details.insert("reason".to_owned(), json!(reason));
    if let WaitDeadlineReason::Read { budget_ms } = deadline {
        details.insert("readBudgetMs".to_owned(), json!(budget_ms));
    }
    details.insert("completedObservations".to_owned(), json!(observations));
    if let Some(observed) = last {
        details.insert("lastStatusKind".to_owned(), json!(observed.status));
        details.insert(
            "lastFreshness".to_owned(),
            json!(freshness_kind(observed.freshness)),
        );
        details.insert("lastRevision".to_owned(), json!(observed.revision.value()));
    }
    WaitAttempt::Done(Err(Failure::new(FailureKind::Timeout)
        .details(details)
        .version(if timeout_ms.is_none() { 2 } else { 1 })))
}

#[cfg(test)]
mod tests;

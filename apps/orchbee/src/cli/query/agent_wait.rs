//! `agent wait`: one held Client Session polls the canonical session
//! document under a single monotonic total deadline until a requested
//! condition matches, the revision fence refuses, the session answer is
//! terminal, or the budget lands. The whole loop runs inside the same
//! armed interruption owner as every Agent read: the wake descriptor joins
//! each exchange and each private backoff sleep, the session is dropped
//! before the single `finish` arbitration, and nothing projects to a
//! `Failure` before that arbitration.
//!
//! Honest semantics, never hidden: the wait SAMPLES the current projected
//! state at each poll. A transition that both begins and ends between two
//! completed observations is missed — pending requests may clear, turn
//! outcomes are replaced by later turns, and stale or disconnected
//! freshness is a property of the sample, not a promise. `--after-revision`
//! is a caller-supplied global-revision fence, never a turn-scoped causal
//! identity, and there is no subscribe/changes guarantee behind the poll.

use std::os::fd::BorrowedFd;
use std::time::{Duration, Instant};

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentObservationFreshness, AgentReadRequest, AgentReadResult,
    AgentSafeInteger, AgentSemanticStatus, AgentSessionDetail, AgentSessionId, AgentWireError,
    ObapRequestId,
};
use serde_json::json;

use super::{
    AGENT_TOTAL_BUDGET, AgentAdmissionError, AgentCancelled, AgentQueryError, AgentWatch,
    agent_protocol_invalid, agent_read_details, agent_session_details, agent_session_rejection,
    agent_session_until, agent_unavailable_details, bounded_deadline, classify_agent_read,
    classify_connect, deadline_after, is_deadline_elapsed, obap_request_id,
};
use crate::cli::results::{AgentWaitDoc, Projection, agent_source};
use crate::cli::source::{ArmedSignals, Finish};
use crate::cli::{Failure, FailureKind};
use crate::invocation::AgentWaitCondition;
use crate::local_client::agent::{AgentReadError, AgentReadFailure, AgentReadOperation};
use crate::local_client::{LocalClient, exchange_wake_fault, exchange_wake_ready};

/// The private first poll interval — no public flag exposes it.
const WAIT_POLL_INITIAL: Duration = Duration::from_millis(50);
/// Consecutive quiet polls double the interval up to this ceiling.
const WAIT_POLL_MAXIMUM: Duration = Duration::from_millis(500);

/// `agent wait`: the armed span owns the whole wait — one admitted Client
/// Session held across every poll, one absolute deadline covering
/// admission, all Gets, and all backoff. The decisive Get's request
/// identity is the emitted document's only honest source.
pub(crate) fn agent_wait(
    session_id: &AgentSessionId,
    until: &[AgentWaitCondition],
    after_revision: Option<AgentSafeInteger>,
    timeout_ms: u32,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    let start = Instant::now();
    let total = deadline_after(Duration::from_millis(u64::from(timeout_ms)))
        .map_err(AgentQueryError::Admission)?;
    let armed = ArmedSignals::arm_interrupts()
        .map_err(|_| AgentQueryError::Admission(Failure::new(FailureKind::InternalFailure)))?;
    let attempt = agent_wait_finish(
        total,
        start,
        AGENT_TOTAL_BUDGET,
        session_id,
        until,
        after_revision,
        timeout_ms,
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
    total: Instant,
    start: Instant,
    read_budget: Duration,
    session_id: &AgentSessionId,
    until: &[AgentWaitCondition],
    after_revision: Option<AgentSafeInteger>,
    timeout_ms: u32,
    armed: impl AgentWatch,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> Result<WaitAttempt, AgentQueryError> {
    let attempt = {
        let wake = armed.poll_fd();
        agent_wait_span(
            total,
            start,
            read_budget,
            session_id,
            until,
            after_revision,
            timeout_ms,
            wake,
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
enum WaitDeadline {
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
    total: Instant,
    start: Instant,
    read_budget: Duration,
    session_id: &AgentSessionId,
    until: &[AgentWaitCondition],
    after_revision: Option<AgentSafeInteger>,
    timeout_ms: u32,
    wake: Option<BorrowedFd<'_>>,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> WaitAttempt {
    let mut client = match acquire(total, wake) {
        Err(error) => return WaitAttempt::Admission(error),
        Ok(client) => client,
    };
    let mut observations: u64 = 0;
    let mut last: Option<WaitObservation> = None;
    let mut backoff = WAIT_POLL_INITIAL;
    let mut first_evaluation = true;

    let outcome = loop {
        // Post-step total settlement before any new step: an expired
        // shared budget is always the wait's own timeout.
        if Instant::now() >= total {
            break wait_timeout(
                WaitDeadline::Total,
                timeout_ms,
                start,
                observations,
                last.as_ref(),
            );
        }
        let request_id = match obap_request_id() {
            Ok(request_id) => request_id,
            Err(failure) => break WaitAttempt::Done(Err(failure)),
        };
        let now = Instant::now();
        let read_deadline = match bounded_deadline(now, read_budget, total) {
            Ok(deadline) => deadline,
            Err(failure) => break WaitAttempt::Done(Err(failure)),
        };
        // The budget this poll's Get was actually admitted with: the
        // read-deadline reason records it verbatim.
        let read_budget_ms = read_deadline.duration_since(now).as_millis() as u64;
        let result = client.submit_agent_read_before_cancellable(
            AgentReadRequest::Get {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
            },
            read_deadline,
            wake,
        );
        // Post-step total settlement before the outcome may match or
        // classify — never a late success.
        let settled = Instant::now() >= total;
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
                        WaitDeadline::Total,
                        timeout_ms,
                        start,
                        observations,
                        last.as_ref(),
                    );
                }
                match result.outcome {
                    AgentGetOutcome::Found { revision, session } => {
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
                                        until: until.to_vec(),
                                        after_revision,
                                        timeout_ms,
                                        observations,
                                        elapsed_ms: start.elapsed().as_millis() as u64,
                                    },
                                )));
                            }
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
                        WaitDeadline::Total,
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
                        WaitDeadline::Total,
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
                        WaitDeadline::Read {
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
        let remaining = total.saturating_duration_since(Instant::now());
        if !remaining.is_zero() {
            if wait_backoff(wake, backoff.min(remaining)) {
                break WaitAttempt::Wake;
            }
            backoff = (backoff.saturating_mul(2)).min(WAIT_POLL_MAXIMUM);
        }
    };
    // The held Client Session is fully dropped before the owner arbitrates.
    drop(client);
    outcome
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
    }
}

/// The canonical match predicate over ONE returned session detail:
/// freshness conditions match the observation's freshness directly;
/// every status condition requires `fresh` freshness and a status kind
/// equal to the condition's canonical name.
fn condition_matches(condition: AgentWaitCondition, session: &AgentSessionDetail) -> bool {
    match condition {
        AgentWaitCondition::Fresh => {
            session.snapshot.observation.freshness == AgentObservationFreshness::Fresh
        }
        AgentWaitCondition::Stale => {
            session.snapshot.observation.freshness == AgentObservationFreshness::Stale
        }
        AgentWaitCondition::Disconnected => {
            session.snapshot.observation.freshness == AgentObservationFreshness::Disconnected
        }
        _ => {
            session.snapshot.observation.freshness == AgentObservationFreshness::Fresh
                && status_kind(&session.status) == condition.as_str()
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
fn wait_io_error(error: &AgentReadError) -> Option<&std::io::Error> {
    match error {
        AgentReadError::RequestNotSubmitted(error) => Some(error),
        AgentReadError::Failed { reason, .. } => match reason {
            AgentReadFailure::Write(error) | AgentReadFailure::ResponseDeadline(error) => {
                Some(error)
            }
            AgentReadFailure::Response(AgentWireError::Io(error)) => Some(error),
            AgentReadFailure::Response(_)
            | AgentReadFailure::UnexpectedResponse
            | AgentReadFailure::CorrelationMismatch { .. } => None,
        },
        AgentReadError::SessionUnavailable | AgentReadError::RequestEncoding(_) => None,
    }
}

/// A provisional wake marker surviving inside the exchange error: the
/// loop stops and the owner's arbitration decides — never classified
/// here.
fn wait_wake_marker(error: &AgentReadError) -> bool {
    wait_io_error(error)
        .is_some_and(|error| exchange_wake_ready(error) || exchange_wake_fault(error))
}

/// A deadline-elapsed exchange error: the per-read bound fired before the
/// shared total — the caller decides which of the two deadlines reports.
fn wait_deadline_elapsed(error: &AgentReadError) -> bool {
    wait_io_error(error).is_some_and(is_deadline_elapsed)
}

/// The interruptible private backoff: poll the borrowed wake descriptor
/// for up to `duration`; `true` means it reported readiness or an
/// unrecoverable poll fault, and the owner's arbitration must run before
/// any further step.
fn wait_backoff(wake: Option<BorrowedFd<'_>>, duration: Duration) -> bool {
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
        Err(nix::errno::Errno::EINTR) => false,
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
    deadline: WaitDeadline,
    timeout_ms: u32,
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
        WaitDeadline::Total => "wait-deadline",
        WaitDeadline::Read { .. } => "read-deadline",
    };
    details.insert("reason".to_owned(), json!(reason));
    if let WaitDeadline::Read { budget_ms } = deadline {
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
    WaitAttempt::Done(Err(Failure::new(FailureKind::Timeout).details(details)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wire-shaped session detail with the caller's freshness and status:
    /// the typed decoder the CLI trusts builds the same struct the loop
    /// evaluates, so the predicate is tested on the real document shape.
    fn detail(freshness: &str, status: serde_json::Value) -> AgentSessionDetail {
        serde_json::from_value(json!({
            "snapshot": {
                "sessionId": "sess-alpha",
                "providerId": "acme.agent",
                "lifecycle": "live",
                "executionLocation": {"executionLocationId": "host:7"},
                "observation": {
                    "fidelity": "rich",
                    "freshness": freshness,
                    "generation": 3,
                    "sequence": 11,
                    "source": "native"
                },
                "parentSessionId": null,
                "activityHint": null,
                "capabilities": [],
                "lastOutcome": null,
                "pendingRequests": [],
                "presentationAttachments": [],
                "turn": {"kind": "idle"}
            },
            "status": status
        }))
        .expect("fixture decodes as an AgentSessionDetail")
    }

    /// The canonical predicate: the three freshness conditions match the
    /// observation's freshness directly, every status condition requires
    /// `fresh` freshness plus an equal status `kind`, and `matchedConditions`
    /// preserves the caller's requested order rather than any canonical sort.
    #[test]
    fn condition_matching_is_freshness_gated_in_requested_order() {
        let fresh_idle = detail("fresh", json!({"kind": "idle"}));
        let stale_idle = detail("stale", json!({"kind": "idle"}));
        let gone_idle = detail("disconnected", json!({"kind": "idle"}));

        // Freshness conditions read the observation directly — no status gate.
        assert!(condition_matches(AgentWaitCondition::Fresh, &fresh_idle));
        assert!(!condition_matches(AgentWaitCondition::Fresh, &stale_idle));
        assert!(condition_matches(AgentWaitCondition::Stale, &stale_idle));
        assert!(!condition_matches(AgentWaitCondition::Stale, &fresh_idle));
        assert!(condition_matches(
            AgentWaitCondition::Disconnected,
            &gone_idle
        ));
        assert!(!condition_matches(
            AgentWaitCondition::Disconnected,
            &fresh_idle
        ));

        // Status conditions require `fresh`: the same `idle` status matches
        // on a fresh observation and fails on stale or disconnected.
        assert!(condition_matches(AgentWaitCondition::Idle, &fresh_idle));
        assert!(!condition_matches(AgentWaitCondition::Idle, &stale_idle));
        assert!(!condition_matches(AgentWaitCondition::Idle, &gone_idle));
        assert!(!condition_matches(AgentWaitCondition::Failed, &fresh_idle));

        // Every status kind answers its canonical condition name.
        for (status, condition) in [
            (
                json!({"kind": "needs-attention", "count": 2}),
                AgentWaitCondition::NeedsAttention,
            ),
            (json!({"kind": "failed"}), AgentWaitCondition::Failed),
            (json!({"kind": "succeeded"}), AgentWaitCondition::Succeeded),
            (json!({"kind": "canceled"}), AgentWaitCondition::Canceled),
            (
                json!({"kind": "interrupted"}),
                AgentWaitCondition::Interrupted,
            ),
            (
                json!({"kind": "working", "activity": null, "phase": "working"}),
                AgentWaitCondition::Working,
            ),
            (json!({"kind": "idle"}), AgentWaitCondition::Idle),
            (
                json!({"kind": "unknown", "activityHint": null}),
                AgentWaitCondition::Unknown,
            ),
            (json!({"kind": "ended"}), AgentWaitCondition::Ended),
        ] {
            let session = detail("fresh", status.clone());
            assert!(
                condition_matches(condition, &session),
                "{status} must match {}",
                condition.as_str()
            );
            let stale_session = detail("stale", status.clone());
            assert!(
                !condition_matches(condition, &stale_session),
                "{status} must not match under stale freshness"
            );
        }

        // A multi-condition request keeps the caller's order: `fresh` and
        // `idle` both hit a fresh idle document, in requested sequence.
        let requested = [
            AgentWaitCondition::Idle,
            AgentWaitCondition::Fresh,
            AgentWaitCondition::Failed,
        ];
        let matched: Vec<_> = requested
            .iter()
            .copied()
            .filter(|condition| condition_matches(*condition, &fresh_idle))
            .collect();
        assert_eq!(
            matched,
            vec![AgentWaitCondition::Idle, AgentWaitCondition::Fresh]
        );
    }
}

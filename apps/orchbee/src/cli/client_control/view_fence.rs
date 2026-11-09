//! Fence a selected Client on the facts a multi-step read depends on.
//!
//! A Client's view revision also steps when nothing such a read depends on changed: the view
//! carries the canonical Workbench revision, so any snapshot commit, including one the read's own
//! requests cause, steps it once the Client installs that commit. A background refresh itself keeps
//! the view `ready`, but a transient step can still make the Client refuse as `busy`. Under the
//! selection fence, a step the Client refused only as stale or busy runs again at the Client's
//! current view revision, while a selection change still fails it.
use super::exchange::exchange;
use crate::{
    cli::{Failure, console_interrupt::SignalWatch, query},
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewControlReason, ClientViewObservation,
    ClientViewOutcome, ClientViewRecord, SessionId,
};
use std::time::{Duration, Instant};

/// How a multi-step read fences the selected Client's view between its steps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cli) enum ViewFence {
    /// Any view change, including a revision step without a selection change, fails the read.
    Exact,
    /// Only a selection change fails the read; a step the Client refused as stale or busy runs
    /// again at the Client's current view revision.
    Selection,
}

/// Each step is addressed at most this many times: enough for a Client that other Clients' commits
/// refresh several times a second, while a Client whose view keeps moving still fails with its own
/// refusal well inside the command deadline.
pub(in crate::cli) const ADDRESSES: usize = 8;

impl ViewFence {
    /// Whether a view observed after `original` was read still answers for it.
    pub(in crate::cli) fn holds(
        self,
        original: &ClientViewRecord,
        view: &ClientViewObservation,
    ) -> bool {
        match self {
            Self::Exact => *view == original.view,
            Self::Selection => view.selection == original.view.selection,
        }
    }

    /// Whether a later record of the Client still answers for `original`.
    pub(in crate::cli) fn holds_record(
        self,
        original: &ClientViewRecord,
        current: &ClientViewRecord,
    ) -> bool {
        current.client_id == original.client_id
            && current.availability == ClientViewAvailability::Current
            && (self == Self::Selection || current.view_revision == original.view_revision)
            && self.holds(original, &current.view)
    }

    /// The Client's current record, or `None` when it no longer answers for `original`.
    pub(in crate::cli) fn reread(
        self,
        client: &mut LocalClient,
        session_id: &SessionId,
        original: &ClientViewRecord,
        deadline: Instant,
        signals: &impl SignalWatch,
    ) -> Result<Option<ClientViewRecord>, Failure> {
        let reply = exchange(
            client,
            session_id,
            ClientViewAction::Get {
                client_id: original.client_id.clone(),
            },
            deadline,
            signals,
        )?;
        Ok(match reply.outcome {
            ClientViewOutcome::Found { client } if self.holds_record(original, &client) => {
                Some(*client)
            }
            // Any other answer means the Client no longer answers for the original observation.
            _ => None,
        })
    }

    /// The record a later step addresses: `original` under the exact fence, and the Client's
    /// current record under the selection fence; a moved Client fails with `client-changed`.
    pub(in crate::cli) fn next_record(
        self,
        client: &mut LocalClient,
        session_id: &SessionId,
        original: &ClientViewRecord,
        deadline: Instant,
        signals: &impl SignalWatch,
    ) -> Result<ClientViewRecord, Failure> {
        match self {
            Self::Exact => Ok(original.clone()),
            Self::Selection => self
                .reread(client, session_id, original, deadline, signals)?
                .ok_or_else(|| query::workbench_failure("client-changed")),
        }
    }

    /// Runs one step addressed at `record`. Under the selection fence, a step the Client refused
    /// as stale or busy runs again, at most [`ADDRESSES`] times in all, at the Client's current
    /// record; a changed selection fails with `client-changed`.
    pub(in crate::cli) fn address<T, S: SignalWatch>(
        self,
        client: &mut LocalClient,
        session_id: &SessionId,
        record: &ClientViewRecord,
        deadline: Instant,
        signals: &mut S,
        mut step: impl FnMut(&mut LocalClient, &ClientViewRecord, &mut S) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let mut current = record.clone();
        for _ in 1..ADDRESSES {
            match step(client, &current, signals) {
                Err(failure)
                    if self == Self::Selection && readdressable(failure.client_refusal) =>
                {
                    settle(failure.client_refusal, deadline, signals)?;
                    current = self.next_record(client, session_id, record, deadline, signals)?;
                }
                result => return result,
            }
        }
        step(client, &current, signals)
    }
}

/// Whether a Client refusal only reports that its view moved or a transient step held it.
pub(in crate::cli) const fn readdressable(reason: Option<ClientViewControlReason>) -> bool {
    matches!(
        reason,
        Some(ClientViewControlReason::StaleView | ClientViewControlReason::Busy)
    )
}

/// Gives a Client held busy by a transient step a moment to settle before it is addressed again.
pub(in crate::cli) fn settle(
    reason: Option<ClientViewControlReason>,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<(), Failure> {
    signals.check_before(deadline)?;
    if reason == Some(ClientViewControlReason::Busy) {
        std::thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
        signals.check_before(deadline)?;
    }
    Ok(())
}

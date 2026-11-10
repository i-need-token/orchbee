//! The bounded retry after a stale view (ORC-413). When the authority refuses a submission only
//! because the view it was resolved from has gone stale, or a source read is outrun before
//! anything is submitted, the command reads a fresh snapshot and resolves every target again under
//! its original deadline, at most `RETRIES` times. It submits again only when every target
//! resolves to the identity it submitted before, and never after an unknown or partial outcome.

use super::{Failure, FailureKind};

/// Fresh reads after the first attempt, whatever caused each one.
pub(super) const RETRIES: u8 = 3;

/// How one attempt ended, as far as the retry is concerned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Ended {
    /// A source read was outrun before anything was submitted.
    Changed,
    /// The authority refused the submission before applying anything because its view was stale.
    Stale,
    /// Anything else: success, another refusal, an unknown outcome or a partial effect.
    Settled,
}

impl Ended {
    /// A failed target resolution: changed only when a source read was outrun.
    pub(super) fn resolution(failure: &Failure) -> Self {
        if stale(failure) {
            Self::Changed
        } else {
            Self::Settled
        }
    }

    /// A failed submission: stale only for a definite refusal that applied nothing.
    pub(super) fn submission(failure: &Failure) -> Self {
        if stale(failure) {
            Self::Stale
        } else {
            Self::Settled
        }
    }
}

/// The marker counts only on a definite refusal; an unknown outcome is never a refusal.
fn stale(failure: &Failure) -> bool {
    failure.kind == FailureKind::Rejected && failure.stale_view
}

/// One command's retry state: the fresh reads spent and the identity it submitted first.
pub(super) struct StaleRetry<I> {
    retries: u8,
    submitted: Option<I>,
    resubmit: bool,
}

impl<I: PartialEq> StaleRetry<I> {
    /// `resubmit` is false when the submission identity is the caller's own: a resubmission needs a
    /// fresh identity, which would leave the caller's handle naming only the refusal.
    pub(super) const fn new(resubmit: bool) -> Self {
        Self {
            retries: 0,
            submitted: None,
            resubmit,
        }
    }

    /// Whether to read a fresh snapshot and resolve again after an attempt that ended so.
    pub(super) fn reread(&mut self, ended: Ended) -> bool {
        let eligible = match ended {
            Ended::Changed => true,
            Ended::Stale => self.resubmit,
            Ended::Settled => false,
        };
        let granted = eligible && self.retries < RETRIES;
        if granted {
            self.retries += 1;
        }
        granted
    }

    /// Whether to submit `identity`: the first submission always, a later one only when every
    /// target resolved to the identity submitted before.
    pub(super) fn submit(&mut self, identity: I) -> bool {
        match &self.submitted {
            Some(submitted) => *submitted == identity,
            None => {
                self.submitted = Some(identity);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests;

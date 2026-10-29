//! The identity state of one Terminal generation: settle window, runs, hand-off, and bounded
//! staleness. Pure: callers supply complete samples, failures, and their sampling instants.
//!
//! Invariants:
//! - Exactly one current result; `unknown` always carries one reason.
//! - A run starts only after the same anchor (identifier and creation time) appears in two
//!   successful samples at least [`SETTLE`] apart, and its sequence number is never reused.
//! - A run continues only while a successful sample contains its exact anchor, or hands off to
//!   a same-provider descendant whose nearest departed ancestor is that anchor; a reused
//!   identifier never continues or transfers a run.
//! - Failed samples never end a run; after [`STALE`] without success the result is
//!   `unknown` with `sample-failed`.
//! - A run is published to Panel reports at most once and stays published until it ends.

use orchbee_agent_observation_protocol::{
    AgentAnchor, AgentIdentityUnknownReason as Reason, AgentIdentityUnsupportedReason,
    AgentProcessIdentity as Identity, AgentProcessRow, AgentReportProvider, AgentSampleVerdict,
};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// Two successful samples at least this far apart settle a new anchor.
pub(crate) const SETTLE: Duration = Duration::from_millis(500);
/// Failed samples keep the last result for at most this long.
pub(crate) const STALE: Duration = Duration::from_secs(2);

/// One Server-minted run bound to this Terminal generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Run {
    pub(crate) sequence: u32,
    pub(crate) pid: u32,
    pub(crate) created: u64,
    pub(crate) provider: AgentReportProvider,
    pub(crate) published: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pending {
    anchor: AgentAnchor,
    since: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdentityState {
    result: Identity,
    run: Option<Run>,
    pending: Option<Pending>,
    last_success: Option<Instant>,
    minted: u32,
}

impl Default for IdentityState {
    fn default() -> Self {
        Self {
            result: Identity::Unknown(Reason::Settling),
            run: None,
            pending: None,
            last_success: None,
            minted: 0,
        }
    }
}

impl IdentityState {
    /// The current result that diagnostics expose; Panel reports read [Self::published].
    pub(crate) const fn result(&self) -> Identity {
        self.result
    }

    #[cfg(test)]
    pub(crate) const fn run(&self) -> Option<Run> {
        self.run
    }

    /// The run Panel reports and screen fallback attribute to this Terminal.
    pub(crate) fn published(&self) -> Option<Run> {
        self.run.filter(|run| run.published)
    }

    /// Whether a run or a settling anchor needs frequent samples.
    pub(crate) const fn tracking(&self) -> bool {
        self.run.is_some() || self.pending.is_some()
    }

    /// Applies one successful sample and returns the run it ended, if any.
    pub(crate) fn observe(
        &mut self,
        at: Instant,
        rows: &[AgentProcessRow],
        verdict: AgentSampleVerdict,
    ) -> Option<Run> {
        self.last_success = Some(at);
        let mut ended = None;
        if let Some(mut run) = self.run {
            if rows
                .iter()
                .any(|row| row.pid == run.pid && row.creation_time == run.created)
            {
                self.result = match verdict {
                    AgentSampleVerdict::Anchor(anchor) if anchor_of(anchor, run) => anchor.result(),
                    AgentSampleVerdict::Unresolved { reason, .. } => Identity::Unknown(reason),
                    _ => Identity::Unknown(Reason::AmbiguousCandidates),
                };
                self.publish();
                return None;
            }
            if let Some(anchor) = hand_off(rows, run, verdict) {
                run.pid = anchor.pid;
                run.created = anchor.creation_time;
                self.run = Some(run);
                self.result = anchor.result();
                self.publish();
                return None;
            }
            self.run = None;
            ended = Some(run);
        }
        self.result = match verdict {
            AgentSampleVerdict::Anchor(anchor) => match self.pending {
                Some(pending)
                    if same_anchor(pending.anchor, anchor)
                        && at.saturating_duration_since(pending.since) >= SETTLE =>
                {
                    self.pending = None;
                    self.minted = self.minted.saturating_add(1);
                    self.run = Some(Run {
                        sequence: self.minted,
                        pid: anchor.pid,
                        created: anchor.creation_time,
                        provider: anchor.provider,
                        published: false,
                    });
                    anchor.result()
                }
                Some(pending) if same_anchor(pending.anchor, anchor) => {
                    Identity::Unknown(Reason::Settling)
                }
                _ => {
                    self.pending = Some(Pending { anchor, since: at });
                    Identity::Unknown(Reason::Settling)
                }
            },
            AgentSampleVerdict::Unresolved { reason, .. } => {
                self.pending = None;
                Identity::Unknown(reason)
            }
            AgentSampleVerdict::None => {
                self.pending = None;
                Identity::None
            }
            AgentSampleVerdict::Remote => {
                self.pending = None;
                Identity::Unsupported(AgentIdentityUnsupportedReason::RemoteExecution)
            }
        };
        self.publish();
        ended
    }

    /// Applies one failed sample; it proves nothing about the anchor, so the run stays.
    pub(crate) fn fail(&mut self, at: Instant) {
        if self
            .last_success
            .is_none_or(|last| at.saturating_duration_since(last) >= STALE)
        {
            self.result = Identity::Unknown(Reason::SampleFailed);
        }
    }

    /// Panel reports show interactive runs, and keep showing runs whose arguments are not yet
    /// classified or readable so shipped Agents with unlisted arguments stay visible.
    fn publish(&mut self) {
        if let Some(run) = self.run.as_mut()
            && matches!(
                self.result,
                Identity::Agent { .. }
                    | Identity::Unknown(
                        Reason::UnclassifiedArguments | Reason::CommandLineUnreadable
                    )
            )
        {
            run.published = true;
        }
    }
}

fn same_anchor(a: AgentAnchor, b: AgentAnchor) -> bool {
    a.pid == b.pid && a.creation_time == b.creation_time && a.provider == b.provider
}

fn anchor_of(anchor: AgentAnchor, run: Run) -> bool {
    anchor.pid == run.pid && anchor.creation_time == run.created && anchor.provider == run.provider
}

/// A same-provider anchor whose nearest departed ancestor is the run's departed anchor.
fn hand_off(
    rows: &[AgentProcessRow],
    run: Run,
    verdict: AgentSampleVerdict,
) -> Option<AgentAnchor> {
    let (AgentSampleVerdict::Anchor(anchor)
    | AgentSampleVerdict::Unresolved {
        anchor: Some(anchor),
        ..
    }) = verdict
    else {
        return None;
    };
    if anchor.provider != run.provider {
        return None;
    }
    let mut row = rows.iter().find(|row| row.pid == anchor.pid)?;
    for _ in 0..rows.len() {
        match row
            .parent_pid
            .and_then(|pid| rows.iter().find(|parent| parent.pid == pid))
        {
            Some(parent) if parent.creation_time <= row.creation_time => row = parent,
            Some(_) => return None,
            None => {
                return (row.parent_pid == Some(run.pid) && row.creation_time >= run.created)
                    .then_some(anchor);
            }
        }
    }
    None
}

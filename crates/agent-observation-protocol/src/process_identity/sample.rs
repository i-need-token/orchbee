//! What one complete Host sample proves on its own: its outermost Agent anchor, or why it
//! cannot name one. Settling, runs, and staleness belong to the Server identity state.

use super::{
    AgentIdentityGrade, AgentIdentityUnknownReason, AgentProcessIdentity, AgentProcessRecognition,
    AgentUnresolvedProcess,
};
use crate::AgentReportProvider;

/// One sampled process with already-decoded facts; only the OS adapter supplies these.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProcessRow {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub creation_time: u64,
    /// The Host could not open the process, so nothing but its identity and parent is known.
    pub opaque: bool,
    /// Proven not attached to the Terminal's console; an unobservable console is not proof.
    pub detached: bool,
    pub recognition: AgentProcessRecognition,
}

/// The outermost candidate of a sample, identified by process identifier and creation time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentAnchor {
    pub pid: u32,
    pub creation_time: u64,
    pub provider: AgentReportProvider,
    pub evidence: AgentAnchorEvidence,
}

/// How far the anchor row alone proves an interactive run in this Terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentAnchorEvidence {
    Interactive,
    UnclassifiedArguments,
    CommandLineUnreadable,
    /// Not attached to the Terminal's console: a GUI program, another console, or a program
    /// that has not attached yet.
    DetachedConsole,
}

/// The verdict of one complete sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentSampleVerdict {
    /// One outermost candidate and no unresolved row outside its subtree.
    Anchor(AgentAnchor),
    /// Evidence that cannot name the Agent; `anchor` is the outermost candidate when one exists.
    Unresolved {
        anchor: Option<AgentAnchor>,
        reason: AgentIdentityUnknownReason,
    },
    /// No candidate and no row that could hold one.
    None,
    /// No candidate while a remote execution client runs.
    Remote,
}

impl AgentAnchor {
    /// The identity this anchor proves once it is settled.
    pub const fn result(self) -> AgentProcessIdentity {
        match self.evidence {
            AgentAnchorEvidence::Interactive => AgentProcessIdentity::Agent {
                provider: self.provider,
                grade: AgentIdentityGrade::Recognized,
            },
            AgentAnchorEvidence::UnclassifiedArguments => {
                AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::UnclassifiedArguments)
            }
            AgentAnchorEvidence::CommandLineUnreadable => {
                AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::CommandLineUnreadable)
            }
            AgentAnchorEvidence::DetachedConsole => {
                AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::DetachedConsole)
            }
        }
    }
}

impl AgentProcessRow {
    /// The provider this row would anchor; diagnostics show it per row.
    pub fn anchor_provider(self) -> Option<AgentReportProvider> {
        self.candidate().map(|(provider, _)| provider)
    }

    /// A row that may be the anchor: an interactive match, or an alias of one provider whose
    /// arguments are unclassified or unreadable.
    fn candidate(self) -> Option<(AgentReportProvider, AgentAnchorEvidence)> {
        let (provider, evidence) = match self.recognition {
            AgentProcessRecognition::Interactive { provider, .. } => {
                (provider, AgentAnchorEvidence::Interactive)
            }
            AgentProcessRecognition::Unresolved {
                provider: Some(provider),
                reason: AgentUnresolvedProcess::UnclassifiedArguments,
            } => (provider, AgentAnchorEvidence::UnclassifiedArguments),
            AgentProcessRecognition::Unresolved {
                provider: Some(provider),
                reason: AgentUnresolvedProcess::CommandLineUnreadable,
            } => (provider, AgentAnchorEvidence::CommandLineUnreadable),
            _ => return None,
        };
        Some((
            provider,
            if self.detached {
                AgentAnchorEvidence::DetachedConsole
            } else {
                evidence
            },
        ))
    }

    /// Why a non-candidate row could still hold an Agent.
    const fn blocker(self) -> Option<AgentIdentityUnknownReason> {
        if self.opaque {
            return Some(AgentIdentityUnknownReason::CommandLineUnreadable);
        }
        match self.recognition {
            AgentProcessRecognition::Unresolved { reason, .. } => Some(reason.reason()),
            _ => None,
        }
    }
}

/// Evaluates one complete sample; `at_limit` means the Host may have omitted members.
/// The anchor is the candidate every other candidate descends from; rows inside its subtree
/// never hold an outer Agent, while unresolved rows elsewhere leave the sample unresolved.
pub fn evaluate_agent_sample(rows: &[AgentProcessRow], at_limit: bool) -> AgentSampleVerdict {
    let unresolved = |anchor, reason| AgentSampleVerdict::Unresolved { anchor, reason };
    if rows.iter().enumerate().any(|(i, p)| {
        p.pid == 0 || p.creation_time == 0 || rows[..i].iter().any(|q| q.pid == p.pid)
    }) {
        return unresolved(None, AgentIdentityUnknownReason::AmbiguousCandidates);
    }
    let candidates: Vec<_> = rows
        .iter()
        .filter_map(|row| row.candidate().map(|c| (row, c)))
        .collect();
    let outermost = candidates.iter().find(|(candidate, _)| {
        candidates
            .iter()
            .all(|(other, _)| descends_from(rows, other.pid, candidate.pid).unwrap_or(false))
    });
    let anchor = match (candidates.is_empty(), outermost) {
        (true, _) => None,
        (false, None) => {
            return unresolved(None, AgentIdentityUnknownReason::AmbiguousCandidates);
        }
        (false, Some((row, (provider, evidence)))) => Some(AgentAnchor {
            pid: row.pid,
            creation_time: row.creation_time,
            provider: *provider,
            evidence: *evidence,
        }),
    };
    let outside = |row: &&AgentProcessRow| {
        anchor.is_none_or(|a| !descends_from(rows, row.pid, a.pid).unwrap_or(false))
    };
    if let Some(reason) = rows.iter().filter(outside).find_map(|row| row.blocker()) {
        return unresolved(anchor, reason);
    }
    if at_limit {
        return unresolved(anchor, AgentIdentityUnknownReason::SampleLimit);
    }
    match anchor {
        Some(anchor) => AgentSampleVerdict::Anchor(anchor),
        None if rows
            .iter()
            .any(|row| row.recognition == AgentProcessRecognition::RemoteClient) =>
        {
            AgentSampleVerdict::Remote
        }
        None => AgentSampleVerdict::None,
    }
}

/// Whether `child` is `ancestor` or descends from it through rows with nondecreasing creation
/// times; `None` when the whole chain is invalid (a cycle or a reused parent identifier).
pub fn descends_from(rows: &[AgentProcessRow], child: u32, ancestor: u32) -> Option<bool> {
    let mut current = rows.iter().find(|p| p.pid == child)?;
    let mut found = child == ancestor;
    for _ in 0..rows.len() {
        let Some(parent) = current
            .parent_pid
            .and_then(|pid| rows.iter().find(|p| p.pid == pid))
        else {
            return Some(found);
        };
        if parent.creation_time > current.creation_time || parent.pid == child {
            return None;
        }
        found |= parent.pid == ancestor;
        current = parent;
    }
    None
}
